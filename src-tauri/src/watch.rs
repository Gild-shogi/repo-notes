use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use serde::{Deserialize, Serialize};

#[cfg(feature = "desktop")]
pub const CHANGED_EVENT: &str = "repo-notes://changed";
#[cfg(feature = "desktop")]
pub const ERROR_EVENT: &str = "repo-notes://watch-error";
pub type Watcher = Debouncer<RecommendedWatcher, RecommendedCache>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(
    tag = "event",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    Changed { repo_keys: Vec<String> },
    WatchError { message: String },
}

pub fn spawn(root: PathBuf, notify: impl Fn(Event) + Send + 'static) -> Result<Watcher, String> {
    let watched = root.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(300),
        None,
        move |result: DebounceEventResult| {
            let event = match result {
                Ok(events) => Event::Changed {
                    repo_keys: events
                        .iter()
                        .flat_map(|e| e.paths.iter())
                        .filter_map(|path| repo_key_of(&watched, path))
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect(),
                },
                Err(errors) => Event::WatchError {
                    message: errors
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; "),
                },
            };
            notify(event);
        },
    )
    .map_err(|e| format!("ファイル監視の初期化に失敗しました: {e}"))?;
    debouncer
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| format!("ファイル監視の開始に失敗しました: {e}"))?;
    Ok(debouncer)
}

fn repo_key_of(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root.join("repos")).ok()?;
    rel.components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{self, DataRoot};
    use std::sync::mpsc;

    #[test]
    fn agent_create_and_update_notify_the_repo_of_the_note() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        std::fs::create_dir_all(tmp.path().join("repos").join(key).join("notes")).unwrap();
        std::fs::write(tmp.path().join("repos").join(key).join("notes/a.md"), "v1").unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = spawn(root.path().to_path_buf(), move |event| {
            let _ = tx.send(event);
        })
        .unwrap();
        while rx.recv_timeout(Duration::from_secs(1)).is_ok() {}

        let notified = |rx: &mpsc::Receiver<Event>| {
            std::iter::from_fn(|| rx.recv_timeout(Duration::from_secs(5)).ok()).any(|event| {
                matches!(event, Event::Changed { repo_keys } if repo_keys.iter().any(|k| k == key))
            })
        };

        store::save_note(&root, key, "b.md", None, "new").unwrap();
        assert!(
            notified(&rx),
            "新規作成したノートのリポジトリへ変更通知が届かない"
        );
        while rx.recv_timeout(Duration::from_secs(1)).is_ok() {}
        store::save_note(&root, key, "a.md", Some("v1"), "v2").unwrap();
        assert!(
            notified(&rx),
            "更新したノートのリポジトリへ変更通知が届かない"
        );
    }
}
