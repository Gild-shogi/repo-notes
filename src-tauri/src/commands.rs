use crate::model::{NoteContent, NoteSummary, RepoSummary};
use crate::store;
use crate::store::StoreState;
use tauri::State;

#[tauri::command]
pub fn get_data_root(state: State<StoreState>) -> Result<String, String> {
    Ok(state.get()?.path().to_string_lossy().into_owned())
}

#[tauri::command]
pub fn list_repos(state: State<StoreState>) -> Result<Vec<RepoSummary>, String> {
    store::list_repos(state.get()?)
}

#[tauri::command]
pub fn list_notes(
    state: State<StoreState>,
    repo_key: String,
    query: Option<String>,
) -> Result<Vec<NoteSummary>, String> {
    store::list_notes(state.get()?, &repo_key, query.as_deref())
}

#[tauri::command]
pub fn set_pinned(
    state: State<StoreState>,
    repo_key: String,
    file_name: String,
    pinned: bool,
) -> Result<(), String> {
    store::set_pinned(state.get()?, &repo_key, &file_name, pinned)
}

#[tauri::command]
pub fn set_repo_pinned(
    state: State<StoreState>,
    repo_key: String,
    pinned: bool,
) -> Result<(), String> {
    store::set_repo_pinned(state.get()?, &repo_key, pinned)
}

#[tauri::command]
pub fn read_note(
    state: State<StoreState>,
    repo_key: String,
    file_name: String,
) -> Result<NoteContent, String> {
    store::read_note(state.get()?, &repo_key, &file_name)
}

#[tauri::command]
pub fn write_note(
    state: State<StoreState>,
    repo_key: String,
    file_name: String,
    content: String,
) -> Result<(), String> {
    store::write_note(state.get()?, &repo_key, &file_name, &content)
}

#[tauri::command]
pub fn create_note(
    state: State<StoreState>,
    repo_key: String,
    title: String,
) -> Result<String, String> {
    store::create_note(state.get()?, &repo_key, &title)
}

#[tauri::command]
pub fn delete_note(
    state: State<StoreState>,
    repo_key: String,
    file_name: String,
) -> Result<(), String> {
    store::delete_note(state.get()?, &repo_key, &file_name)
}

/// OSネイティブの印刷ダイアログを開く（macOSでは「PDFとして保存」も可能）
#[tauri::command]
pub fn print_view(webview_window: tauri::WebviewWindow) -> Result<(), String> {
    webview_window.print().map_err(|e| e.to_string())
}
