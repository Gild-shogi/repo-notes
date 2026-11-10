use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

const KEY: &str = "demo--12345678";

fn cli(root: &Path) -> Command {
    #[cfg(feature = "desktop")]
    let binary = env!("CARGO_BIN_EXE_repo-notes");
    #[cfg(not(feature = "desktop"))]
    let binary = env!("CARGO_BIN_EXE_repo-notes-backend");
    let mut command = Command::new(binary);
    command.env("REPO_NOTES_DIR", root).args(["note"]);
    command
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn setup(root: &Path) {
    fs::create_dir_all(root.join("repos").join(KEY).join("notes")).unwrap();
}

#[test]
fn headless_read_write_preserves_raw_and_reports_stale_save() {
    let tmp = tempfile::tempdir().unwrap();
    setup(tmp.path());
    let draft = tmp.path().join("draft.md");
    let expected = tmp.path().join("snapshot.md");
    fs::write(&draft, "original without newline").unwrap();
    let created = cli(tmp.path())
        .args([
            "write",
            "--repo-key",
            KEY,
            "--file-name",
            "a.md",
            "--new",
            "--content-file",
        ])
        .arg(&draft)
        .output()
        .unwrap();
    assert_success(&created);
    let read = cli(tmp.path())
        .args([
            "read",
            "--repo-key",
            KEY,
            "--file-name",
            "a.md",
            "--output",
            "raw",
        ])
        .output()
        .unwrap();
    assert_success(&read);
    assert_eq!(read.stdout, b"original without newline");
    fs::write(&expected, &read.stdout).unwrap();
    fs::write(&draft, "first update\n").unwrap();
    let saved = cli(tmp.path())
        .args([
            "write",
            "--repo-key",
            KEY,
            "--file-name",
            "a.md",
            "--expected-file",
        ])
        .arg(&expected)
        .arg("--content-file")
        .arg(&draft)
        .output()
        .unwrap();
    assert_success(&saved);
    fs::write(&draft, "stale second update").unwrap();
    let stale = cli(tmp.path())
        .args([
            "write",
            "--repo-key",
            KEY,
            "--file-name",
            "a.md",
            "--expected-file",
        ])
        .arg(&expected)
        .arg("--content-file")
        .arg(&draft)
        .output()
        .unwrap();
    assert_eq!(stale.status.code(), Some(3));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&stale.stderr).unwrap()["kind"],
        "conflict"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("repos").join(KEY).join("notes/a.md")).unwrap(),
        "first update\n"
    );
    assert_eq!(fs::read_to_string(&draft).unwrap(), "stale second update");
}

#[test]
fn independent_writer_processes_commit_only_one_update_from_the_same_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    setup(tmp.path());
    let note = tmp.path().join("repos").join(KEY).join("notes/a.md");
    let expected = tmp.path().join("expected.md");
    fs::write(&note, "baseline").unwrap();
    fs::write(&expected, "baseline").unwrap();
    let children: Vec<_> = (0..8)
        .map(|index| {
            let draft = tmp.path().join(format!("draft-{index}.md"));
            let content = format!("writer-{index}:{}", "x".repeat(65536));
            fs::write(&draft, &content).unwrap();
            let child = cli(tmp.path())
                .args([
                    "write",
                    "--repo-key",
                    KEY,
                    "--file-name",
                    "a.md",
                    "--expected-file",
                ])
                .arg(&expected)
                .arg("--content-file")
                .arg(&draft)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            (child, content)
        })
        .collect();
    let mut winners = Vec::new();
    for (child, content) in children {
        let result = child.wait_with_output().unwrap();
        if result.status.success() {
            winners.push(content);
        } else {
            assert_eq!(
                result.status.code(),
                Some(3),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
    assert_eq!(winners.len(), 1);
    assert_eq!(fs::read_to_string(&note).unwrap(), winners[0]);
}

#[test]
fn dry_run_does_not_create_note_lock_or_temporary_files() {
    let tmp = tempfile::tempdir().unwrap();
    setup(tmp.path());
    let draft = tmp.path().join("draft.html");
    fs::write(&draft, "<title>new</title>").unwrap();
    let result = cli(tmp.path())
        .args([
            "write",
            "--repo-key",
            KEY,
            "--file-name",
            "a.html",
            "--new",
            "--dry-run",
            "--content-file",
        ])
        .arg(&draft)
        .output()
        .unwrap();
    assert_success(&result);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap()["dryRun"],
        true
    );
    assert!(!tmp.path().join(".repo-notes.write.lock").exists());
    assert_eq!(
        fs::read_dir(tmp.path().join("repos").join(KEY).join("notes"))
            .unwrap()
            .count(),
        0
    );
}
