pub use crate::watch::Event;
use crate::{
    asset,
    model::SaveError,
    store::{self, DataRoot},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
type Output = Arc<Mutex<io::Stdout>>;

pub const PROTOCOL_VERSION: u32 = 1;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    pub id: u64,
    #[serde(flatten)]
    pub operation: Operation,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(
    tag = "method",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Operation {
    Hello {
        app_version: String,
        protocol_version: u32,
        data_root: Option<PathBuf>,
    },
    GetDataRoot,
    ListRepos,
    ListNotes {
        repo_key: String,
        query: Option<String>,
    },
    ReadNote {
        repo_key: String,
        file_name: String,
    },
    WriteNote {
        repo_key: String,
        file_name: String,
        content: String,
        expected_raw: String,
    },
    CreateNote {
        repo_key: String,
        title: String,
    },
    DeleteNote {
        repo_key: String,
        file_name: String,
    },
    SetPinned {
        repo_key: String,
        file_name: String,
        pinned: bool,
    },
    SetRepoPinned {
        repo_key: String,
        pinned: bool,
    },
    ReadAsset {
        path: String,
    },
    ResolveNotePath {
        repo_key: String,
        file_name: String,
    },
    ResolveLocalPath {
        path: String,
    },
    Shutdown,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum Error {
    Conflict,
    Failure(String),
    InvalidRequest(String),
    VersionMismatch(String),
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Failure(message)
    }
}

impl From<SaveError> for Error {
    fn from(error: SaveError) -> Self {
        match error {
            SaveError::Conflict => Self::Conflict,
            SaveError::Failure(message) => Self::Failure(message),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Outcome {
    Success { result: Value },
    Failure { error: Error },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Response {
        id: Option<u64>,
        #[serde(flatten)]
        outcome: Outcome,
    },
    Event {
        #[serde(flatten)]
        event: Event,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub app_version: String,
    pub protocol_version: u32,
    pub data_root: PathBuf,
    pub home: PathBuf,
    pub distro: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Asset {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

fn value(result: impl Serialize) -> Result<Value, Error> {
    serde_json::to_value(result).map_err(|e| Error::Failure(e.to_string()))
}

fn dispatch(root: &DataRoot, operation: Operation) -> Result<Value, Error> {
    match operation {
        Operation::Hello { .. } => Err(Error::InvalidRequest(
            "接続中の保存先は変更できません".into(),
        )),
        Operation::GetDataRoot => value(root.path()),
        Operation::ListRepos => value(store::list_repos(root)?),
        Operation::ListNotes { repo_key, query } => {
            value(store::list_notes(root, &repo_key, query.as_deref())?)
        }
        Operation::ReadNote {
            repo_key,
            file_name,
        } => value(store::read_note(root, &repo_key, &file_name)?),
        Operation::WriteNote {
            repo_key,
            file_name,
            content,
            expected_raw,
        } => value(store::write_note(
            root,
            &repo_key,
            &file_name,
            &content,
            &expected_raw,
        )?),
        Operation::CreateNote { repo_key, title } => {
            value(store::create_note(root, &repo_key, &title)?)
        }
        Operation::DeleteNote {
            repo_key,
            file_name,
        } => value(store::delete_note(root, &repo_key, &file_name)?),
        Operation::SetPinned {
            repo_key,
            file_name,
            pinned,
        } => value(store::set_pinned(root, &repo_key, &file_name, pinned)?),
        Operation::SetRepoPinned { repo_key, pinned } => {
            value(store::set_repo_pinned(root, &repo_key, pinned)?)
        }
        Operation::ReadAsset { path } => {
            let encoded =
                percent_encoding::utf8_percent_encode(&path, percent_encoding::NON_ALPHANUMERIC);
            let request = http::Request::builder()
                .uri(format!("asset://localhost/{encoded}"))
                .body(Vec::new())
                .map_err(|e| Error::Failure(e.to_string()))?;
            let response = asset::response(root, request);
            value(Asset {
                status: response.status().as_u16(),
                headers: response
                    .headers()
                    .iter()
                    .map(|(name, v)| {
                        Ok((
                            name.to_string(),
                            v.to_str()
                                .map_err(|e| Error::Failure(e.to_string()))?
                                .to_owned(),
                        ))
                    })
                    .collect::<Result<_, Error>>()?,
                body: STANDARD.encode(response.body()),
            })
        }
        Operation::ResolveNotePath {
            repo_key,
            file_name,
        } => value(store::note_external_path(root, &repo_key, &file_name)?),
        Operation::ResolveLocalPath { path } => {
            let home = dirs::home_dir()
                .ok_or_else(|| Error::Failure("ホームディレクトリを解決できません".into()))?;
            value(crate::paths::validate_openable(&path, &home)?)
        }
        Operation::Shutdown => Ok(Value::Null),
    }
}

fn send_message(output: &Output, message: &Message) -> Result<(), String> {
    let mut output = output.lock().map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut *output, message).map_err(|e| e.to_string())?;
    output
        .write_all(b"\n")
        .and_then(|()| output.flush())
        .map_err(|e| e.to_string())
}

fn send(output: &Output, id: Option<u64>, outcome: Outcome) -> Result<(), String> {
    send_message(output, &Message::Response { id, outcome })
}

fn parse(line: &str) -> Result<Request, Error> {
    serde_json::from_str(line).map_err(|e| Error::InvalidRequest(e.to_string()))
}

pub fn run_stdio() -> Result<(), String> {
    let input = io::stdin();
    let mut lines = input.lock().lines();
    let output = Arc::new(Mutex::new(io::stdout()));
    let Some(line) = lines.next().transpose().map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let request = match parse(&line) {
        Ok(request) => request,
        Err(error) => {
            send(&output, None, Outcome::Failure { error })?;
            return Err("hello が不正です".into());
        }
    };
    let Operation::Hello {
        app_version,
        protocol_version,
        data_root,
    } = request.operation
    else {
        send(
            &output,
            Some(request.id),
            Outcome::Failure {
                error: Error::InvalidRequest("最初に hello を送信してください".into()),
            },
        )?;
        return Err("hello を受信できませんでした".into());
    };
    if app_version != APP_VERSION || protocol_version != PROTOCOL_VERSION {
        send(
            &output,
            Some(request.id),
            Outcome::Failure {
                error: Error::VersionMismatch("UIとバックエンドのバージョンが一致しません".into()),
            },
        )?;
        return Err("バージョンが一致しないため接続を終了しました".into());
    }
    let opened = (|| {
        let home = dirs::home_dir().ok_or("ホームディレクトリを解決できません")?;
        let root = DataRoot::open(&data_root.unwrap_or_else(store::data_root))?;
        let hello = value(Hello {
            app_version: APP_VERSION.into(),
            protocol_version: PROTOCOL_VERSION,
            data_root: root.path().to_path_buf(),
            home,
            distro: std::env::var("WSL_DISTRO_NAME").ok(),
        })
        .map_err(|e| format!("{e:?}"))?;
        Ok::<_, String>((root, hello))
    })();
    let (root, hello) = match opened {
        Ok(opened) => opened,
        Err(message) => {
            send(
                &output,
                Some(request.id),
                Outcome::Failure {
                    error: Error::Failure(message.clone()),
                },
            )?;
            return Err(message);
        }
    };
    let notifications = Arc::clone(&output);
    let _watcher = match crate::watch::spawn(root.path().to_path_buf(), move |event| {
        if let Err(error) = send_message(&notifications, &Message::Event { event }) {
            eprintln!("変更通知の送信に失敗しました: {error}");
            std::process::exit(1);
        }
    }) {
        Ok(watcher) => watcher,
        Err(message) => {
            send(
                &output,
                Some(request.id),
                Outcome::Failure {
                    error: Error::Failure(message.clone()),
                },
            )?;
            return Err(message);
        }
    };
    send(
        &output,
        Some(request.id),
        Outcome::Success { result: hello },
    )?;
    send_message(
        &output,
        &Message::Event {
            event: Event::Changed {
                repo_keys: Vec::new(),
            },
        },
    )?;
    for line in lines {
        let line = line.map_err(|e| e.to_string())?;
        let request = match parse(&line) {
            Ok(request) => request,
            Err(error) => {
                send(&output, None, Outcome::Failure { error })?;
                return Err("要求が不正なため接続を終了しました".into());
            }
        };
        let shutdown = matches!(request.operation, Operation::Shutdown);
        let outcome = match dispatch(&root, request.operation) {
            Ok(result) => Outcome::Success { result },
            Err(error) => Outcome::Failure { error },
        };
        send(&output, Some(request.id), outcome)?;
        if shutdown {
            break;
        }
    }
    Ok(())
}
