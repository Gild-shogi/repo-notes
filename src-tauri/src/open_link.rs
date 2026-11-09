use std::path::{Path, PathBuf};

use crate::store::{self, StoreState};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn open_note(
    app: tauri::AppHandle,
    state: tauri::State<StoreState>,
    repo_key: String,
    file_name: String,
    reveal: bool,
) -> Result<(), String> {
    let path = store::note_external_path(state.get()?, &repo_key, &file_name)?;
    if reveal {
        app.opener().reveal_item_in_dir(path)
    } else {
        app.opener().open_path(path.to_string_lossy(), None::<&str>)
    }
    .map_err(|error| format!("ノートを外部アプリで開けません: {error}"))
}

/// 実行系のファイルはノート内リンクから開かせない（openで即実行され得るもの）
const BLOCKED_EXTENSIONS: &[&str] = &[
    "app",
    "command",
    "terminal",
    "workflow",
    "scpt",
    "applescript",
    "jar",
    "pkg",
    "dmg",
];

/// ノート内リンクから開いてよいパスかを検証し、正規化済みパスを返す。
/// - 実在すること（canonicalize で symlink / `..` を解決）
/// - 解決後が home 配下であること
/// - 実行系拡張子でないこと
pub fn validate_openable(path: &str, home: &Path) -> Result<PathBuf, String> {
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|_| format!("ファイルが見つかりません: {path}"))?;
    let canonical_home = home
        .canonicalize()
        .map_err(|e| format!("ホームディレクトリを解決できません: {e}"))?;
    if !canonical.starts_with(&canonical_home) {
        return Err(format!(
            "ホームディレクトリ外のパスは開けません: {}",
            canonical.display()
        ));
    }
    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if BLOCKED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("このファイル形式はリンクから開けません: .{ext}"));
    }
    Ok(canonical)
}

/// ノート内リンクのローカルパスを、検証した上でデフォルトアプリで開く。
#[tauri::command]
pub fn open_local_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("ホームディレクトリが見つかりません")?;
    let canonical = validate_openable(&path, &home)?;
    app.opener()
        .open_path(canonical.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("ファイルを開けませんでした: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_outside_home_and_blocked_extensions() {
        let home = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();

        // home配下の通常ファイルはOK
        let ok_file = home.path().join("doc.md");
        fs::write(&ok_file, "x").unwrap();
        assert!(validate_openable(ok_file.to_str().unwrap(), home.path()).is_ok());

        // home外は拒否
        let out_file = outside.path().join("doc.md");
        fs::write(&out_file, "x").unwrap();
        assert!(validate_openable(out_file.to_str().unwrap(), home.path()).is_err());

        // `..` で home 外へ遡るのは canonicalize 後に検出される
        let sneaky = format!("{}/../{}", home.path().display(), out_file.display());
        assert!(validate_openable(&sneaky, home.path()).is_err());

        // symlink で home 外を指すのも拒否
        let link = home.path().join("link.md");
        std::os::unix::fs::symlink(&out_file, &link).unwrap();
        assert!(validate_openable(link.to_str().unwrap(), home.path()).is_err());

        // 実行系拡張子は拒否
        let cmd = home.path().join("evil.command");
        fs::write(&cmd, "x").unwrap();
        assert!(validate_openable(cmd.to_str().unwrap(), home.path()).is_err());

        // 存在しないファイルは拒否
        assert!(
            validate_openable(home.path().join("nope.md").to_str().unwrap(), home.path()).is_err()
        );
    }
}
