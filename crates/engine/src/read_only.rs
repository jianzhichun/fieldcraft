//! The host's read-only mode.
//!
//! An embedding host (the render server, a viewer) opens a session that must not write: no
//! file, no undo history, no document replacement. Two mechanisms cover the commands, because
//! they differ in what they can take back:
//!
//! - [`blocked`] refuses, before anything runs, the commands that write without a snapshot to
//!   restore: saving, undo/redo, opening or replacing the document, deck import/export and the
//!   export/image writers ([`Session::execute`] returns [`EngineError::Disabled`]).
//! - a Model command *is* run (only it takes a snapshot), and when it changed the document
//!   [`Session::execute`] turns its result into [`EngineError::Disabled`]; the existing error
//!   path restores the snapshot and the crash journal records a failure, which a replay never
//!   repeats. A Model command that changes nothing but results (a solve) is let through.
//!
//! The reason is process-wide, not thread-local: a long command is applied on a worker thread
//! on a clone of the session, where a thread-local set on the UI thread would be invisible.

use std::sync::{Mutex, OnceLock};

/// Commands that write the session or a file and take no snapshot a failure could restore.
const WRITES: &[&str] = &[
    "file.new",
    "file.open",
    "file.save",
    "file.save_as",
    "file.sample",
    "file.recover",
    "file.recovery.discard",
    "edit.undo",
    "edit.redo",
    "deck.read",
    "deck.write",
    "results.render.save",
    "view.image.save",
    "mesh.export.stl",
];

fn cell() -> &'static Mutex<Option<String>> {
    static CELL: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

fn lock() -> std::sync::MutexGuard<'static, Option<String>> {
    cell().lock().unwrap_or_else(|e| e.into_inner())
}

/// The reason the session is read-only, if it is.
pub fn reason() -> Option<String> {
    lock().clone()
}

/// Make the session read-only with `reason` (shown to the user and to agents).
pub fn set(reason: impl Into<String>) {
    *lock() = Some(reason.into());
}

/// Let the session write again.
pub fn clear() {
    *lock() = None;
}

/// Whether `id` is a command that writes without a snapshot (`WRITES`, or a
/// `results.export.*`).
pub fn is_write(id: &str) -> bool {
    WRITES.contains(&id) || id.starts_with("results.export.")
}

/// The reason to refuse `id` with, when the session is read-only and `id` writes without a
/// snapshot to restore.
pub fn blocked(id: &str) -> Option<String> {
    reason().filter(|_| is_write(id))
}
