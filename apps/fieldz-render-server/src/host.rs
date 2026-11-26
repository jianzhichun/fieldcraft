//! The host loop: one thread owns the app and the egui context, runs a pass on demand, paints
//! it offscreen, diffs and encodes the changed rectangles, and answers the host's calls.
//!
//! A pass follows eframe's native wgpu integration, minus the window: build `RawInput`
//! (`raw_input_hook` injects the control channel's events), `run_ui(logic + ui)`, tessellate,
//! paint into the offscreen target, read back, diff against the last *sent* frame, encode and
//! send within the credit window.
//!
//! Threads: this loop; a reader that parses inbound messages and only sends into the loop's
//! channel; and the writer, a locked socket shared by both. `Painter` is the only GPU-facing
//! trait, so tests drive the whole loop with a stub painter.

use std::collections::BTreeMap;
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use fieldcraft_engine::Session;
use fieldcraft_ui_egui::ModelApp;
use serde_json::{Value, json};

use crate::diff;
use crate::encode;
use crate::engine::{self, Batch, Dispatch};
use crate::input::{self, Directive};
use crate::ops::{self, Kind, Ops};
use crate::wire::{self, HostMsg, StopBook, Writer};

/// Largest frame side in device pixels (the same as `ui.capture`'s).
pub const MAX_SIDE: u32 = 8192;
/// Most device pixels in one frame.
pub const MAX_PIXELS: u64 = 4096 * 4096;
/// Credits at the start: frames in flight the host has not acknowledged.
pub const START_CREDITS: u32 = 2;
/// Most rectangles in one frame: a checkerboard change could otherwise make the JSON header
/// larger than its 64 KiB limit. Past this, the whole frame goes as one (PNG).
pub const MAX_RECTS: usize = 256;

/// The GPU side of a pass: whatever turns egui primitives into RGBA pixels.
pub trait Painter: Send {
    /// The render state the app's 3D viewport draws with, when there is one.
    fn render_state(&self) -> Option<&egui_wgpu::RenderState> {
        None
    }
    /// Largest texture side egui may upload.
    fn max_texture_side(&self) -> usize {
        2048
    }
    /// The device-pixel size changed: recreate targets.
    fn resize(&mut self, width_px: u32, height_px: u32) -> Result<(), String>;
    /// The theme switched: the next pass's clear colour follows it (a stub painter may ignore this).
    fn set_dark(&mut self, _dark: bool) {}
    /// Draw one pass and read it back as tightly packed RGBA (`width_px × height_px`).
    fn paint(&mut self, primitives: &[egui::ClippedPrimitive], textures: egui::TexturesDelta, pixels_per_point: f32) -> Result<Vec<u8>, String>;
}

/// The window's theme: a fixed light or dark, or `auto`, which follows the page's `theme`
/// messages (the host's own light/dark switch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    Dark,
    Light,
    #[default]
    Auto,
}

impl ThemeMode {
    pub fn parse(s: &str) -> Result<ThemeMode, String> {
        match s {
            "dark" => Ok(ThemeMode::Dark),
            "light" => Ok(ThemeMode::Light),
            "auto" => Ok(ThemeMode::Auto),
            other => Err(format!("`--theme` is dark, light or auto, not `{other}`")),
        }
    }

    /// The theme the first pass uses: `auto` starts dark until the page says otherwise.
    pub fn dark(self) -> bool {
        self != ThemeMode::Light
    }
}

/// The view the host asked for: CSS points, pixels per point, cap, and theme.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub width: f32,
    pub height: f32,
    pub dpr: f32,
    pub fps_cap: f32,
    /// The theme in effect; a `theme` message updates it in [`ThemeMode::Auto`].
    pub dark: bool,
    pub theme: ThemeMode,
}

impl Default for Config {
    fn default() -> Self {
        Config { width: 1280.0, height: 800.0, dpr: 1.0, fps_cap: 60.0, dark: true, theme: ThemeMode::Auto }
    }
}

/// The device-pixel size for a requested view: `dpr` is lowered until the frame fits
/// [`MAX_SIDE`] and [`MAX_PIXELS`] (the host is told the `dpr` actually used).
pub fn fit(width: f32, height: f32, dpr: f32) -> Result<(u32, u32, f32), String> {
    if !(width.is_finite() && height.is_finite() && dpr.is_finite()) || width <= 0.0 || height <= 0.0 {
        return Err(format!("a view of {width} × {height} points at {dpr} is not a size"));
    }
    let (css_w, css_h) = (width.clamp(1.0, MAX_SIDE as f32), height.clamp(1.0, MAX_SIDE as f32));
    let mut dpr = dpr.clamp(0.5, 4.0);
    for _ in 0..64 {
        let (w, h) = ((css_w * dpr).round() as u32, (css_h * dpr).round() as u32);
        if (1..=MAX_SIDE).contains(&w) && (1..=MAX_SIDE).contains(&h) && u64::from(w) * u64::from(h) <= MAX_PIXELS {
            return Ok((w, h, dpr));
        }
        dpr *= 0.9;
        if dpr < 0.5 {
            break;
        }
    }
    Err(format!("{css_w} × {css_h} points does not fit {MAX_SIDE} a side and {MAX_PIXELS} pixels at any dpr 0.5..=4.0"))
}

/// What the reader sends into the loop.
enum Event {
    Hello,
    Msg(HostMsg),
    /// A repaint the app asked for, after this delay (zero: as soon as possible).
    Wake(Duration),
    Closed(String),
}

/// The longest repaint delay worth sleeping for: anything above is "no repaint" (egui sends `Duration::MAX`).
const REPAINT_CEILING: Duration = Duration::from_secs(3600);

/// A frame's rectangles and bytes, ready for the writer.
struct Frame {
    seq: u64,
    full: bool,
    headers: Vec<wire::RectHeader>,
    payload: Vec<u8>,
}

/// The server side of one session.
pub struct Host {
    app: ModelApp,
    ctx: egui::Context,
    painter: Box<dyn Painter>,
    cfg: Config,
    size_px: (u32, u32),
    dpr_used: f32,
    visible: bool,
    focused: bool,
    /// `hello` arrived: nothing is drawn or sent before it.
    greeted: bool,
    credits: u32,
    seq: u64,
    /// The last frame **sent**: what the diff compares against.
    prev: Option<Vec<u8>>,
    need_full: bool,
    damage: bool,
    last_frame: Option<Instant>,
    next_repaint: Option<Instant>,
    started: Instant,
    ops: Ops,
    batches: BTreeMap<u64, Batch>,
    pending: BTreeMap<u64, Receiver<Value>>,
}

impl Host {
    /// Assemble the app for this session. The painter's render state, when it has one, is the
    /// window's 3D viewport target, installed before the first pass.
    pub fn new(session: Session, cfg: Config, painter: Box<dyn Painter>) -> Result<Host, String> {
        let (w, h, dpr_used) = fit(cfg.width, cfg.height, cfg.dpr)?;
        let ctx = egui::Context::default();
        fieldcraft_ui_egui::theme::apply(&ctx, cfg.dark);
        // The host's window shows the workspace, not the desktop app's community and updater
        // chrome: no Donate/Discord, Report a bug, Update offer or version in the bar.
        let mut app = ModelApp::new(session).with_embedded();
        if let Some(rs) = painter.render_state() {
            // The whole-window pass is 1 sample with a depth buffer (the viewport's pipelines
            // expect one); the 3D viewport paints into this same pass.
            app.set_wgpu(rs, 24, 1, &ctx);
        }
        let mut painter = painter;
        painter.resize(w, h)?;
        Ok(Host {
            app,
            ctx,
            painter,
            cfg,
            size_px: (w, h),
            dpr_used,
            visible: true,
            focused: true,
            greeted: false,
            credits: START_CREDITS,
            seq: 0,
            prev: None,
            need_full: true,
            damage: true,
            last_frame: None,
            next_repaint: Some(Instant::now()),
            started: Instant::now(),
            ops: Ops::default(),
            batches: BTreeMap::new(),
            pending: BTreeMap::new(),
        })
    }

    /// Open a model at startup (`--open`), before any read-only policy is applied: opening is
    /// the host's own act.
    pub fn open(&mut self, path: &str) -> Result<Value, String> {
        self.app.run("file.open", json!({ "path": path }))
    }

    /// Serve `stream` until `stop` or the host goes away. `token` is the one `hello` must carry.
    pub fn serve(mut self, stream: TcpStream, token: String) -> Result<(), String> {
        let _ = stream.set_nodelay(true);
        let write = stream.try_clone().map_err(|e| format!("the socket: {e}"))?;
        let writer = Writer::new(write);
        let book = StopBook::default();
        let (tx, rx) = mpsc::channel();
        {
            let tx = tx.clone();
            self.ctx.set_request_repaint_callback(move |info| {
                let _ = tx.send(Event::Wake(info.delay));
            });
        }
        let reader = {
            let (token, tx, writer, book) = (token, tx.clone(), writer.clone(), book.clone());
            std::thread::spawn(move || read_loop(stream, token, tx, writer, book))
        };
        let result = self.run(&rx, &writer, &book);
        writer.shutdown();
        let _ = reader.join();
        result
    }

    fn run(&mut self, rx: &Receiver<Event>, writer: &Writer, book: &StopBook) -> Result<(), String> {
        loop {
            let now = Instant::now();
            let timeout = self.timeout(now);
            match rx.recv_timeout(timeout) {
                Ok(event) => {
                    if !self.event(event, writer, book)? {
                        return Ok(());
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
            // Take everything else that is already here into this pass.
            while let Ok(event) = rx.try_recv() {
                if !self.event(event, writer, book)? {
                    return Ok(());
                }
            }
            self.service(writer, book)?;
            self.frame_once(writer)?;
        }
    }

    /// Apply one event; `false` ends the session.
    fn event(&mut self, event: Event, writer: &Writer, book: &StopBook) -> Result<bool, String> {
        match event {
            Event::Hello => {
                self.greeted = true;
                writer
                    .send(
                        &json!({"t":"ready","v":wire::VERSION,"w":self.cfg.width,"h":self.cfg.height,"dpr":self.dpr_used,"pid":std::process::id()}),
                        &[],
                    )
                    .map_err(|e| e.to_string())?;
                self.dirty();
            }
            Event::Msg(msg) => return self.message(msg, writer, book),
            Event::Wake(delay) => {
                let at = Instant::now() + delay;
                self.next_repaint = Some(self.next_repaint.map_or(at, |t| t.min(at)));
            }
            Event::Closed(_why) => return Ok(false),
        }
        Ok(true)
    }

    fn message(&mut self, msg: HostMsg, writer: &Writer, book: &StopBook) -> Result<bool, String> {
        match msg {
            HostMsg::Hello { .. } => {
                // The reader authenticates the first hello; a second one is a protocol error.
                writer.send(&json!({"t":"error","error":"already greeted"}), &[]).map_err(|e| e.to_string())?;
                return Ok(false);
            }
            HostMsg::Input { events } => {
                for event in &events {
                    if let Some(Directive::Focus(gained)) = input::apply(&mut self.app, &self.ctx, event) {
                        self.focused = gained;
                    }
                }
                self.dirty();
            }
            HostMsg::Resize { w, h, dpr } => {
                self.cfg.width = w;
                self.cfg.height = h;
                self.cfg.dpr = dpr;
                let (width, height, used) = fit(w, h, dpr)?;
                self.size_px = (width, height);
                self.dpr_used = used;
                self.painter.resize(width, height)?;
                self.prev = None;
                self.need_full = true;
                self.dirty();
            }
            HostMsg::Visible { visible } => {
                self.visible = visible;
                if visible {
                    self.need_full = true;
                    self.dirty();
                }
            }
            HostMsg::Ack { seq: _ } => {
                self.credits = (self.credits + 1).min(START_CREDITS);
                if self.damage {
                    self.next_repaint = Some(Instant::now());
                }
            }
            HostMsg::Theme { dark } => self.theme(dark),
            HostMsg::Call { id, method, params, op } => {
                self.call(writer, book, id, method, params, op);
                self.dirty();
            }
            HostMsg::Stop { why } => {
                writer.send(&json!({"t":"bye","why": why.unwrap_or_else(|| "stop".into())}), &[]).map_err(|e| e.to_string())?;
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn call(&mut self, writer: &Writer, book: &StopBook, id: u64, method: String, params: Value, op: Option<String>) {
        let now = self.now_ms();
        let op = op.unwrap_or_else(|| format!("call/{id}"));
        match method.as_str() {
            "ui.theme" => return self.reply(writer, id, json!({"ok": true, "result": {"dark": self.cfg.dark}})),
            "ui.operations" => return self.operations(writer, book, id, &params),
            "engine.batch" => {
                return match Batch::new(op.clone(), &params) {
                    Ok(batch) => {
                        self.ops.begin(id, op, Kind::Sync, now);
                        self.batches.insert(id, batch);
                    }
                    Err(e) => self.reply(writer, id, json!({"ok": false, "error": e})),
                };
            }
            _ => {}
        }
        let started = Instant::now();
        match engine::dispatch(&mut self.app, &self.ctx, &method, params) {
            Dispatch::Done(v) => {
                let elapsed_ms = started.elapsed().as_millis() as u64;
                let mut v = v;
                // A synchronous command cannot be preempted: report the overrun when it returns.
                if elapsed_ms >= ops::STEP_BUDGET_MS
                    && let Some(o) = v.as_object_mut()
                {
                    o.insert("over_budget".into(), json!(true));
                    o.insert("elapsed_ms".into(), json!(elapsed_ms));
                }
                self.reply(writer, id, v);
            }
            Dispatch::Later(rx) => {
                self.ops.begin(id, op, Kind::Job, now);
                self.pending.insert(id, rx);
            }
        }
    }

    fn operations(&mut self, writer: &Writer, book: &StopBook, id: u64, params: &Value) {
        let now = self.now_ms();
        match params.get("cancel").and_then(Value::as_str) {
            None => {
                let list: Vec<Value> = self
                    .ops
                    .calls()
                    .map(|c| {
                        json!({"id": c.id, "op": c.op, "elapsed_ms": now.saturating_sub(c.started_ms), "kind": match c.kind { Kind::Sync => "sync", Kind::Job => "job" }})
                    })
                    .collect();
                self.reply(writer, id, json!({"ok": true, "result": {"operations": list, "batches": self.batches.len()}}));
            }
            Some("agent") => {
                // The whole turn: every in-flight call and every unstarted batch step.
                self.ops.agent_stop(id, now);
                let batches: Vec<u64> = self.batches.keys().copied().collect();
                for call in batches {
                    if let Some(batch) = self.batches.remove(&call) {
                        self.ops.end(call);
                        self.reply(writer, call, batch.stopped_reply("stopped: 整轮已停止（agent）"));
                    }
                }
                // The job's waiters answer `cancelled` at once; those replies are what confirms
                // the stop, so drain them before asking.
                self.app.cancel_loading();
                self.collect_pending(writer);
                self.confirm_stop(writer, book, id, now);
                self.dirty();
            }
            Some(want) => {
                let calls = self.ops.cancel_op(want);
                let had_job = calls.iter().any(|c| c.kind == Kind::Job);
                for call in &calls {
                    self.pending.remove(&call.id);
                    self.reply(writer, call.id, json!({"ok": false, "error": format!("cancelled: {want}"), "cancelled": true}));
                }
                let batches: Vec<u64> = self.batches.iter().filter(|(_, b)| b.op == want).map(|(id, _)| *id).collect();
                let had_batch = !batches.is_empty();
                for call in batches {
                    if let Some(batch) = self.batches.remove(&call) {
                        self.ops.end(call);
                        self.reply(writer, call, batch.stopped_reply(&format!("cancelled: {want}")));
                    }
                }
                // Only this operation's background work stops, not another turn's.
                if had_job || had_batch {
                    self.app.cancel_loading();
                }
                self.reply(writer, id, json!({"ok": true, "result": {"cancelled": want, "calls": calls.len()}}));
            }
        }
    }

    /// Take finished background replies and budget overruns; run batches; confirm stops.
    fn service(&mut self, writer: &Writer, book: &StopBook) -> Result<(), String> {
        let now = self.now_ms();
        self.collect_pending(writer);
        for call in self.ops.tick(now) {
            self.app.cancel_loading();
            self.pending.remove(&call.id);
            self.batches.remove(&call.id);
            self.reply(writer, call.id, json!({"ok": false, "error": "stopped: 单步预算 120 秒已用完", "code": "budget", "stopped": true}));
        }
        // A batch's budget is per step, timed from the step's start.
        let over: Vec<u64> =
            self.batches.iter().filter(|(_, b)| b.waiting_ms(now).is_some_and(|ms| ms >= ops::STEP_BUDGET_MS)).map(|(id, _)| *id).collect();
        for call in over {
            if let Some(batch) = self.batches.remove(&call) {
                self.app.cancel_loading();
                self.ops.end(call);
                self.reply(writer, call, batch.stopped_reply("stopped: 单步预算 120 秒已用完"));
            }
        }
        let running: Vec<u64> = self.batches.keys().copied().collect();
        for call in running {
            if let Some(mut batch) = self.batches.remove(&call) {
                match batch.poll(&mut self.app, &self.ctx, now) {
                    Some(reply) => {
                        self.ops.end(call);
                        self.reply(writer, call, reply);
                    }
                    None => {
                        self.batches.insert(call, batch);
                    }
                }
            }
        }
        let stops = self.ops.pending_stops();
        for stop in stops {
            self.confirm_stop(writer, book, stop, now);
        }
        Ok(())
    }

    fn collect_pending(&mut self, writer: &Writer) {
        let mut done = Vec::new();
        for (id, rx) in &self.pending {
            match rx.try_recv() {
                Ok(v) => done.push((*id, v)),
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    done.push((*id, json!({"ok": false, "error": "the engine stopped without an answer"})));
                }
            }
        }
        for (id, value) in done {
            self.pending.remove(&id);
            self.ops.end(id);
            self.reply(writer, id, value);
        }
    }

    /// Answer stop request `stop` once ops.rs can: every call answered, or the 5 s window
    /// closed. The stop book lets a reader watchdog answer a stuck loop instead.
    fn confirm_stop(&mut self, writer: &Writer, book: &StopBook, stop: u64, now: u64) {
        if let Some(reply) = self.ops.stop_reply(stop, now)
            && book.claim(stop)
        {
            self.reply(writer, stop, json!({"stopped": reply.stopped, "at": reply.at, "elapsed_ms": reply.elapsed_ms, "reason": reply.reason}));
        }
    }

    fn reply(&self, writer: &Writer, id: u64, result: Value) {
        if let Err(e) = writer.send(&json!({"t": "reply", "id": id, "result": result}), &[]) {
            log::warn!("reply {id}: {e}");
        }
    }

    fn timeout(&self, now: Instant) -> Duration {
        if !self.greeted {
            return Duration::from_millis(250);
        }
        if !self.batches.is_empty() || !self.pending.is_empty() {
            // Batches yield between their steps quickly; a background job's reply is taken in by
            // `logic` (which asks for a pass every 50 ms), so polling it can be slower.
            return if self.batches.is_empty() { Duration::from_millis(20) } else { Duration::from_millis(2) };
        }
        if self.credits == 0 {
            // Every credit is spent on a frame the host has not acknowledged: nothing new is
            // painted until one comes back (`ack` marks the accumulated damage dirty again).
            return Duration::from_millis(250);
        }
        let mut wait = Duration::from_millis(250);
        if let Some(at) = self.next_repaint {
            wait = wait.min(at.saturating_duration_since(now));
        }
        if let Some(last) = self.last_frame {
            let next = last + self.frame_interval();
            if next > now {
                wait = wait.min(next - now);
            }
        }
        wait.max(Duration::from_millis(1))
    }

    fn frame_interval(&self) -> Duration {
        let cap = if self.cfg.fps_cap.is_finite() { self.cfg.fps_cap.clamp(1.0, 240.0) } else { 60.0 };
        Duration::from_secs_f64(1.0 / f64::from(cap))
    }

    /// A page's `theme` message: in `auto`, a change repaints the whole window in the new theme
    /// (the clear colour and every widget move together). A fixed `--theme` ignores it.
    fn theme(&mut self, dark: bool) {
        if self.cfg.theme != ThemeMode::Auto || self.cfg.dark == dark {
            return;
        }
        self.cfg.dark = dark;
        fieldcraft_ui_egui::theme::apply(&self.ctx, dark);
        self.painter.set_dark(dark);
        self.need_full = true;
        self.dirty();
    }

    fn dirty(&mut self) {
        self.damage = true;
        self.next_repaint = Some(Instant::now());
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// Run a pass if one is due; paint it; send the changed rectangles if a credit is left.
    fn frame_once(&mut self, writer: &Writer) -> Result<(), String> {
        if !self.greeted {
            return Ok(());
        }
        let now = Instant::now();
        let busy = !self.pending.is_empty() || !self.batches.is_empty();
        if !self.visible && !busy {
            return Ok(());
        }
        let due = self.next_repaint.is_some_and(|at| at <= now) || self.damage || self.need_full;
        if !due {
            return Ok(());
        }
        if let Some(last) = self.last_frame
            && now < last + self.frame_interval()
        {
            self.next_repaint = Some(last + self.frame_interval());
            return Ok(());
        }
        // Painting without a credit would only repeat the same picture: keep the damage and
        // wait for an `ack`, unless a job's results must still be taken in (`busy`).
        if self.credits == 0 && !busy {
            self.next_repaint = None;
            return Ok(());
        }
        // This pass answers the requests made so far; one made during it (the viewport's
        // `repaint_delay`, a job's next poll) schedules the next.
        self.next_repaint = None;
        let (width, height) = self.size_px;
        let started = self.started;
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(self.cfg.width, self.cfg.height))),
            time: Some(now.duration_since(started).as_secs_f64()),
            focused: self.focused,
            max_texture_side: Some(self.painter.max_texture_side()),
            ..Default::default()
        };
        raw.viewports.insert(egui::ViewportId::ROOT, egui::ViewportInfo { native_pixels_per_point: Some(self.dpr_used), ..Default::default() });
        self.app.raw_input_hook(&self.ctx, &mut raw);
        let ctx = self.ctx.clone();
        let out = self.ctx.run_ui(raw, |ui| {
            self.app.logic(&ctx);
            self.app.ui(ui);
        });
        let ppp = out.pixels_per_point;
        let primitives = self.ctx.tessellate(out.shapes, ppp);
        // The app's own pacing (a job in progress asks for a pass every 50 ms).
        if let Some(viewport) = out.viewport_output.get(&egui::ViewportId::ROOT) {
            // egui asks for nothing with `Duration::MAX` ("no repaint"): adding it to an `Instant` overflows.
            let delay = viewport.repaint_delay;
            if !delay.is_zero() && delay < REPAINT_CEILING {
                let at = Instant::now() + delay;
                self.next_repaint = Some(self.next_repaint.map_or(at, |t| t.min(at)));
            }
        }
        let pixels = self.painter.paint(&primitives, out.textures_delta, ppp)?;
        self.last_frame = Some(now);
        self.damage = false;
        if !self.visible {
            return Ok(());
        }
        if pixels.len() != (width as usize) * (height as usize) * 4 {
            return Err(format!("the painter returned {} bytes for {width} × {height} RGBA", pixels.len()));
        }
        let mut full = self.need_full || self.prev.is_none();
        let whole = || vec![diff::full(width as usize, height as usize)];
        let mut rects = if full { whole() } else { diff::dirty(self.prev.as_deref(), &pixels, width as usize, height as usize) };
        if rects.len() > MAX_RECTS {
            full = true;
            rects = whole();
        }
        if rects.is_empty() {
            return Ok(());
        }
        if self.credits == 0 {
            self.damage = true;
            return Ok(());
        }
        let (headers, payload) = encode::encode(&pixels, width, height, &rects, full)?;
        self.seq += 1;
        let frame = Frame { seq: self.seq, full, headers, payload };
        self.send_frame(writer, frame)?;
        self.prev = Some(pixels);
        self.need_full = false;
        Ok(())
    }

    fn send_frame(&mut self, writer: &Writer, frame: Frame) -> Result<(), String> {
        let (width, height) = self.size_px;
        let header = json!({
            "t": "frame",
            "seq": frame.seq,
            "w": width,
            "h": height,
            "dpr": self.cfg.dpr,
            "dprUsed": self.dpr_used,
            "full": frame.full,
            "rects": frame.headers,
        });
        writer.send(&header, &frame.payload).map_err(|e| e.to_string())?;
        self.credits -= 1;
        Ok(())
    }
}

/// Constant-time token comparison (the length still differs early; the bytes do not).
fn token_matches(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

/// The reader: the first message must be `hello` with the token; then messages go to the loop.
fn read_loop(mut stream: TcpStream, token: String, tx: Sender<Event>, writer: Writer, book: StopBook) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut authed = false;
    loop {
        let (header, _payload) = match wire::read_message(&mut stream) {
            Ok(message) => message,
            Err(e) => {
                // A framing or JSON error leaves the stream's framing unknown: say what was
                // wrong, then close.
                if matches!(&e, wire::WireError::Protocol(_)) {
                    let _ = writer.send(&json!({"t": "error", "error": e.to_string()}), &[]);
                }
                let _ = tx.send(Event::Closed(e.to_string()));
                break;
            }
        };
        let msg = match wire::parse_host(&header) {
            Ok(msg) => msg,
            Err(e) => {
                let _ = writer.send(&json!({"t": "error", "error": e.to_string()}), &[]);
                break;
            }
        };
        if !authed {
            let HostMsg::Hello { v, token: sent } = msg else {
                let _ = writer.send(&json!({"t": "error", "error": "hello must be the first message"}), &[]);
                break;
            };
            if v != wire::VERSION {
                let _ = writer.send(&json!({"t": "error", "error": format!("protocol {v}, this server speaks {}", wire::VERSION)}), &[]);
                break;
            }
            if !token_matches(sent.as_bytes(), token.as_bytes()) {
                let _ = writer.send(&json!({"t": "error", "error": "bad token"}), &[]);
                break;
            }
            authed = true;
            let _ = stream.set_read_timeout(None);
            if tx.send(Event::Hello).is_err() {
                break;
            }
            continue;
        }
        // A whole-turn stop is confirmed by the loop; a loop stuck inside a synchronous command
        // cannot answer, so this reader answers `stopped: false` after the window closes.
        if let HostMsg::Call { id, ref method, ref params, .. } = msg
            && method == "ui.operations"
            && params.get("cancel").and_then(Value::as_str) == Some("agent")
        {
            let (writer, book) = (writer.clone(), book.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(ops::STOP_CONFIRM_MS));
                if book.claim(id) {
                    // The loop never confirmed: this side has no access to its clock, so `at`
                    // is null (PROTOCOL.md, the whole-turn stop).
                    let _ = writer.send(
                        &json!({"t":"reply","id":id,"result":{"stopped":false,"reason":"ui thread busy","at":Value::Null,"elapsed_ms":ops::STOP_CONFIRM_MS}}),
                        &[],
                    );
                }
            });
        }
        if tx.send(Event::Msg(msg)).is_err() {
            break;
        }
    }
    writer.shutdown();
    let _ = tx.send(Event::Closed("reader ended".into()));
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};

    use fieldcraft_engine::Session;

    use super::*;
    use crate::wire::{read_message, write_message};

    /// Draws a flat frame without a GPU; the top-left pixel encodes the pass number so two
    /// passes differ.
    struct StubPainter {
        w: u32,
        h: u32,
        frames: usize,
    }

    impl Painter for StubPainter {
        fn max_texture_side(&self) -> usize {
            4096
        }
        fn resize(&mut self, w: u32, h: u32) -> Result<(), String> {
            self.w = w;
            self.h = h;
            Ok(())
        }
        fn paint(&mut self, primitives: &[egui::ClippedPrimitive], _textures: egui::TexturesDelta, _ppp: f32) -> Result<Vec<u8>, String> {
            self.frames += 1;
            let mut img = vec![0u8; (self.w * self.h * 4) as usize];
            for px in img.chunks_mut(4) {
                px.copy_from_slice(&[24, 24, 28, 255]);
            }
            img[0] = (self.frames % 256) as u8;
            let _ = primitives;
            Ok(img)
        }
    }

    fn next(client: &mut TcpStream) -> (Value, Vec<u8>) {
        read_message(client).expect("a message from the server")
    }

    /// Acknowledge a frame the helper skips: a real host acks what it is shown, and without
    /// its acks the two-frame credit window would stall the frames the test waits for.
    fn ack_if_frame(out: &mut TcpStream, header: &Value) {
        if header.get("t").and_then(Value::as_str) == Some("frame")
            && let Some(seq) = header.get("seq")
        {
            let _ = write_message(out, &json!({ "t": "ack", "seq": seq }), &[]);
        }
    }

    fn read_type(client: &mut TcpStream, out: &mut TcpStream, t: &str) -> (Value, Vec<u8>) {
        for _ in 0..200 {
            let (header, payload) = next(client);
            if header.get("t").and_then(Value::as_str) == Some(t) {
                return (header, payload);
            }
            ack_if_frame(out, &header);
        }
        panic!("no `{t}` message");
    }

    fn read_reply(client: &mut TcpStream, out: &mut TcpStream, id: u64) -> Value {
        for _ in 0..200 {
            let (header, _) = next(client);
            if header.get("t").and_then(Value::as_str) == Some("reply") && header.get("id").and_then(Value::as_u64) == Some(id) {
                return header.get("result").cloned().unwrap_or(Value::Null);
            }
            ack_if_frame(out, &header);
        }
        panic!("no reply {id}");
    }

    #[test]
    fn hello_input_engine_execute_and_stop_end_to_end() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let painter = StubPainter { w: 0, h: 0, frames: 0 };
            let cfg = Config { width: 320.0, height: 200.0, dpr: 1.0, fps_cap: 120.0, dark: true, theme: ThemeMode::Auto };
            let host = Host::new(Session::default(), cfg, Box::new(painter)).unwrap();
            host.serve(stream, "secret".into())
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        let mut out = client.try_clone().unwrap();
        write_message(&mut out, &json!({"t":"hello","v":1,"token":"secret"}), &[]).unwrap();
        let (ready, _) = read_type(&mut client, &mut out, "ready");
        assert_eq!(ready["v"], 1);
        assert_eq!(ready["w"], 320.0);

        // The first frame is a full PNG, and a credit is spent.
        let (frame, payload) = read_type(&mut client, &mut out, "frame");
        assert_eq!(frame["full"], true);
        assert_eq!(frame["seq"], 1);
        assert_eq!(frame["w"], 320);
        let rects = frame["rects"].as_array().unwrap();
        assert_eq!(rects[0]["codec"], "png");
        assert_eq!(payload.len(), rects[0]["len"].as_u64().unwrap() as usize);
        write_message(&mut out, &json!({"t":"ack","seq":frame["seq"]}), &[]).unwrap();

        // Input reaches the app (a pointer move is queued for the next pass).
        write_message(&mut out, &json!({"t":"input","events":[{"k":"move","x":40.0,"y":30.0},{"k":"text","text":"hi"}]}), &[]).unwrap();
        let (frame, _) = read_type(&mut client, &mut out, "frame");
        assert_eq!(frame["full"], false, "a later frame is dirty-rect encoded");
        write_message(&mut out, &json!({"t":"ack","seq":frame["seq"]}), &[]).unwrap();

        // A synchronous engine call is answered with the control handler's own reply.
        write_message(
            &mut out,
            &json!({"t":"call","id":1,"method":"engine.execute","params":{"command":"model.units","params":{"system":"m-kg-s-N-Pa"}},"op":"turn/step"}),
            &[],
        )
        .unwrap();
        let reply = read_reply(&mut client, &mut out, 1);
        assert_eq!(reply["ok"], true, "{reply}");
        assert_eq!(reply["result"]["system"], "m-kg-s-N-Pa");

        // And the same Session answers the next call.
        write_message(&mut out, &json!({"t":"call","id":2,"method":"engine.execute","params":{"command":"model.info"}}), &[]).unwrap();
        let reply = read_reply(&mut client, &mut out, 2);
        assert_eq!(reply["ok"], true, "{reply}");
        assert_eq!(reply["result"]["nodes"], 0);

        write_message(&mut out, &json!({"t":"stop","why":"test"}), &[]).unwrap();
        let (bye, _) = read_type(&mut client, &mut out, "bye");
        assert_eq!(bye["why"], "test");
        server.join().unwrap().unwrap();
    }

    #[test]
    fn a_theme_message_repaints_the_window_full_and_ui_theme_reports_it() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let painter = StubPainter { w: 0, h: 0, frames: 0 };
            let cfg = Config { width: 320.0, height: 200.0, dpr: 1.0, fps_cap: 120.0, dark: true, theme: ThemeMode::Auto };
            Host::new(Session::default(), cfg, Box::new(painter)).unwrap().serve(stream, "secret".into())
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        let mut out = client.try_clone().unwrap();
        write_message(&mut out, &json!({"t":"hello","v":1,"token":"secret"}), &[]).unwrap();
        let (ready, _) = read_type(&mut client, &mut out, "ready");
        assert_eq!(ready["v"], 1);
        let (first, _) = read_type(&mut client, &mut out, "frame");
        assert_eq!(first["full"], true, "the first frame is full");
        write_message(&mut out, &json!({"t":"ack","seq":first["seq"]}), &[]).unwrap();

        // `ui.theme` speaks the applied theme (dark until the page says otherwise).
        write_message(&mut out, &json!({"t":"call","id":3,"method":"ui.theme"}), &[]).unwrap();
        let reply = read_reply(&mut client, &mut out, 3);
        assert_eq!(reply["ok"], true, "{reply}");
        assert_eq!(reply["result"]["dark"], true);

        // A page theme change asks for the whole window again (the clear colour moved with it). An earlier
        // dirty frame (the `ui.theme` call itself marks one) may still be in flight first: read on to the full one.
        write_message(&mut out, &json!({"t":"theme","dark":false}), &[]).unwrap();
        let mut themed = None;
        for _ in 0..50 {
            let (header, _) = next(&mut client);
            if header.get("t").and_then(Value::as_str) == Some("frame") {
                if let Some(seq) = header.get("seq") {
                    let _ = write_message(&mut out, &json!({"t":"ack","seq":seq}), &[]);
                }
                if header["full"].as_bool() == Some(true) {
                    themed = Some(header);
                    break;
                }
            }
        }
        let next = themed.expect("a full frame after a theme change");
        assert!(next["seq"].as_u64().unwrap() > first["seq"].as_u64().unwrap());

        write_message(&mut out, &json!({"t":"call","id":4,"method":"ui.theme"}), &[]).unwrap();
        let reply = read_reply(&mut client, &mut out, 4);
        assert_eq!(reply["result"]["dark"], false, "ui.theme follows the change");

        write_message(&mut out, &json!({"t":"stop","why":"test"}), &[]).unwrap();
        let (bye, _) = read_type(&mut client, &mut out, "bye");
        assert_eq!(bye["why"], "test");
        server.join().unwrap().unwrap();
    }

    #[test]
    fn a_fixed_theme_ignores_the_page() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let painter = StubPainter { w: 0, h: 0, frames: 0 };
            let cfg = Config { width: 320.0, height: 200.0, dpr: 1.0, fps_cap: 120.0, dark: true, theme: ThemeMode::Dark };
            Host::new(Session::default(), cfg, Box::new(painter)).unwrap().serve(stream, "secret".into())
        });
        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        let mut out = client.try_clone().unwrap();
        write_message(&mut out, &json!({"t":"hello","v":1,"token":"secret"}), &[]).unwrap();
        let _ = read_type(&mut client, &mut out, "ready");
        let (first, _) = read_type(&mut client, &mut out, "frame");
        write_message(&mut out, &json!({"t":"ack","seq":first["seq"]}), &[]).unwrap();
        write_message(&mut out, &json!({"t":"theme","dark":false}), &[]).unwrap();
        write_message(&mut out, &json!({"t":"call","id":5,"method":"ui.theme"}), &[]).unwrap();
        let reply = read_reply(&mut client, &mut out, 5);
        assert_eq!(reply["result"]["dark"], true, "a fixed --theme dark stays dark");
        write_message(&mut out, &json!({"t":"stop","why":"test"}), &[]).unwrap();
        let _ = read_type(&mut client, &mut out, "bye");
        server.join().unwrap().unwrap();
    }

    #[test]
    fn a_wrong_token_closes_the_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let painter = StubPainter { w: 0, h: 0, frames: 0 };
            let host = Host::new(Session::default(), Config::default(), Box::new(painter)).unwrap();
            host.serve(stream, "secret".into())
        });
        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut out = client.try_clone().unwrap();
        write_message(&mut out, &json!({"t":"hello","v":1,"token":"wrong"}), &[]).unwrap();
        let (error, _) = read_type(&mut client, &mut out, "error");
        assert_eq!(error["error"], "bad token");
        server.join().unwrap().unwrap();
    }

    #[test]
    fn a_screen_fits_or_lowers_its_dpr() {
        assert_eq!(fit(1280.0, 800.0, 1.0).unwrap(), (1280, 800, 1.0));
        assert_eq!(fit(100.0, 50.0, 2.0).unwrap(), (200, 100, 2.0));
        let (w, h, dpr) = fit(4000.0, 4000.0, 4.0).unwrap();
        assert!(w <= MAX_SIDE && h <= MAX_SIDE && u64::from(w) * u64::from(h) <= MAX_PIXELS);
        assert!((0.5..4.0).contains(&dpr));
        assert!(fit(f32::NAN, 100.0, 1.0).is_err());
        assert!(fit(100.0, 100.0, 0.0).is_ok(), "dpr is clamped into range");
    }

    #[test]
    fn the_token_comparison_is_exact() {
        assert!(token_matches(b"abc", b"abc"));
        assert!(!token_matches(b"abc", b"abd"));
        assert!(!token_matches(b"abc", b"ab"));
        assert!(!token_matches(b"", b"x"));
    }
}
