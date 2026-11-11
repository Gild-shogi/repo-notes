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
