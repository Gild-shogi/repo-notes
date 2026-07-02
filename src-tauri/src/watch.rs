use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;
use tauri::{AppHandle, Emitter};

pub const CHANGED_EVENT: &str = "repo-notes://changed";

/// データルートを監視し、変更があったリポジトリのキーをフロントへ通知する。
/// データルートが未作成の場合は作成されるまで待ってから監視を開始する。
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let root = crate::store::data_root();
        while !root.exists() {
            std::thread::sleep(Duration::from_secs(3));
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let mut debouncer = match new_debouncer(Duration::from_millis(300), None, tx) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("ファイル監視の初期化に失敗: {e}");
                return;
            }
        };
        if let Err(e) = debouncer.watch(&root, RecursiveMode::Recursive) {
            eprintln!("ファイル監視の開始に失敗: {e}");
            return;
        }

        // 監視開始前（ルート作成待ちの間）の変更を拾うため、初回に一度通知する
        emit_changed(&app, HashSet::new());

        for result in rx {
            let Ok(events) = result else { continue };
            let keys: HashSet<String> = events
                .iter()
                .flat_map(|e| e.paths.iter())
                .filter_map(|p| repo_key_of(&root, p))
                .collect();
            emit_changed(&app, keys);
        }
    });
}

fn emit_changed(app: &AppHandle, repo_keys: HashSet<String>) {
    let payload = serde_json::json!({
        "repoKeys": repo_keys.into_iter().collect::<Vec<_>>(),
    });
    if let Err(e) = app.emit(CHANGED_EVENT, payload) {
        eprintln!("変更イベントの送信に失敗: {e}");
    }
}

/// 変更パスから repos/<repo-key>/ のキー部分を取り出す。
fn repo_key_of(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root.join("repos")).ok()?;
    rel.components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
}
