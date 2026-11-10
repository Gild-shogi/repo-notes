use crate::{
    backend::{Error, Operation},
    model::{DeleteOutcome, NoteContent, NoteSummary, RepoSummary, SaveOutcome},
    session::Failure,
    windows::{self, Config, View, Windows},
};
use serde::{de::DeserializeOwned, Serialize};
use tauri::{AppHandle, Manager};

pub(crate) async fn blocking<T, E>(
    operation: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: From<String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| E::from(format!("処理を実行できません: {error}")))?
}

async fn call<T: DeserializeOwned + Send + 'static>(
    app: AppHandle,
    connection_id: Option<u64>,
    operation: Operation,
) -> Result<T, String> {
    blocking(move || {
        let value = app
            .state::<Windows>()
            .request(&app, connection_id, operation)
            .map_err(windows::failure_message)?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub async fn get_connection(app: AppHandle) -> Result<View, String> {
    blocking(move || app.state::<Windows>().refresh()).await
}

#[tauri::command]
pub async fn connect_backend(app: AppHandle, config: Config) -> Result<View, String> {
    blocking(move || app.state::<Windows>().connect(&app, config)).await
}

#[tauri::command]
pub async fn get_data_root(app: AppHandle, connection_id: Option<u64>) -> Result<String, String> {
    call(app, connection_id, Operation::GetDataRoot).await
}

#[tauri::command]
pub async fn list_repos(
    app: AppHandle,
    connection_id: Option<u64>,
) -> Result<Vec<RepoSummary>, String> {
    call(app, connection_id, Operation::ListRepos).await
}

#[tauri::command]
pub async fn list_notes(
    app: AppHandle,
    connection_id: Option<u64>,
    repo_key: String,
    query: Option<String>,
) -> Result<Vec<NoteSummary>, String> {
    call(app, connection_id, Operation::ListNotes { repo_key, query }).await
}

#[tauri::command]
pub async fn read_note(
    app: AppHandle,
    connection_id: Option<u64>,
    repo_key: String,
    file_name: String,
) -> Result<NoteContent, String> {
    call(
        app,
        connection_id,
        Operation::ReadNote {
            repo_key,
            file_name,
        },
    )
    .await
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum SaveFailure {
    Conflict,
    Failure(String),
    Indeterminate(String),
}
impl From<String> for SaveFailure {
    fn from(message: String) -> Self {
        Self::Failure(message)
    }
}

#[tauri::command]
pub async fn write_note(
    app: AppHandle,
    connection_id: Option<u64>,
    repo_key: String,
    file_name: String,
    content: String,
    expected_raw: String,
) -> Result<SaveOutcome, SaveFailure> {
    blocking(move || {
        let value = app
            .state::<Windows>()
            .request(
                &app,
                connection_id,
                Operation::WriteNote {
                    repo_key,
                    file_name,
                    content,
                    expected_raw,
                },
            )
            .map_err(|error| match error {
                Failure::Backend(Error::Conflict) => SaveFailure::Conflict,
                Failure::Disconnected {
                    message,
                    sent: true,
                } => SaveFailure::Indeterminate(message),
                error => SaveFailure::Failure(windows::failure_message(error)),
            })?;
        serde_json::from_value(value)
            .map_err(|e| SaveFailure::Indeterminate(format!("保存結果を読み取れません: {e}")))
    })
    .await
}

#[tauri::command]
pub async fn create_note(
    app: AppHandle,
    connection_id: Option<u64>,
    repo_key: String,
    title: String,
) -> Result<String, String> {
    call(
        app,
        connection_id,
        Operation::CreateNote { repo_key, title },
    )
    .await
}

#[tauri::command]
pub async fn delete_note(
    app: AppHandle,
    connection_id: Option<u64>,
    repo_key: String,
    file_name: String,
) -> Result<DeleteOutcome, String> {
    call(
        app,
        connection_id,
        Operation::DeleteNote {
            repo_key,
            file_name,
        },
    )
    .await
}

#[tauri::command]
pub async fn set_pinned(
    app: AppHandle,
    connection_id: Option<u64>,
    repo_key: String,
    file_name: String,
    pinned: bool,
) -> Result<(), String> {
    call(
        app,
        connection_id,
        Operation::SetPinned {
            repo_key,
            file_name,
            pinned,
        },
    )
    .await
}

#[tauri::command]
pub async fn set_repo_pinned(
    app: AppHandle,
    connection_id: Option<u64>,
    repo_key: String,
    pinned: bool,
) -> Result<(), String> {
    call(
        app,
        connection_id,
        Operation::SetRepoPinned { repo_key, pinned },
    )
    .await
}

#[tauri::command]
pub async fn open_note(
    app: AppHandle,
    connection_id: Option<u64>,
    repo_key: String,
    file_name: String,
    reveal: bool,
) -> Result<(), String> {
    blocking(move || {
        app.state::<Windows>().external(
            &app,
            connection_id,
            Operation::ResolveNotePath {
                repo_key,
                file_name,
            },
            reveal,
        )
    })
    .await
}

#[tauri::command]
pub async fn open_local_path(
    app: AppHandle,
    connection_id: Option<u64>,
    path: String,
) -> Result<(), String> {
    blocking(move || {
        app.state::<Windows>().external(
            &app,
            connection_id,
            Operation::ResolveLocalPath { path },
            false,
        )
    })
    .await
}

#[tauri::command]
pub fn print_view(webview_window: tauri::WebviewWindow) -> Result<(), String> {
    webview_window.print().map_err(|e| e.to_string())
}
