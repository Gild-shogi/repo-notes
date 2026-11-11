use crate::model::{DeleteOutcome, NoteContent, NoteSummary, RepoSummary, SaveError, SaveOutcome};
use crate::store;
use crate::store::StoreState;
use tauri::{AppHandle, Manager, State};

pub(crate) async fn blocking<T, E>(
    operation: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: From<String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| E::from(format!("ファイル処理の実行に失敗しました: {error}")))?
}

pub(crate) async fn store_io<T, E>(
    app: AppHandle,
    operation: impl FnOnce(&store::DataRoot) -> Result<T, E> + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: From<String> + Send + 'static,
{
    blocking(move || {
        let state = app.state::<StoreState>();
        operation(state.get().map_err(E::from)?)
    })
    .await
}

#[tauri::command]
pub fn get_data_root(state: State<StoreState>) -> Result<String, String> {
    Ok(state.get()?.path().to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn list_repos(app: AppHandle) -> Result<Vec<RepoSummary>, String> {
    store_io(app, store::list_repos).await
}

#[tauri::command]
pub async fn list_notes(
    app: AppHandle,
    repo_key: String,
    query: Option<String>,
) -> Result<Vec<NoteSummary>, String> {
    store_io(app, move |root| {
        store::list_notes(root, &repo_key, query.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn set_pinned(
    app: AppHandle,
    repo_key: String,
    file_name: String,
    pinned: bool,
) -> Result<(), String> {
    store_io(app, move |root| {
        store::set_pinned(root, &repo_key, &file_name, pinned)
    })
    .await
}

#[tauri::command]
pub async fn set_repo_pinned(app: AppHandle, repo_key: String, pinned: bool) -> Result<(), String> {
    store_io(app, move |root| {
        store::set_repo_pinned(root, &repo_key, pinned)
    })
    .await
}

#[tauri::command]
pub async fn read_note(
    app: AppHandle,
    repo_key: String,
    file_name: String,
) -> Result<NoteContent, String> {
    store_io(app, move |root| {
        store::read_note(root, &repo_key, &file_name)
    })
    .await
}

#[tauri::command]
pub async fn write_note(
    app: AppHandle,
    repo_key: String,
    file_name: String,
    content: String,
    expected_raw: String,
) -> Result<SaveOutcome, SaveError> {
    store_io(app, move |root| {
        store::write_note(root, &repo_key, &file_name, &content, &expected_raw)
    })
    .await
}

#[tauri::command]
pub async fn create_note(
    app: AppHandle,
    repo_key: String,
    title: String,
) -> Result<String, String> {
    store_io(app, move |root| store::create_note(root, &repo_key, &title)).await
}

#[tauri::command]
pub async fn delete_note(
    app: AppHandle,
    repo_key: String,
    file_name: String,
) -> Result<DeleteOutcome, String> {
    store_io(app, move |root| {
        store::delete_note(root, &repo_key, &file_name)
    })
    .await
}

/// OSネイティブの印刷ダイアログを開く（macOSでは「PDFとして保存」も可能）
#[tauri::command]
pub fn print_view(webview_window: tauri::WebviewWindow) -> Result<(), String> {
    webview_window.print().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_connection() -> serde_json::Value {
    serde_json::json!({"platform":"local","state":"connected","config":null,"dataRoot":null,"home":null,"revision":0,"message":null,"externalAvailable":true,"connectionId":0,"distributions":[]})
}
