//! `fieldz-render-server`: the FieldCraft window drawn without a window, streamed over
//! loopback TCP, with the host's input and the model's engine calls on the same connection.
//!
//! The design is `docs/fieldz-render-server.md`; the wire is `PROTOCOL.md` beside this file.
//! One connection, one session; the first message must be `hello` with the token.

mod diff;
mod encode;
mod engine;
mod gpu;
mod host;
mod input;
mod ops;
mod policy;
mod readback;
mod wire;

use std::io::Write;
use std::net::TcpListener;

use fieldcraft_engine::Session;
use fieldcraft_gpu::select;
use serde_json::json;

use crate::host::{Config, Host, ThemeMode};

const USAGE: &str = "\
fieldz-render-server — a windowless FieldCraft window, streamed over loopback TCP.

Usage: fieldz-render-server [flags]

Flags:
  --port <N>             Listen on 127.0.0.1:<N> (0 = an ephemeral port, and the default;
                         `FIELDCRAFT_RENDER_PORT` sets it too). The chosen port is printed on
                         stdout as one line: `ready {\"v\":1,\"port\":N,\"pid\":N}`.
  --token-file <PATH>    Read the `hello` token from PATH (trimmed), else from
                         `FIELDCRAFT_RENDER_TOKEN`. With neither, a token is generated and
                         printed in the `ready` line (only then).
  --width <PTS>          Canvas width in CSS points (default 1280).
  --height <PTS>         Canvas height in CSS points (default 800).
  --dpr <N>              Pixels per point (default 1; clamped to 0.5..=4.0, and lowered further
                         when the frame would exceed 8192 pixels a side or 4096×4096 in all; the
                         frame header says what was used as `dprUsed`).
  --theme <MODE>         dark, light or auto (default auto). `auto` follows the page's `theme`
                         messages (the host's own light/dark switch); dark/light are fixed and
                         ignore them.
  --lang <zh|en>         Interface language (default zh; `FIELDCRAFT_LANG` sets it too; `en`
                         keeps the English interface). `ui.language` changes it at runtime.
  --read-only <REASON>   Open the session read-only: saving, undo/redo, opening or replacing the
                         document, deck import/export and the export/image writers are refused
                         with `EngineError::Disabled`, and a model edit is undone (and, where a
                         crash journal is active, recorded as failed). `FIELDCRAFT_READ_ONLY` sets the reason too.
  --open <FILE>          Open a .fieldcraft file before serving (before the read-only policy
                         applies: opening it is the host's own act).
  --fps-cap <N>          Most passes a second (default 60; 1..=240).
  -h, --help             This text.

Environment:
  FIELDCRAFT_GPU         auto (default), hardware, software, cpu. `cpu` cannot stream frames.
  FIELDCRAFT_RENDER_PORT / FIELDCRAFT_RENDER_TOKEN / FIELDCRAFT_READ_ONLY / FIELDCRAFT_LANG as above.

The protocol (message by message, with the mapping to egui events) is PROTOCOL.md.
";

/// The flags, parsed by hand (the server has nine and no other reason to take a dependency).
#[derive(Debug, Default)]
struct Args {
    port: u16,
    token_file: Option<String>,
    width: f32,
    height: f32,
    dpr: f32,
    read_only: Option<String>,
    open: Option<String>,
    fps_cap: f32,
    theme: ThemeMode,
    language: &'static str,
}

impl Args {
    /// `Ok(None)` when `--help` was asked for.
    fn parse(argv: impl Iterator<Item = String>) -> Result<Option<Args>, String> {
        let mut args = Args {
            port: std::env::var("FIELDCRAFT_RENDER_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(0),
            width: 1280.0,
            height: 800.0,
            dpr: 1.0,
            fps_cap: 60.0,
            language: fieldcraft_ui_egui::theme::language_preference(),
            ..Default::default()
        };
        let mut argv = argv;
        while let Some(flag) = argv.next() {
            let mut value = || argv.next().ok_or_else(|| format!("{flag} needs a value"));
            match flag.as_str() {
                "-h" | "--help" => return Ok(None),
                "--port" => args.port = value()?.parse().map_err(|_| "`--port` must be 0..=65535".to_string())?,
                "--token-file" => args.token_file = Some(value()?),
                "--width" => args.width = value()?.parse().map_err(|_| "`--width` must be a number of points".to_string())?,
                "--height" => args.height = value()?.parse().map_err(|_| "`--height` must be a number of points".to_string())?,
                "--dpr" => args.dpr = value()?.parse().map_err(|_| "`--dpr` must be a number".to_string())?,
                "--read-only" => args.read_only = Some(value()?),
                "--open" => args.open = Some(value()?),
                "--fps-cap" => args.fps_cap = value()?.parse().map_err(|_| "`--fps-cap` must be a number".to_string())?,
                "--theme" => args.theme = ThemeMode::parse(&value()?)?,
                "--lang" => {
                    args.language = match value()?.as_str() {
                        "zh" => "zh",
                        "en" => "en",
                        other => return Err(format!("`--lang` must be `zh` or `en` (got `{other}`)")),
                    }
                }
                other => return Err(format!("unknown flag `{other}` (try --help)")),
            }
        }
        Ok(Some(args))
    }
}

/// The token and whether this run generated it (and so must tell the host in `ready`).
struct Token {
    value: String,
    generated: bool,
}

impl Token {
    fn resolve(file: Option<String>) -> Result<Token, String> {
        if let Some(path) = file {
            let read = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
            let trimmed = read.trim();
            if trimmed.is_empty() {
                return Err(format!("{path} holds no token"));
            }
            return Ok(Token { value: trimmed.to_string(), generated: false });
        }
        if let Ok(token) = std::env::var("FIELDCRAFT_RENDER_TOKEN") {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                return Ok(Token { value: trimmed.to_string(), generated: false });
            }
        }
        Ok(Token { value: generate_token(), generated: true })
    }
}

/// 32 hex characters of randomness from the OS (`/dev/urandom` on Unix); on other platforms a
/// time-and-pid mix, enough for a loopback-only guard.
fn generate_token() -> String {
    #[cfg(unix)]
    {
        use std::io::Read as _;
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            let mut bytes = [0u8; 16];
            if f.read_exact(&mut bytes).is_ok() {
                return bytes.iter().map(|b| format!("{b:02x}")).collect();
            }
        }
    }
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0).hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    format!("{:016x}{:016x}", hasher.finish(), hasher.finish().rotate_left(7))
}

fn run() -> Result<(), String> {
    let Some(args) = Args::parse(std::env::args().skip(1))? else {
        print!("{USAGE}");
        return Ok(());
    };
    let token = Token::resolve(args.token_file.clone())?;
    let listener = TcpListener::bind(("127.0.0.1", args.port)).map_err(|e| format!("binding 127.0.0.1:{}: {e}", args.port))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let mut ready = json!({"v": wire::VERSION, "port": port, "pid": std::process::id()});
    if token.generated {
        ready["token"] = json!(token.value.clone());
    }
    println!("ready {ready}");
    std::io::stdout().flush().map_err(|e| e.to_string())?;

    // Device and pipelines first: `hello` may already be waiting in the listen backlog, and the
    // first frame only runs once it arrives.
    let policy = select::Policy::from_env().map_err(|e| e.to_string())?;
    let (width, height, dpr_used) = host::fit(args.width, args.height, args.dpr)?;
    let dark = args.theme.dark();
    let gpu = gpu::Gpu::new(policy, width, height, dark)?;
    let (stream, peer) = listener.accept().map_err(|e| format!("accepting a connection: {e}"))?;
    log::info!("serving {} (dpr {dpr_used})", peer);
    let config =
        Config { width: args.width, height: args.height, dpr: args.dpr, fps_cap: args.fps_cap, dark, theme: args.theme, language: args.language };
    let mut host = Host::new(Session::default(), config, Box::new(gpu))?;
    if let Some(path) = &args.open {
        host.open(path)?;
    }
    policy::apply(args.read_only);
    host.serve(stream, token.value)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("fieldz-render-server: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Args>, String> {
        Args::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn the_flags_parse() {
        let a = parse(&[
            "--port",
            "0",
            "--width",
            "640",
            "--height",
            "480",
            "--dpr",
            "2",
            "--fps-cap",
            "30",
            "--read-only",
            "trial",
            "--open",
            "m.fieldcraft",
            "--token-file",
            "t",
            "--lang",
            "en",
        ])
        .unwrap()
        .unwrap();
        assert_eq!((a.width, a.height, a.dpr, a.fps_cap), (640.0, 480.0, 2.0, 30.0));
        assert_eq!(a.read_only.as_deref(), Some("trial"));
        assert_eq!(a.open.as_deref(), Some("m.fieldcraft"));
        assert_eq!(a.token_file.as_deref(), Some("t"));
        assert_eq!(a.language, "en");
        assert!(parse(&["--help"]).unwrap().is_none());
        assert!(parse(&["--nope"]).unwrap_err().contains("unknown flag"));
        assert!(parse(&["--port"]).unwrap_err().contains("needs a value"));
        assert!(parse(&["--port", "x"]).unwrap_err().contains("0..=65535"));
        assert!(parse(&["--lang", "fr"]).unwrap_err().contains("`--lang` must be"));
    }

    #[test]
    fn a_generated_token_is_hex_and_a_file_token_is_trimmed() {
        let t = generate_token();
        assert_eq!(t.len(), 32);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
        let dir = std::env::temp_dir().join(format!("fieldz-render-token-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("token");
        std::fs::write(&path, "  abc123\n").unwrap();
        let token = Token::resolve(Some(path.to_string_lossy().into_owned())).unwrap();
        assert_eq!(token.value, "abc123");
        assert!(!token.generated);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
