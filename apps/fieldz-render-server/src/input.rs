//! The compact `input` message translated into the UI's own control calls.
//!
//! The mapping is not rewritten here: each wire event becomes one of the `ui.*` methods of
//! `fieldcraft_ui_egui::control` (`ui.move`, `ui.click`, `ui.drag`, `ui.scroll`, `ui.key`,
//! `ui.text`, `ui.drop`), which push `egui::Event`s into [`ModelApp::synthetic`]; the app's
//! `raw_input_hook` feeds them to egui one pointer event or one key sequence per frame. That
//! keeps a single set of rules (control.rs:557-624) for the window, the control channel and
//! this server.
//!
//! v1 contracts (PROTOCOL.md):
//! - `press` and `release` are the two halves of a live pointer gesture: they queue an
//!   `egui::Event::PointerButton` (pressed / released) for the next pass, never `ui.click`.
//!   `move` carries the pointer while a button is held. They arrive as they happen, so the
//!   app sees a drag frame by frame instead of one whole-drag jump.
//! - `button` is still a whole click: the control method produces the press and the release, so
//!   a `button` with `down: false` is accepted and ignored (send one `button` per click). A
//!   host that sends `press`/`release` should not send `button` for the same gesture.
//! - `key` is a whole tap for the same reason; `down: false` is ignored. `physical` is
//!   accepted but unused in v1 (`ui.key` sends no physical key).
//! - `wheel` carries egui's `delta` in lines; `dx` is accepted and dropped (`ui.scroll`
//!   scrolls vertically only in v1). A wheel is applied alone in its own pass.
//! - `focus` is not a control call: it sets `RawInput.focused`, which the host loop owns.

use fieldcraft_ui_egui::ModelApp;
use fieldcraft_ui_egui::control::{self, ControlRequest, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Raw modifiers as the guest reports them; the server decides what "command" means.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub meta: bool,
}

/// One input event, as the host sends it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "lowercase")]
pub enum InputEvent {
    /// Pointer moved to `(x, y)` in CSS points, whether a button is held or not.
    Move { x: f32, y: f32 },
    /// A button went down at `(x, y)`: the first half of a live click or drag.
    Press {
        x: f32,
        y: f32,
        #[serde(default)]
        button: Option<String>,
        #[serde(default)]
        modifiers: Modifiers,
    },
    /// A button came up at `(x, y)`: the second half.
    Release {
        x: f32,
        y: f32,
        #[serde(default)]
        button: Option<String>,
        #[serde(default)]
        modifiers: Modifiers,
    },
    /// A whole click at `(x, y)`: press and release.
    Button {
        x: f32,
        y: f32,
        #[serde(default)]
        button: Option<String>,
        #[serde(default)]
        down: bool,
        #[serde(default)]
        double: bool,
        #[serde(default)]
        modifiers: Modifiers,
    },
    /// A wheel at `(x, y)`; `dy` is egui's delta in lines (positive scrolls down).
    Wheel {
        x: f32,
        y: f32,
        #[serde(default)]
        dx: f32,
        #[serde(default)]
        dy: f32,
        #[serde(default)]
        unit: Option<String>,
        #[serde(default)]
        modifiers: Modifiers,
    },
    /// A whole key tap; `key` is an `egui::Key` name (`"ArrowDown"`, `"A"`, …).
    Key {
        key: String,
        #[serde(default)]
        physical: Option<String>,
        #[serde(default)]
        down: bool,
        #[serde(default)]
        modifiers: Modifiers,
    },
    /// Typed or composed text.
    Text { text: String },
    /// A drag from `from` to `to` (CSS points), interpolated in `steps` moves.
    Drag {
        from: [f32; 2],
        to: [f32; 2],
        #[serde(default)]
        steps: Option<f32>,
        #[serde(default)]
        button: Option<String>,
        #[serde(default)]
        modifiers: Modifiers,
    },
    /// Files dropped on the window.
    Drop { paths: Vec<String> },
    /// The host's window gained or lost the keyboard.
    Focus { gained: bool },
}

/// What an event does to the host loop itself rather than to the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Directive {
    Focus(bool),
}

/// Apply one event to `app`: a control call, or a [`Directive`] for the loop.
///
/// A refused mapping (an unknown key, an empty drop) is logged and ignored: one bad event
/// does not close the session.
pub fn apply(app: &mut ModelApp, ctx: &egui::Context, event: &InputEvent) -> Option<Directive> {
    match event {
        InputEvent::Move { x, y } => {
            call(app, ctx, "ui.move", json!({ "x": x, "y": y }));
            None
        }
        InputEvent::Press { x, y, button, modifiers } => {
            push_button(app, *x, *y, button.as_deref(), true, *modifiers);
            None
        }
        InputEvent::Release { x, y, button, modifiers } => {
            push_button(app, *x, *y, button.as_deref(), false, *modifiers);
            None
        }
        InputEvent::Button { x, y, button, down, double, modifiers } => {
            if *down {
                let mut params = json!({ "x": x, "y": y, "double": double });
                put(&mut params, "button", button);
                put_mods(&mut params, modifiers);
                call(app, ctx, "ui.click", params);
            }
            None
        }
        InputEvent::Wheel { x, y, dx: _, dy, unit: _, modifiers: _ } => {
            let params = json!({ "x": x, "y": y, "delta": dy });
            call(app, ctx, "ui.scroll", params);
            None
        }
        InputEvent::Key { key, physical: _, down, modifiers } => {
            if *down {
                let mut params = json!({ "key": key });
                put_mods(&mut params, modifiers);
                // control.rs reads `cmd` as the command modifier and sets both `command` and
                // `ctrl` from it; `meta` is the command key on macOS, ctrl elsewhere.
                let command = if cfg!(target_os = "macos") { modifiers.meta } else { modifiers.ctrl };
                params["cmd"] = json!(command);
                params["ctrl"] = json!(modifiers.ctrl);
                params["shift"] = json!(modifiers.shift);
                params["alt"] = json!(modifiers.alt);
                call(app, ctx, "ui.key", params);
            }
            None
        }
        InputEvent::Text { text } => {
            call(app, ctx, "ui.text", json!({ "text": text }));
            None
        }
        InputEvent::Drag { from, to, steps, button, modifiers } => {
            let mut params = json!({ "x0": from[0], "y0": from[1], "x1": to[0], "y1": to[1], "steps": steps });
            put(&mut params, "button", button);
            put_mods(&mut params, modifiers);
            call(app, ctx, "ui.drag", params);
            None
        }
        InputEvent::Drop { paths } => {
            call(app, ctx, "ui.drop", json!({ "paths": paths }));
            None
        }
        InputEvent::Focus { gained } => Some(Directive::Focus(*gained)),
    }
}

/// Queue a live pointer button event. The app's `raw_input_hook` feeds it to egui in its own
/// pass (preceded by the pointer at that point), so a press and a release never share one.
fn push_button(app: &mut ModelApp, x: f32, y: f32, button: Option<&str>, pressed: bool, modifiers: Modifiers) {
    app.synthetic.push(egui::Event::PointerButton {
        pos: egui::pos2(x, y),
        button: pointer_button(button),
        pressed,
        modifiers: pointer_mods(modifiers),
    });
}

fn pointer_button(name: Option<&str>) -> egui::PointerButton {
    match name {
        Some("right") => egui::PointerButton::Secondary,
        Some("middle") => egui::PointerButton::Middle,
        _ => egui::PointerButton::Primary,
    }
}

/// egui's modifiers for a pointer event: `command` is meta on macOS and ctrl elsewhere, as `ui.key`.
fn pointer_mods(m: Modifiers) -> egui::Modifiers {
    let command = if cfg!(target_os = "macos") { m.meta } else { m.ctrl };
    egui::Modifiers { shift: m.shift, ctrl: m.ctrl, alt: m.alt, command, ..Default::default() }
}

fn put<T: Serialize>(params: &mut Value, key: &str, value: &Option<T>) {
    if let Some(v) = value {
        params[key] = json!(v);
    }
}

fn put_mods(params: &mut Value, m: &Modifiers) {
    params["shift"] = json!(m.shift);
    params["ctrl"] = json!(m.ctrl);
    params["alt"] = json!(m.alt);
}

fn call(app: &mut ModelApp, ctx: &egui::Context, method: &str, params: Value) -> Option<Value> {
    let (tx, _rx) = std::sync::mpsc::channel();
    match control::handle(app, ctx, &ControlRequest::new(method, params, tx)) {
        Outcome::Done(v) => {
            if v.get("ok").and_then(Value::as_bool) == Some(false) {
                log::warn!("{method} refused an input event: {}", v.get("error").and_then(Value::as_str).unwrap_or("?"));
            }
            Some(v)
        }
        Outcome::Pending | Outcome::Screenshot { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use fieldcraft_engine::Session;

    use super::*;

    fn app() -> (ModelApp, egui::Context) {
        (ModelApp::new(Session::default()), egui::Context::default())
    }

    fn apply_all(events: &[InputEvent]) -> (ModelApp, egui::Context, Vec<Option<Directive>>) {
        let (mut app, ctx) = app();
        let directives = events.iter().map(|e| apply(&mut app, &ctx, e)).collect();
        (app, ctx, directives)
    }

    /// Run `raw_input_hook` until its queue is empty, recording each pass's events.
    fn passes(app: &mut ModelApp, ctx: &egui::Context) -> Vec<Vec<egui::Event>> {
        let mut out = Vec::new();
        while !app.synthetic.is_empty() {
            let mut raw = egui::RawInput::default();
            app.raw_input_hook(ctx, &mut raw);
            out.push(raw.events);
        }
        out
    }

    /// The pointer and button events of a pass: `ModifiersChanged` is egui bookkeeping, not the
    /// gesture.
    fn pointer(events: &[egui::Event]) -> Vec<egui::Event> {
        events.iter().filter(|e| matches!(e, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. })).cloned().collect()
    }

    fn moved(x: f32, y: f32) -> egui::Event {
        egui::Event::PointerMoved(egui::pos2(x, y))
    }

    #[test]
    fn a_live_press_moves_and_release_keep_their_order_across_passes() {
        let (mut app, ctx) = app();
        let mut all: Vec<Vec<egui::Event>> = Vec::new();
        let mut record = |app: &mut ModelApp| {
            for pass in passes(app, &ctx) {
                all.push(pointer(&pass));
            }
        };
        let press = InputEvent::Press { x: 10.0, y: 20.0, button: Some("left".into()), modifiers: Modifiers::default() };
        let release = InputEvent::Release { x: 50.0, y: 20.0, button: Some("left".into()), modifiers: Modifiers::default() };
        apply(&mut app, &ctx, &press);
        record(&mut app);
        apply(&mut app, &ctx, &InputEvent::Move { x: 30.0, y: 20.0 });
        record(&mut app);
        apply(&mut app, &ctx, &InputEvent::Move { x: 50.0, y: 20.0 });
        record(&mut app);
        apply(&mut app, &ctx, &release);
        record(&mut app);

        let button = |pos: egui::Pos2, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        // The presses are direct egui events, not `ui.click`; the moves between are delivered in
        // order, one pass each; the press and the release never share a pass.
        assert_eq!(
            all,
            vec![
                vec![moved(10.0, 20.0), button(egui::pos2(10.0, 20.0), true)],
                vec![moved(30.0, 20.0)],
                vec![moved(50.0, 20.0)],
                vec![moved(50.0, 20.0), button(egui::pos2(50.0, 20.0), false)],
            ]
        );
    }

    #[test]
    fn a_move_is_one_pointer_moved() {
        let (app, _, _) = apply_all(&[InputEvent::Move { x: 10.0, y: 20.0 }]);
        assert_eq!(app.synthetic, vec![egui::Event::PointerMoved(egui::pos2(10.0, 20.0))]);
        assert!(app.synthetic_drops.is_empty());
    }

    #[test]
    fn a_click_is_a_move_press_and_release() {
        let (app, _, _) = apply_all(&[InputEvent::Button {
            x: 1.0,
            y: 2.0,
            button: Some("right".into()),
            down: true,
            double: false,
            modifiers: Modifiers::default(),
        }]);
        assert_eq!(app.synthetic.len(), 3);
        assert_eq!(app.synthetic[0], egui::Event::PointerMoved(egui::pos2(1.0, 2.0)));
        assert!(matches!(app.synthetic[1], egui::Event::PointerButton { button: egui::PointerButton::Secondary, pressed: true, .. }));
        assert!(matches!(app.synthetic[2], egui::Event::PointerButton { pressed: false, .. }));
    }

    #[test]
    fn a_release_only_button_is_ignored() {
        let (app, _, _) =
            apply_all(&[InputEvent::Button { x: 1.0, y: 2.0, button: None, down: false, double: false, modifiers: Modifiers::default() }]);
        assert!(app.synthetic.is_empty(), "ui.click already produces the release; one click per `button`");
    }

    #[test]
    fn a_double_click_is_two_click_pairs() {
        let (app, _, _) =
            apply_all(&[InputEvent::Button { x: 0.0, y: 0.0, button: None, down: true, double: true, modifiers: Modifiers::default() }]);
        // One move, then two press/release pairs.
        assert_eq!(app.synthetic.len(), 5);
    }

    #[test]
    fn a_wheel_is_its_delta_in_lines() {
        let (app, _, _) =
            apply_all(&[InputEvent::Wheel { x: 3.0, y: 4.0, dx: 9.0, dy: -2.0, unit: Some("line".into()), modifiers: Modifiers::default() }]);
        assert_eq!(app.synthetic.len(), 2);
        assert_eq!(app.synthetic[0], egui::Event::PointerMoved(egui::pos2(3.0, 4.0)));
        match &app.synthetic[1] {
            egui::Event::MouseWheel { delta, unit, .. } => {
                assert_eq!(*delta, egui::vec2(0.0, -2.0));
                assert_eq!(*unit, egui::MouseWheelUnit::Line);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_key_tap_is_press_and_release_with_modifiers() {
        let mods = Modifiers { alt: false, ctrl: true, shift: true, meta: true };
        let (app, _, _) = apply_all(&[InputEvent::Key { key: "A".into(), physical: Some("KeyA".into()), down: true, modifiers: mods }]);
        assert_eq!(app.synthetic.len(), 2);
        match (&app.synthetic[0], &app.synthetic[1]) {
            (egui::Event::Key { key, pressed: true, modifiers, .. }, egui::Event::Key { pressed: false, .. }) => {
                assert_eq!(*key, egui::Key::A);
                assert!(modifiers.shift);
                // Command is meta on macOS and ctrl elsewhere; with both held it is set on both.
                assert!(modifiers.command);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_key_release_is_ignored() {
        let (app, _, _) = apply_all(&[InputEvent::Key { key: "A".into(), physical: None, down: false, modifiers: Modifiers::default() }]);
        assert!(app.synthetic.is_empty());
    }

    #[test]
    fn an_unknown_key_is_refused_without_panicking() {
        let (app, _, _) = apply_all(&[InputEvent::Key { key: "NotAKey".into(), physical: None, down: true, modifiers: Modifiers::default() }]);
        assert!(app.synthetic.is_empty());
    }

    #[test]
    fn text_is_one_event() {
        let (app, _, _) = apply_all(&[InputEvent::Text { text: "héllo".into() }]);
        assert_eq!(app.synthetic, vec![egui::Event::Text("héllo".into())]);
    }

    #[test]
    fn a_drag_is_interpolated() {
        let (app, _, _) =
            apply_all(&[InputEvent::Drag { from: [0.0, 0.0], to: [10.0, 0.0], steps: Some(4.0), button: None, modifiers: Modifiers::default() }]);
        // move, press, four moves, release.
        assert_eq!(app.synthetic.len(), 7);
        assert!(matches!(app.synthetic[1], egui::Event::PointerButton { pressed: true, .. }));
        assert!(matches!(app.synthetic[6], egui::Event::PointerButton { pressed: false, .. }));
        match &app.synthetic[5] {
            egui::Event::PointerMoved(p) => assert_eq!(*p, egui::pos2(10.0, 0.0)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_whole_click_still_works() {
        let (mut app, ctx) = app();
        apply(&mut app, &ctx, &InputEvent::Button { x: 5.0, y: 6.0, button: None, down: true, double: false, modifiers: Modifiers::default() });
        let button = |pressed| egui::Event::PointerButton {
            pos: egui::pos2(5.0, 6.0),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        // The control method still produces the whole click: move, press, release, each its own pass.
        let seq: Vec<Vec<egui::Event>> = passes(&mut app, &ctx).iter().map(|p| pointer(p)).collect();
        assert_eq!(seq, vec![vec![moved(5.0, 6.0)], vec![moved(5.0, 6.0), button(true)], vec![moved(5.0, 6.0), button(false)]]);
    }

    #[test]
    fn a_live_drag_delivers_at_most_64_moves_a_pass_in_order() {
        let (mut app, ctx) = app();
        for i in 0..70 {
            apply(&mut app, &ctx, &InputEvent::Move { x: i as f32, y: 0.0 });
        }
        let passes = passes(&mut app, &ctx);
        assert_eq!(passes.len(), 2, "70 moves are two passes");
        let positions = |p: &[egui::Event]| -> Vec<f32> {
            pointer(p)
                .into_iter()
                .filter_map(|e| match e {
                    egui::Event::PointerMoved(p) => Some(p.x),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(positions(&passes[0]), (0..64).map(|i| i as f32).collect::<Vec<_>>());
        assert_eq!(positions(&passes[1]), (64..70).map(|i| i as f32).collect::<Vec<_>>());
    }

    #[test]
    fn wire_press_and_release_map_to_pointer_buttons() {
        let press: InputEvent = serde_json::from_value(json!({"k":"press","x":1.0,"y":2.0,"button":"right","modifiers":{"shift":true}})).unwrap();
        assert_eq!(
            press,
            InputEvent::Press { x: 1.0, y: 2.0, button: Some("right".into()), modifiers: Modifiers { shift: true, ..Default::default() } }
        );
        let (app, _, _) = apply_all(&[press]);
        match &app.synthetic[..] {
            [egui::Event::PointerButton { button: egui::PointerButton::Secondary, pressed: true, modifiers, .. }] => assert!(modifiers.shift),
            other => panic!("{other:?}"),
        }
        let release: InputEvent = serde_json::from_value(json!({"k":"release","x":1.0,"y":2.0})).unwrap();
        assert_eq!(release, InputEvent::Release { x: 1.0, y: 2.0, button: None, modifiers: Modifiers::default() });
        let (app, _, _) = apply_all(&[release]);
        assert!(matches!(app.synthetic[..], [egui::Event::PointerButton { pressed: false, .. }]));
    }

    #[test]
    fn a_drop_goes_to_the_synthetic_drops() {
        let (app, _, _) = apply_all(&[InputEvent::Drop { paths: vec!["/tmp/a.step".into()] }]);
        assert_eq!(app.synthetic_drops, vec!["/tmp/a.step".to_string()]);
    }

    #[test]
    fn focus_is_a_directive_not_a_control_call() {
        let (app, _, directives) = apply_all(&[InputEvent::Focus { gained: false }]);
        assert_eq!(directives, vec![Some(Directive::Focus(false))]);
        assert!(app.synthetic.is_empty());
    }

    #[test]
    fn wire_json_maps_to_the_same_events() {
        let text = json!({"k":"drag","from":[1.0,2.0],"to":[3.0,4.0],"steps":2.0,"button":"middle","modifiers":{"shift":true}});
        let event: InputEvent = serde_json::from_value(text).unwrap();
        assert_eq!(
            event,
            InputEvent::Drag {
                from: [1.0, 2.0],
                to: [3.0, 4.0],
                steps: Some(2.0),
                button: Some("middle".into()),
                modifiers: Modifiers { shift: true, ..Default::default() }
            }
        );
    }
}
