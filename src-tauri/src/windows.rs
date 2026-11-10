use crate::{
    backend::{self, Hello, Operation},
    session::{Failure, Notification, Session},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

pub const CONNECTION_EVENT: &str = "repo-notes://connection";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub distro: String,
    pub user: String,
    pub data_root: Option<String>,
}

impl Config {
    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("WSL", self.distro.as_str()),
            ("Linuxユーザー", self.user.as_str()),
        ] {
            if value.trim().is_empty()
                || value.starts_with('-')
                || value
                    .chars()
                    .any(|c| matches!(c, '\0' | '\n' | '\r' | '/' | '\\'))
            {
                return Err(format!("{name}を指定してください"));
            }
        }
        if let Some(root) = &self.data_root {
            if !root.starts_with('/') || root.contains('\0') {
                return Err("保存先はLinuxの絶対パスで指定してください".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub platform: &'static str,
    pub state: &'static str,
    pub config: Option<Config>,
    pub data_root: Option<String>,
    pub message: Option<String>,
    pub external_available: bool,
    pub connection_id: u64,
    pub distributions: Vec<String>,
}

struct Active {
    session: Option<Session>,
    home: Option<String>,
}

pub struct Windows {
    active: Mutex<Active>,
    view: Arc<Mutex<View>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    version: String,
    protocol_version: u32,
    sha256: String,
}

fn output_text(bytes: &[u8]) -> Result<String, String> {
    if bytes.contains(&0) {
        if bytes.len() % 2 != 0 {
            return Err("WSLの一覧を読み取れません".into());
        }
        String::from_utf16(
            &bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<_>>(),
        )
        .map_err(|e| e.to_string())
    } else {
        String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string())
    }
}

fn wsl(config: &Config, selected_user: bool) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("wsl.exe");
    command
        .creation_flags(0x08000000)
        .arg("--distribution")
        .arg(&config.distro);
    if selected_user {
        command.arg("--user").arg(&config.user);
    }
    command.arg("--exec");
    command
}

fn execute(config: &Config, selected_user: bool, args: &[&str]) -> Result<String, String> {
    let output = wsl(config, selected_user)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            format!("WSLを起動できません。WSL2とUbuntuを導入してから再接続してください: {e}")
        })?;
    if !output.status.success() {
        return Err(format!(
            "WSLコマンドが失敗しました: {}{}",
            output_text(&output.stdout)?,
            output_text(&output.stderr)?
        ));
    }
    Ok(output_text(&output.stdout)?.trim().to_owned())
}

fn distributions() -> Result<Vec<String>, String> {
    use std::os::windows::process::CommandExt;
    let output = Command::new("wsl.exe")
        .creation_flags(0x08000000)
        .args(["--list", "--quiet"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            format!("WSLが見つかりません。WindowsでWSL2とUbuntuを導入してください: {e}")
        })?;
    if !output.status.success() {
        return Err(format!(
            "WSLを利用できません。WSL2とUbuntuの導入状態を確認してください: {}{}",
            output_text(&output.stdout)?,
            output_text(&output.stderr)?
        ));
    }
    let names = output_text(&output.stdout)?
        .lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}').to_owned())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if names.is_empty() {
        return Err("WSLの対象環境がありません。Ubuntuを導入して初期設定を完了してください".into());
    }
    Ok(names)
}

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("wsl.json"))
}

fn load_config(app: &tauri::AppHandle) -> Result<Option<Config>, String> {
    let path = config_path(app)?;
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| format!("接続設定を読み取れません。設定を確認してください: {e}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("接続設定を読み取れません: {error}")),
    }
}

fn save_config(app: &tauri::AppHandle, config: &Config) -> Result<(), String> {
    let path = config_path(app)?;
    fs::create_dir_all(path.parent().ok_or("接続設定の場所が不正です")?)
        .map_err(|e| e.to_string())?;
    fs::write(
        &path,
        serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("接続設定を保存できません: {e}"))
}

fn backend_path(app: &tauri::AppHandle, config: &Config, home: &str) -> Result<String, String> {
    let binary = app
        .path()
        .resolve(
            "backend/repo-notes-backend",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| e.to_string())?;
    let manifest_path = app
        .path()
        .resolve(
            "backend/manifest.json",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| e.to_string())?;
    let bytes = fs::read(&binary).map_err(|e| {
        format!("同梱バックエンドを読めません。インストーラーを再導入してください: {e}")
    })?;
    let manifest: Manifest = serde_json::from_slice(
        &fs::read(manifest_path).map_err(|e| format!("配布情報を読めません: {e}"))?,
    )
    .map_err(|e| format!("配布情報が不正です: {e}"))?;
    if manifest.version != backend::APP_VERSION
        || manifest.protocol_version != backend::PROTOCOL_VERSION
        || format!("{:x}", Sha256::digest(&bytes)) != manifest.sha256
    {
        return Err("同梱バックエンドの版または検証値が一致しません。同じリリースのインストーラーを再導入してください".into());
    }
    let directory = format!(
        "{home}/.local/share/repo-notes/backend/{}",
        backend::APP_VERSION
    );
    let target = format!("{directory}/repo-notes-backend");
    let existing = execute(config, true, &["sha256sum", "--", &target]);
    let matches = existing
        .as_ref()
        .ok()
        .and_then(|line| line.split_whitespace().next())
        .is_some_and(|hash| hash == manifest.sha256);
    if !matches {
        execute(config, true, &["mkdir", "-p", "--", &directory])?;
        let script="umask 077; target=$1; temp=$(mktemp \"${target}.XXXXXX\") || exit; trap 'rm -f -- \"$temp\"' EXIT; cat > \"$temp\" && chmod 700 -- \"$temp\" && mv -T -- \"$temp\" \"$target\"";
        let mut child = wsl(config, true)
            .args(["sh", "-c", script, "sh", &target])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        child
            .stdin
            .take()
            .ok_or("配置先の入力を開けません")?
            .write_all(&bytes)
            .map_err(|e| e.to_string())?;
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "バックエンドを配置できません: {}{}",
                output_text(&output.stdout)?,
                output_text(&output.stderr)?
            ));
        }
    }
    if execute(config, true, &["sha256sum", "--", &target])?
        .split_whitespace()
        .next()
        != Some(manifest.sha256.as_str())
    {
        return Err("配置したバックエンドの検証に失敗しました".into());
    }
    execute(config, true, &["test", "-x", &target])?;
    Ok(target)
}

impl Windows {
    pub fn new(app: &tauri::AppHandle) -> Self {
        let loaded = load_config(app);
        let message = loaded.as_ref().err().cloned();
        Self {
            active: Mutex::new(Active {
                session: None,
                home: None,
            }),
            view: Arc::new(Mutex::new(View {
                platform: "windows",
                state: "disconnected",
                config: loaded.ok().flatten(),
                data_root: None,
                message,
                external_available: false,
                connection_id: 0,
                distributions: Vec::new(),
            })),
        }
    }
    pub fn disconnect(&self) {
        if let Ok(mut active) = self.active.lock() {
            active.session.take();
            active.home = None;
        }
    }
    pub fn view(&self) -> Result<View, String> {
        self.view
            .lock()
            .map(|view| view.clone())
            .map_err(|e| e.to_string())
    }
    fn update(&self, app: &tauri::AppHandle, change: impl FnOnce(&mut View)) -> Result<(), String> {
        let view = {
            let mut view = self.view.lock().map_err(|e| e.to_string())?;
            change(&mut view);
            view.clone()
        };
        app.emit(CONNECTION_EVENT, view).map_err(|e| e.to_string())
    }
    pub fn refresh(&self) -> Result<View, String> {
        let names = distributions();
        let mut view = self.view.lock().map_err(|e| e.to_string())?;
        match names {
            Ok(names) => view.distributions = names,
            Err(error) => view.message = Some(error),
        }
        Ok(view.clone())
    }
    pub fn connect(&self, app: &tauri::AppHandle, config: Config) -> Result<View, String> {
        config.validate()?;
        let mut active = self.active.lock().map_err(|e| e.to_string())?;
        active.session.take();
        self.update(app, |view| {
            view.state = "connecting";
            view.message = None;
            view.external_available = false;
            view.connection_id += 1;
        })?;
        let connected = (|| {
            let names = distributions()?;
            if !names.contains(&config.distro) {
                return Err("選択したWSLが見つかりません。接続設定を確認してください".into());
            }
            let kernel = execute(&config, true, &["uname", "-r"])?;
            if !kernel.to_ascii_lowercase().contains("wsl2") {
                return Err("WSL2のUbuntuを選択してください。WSL1は対応していません".into());
            }
            let actual_user = execute(&config, true, &["id", "-un"])?;
            if actual_user != config.user {
                return Err("WSLの実行ユーザーが接続設定と一致しません".into());
            }
            let home = execute(&config, true, &["printenv", "HOME"])?;
            if !home.starts_with('/') {
                return Err("Linuxホームを解決できません".into());
            }
            let root = config
                .data_root
                .clone()
                .unwrap_or_else(|| format!("{home}/.repo-notes"));
            let target = backend_path(app, &config, &home)?;
            let generation = self.view()?.connection_id;
            let view = Arc::clone(&self.view);
            let app_events = app.clone();
            let mut session = Session::spawn(
                {
                    let mut command = wsl(&config, true);
                    command.args([&target, "--stdio"]);
                    command
                },
                move |notification| {
                    if view
                        .lock()
                        .map(|view| view.connection_id != generation)
                        .unwrap_or(true)
                    {
                        return;
                    }
                    match notification {
                        Notification::Event(backend::Event::Changed { repo_keys }) => {
                            let _ = app_events.emit(
                                "repo-notes://changed",
                                serde_json::json!({"repoKeys":repo_keys}),
                            );
                        }
                        Notification::Event(backend::Event::WatchError { message }) => {
                            let _ = app_events.emit(
                                "repo-notes://watch-error",
                                serde_json::json!({"message":message}),
                            );
                        }
                        Notification::Disconnected(message) => {
                            if let Ok(mut view) = view.lock() {
                                if view.connection_id != generation {
                                    return;
                                }
                                view.state = "disconnected";
                                view.message = Some(message);
                                view.external_available = false;
                                let _ = app_events.emit(CONNECTION_EVENT, view.clone());
                            }
                        }
                    }
                },
            )?;
            let hello: Hello = serde_json::from_value(
                session
                    .request(Operation::Hello {
                        app_version: backend::APP_VERSION.into(),
                        protocol_version: backend::PROTOCOL_VERSION,
                        data_root: Some(root.clone().into()),
                    })
                    .map_err(failure_message)?,
            )
            .map_err(|e| e.to_string())?;
            let expected_root = execute(&config, true, &["realpath", "--", &root])?;
            if hello.app_version != backend::APP_VERSION
                || hello.protocol_version != backend::PROTOCOL_VERSION
                || hello.distro.as_deref() != Some(config.distro.as_str())
                || hello.data_root.to_string_lossy() != expected_root
                || hello.home.to_string_lossy() != home
            {
                return Err("接続したバックエンドの版・WSL・保存先が選択内容と一致しません".into());
            }
            let external = execute(&config, false, &["id", "-un"])? == config.user;
            save_config(app, &config)?;
            active.home = Some(home);
            active.session = Some(session);
            self.update(app, |view| {
                view.state = "connected";
                view.config = Some(config);
                view.data_root = Some(expected_root);
                view.external_available = external;
                view.distributions = names;
            })?;
            self.view()
        })();
        if let Err(error) = &connected {
            self.update(app, |view| {
                view.state = "disconnected";
                view.message = Some(error.clone());
            })?;
        }
        connected
    }
    pub fn request(
        &self,
        app: &tauri::AppHandle,
        id: Option<u64>,
        operation: Operation,
    ) -> Result<Value, Failure> {
        let mut active = self.active.lock().map_err(|e| Failure::Disconnected {
            message: e.to_string(),
            sent: false,
        })?;
        let view = self.view().map_err(|message| Failure::Disconnected {
            message,
            sent: false,
        })?;
        if id != Some(view.connection_id) || view.state != "connected" {
            return Err(Failure::Disconnected {
                message: "接続先が変更されたか、WSLとの接続がありません".into(),
                sent: false,
            });
        }
        let result = active
            .session
            .as_mut()
            .ok_or_else(|| Failure::Disconnected {
                message: "バックエンドへ接続してください".into(),
                sent: false,
            })?
            .request(operation);
        if let Err(Failure::Disconnected { message, .. }) = &result {
            let _ = self.update(app, |view| {
                view.state = "disconnected";
                view.message = Some(message.clone());
                view.external_available = false;
            });
        }
        result
    }
    pub fn external(
        &self,
        app: &tauri::AppHandle,
        id: Option<u64>,
        operation: Operation,
        reveal: bool,
    ) -> Result<(), String> {
        let path: String =
            serde_json::from_value(self.request(app, id, operation).map_err(failure_message)?)
                .map_err(|e| e.to_string())?;
        let view = self.view()?;
        if !view.external_available {
            return Err("外部操作はWSLの既定ユーザーで行われます。選択ユーザーと一致する接続設定を使ってください。アプリ内の編集は利用できます".into());
        }
        let config = view.config.ok_or("接続設定がありません")?;
        if reveal {
            let unc = format!(
                "\\\\wsl.localhost\\{}{}",
                config.distro,
                path.replace('/', "\\")
            );
            app.opener()
                .reveal_item_in_dir(unc)
                .map_err(|e| format!("Explorerで場所を表示できません: {e}"))
        } else {
            use std::os::windows::process::CommandExt;
            let editor = std::env::var_os("PATH")
                .and_then(|paths| {
                    std::env::split_paths(&paths).find_map(|directory| {
                        if !directory.join("code.cmd").is_file() {
                            return None;
                        }
                        let executable = directory.parent()?.join("Code.exe");
                        executable.is_file().then_some(executable)
                    })
                })
                .ok_or("Windows版VS CodeとWSL拡張を導入し、codeをPATHへ登録してください")?;
            let escaped = path
                .split('/')
                .map(|part| {
                    percent_encoding::utf8_percent_encode(part, percent_encoding::NON_ALPHANUMERIC)
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join("/");
            let uri = format!("vscode-remote://wsl+{}{escaped}", config.distro);
            let output = Command::new(editor)
                .creation_flags(0x08000000)
                .args([
                    "--remote",
                    &format!("wsl+{}", config.distro),
                    "--file-uri",
                    &uri,
                ])
                .stdin(Stdio::null())
                .output()
                .map_err(|e| {
                    format!(
                        "VS Codeを起動できません。Windows版VS CodeとWSL拡張を導入してください: {e}"
                    )
                })?;
            if !output.status.success() {
                return Err(format!("VS CodeのWSL接続を開始できません。Windows版VS CodeとWSL拡張を確認してください: {}{}",String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr)));
            }
            Ok(())
        }
    }
    pub fn asset(
        &self,
        app: &tauri::AppHandle,
        id: Option<u64>,
        path: String,
    ) -> Result<http::Response<Vec<u8>>, String> {
        let asset: backend::Asset = serde_json::from_value(
            self.request(app, id, Operation::ReadAsset { path })
                .map_err(failure_message)?,
        )
        .map_err(|e| e.to_string())?;
        let mut response = http::Response::builder().status(asset.status);
        for (name, value) in asset.headers {
            response = response.header(name, value);
        }
        response
            .body(STANDARD.decode(asset.body).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }
}

pub fn failure_message(failure: Failure) -> String {
    match failure {
        Failure::Disconnected { message, .. } => message,
        Failure::Backend(backend::Error::Conflict) => {
            "保存先が更新されています。下書きを保持しました".into()
        }
        Failure::Backend(
            backend::Error::Failure(message)
            | backend::Error::InvalidRequest(message)
            | backend::Error::VersionMismatch(message),
        ) => message,
    }
}

pub fn asset_response(
    app: &tauri::AppHandle,
    request: http::Request<Vec<u8>>,
) -> Result<http::Response<Vec<u8>>, String> {
    let encoded = request
        .uri()
        .path()
        .strip_prefix("/wsl/")
        .ok_or("アセットの接続先が不正です")?;
    let (id, path) = encoded.split_once('/').ok_or("アセットのパスが不正です")?;
    let id = id.parse::<u64>().map_err(|e| e.to_string())?;
    let path = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .map_err(|e| e.to_string())?
        .into_owned();
    app.state::<Windows>().asset(app, Some(id), path)
}
