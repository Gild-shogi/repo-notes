#![cfg(feature = "backend")]

use repo_notes_lib::{
    backend::{self, Operation},
    session::{Failure, Notification, Session},
};
use std::{fs, process::Command, sync::mpsc, time::Duration};

#[test]
fn client_keeps_structured_errors_and_changes_on_the_owned_connection() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("repos/fixture/notes");
    fs::create_dir_all(&notes).unwrap();
    fs::write(notes.join("note.md"), "original").unwrap();
    let (sender, events) = mpsc::channel();
    let mut command = Command::new(env!("CARGO_BIN_EXE_repo-notes-backend"));
    command.arg("--stdio");
    let mut session = Session::spawn(command, move |event| {
        let _ = sender.send(event);
    })
    .unwrap();
    let hello = session
        .request(Operation::Hello {
            app_version: backend::APP_VERSION.into(),
            protocol_version: backend::PROTOCOL_VERSION,
            data_root: Some(root.path().into()),
        })
        .unwrap();
    assert_eq!(
        hello["dataRoot"],
        root.path().canonicalize().unwrap().to_str().unwrap()
    );
    let note = session
        .request(Operation::ReadNote {
            repo_key: "fixture".into(),
            file_name: "note.md".into(),
        })
        .unwrap();
    assert_eq!(note["raw"], "original");
    let stale = session.request(Operation::WriteNote {
        repo_key: "fixture".into(),
        file_name: "note.md".into(),
        content: "must-not-save".into(),
        expected_raw: "stale".into(),
    });
    assert!(matches!(
        stale,
        Err(Failure::Backend(backend::Error::Conflict))
    ));
    fs::write(notes.join("note.md"), "external update").unwrap();
    let changed = std::time::Instant::now();
    loop {
        assert!(
            changed.elapsed() < Duration::from_secs(5),
            "external change was not forwarded"
        );
        if let Notification::Event(backend::Event::Changed { repo_keys }) =
            events.recv_timeout(Duration::from_secs(5)).unwrap()
        {
            if repo_keys.iter().any(|key| key == "fixture") {
                break;
            }
        }
    }
    assert_eq!(
        fs::read_to_string(notes.join("note.md")).unwrap(),
        "external update"
    );
    drop(session);
    let stopped = std::time::Instant::now();
    loop {
        assert!(
            stopped.elapsed() < Duration::from_secs(5),
            "owned backend did not close"
        );
        if matches!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Notification::Disconnected(_)
        ) {
            break;
        }
    }
}
