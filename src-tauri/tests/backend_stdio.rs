use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

fn exchange(root: &Path, request: serde_json::Value) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_repo-notes-backend"))
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let input = serde_json::to_string(&request).unwrap() + "\n";
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!root.exists());
    output
}

#[test]
fn incompatible_ui_is_rejected_before_the_data_root_is_created() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("must-not-be-created");
    let output = exchange(
        &root,
        serde_json::json!({
            "id": 1, "method": "hello", "appVersion": "incompatible",
            "protocolVersion": 1, "dataRoot": root,
        }),
    );
    assert!(
        !output.status.success(),
        "an incompatible client must not be accepted"
    );
    let reply: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reply["error"]["kind"], "versionMismatch");
}

struct Session {
    child: std::process::Child,
    input: Option<std::process::ChildStdin>,
    messages: std::sync::mpsc::Receiver<serde_json::Value>,
    next_id: u64,
}

impl Session {
    fn start(root: &Path) -> Self {
        use std::io::BufRead;
        let mut child = Command::new(env!("CARGO_BIN_EXE_repo-notes-backend"))
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (sender, messages) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let value = serde_json::from_str(&line.unwrap()).unwrap();
                if sender.send(value).is_err() {
                    break;
                }
            }
        });
        let mut session = Self {
            child,
            input,
            messages,
            next_id: 0,
        };
        let hello = session.request(serde_json::json!({"method":"hello", "appVersion":env!("CARGO_PKG_VERSION"), "protocolVersion":1, "dataRoot":root}));
        assert_eq!(
            hello["result"]["dataRoot"],
            root.canonicalize().unwrap().to_str().unwrap()
        );
        session
    }

    fn request(&mut self, mut request: serde_json::Value) -> serde_json::Value {
        self.next_id += 1;
        request["id"] = self.next_id.into();
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{request}").unwrap();
        input.flush().unwrap();
        loop {
            let message = self
                .messages
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("backend must reply without hanging");
            if message["type"] == "response" {
                assert_eq!(message["id"], self.next_id);
                return message;
            }
        }
    }

    fn finish(mut self) {
        self.input.take();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "stdin EOF must stop the backend"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.input.take();
        if self.child.try_wait().unwrap().is_none() {
            self.child.kill().unwrap();
        }
        self.child.wait().unwrap();
    }
}

#[test]
fn accepted_session_preserves_conflicts_and_serves_assets_from_the_same_root() {
    let temporary = tempfile::tempdir().unwrap();
    let notes = temporary
        .path()
        .canonicalize()
        .unwrap()
        .join("repos/demo--12345678/notes");
    std::fs::create_dir_all(&notes).unwrap();
    std::fs::write(notes.join("a.md"), "original").unwrap();
    let image = notes.join("画像 #1.svg");
    std::fs::write(&image, "<svg></svg>").unwrap();
    let mut session = Session::start(temporary.path());
    let read = session.request(
        serde_json::json!({"method":"read_note", "repoKey":"demo--12345678", "fileName":"a.md"}),
    );
    assert_eq!(read["result"]["raw"], "original");
    let saved = session.request(serde_json::json!({"method":"write_note", "repoKey":"demo--12345678", "fileName":"a.md", "expectedRaw":"original", "content":"updated"}));
    assert_eq!(saved["result"]["cleanupWarnings"], serde_json::json!([]));
    let stale = session.request(serde_json::json!({"method":"write_note", "repoKey":"demo--12345678", "fileName":"a.md", "expectedRaw":"original", "content":"must-not-overwrite"}));
    assert_eq!(stale["error"]["kind"], "conflict");
    let reread = session.request(
        serde_json::json!({"method":"read_note", "repoKey":"demo--12345678", "fileName":"a.md"}),
    );
    assert_eq!(reread["result"]["raw"], "updated");
    let asset = session.request(serde_json::json!({"method":"read_asset", "path":image}));
    assert_eq!(asset["result"]["status"], 200);
    assert_eq!(asset["result"]["body"], "PHN2Zz48L3N2Zz4=");
    assert_eq!(asset["result"]["headers"]["content-type"], "image/svg+xml");
    assert!(asset["result"]["headers"]["content-security-policy"]
        .as_str()
        .unwrap()
        .contains("script-src 'none'"));
    session.finish();
}

#[test]
fn external_writes_emit_repository_changes_on_the_existing_session() {
    let temporary = tempfile::tempdir().unwrap();
    let notes = temporary.path().join("repos/demo--12345678/notes");
    std::fs::create_dir_all(&notes).unwrap();
    let mut session = Session::start(temporary.path());
    session.request(serde_json::json!({"method":"list_notes", "repoKey":"demo--12345678"}));
    std::fs::write(notes.join("agent.md"), "from another writer").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let message = session
            .messages
            .recv_timeout(remaining)
            .expect("external updates must reach the UI connection");
        if message["event"] == "changed"
            && message["repoKeys"]
                .as_array()
                .is_some_and(|keys| keys.contains(&serde_json::json!("demo--12345678")))
        {
            break;
        }
    }
    session.finish();
}
