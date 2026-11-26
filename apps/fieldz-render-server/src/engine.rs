//! The engine bridge: a `call` from the host becomes a [`ControlRequest`] for the app's own
//! control handler, and `engine.batch` loops steps here.
//!
//! Nothing in the engine's semantics is re-implemented: `engine.execute` (long commands as
//! background jobs, `"wait": false`, the deferred reply), `engine.script`, `engine.commands`,
//! `job.*` and every `ui.*` method go through `fieldcraft_ui_egui::control::handle`, the same
//! function the desktop's control channel calls. `engine.batch` and `ui.operations` are the two
//! methods this server adds.

use std::sync::mpsc::{Receiver, TryRecvError};

use fieldcraft_ui_egui::ModelApp;
use fieldcraft_ui_egui::control::{self, ControlRequest, Outcome};
use serde_json::{Value, json};

/// Most steps in one `engine.batch` (the same cap as the MCP server's batch).
pub const MAX_STEPS: usize = 10_000;
/// How many synchronous steps one poll may run before yielding to the frame loop, so a long
/// batch of quick commands still lets the UI draw (and see `stop`).
pub const MAX_STEPS_PER_POLL: usize = 64;

/// A control request's answer: now, or later through a background job or a screenshot.
pub enum Dispatch {
    Done(Value),
    Later(Receiver<Value>),
}

/// Run `method` through the app's control handler. `params` is the wire `call`'s `params`.
pub fn dispatch(app: &mut ModelApp, ctx: &egui::Context, method: &str, params: Value) -> Dispatch {
    let (tx, rx) = std::sync::mpsc::channel();
    match control::handle(app, ctx, &ControlRequest::new(method, params, tx)) {
        Outcome::Done(v) => Dispatch::Done(v),
        // A background job (`Pending`) answers when it finishes; a screenshot answers once a
        // frame has been presented. The app's `logic` polls both each pass.
        Outcome::Pending | Outcome::Screenshot { .. } => Dispatch::Later(rx),
    }
}

/// `engine.batch`: steps run in order; the first failure stops the batch (there is no rollback:
/// the steps already done stay done, as the MCP server's batch and `Session::run_script`'s).
#[derive(Debug)]
pub struct Batch {
    pub op: String,
    steps: Vec<Value>,
    next: usize,
    pending: Option<Receiver<Value>>,
    /// When the running step started (a job's wait is budgeted from here).
    waiting_since_ms: Option<u64>,
    results: Vec<Value>,
    failed: Option<usize>,
}

impl Batch {
    /// Parse `engine.batch`'s params: `{"steps":[{"command": id, "params": {…}, "wait"?: bool}, …]}`.
    pub fn new(op: impl Into<String>, params: &Value) -> Result<Batch, String> {
        let steps = params.get("steps").and_then(Value::as_array).ok_or("missing `steps` (a list of {command, params})")?;
        if steps.len() > MAX_STEPS {
            return Err(format!("{} steps exceeds the {MAX_STEPS} a batch takes", steps.len()));
        }
        for (i, step) in steps.iter().enumerate() {
            if !step.is_object() || step.get("command").and_then(Value::as_str).is_none() {
                return Err(format!("step {i} has no `command`"));
            }
        }
        Ok(Batch { op: op.into(), steps: steps.clone(), next: 0, pending: None, waiting_since_ms: None, results: Vec::new(), failed: None })
    }

    /// How long the step now running has waited, in milliseconds.
    pub fn waiting_ms(&self, now_ms: u64) -> Option<u64> {
        self.waiting_since_ms.map(|t| now_ms.saturating_sub(t))
    }

    /// Advance the batch: start the next step, or take a finished step's reply. `Some(reply)` is
    /// the batch's final answer; `None` means a step is still running (or the poll's share of
    /// steps is used — poll again next frame).
    pub fn poll(&mut self, app: &mut ModelApp, ctx: &egui::Context, now_ms: u64) -> Option<Value> {
        if let Some(index) = self.failed {
            return Some(self.failed_reply(index, &json!({"ok": false, "error": "batch already failed"})));
        }
        let mut ran = 0;
        loop {
            if let Some(rx) = &self.pending {
                match rx.try_recv() {
                    Ok(v) => {
                        self.pending = None;
                        self.waiting_since_ms = None;
                        let index = self.next - 1;
                        if v.get("ok").and_then(Value::as_bool) == Some(true) {
                            self.results.push(v.get("result").cloned().unwrap_or(Value::Null));
                        } else {
                            self.failed = Some(index);
                            return Some(self.failed_reply(index, &v));
                        }
                    }
                    Err(TryRecvError::Empty) => return None,
                    Err(TryRecvError::Disconnected) => {
                        self.pending = None;
                        self.waiting_since_ms = None;
                        let index = self.next - 1;
                        self.failed = Some(index);
                        return Some(self.failed_reply(index, &json!({"ok": false, "error": "the engine stopped without an answer"})));
                    }
                }
            }
            if self.next >= self.steps.len() {
                return Some(json!({"ok": true, "completed": self.next, "results": self.results}));
            }
            if ran >= MAX_STEPS_PER_POLL {
                return None;
            }
            let step = self.steps[self.next].clone();
            let index = self.next;
            self.next += 1;
            ran += 1;
            self.waiting_since_ms = Some(now_ms);
            match dispatch(app, ctx, "engine.execute", step) {
                Dispatch::Done(v) => {
                    self.waiting_since_ms = None;
                    if v.get("ok").and_then(Value::as_bool) == Some(true) {
                        self.results.push(v.get("result").cloned().unwrap_or(Value::Null));
                    } else {
                        self.failed = Some(index);
                        return Some(self.failed_reply(index, &v));
                    }
                }
                Dispatch::Later(rx) => {
                    self.pending = Some(rx);
                    return None;
                }
            }
        }
    }

    /// The batch's reply when step `index` failed (or when the whole turn stopped it).
    pub fn failed_reply(&self, index: usize, error: &Value) -> Value {
        let command = self.steps.get(index).and_then(|s| s.get("command")).cloned().unwrap_or(Value::Null);
        json!({"ok": false, "completed": index, "failed": {"command": command, "error": error}, "results": self.results})
    }

    /// The reply when the turn was stopped or the step's budget ran out.
    pub fn stopped_reply(&self, error: &str) -> Value {
        let index = self.next.saturating_sub(1).min(self.steps.len().saturating_sub(1));
        self.failed_reply(index, &json!({"ok": false, "error": error}))
    }
}

#[cfg(test)]
mod tests {
    use fieldcraft_engine::Session;

    use super::*;

    fn app() -> (ModelApp, egui::Context) {
        (ModelApp::new(Session::default()), egui::Context::default())
    }

    fn batch(steps: Value) -> Batch {
        Batch::new("turn/test", &json!({ "steps": steps })).unwrap()
    }

    #[test]
    fn a_batch_runs_its_steps_in_order() {
        let (mut app, ctx) = app();
        let mut b = batch(json!([
            {"command": "model.units", "params": {"system": "m-kg-s-N-Pa"}},
            {"command": "model.info", "params": {}}
        ]));
        let reply = b.poll(&mut app, &ctx, 0).expect("both steps answer at once");
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["completed"], 2);
        assert_eq!(reply["results"].as_array().unwrap().len(), 2);
        assert_eq!(reply["results"][0]["system"], "m-kg-s-N-Pa", "{reply}");
    }

    #[test]
    fn the_first_failure_stops_the_batch() {
        let (mut app, ctx) = app();
        let mut b = batch(json!([
            {"command": "model.units", "params": {"system": "m-kg-s-N-Pa"}},
            {"command": "model.rename", "params": {"kind": "step", "from": "Nope", "to": "Nope2"}},
            {"command": "model.info", "params": {}}
        ]));
        let reply = b.poll(&mut app, &ctx, 0).expect("a refused step answers at once");
        assert_eq!(reply["ok"], false);
        assert_eq!(reply["completed"], 1);
        assert_eq!(reply["failed"]["command"], "model.rename");
        assert_eq!(reply["results"].as_array().unwrap().len(), 1);
        // Polling again does not run the third step.
        assert_eq!(b.poll(&mut app, &ctx, 1).unwrap()["completed"], 1);
    }

    #[test]
    fn a_malformed_batch_is_refused() {
        assert!(Batch::new("turn/test", &json!({})).unwrap_err().contains("steps"));
        assert!(Batch::new("turn/test", &json!({"steps": [{"params": {}}]})).unwrap_err().contains("command"));
        let too_many: Vec<Value> = (0..=MAX_STEPS).map(|_| json!({"command": "model.info"})).collect();
        assert!(Batch::new("turn/test", &json!({"steps": too_many})).unwrap_err().contains("10000"));
    }

    #[test]
    fn an_empty_batch_is_done() {
        let (mut app, ctx) = app();
        let mut b = batch(json!([]));
        let reply = b.poll(&mut app, &ctx, 0).unwrap();
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["completed"], 0);
    }

    #[test]
    fn a_stopped_batch_names_its_failed_step() {
        let b = batch(json!([{"command": "model.info"}, {"command": "model.info"}]));
        let reply = b.stopped_reply("stopped: 单步预算 120 秒已用完");
        assert_eq!(reply["ok"], false);
        assert_eq!(reply["failed"]["command"], "model.info");
        assert!(reply["failed"]["error"]["error"].as_str().unwrap().contains("stopped"));
    }
}
