use crate::model::{NoteContent, NoteSummary, RepoSummary};
use crate::store;

#[tauri::command]
pub fn get_data_root() -> String {
    store::data_root().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn list_repos() -> Vec<RepoSummary> {
    store::list_repos(&store::data_root())
}

#[tauri::command]
pub fn list_notes(repo_key: String, query: Option<String>) -> Result<Vec<NoteSummary>, String> {
    store::list_notes(&store::data_root(), &repo_key, query.as_deref())
}

#[tauri::command]
pub fn set_pinned(repo_key: String, file_name: String, pinned: bool) -> Result<(), String> {
    store::set_pinned(&store::data_root(), &repo_key, &file_name, pinned)
}

#[tauri::command]
pub fn set_repo_pinned(repo_key: String, pinned: bool) -> Result<(), String> {
    store::set_repo_pinned(&store::data_root(), &repo_key, pinned)
}

#[tauri::command]
pub fn read_note(repo_key: String, file_name: String) -> Result<NoteContent, String> {
    store::read_note(&store::data_root(), &repo_key, &file_name)
}

#[tauri::command]
pub fn write_note(repo_key: String, file_name: String, content: String) -> Result<(), String> {
    store::write_note(&store::data_root(), &repo_key, &file_name, &content)
}

#[tauri::command]
pub fn create_note(repo_key: String, title: String) -> Result<String, String> {
    store::create_note(&store::data_root(), &repo_key, &title)
}

#[tauri::command]
pub fn delete_note(repo_key: String, file_name: String) -> Result<(), String> {
    store::delete_note(&store::data_root(), &repo_key, &file_name)
}
