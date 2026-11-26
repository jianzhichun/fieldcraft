//! The wire, v1: `u32` big-endian header length, a JSON header, then the binary payload.
//!
//! Every header carries `"len"`, the payload's length in bytes ([`write_message`] inserts it);
//! a reader takes the payload from it, so the header stays typed (serde) and the payload stays
//! binary. Inbound messages are header-only in v1; frames carry their rectangles' bytes.
//!
//! Limits: a header is at most [`MAX_HEADER`] bytes; a payload at most [`MAX_PAYLOAD`] (a
//! 4K × 4K RGBA frame is 64 MiB). The first inbound message must be `hello` with the token;
//! see [`HostMsg`].

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::input::InputEvent;

/// Most bytes in a message's JSON header.
pub const MAX_HEADER: usize = 64 * 1024;
/// Most bytes in a message's payload.
pub const MAX_PAYLOAD: usize = 128 * 1024 * 1024;
/// The protocol version this build speaks.
pub const VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Protocol(String),
}

/// A message the host sends. The first one must be [`HostMsg::Hello`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum HostMsg {
    Hello {
        v: u32,
        token: String,
    },
    /// One pass's input, in order.
    Input {
        events: Vec<InputEvent>,
    },
    /// The canvas size in CSS points (points = CSS px, zoom 1) and pixels per point.
    Resize {
        w: f32,
        h: f32,
        dpr: f32,
    },
    Visible {
        visible: bool,
    },
    /// A sent frame was shown: one credit back.
    Ack {
        seq: u64,
    },
    /// The host's window switched between its light and dark themes: draw the whole window
    /// again in `dark`. Ignored when the server was started with a fixed `--theme`.
    Theme {
        dark: bool,
    },
    /// A request answered by `control::handle` (or the server's own methods); `op` names the
    /// operation the turn belongs to (`turn/step`), for cancellation.
    Call {
        id: u64,
        method: String,
        #[serde(default)]
        params: Value,
        #[serde(default)]
        op: Option<String>,
    },
    /// End the session: `bye {"why": …}`, exit.
    Stop {
        #[serde(default)]
        why: Option<String>,
    },
}

/// One rectangle of a frame. `off`/`len` index the payload; `rows` is the stripe's height in
/// pixels (equal to `h` for raw and PNG).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RectHeader {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// `"raw"`, `"jpeg"` or `"png"`.
    pub codec: String,
    pub off: usize,
    pub len: usize,
    pub rows: u32,
}

/// Serialize `header` as a JSON object with `"len"` set to `payload.len()`, then write
/// `u32 BE header_len | header | payload`.
pub fn write_message(w: &mut impl Write, header: &impl Serialize, payload: &[u8]) -> Result<(), WireError> {
    let mut header = serde_json::to_value(header).map_err(|e| WireError::Protocol(format!("header: {e}")))?;
    let object = header.as_object_mut().ok_or_else(|| WireError::Protocol("a header must be a JSON object".into()))?;
    object.insert("len".into(), Value::from(payload.len()));
    let bytes = serde_json::to_vec(&header).map_err(|e| WireError::Protocol(format!("header: {e}")))?;
    if bytes.len() > MAX_HEADER {
        return Err(WireError::Protocol(format!("header of {} bytes exceeds {MAX_HEADER}", bytes.len())));
    }
    w.write_all(&(bytes.len() as u32).to_be_bytes())?;
    w.write_all(&bytes)?;
    w.write_all(payload)?;
    w.flush()?;
    Ok(())
}

/// Read one message: its header as a JSON value and its payload (empty when `"len"` is 0).
pub fn read_message(r: &mut impl Read) -> Result<(Value, Vec<u8>), WireError> {
    let mut length = [0u8; 4];
    r.read_exact(&mut length)?;
    let n = u32::from_be_bytes(length) as usize;
    if n == 0 || n > MAX_HEADER {
        return Err(WireError::Protocol(format!("header length {n} is not 1..={MAX_HEADER}")));
    }
    let mut header = vec![0u8; n];
    r.read_exact(&mut header)?;
    let header: Value = serde_json::from_slice(&header).map_err(|e| WireError::Protocol(format!("header: {e}")))?;
    let len = header.get("len").and_then(Value::as_u64).unwrap_or(0) as usize;
    if len > MAX_PAYLOAD {
        return Err(WireError::Protocol(format!("payload length {len} exceeds {MAX_PAYLOAD}")));
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok((header, payload))
}

/// Parse a host message from its header.
pub fn parse_host(header: &Value) -> Result<HostMsg, WireError> {
    serde_json::from_value(header.clone()).map_err(|e| WireError::Protocol(format!("message: {e}")))
}

/// The outbound side: one socket behind a lock, so frames and replies never interleave.
#[derive(Clone)]
pub struct Writer {
    stream: Arc<Mutex<std::net::TcpStream>>,
}

impl Writer {
    pub fn new(stream: std::net::TcpStream) -> Self {
        let _ = stream.set_nodelay(true);
        Self { stream: Arc::new(Mutex::new(stream)) }
    }

    pub fn send(&self, header: &impl Serialize, payload: &[u8]) -> Result<(), WireError> {
        let mut stream = self.stream.lock().unwrap_or_else(|e| e.into_inner());
        write_message(&mut *stream, header, payload)
    }

    /// Stop the connection, waking a reader on the other end.
    pub fn shutdown(&self) {
        if let Ok(stream) = self.stream.lock() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }
}

/// The ids of stop requests already answered, whichever thread answered them.
///
/// The frame loop normally answers a whole-turn stop within a frame; a reader that saw the
/// request arms a 5 s watchdog for the case the loop is stuck inside a synchronous command.
/// [`claim`](Self::claim) lets exactly one of them write the reply.
#[derive(Clone, Default)]
pub struct StopBook {
    answered: Arc<Mutex<std::collections::BTreeSet<u64>>>,
    /// The session's final `bye` is spoken by exactly one thread (the loop's `stop`, or the
    /// device-loss helper that ends a stuck session).
    ended: Arc<std::sync::atomic::AtomicBool>,
}

impl StopBook {
    /// `true` for the first caller with `id`; that caller writes the reply.
    pub fn claim(&self, id: u64) -> bool {
        self.answered.lock().unwrap_or_else(|e| e.into_inner()).insert(id)
    }

    /// `true` for the first caller: it writes the session's final `bye`. A later `stop` or a
    /// device-loss notice then stays silent.
    pub fn claim_end(&self) -> bool {
        !self.ended.swap(true, std::sync::atomic::Ordering::AcqRel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A reader that hands over one byte at a time, as a slow TCP peer does.
    struct Drip<'a>(&'a [u8]);

    impl Read for Drip<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let Some((&b, rest)) = self.0.split_first() else { return Ok(0) };
            self.0 = rest;
            buf[0] = b;
            Ok(1)
        }
    }

    #[test]
    fn a_partial_peer_is_read_whole() {
        let mut encoded = Vec::new();
        write_message(&mut encoded, &json!({"t":"hello","v":1,"token":"abc"}), b"").unwrap();
        let mut drip = Drip(&encoded);
        let (header, payload) = read_message(&mut drip).unwrap();
        assert_eq!(header["t"], "hello");
        assert_eq!(header["len"], 0);
        assert!(payload.is_empty());
        assert_eq!(parse_host(&header).unwrap(), HostMsg::Hello { v: 1, token: "abc".into() });
    }

    #[test]
    fn a_payload_round_trips_and_is_counted_in_the_header() {
        let payload: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let mut encoded = Vec::new();
        write_message(&mut encoded, &json!({"t":"frame","seq":7}), &payload).unwrap();
        let mut cursor = std::io::Cursor::new(encoded);
        let (header, read) = read_message(&mut cursor).unwrap();
        assert_eq!(header["len"], 1000);
        assert_eq!(read, payload);
        assert_eq!(cursor.position(), cursor.get_ref().len() as u64);
    }

    #[test]
    fn a_header_over_the_limit_is_refused() {
        let mut encoded = (MAX_HEADER as u32 + 1).to_be_bytes().to_vec();
        encoded.extend(std::iter::repeat_n(b'{', MAX_HEADER + 1));
        let err = read_message(&mut std::io::Cursor::new(encoded)).unwrap_err();
        assert!(err.to_string().contains("header length"), "{err}");
    }

    #[test]
    fn a_zero_header_is_refused() {
        let err = read_message(&mut std::io::Cursor::new(0u32.to_be_bytes())).unwrap_err();
        assert!(err.to_string().contains("header length"), "{err}");
    }

    #[test]
    fn a_payload_over_the_limit_is_refused() {
        let header = json!({"t":"frame","len": MAX_PAYLOAD + 1}).to_string();
        let mut encoded = (header.len() as u32).to_be_bytes().to_vec();
        encoded.extend(header.as_bytes());
        let err = read_message(&mut std::io::Cursor::new(encoded)).unwrap_err();
        assert!(err.to_string().contains("payload length"), "{err}");
    }

    #[test]
    fn a_truncated_message_is_an_io_error_not_a_panic() {
        let mut encoded = Vec::new();
        write_message(&mut encoded, &json!({"t":"ack","seq":1}), b"").unwrap();
        encoded.pop();
        assert!(read_message(&mut std::io::Cursor::new(encoded)).is_err());
    }

    #[test]
    fn input_messages_parse_into_events() {
        let header = json!({"t":"input","events":[{"k":"move","x":1.5,"y":2.0},{"k":"text","text":"hi"}]});
        let msg = parse_host(&header).unwrap();
        match msg {
            HostMsg::Input { events } => {
                assert_eq!(events.len(), 2);
                assert_eq!(events[0], InputEvent::Move { x: 1.5, y: 2.0 });
                assert_eq!(events[1], InputEvent::Text { text: "hi".into() });
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_stop_request_may_omit_its_reason() {
        let msg = parse_host(&json!({"t":"stop"})).unwrap();
        assert_eq!(msg, HostMsg::Stop { why: None });
    }

    #[test]
    fn the_stop_book_lets_exactly_one_writer_answer() {
        let book = StopBook::default();
        assert!(book.claim(1));
        assert!(!book.claim(1));
        assert!(book.claim(2));
    }

    #[test]
    fn the_stop_book_speaks_one_final_bye() {
        let book = StopBook::default();
        assert!(book.claim_end());
        assert!(!book.claim_end(), "a device-loss notice after a stop stays silent");
    }
}
