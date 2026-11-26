//! The read-only hook (`fieldcraft_engine::read_only`), end to end through `Session::execute`:
//! commands that write without a snapshot are refused before they run; a Model command that
//! changed the document is undone and journalled as a failure.
//!
//! One test, because the flag is process-wide: a second test running beside it could see the
//! session it makes read-only.

use std::path::{Path, PathBuf};

use fieldcraft_engine::{EngineError, Session, crash, read_only};
use serde_json::json;

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fieldcraft-read-only-{name}-{}", std::process::id()))
}

fn journal_lines(dir: &Path) -> String {
    let mut text = String::new();
    for entry in std::fs::read_dir(dir.join("sessions")).expect("the journal folder").flatten() {
        if entry.path().extension().is_some_and(|e| e == "jsonl") {
            text.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
            text.push('\n');
        }
    }
    text
}

#[test]
fn read_only_refuses_writes_and_undoes_model_changes() {
    let dir = temp_dir("hook");
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = Session::default();
    s.crash_journal = true;
    s.execute("file.sample", &json!({ "name": "cantilever" })).expect("the sample");
    // Start the crash journal after the sample: the commands below are what it records.
    let _ = crash::install(dir.clone(), crash::Build { version: "test", commit: "test" });
    read_only::set("the host opened this session read-only");

    // E3: a command that writes without a snapshot is refused right after `find_command`,
    // before anything runs or is journalled.
    let (model, revision, journal) = (s.model.clone(), s.revision(), s.journal.len());
    let target = dir.join("never-written");
    let params = json!({ "path": target.to_string_lossy() });
    for id in ["file.save", "file.save_as", "edit.undo", "edit.redo", "file.open", "mesh.export.stl", "results.export.vtu"] {
        let e = s.execute(id, &params).expect_err(id);
        assert!(matches!(&e, EngineError::Disabled { command, .. } if command == id), "{id}: {e}");
    }
    assert_eq!(s.model, model, "a refused command changed nothing");
    assert_eq!(s.revision(), revision);
    assert_eq!(s.journal.len(), journal);
    assert!(!target.exists(), "a refused export wrote nothing");

    // The refusal is after `find_command`: an unknown id is still unknown, not disabled.
    let e = s.execute("no.such.command", &json!({})).unwrap_err();
    assert!(matches!(e, EngineError::UnknownCommand(_)), "{e}");

    // E4: a Model command runs, changes the document, and is undone by the existing error path.
    let (model, revision, journal) = (s.model.clone(), s.revision(), s.journal.len());
    let e = s.execute("model.rename", &json!({ "kind": "step", "from": "Load", "to": "Load2" })).expect_err("the rename is undone");
    assert!(matches!(&e, EngineError::Disabled { command, .. } if command == "model.rename"), "{e}");
    assert!(e.to_string().contains("read-only"), "{e}");
    assert_eq!(s.model, model, "the changed model was restored");
    assert_eq!(s.revision(), revision, "the revision did not move");
    assert_eq!(s.journal.len(), journal, "nothing was added to the undo history");

    // A Model command that changes no document (a query) is still allowed.
    assert!(s.execute("model.info", &json!({})).is_ok(), "a query needs no write");
    // And a Model command that only writes results is not the document either: solve.run is
    // allowed here for the same reason (it is not run in this test, which has no step solved).

    // The crash journal recorded the refused change as a failure, so a replay never repeats it.
    let lines = journal_lines(&dir);
    assert!(lines.contains("\"run\":\"model.rename\""), "the refused command is in the journal:\n{lines}");
    assert!(lines.contains("\"ok\":false"), "it is journalled as failed:\n{lines}");
    assert!(!lines.contains("\"ok\":true"), "no command succeeded, so there is nothing to replay:\n{lines}");

    read_only::clear();
    let _ = std::fs::remove_dir_all(&dir);
}
