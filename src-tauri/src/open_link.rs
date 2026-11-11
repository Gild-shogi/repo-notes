use crate::{commands, paths::validate_openable, store};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn open_note(
    app: tauri::AppHandle,
    repo_key: String,
    file_name: String,
    reveal: bool,
) -> Result<(), String> {
    let opener = app.clone();
    commands::store_io(app, move |root| {
        let path = store::note_external_path(root, &repo_key, &file_name)?;
        if reveal {
            opener.opener().reveal_item_in_dir(path)
        } else {
            opener
                .opener()
                .open_path(path.to_string_lossy(), None::<&str>)
        }
        .map_err(|error| format!("ノートを外部アプリで開けません: {error}"))
    })
    .await
}

/// ノート内リンクのローカルパスを、検証した上でデフォルトアプリで開く。
#[tauri::command]
pub async fn open_local_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    commands::blocking(move || {
        let home = dirs::home_dir().ok_or("ホームディレクトリが見つかりません")?;
        let canonical = validate_openable(&path, &home)?;
        app.opener()
            .open_path(canonical.to_string_lossy(), None::<&str>)
            .map_err(|e| format!("ファイルを開けませんでした: {e}"))
    })
    .await
}
