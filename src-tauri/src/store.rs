use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cap_std::fs::{Dir, OpenOptions};

use crate::model::{
    DeleteOutcome, NoteContent, NoteKind, NoteMeta, NoteSummary, RepoMeta, RepoSummary, SaveError,
    SaveOutcome,
};
use crate::parse;

pub fn data_root() -> PathBuf {
    if let Ok(dir) = std::env::var("REPO_NOTES_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::home_dir()
        .expect("home directory not found")
        .join(".repo-notes")
}

pub struct DataRoot {
    path: PathBuf,
    dir: Dir,
}

impl DataRoot {
    pub fn open(path: &Path) -> Result<Self, String> {
        fs::create_dir_all(path).map_err(storage_error)?;
        Self::open_existing(path)
    }

    pub fn open_existing(path: &Path) -> Result<Self, String> {
        let path = path.canonicalize().map_err(storage_error)?;
        let dir =
            Dir::open_ambient_dir(&path, cap_std::ambient_authority()).map_err(storage_error)?;
        Ok(Self { path, dir })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.dir.read(path)
    }

    fn lock(&self) -> Result<fs::File, String> {
        // macOSでcreate(true)の初回同時openがENOENTになるため、作成と既存openを分ける。
        // このinodeを置換・削除すると別プロセスとの排他が分断される。
        let path = ".repo-notes.write.lock";
        let file = match self.dir.open_with(
            path,
            OpenOptions::new().read(true).write(true).create_new(true),
        ) {
            Ok(file) => Ok(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => self
                .dir
                .open_with(path, OpenOptions::new().read(true).write(true)),
            Err(error) => Err(error),
        }
        .map_err(|error| format!("排他ファイルを開けません: {error}"))?
        .into_std();
        file.lock()
            .map_err(|error| format!("排他ファイルをロックできません: {error}"))?;
        Ok(file)
    }
}

fn atomic_write(
    root: &DataRoot,
    path: &Path,
    content: &[u8],
    new: bool,
    permissions: Option<cap_std::fs::Permissions>,
) -> io::Result<SaveOutcome> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("保存先が不正です"))?;
    let (temp, mut file) = loop {
        let name = format!(
            ".repo-notes-save-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let temp = parent.join(name);
        match root
            .dir
            .open_with(&temp, OpenOptions::new().write(true).create_new(true))
        {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        if new {
            root.dir.hard_link(&temp, &root.dir, path)?;
        } else {
            root.dir.rename(&temp, &root.dir, path)?;
        }
        Ok(())
    })();
    let cleanup = match root.dir.remove_file(&temp) {
        Ok(()) => None,
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => Some(format!("一時ファイルを削除できません: {error}")),
    };
    match (result, cleanup) {
        (Ok(()), warning) => Ok(SaveOutcome {
            cleanup_warnings: warning.into_iter().collect(),
        }),
        (Err(error), None) => Err(error),
        (Err(error), Some(warning)) => {
            Err(io::Error::new(error.kind(), format!("{error}; {warning}")))
        }
    }
}

#[cfg(feature = "desktop")]
pub struct StoreState(pub Result<DataRoot, String>);

#[cfg(feature = "desktop")]
impl StoreState {
    pub fn get(&self) -> Result<&DataRoot, String> {
        self.0.as_ref().map_err(Clone::clone)
    }
}

fn storage_error(error: impl std::fmt::Display) -> String {
    format!("保存領域の操作に失敗しました: {error}")
}

/// repo_key / file_name として安全なパスセグメントかを検証する。
fn validate_segment(segment: &str) -> Result<(), String> {
    if segment.is_empty() {
        return Err("空のパスセグメントは許可されていません".into());
    }
    if segment.starts_with('.') {
        return Err(format!("'.'で始まる名前は許可されていません: {segment}"));
    }
    if segment
        .chars()
        .any(|c| matches!(c, '/' | '\\' | '\0' | ':'))
    {
        return Err(format!("不正な文字を含む名前です: {segment}"));
    }
    Ok(())
}

fn note_kind(file_name: &str) -> Result<NoteKind, String> {
    if file_name.ends_with(".md") {
        Ok(NoteKind::Md)
    } else if file_name.ends_with(".html") {
        Ok(NoteKind::Html)
    } else {
        Err(format!("対応していない拡張子です: {file_name}"))
    }
}

fn notes_dir(root: &DataRoot, repo_key: &str) -> Result<PathBuf, String> {
    validate_segment(repo_key)?;
    let dir = Path::new("repos").join(repo_key).join("notes");
    root.dir.open_dir(&dir).map_err(storage_error)?;
    Ok(dir)
}

fn note_path(root: &DataRoot, repo_key: &str, file_name: &str) -> Result<PathBuf, String> {
    validate_segment(file_name)?;
    note_kind(file_name)?;
    Ok(notes_dir(root, repo_key)?.join(file_name))
}

pub fn note_external_path(
    root: &DataRoot,
    repo_key: &str,
    file_name: &str,
) -> Result<PathBuf, String> {
    let relative = note_path(root, repo_key, file_name)?;
    let file = root.dir.open(&relative).map_err(storage_error)?;
    if !file.metadata().map_err(storage_error)?.is_file() {
        return Err("ノートが通常ファイルではありません".into());
    }
    let relative = root.dir.canonicalize(relative).map_err(storage_error)?;
    note_kind(
        relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("ノートのファイル名が不正です")?,
    )?;
    Ok(root.path.join(relative))
}

fn mtime_ms(root: &DataRoot, path: &Path) -> i64 {
    root.dir
        .metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.into_std().duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn list_repos(root: &DataRoot) -> Result<Vec<RepoSummary>, String> {
    let repos_dir = Path::new("repos");
    let entries = match root.dir.read_dir(repos_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(storage_error(e)),
    };
    let pins = load_repo_pins(root)?;
    let mut repos = Vec::new();
    for entry in entries {
        let entry = entry.map_err(storage_error)?;
        let repo = repos_dir.join(entry.file_name());
        if !root.dir.metadata(&repo).map_err(storage_error)?.is_dir() {
            continue;
        }
        let meta_path = repo.join("meta.json");
        let raw = root.dir.read_to_string(meta_path).map_err(storage_error)?;
        let meta: RepoMeta = serde_json::from_str(&raw).map_err(storage_error)?;
        let notes = list_note_files(root, &repo.join("notes"))?;
        let last_note_at_ms = notes.iter().map(|p| mtime_ms(root, p)).max();
        repos.push(RepoSummary {
            pinned: pins.contains(&meta.repo_key),
            repo_key: meta.repo_key,
            repo_name: meta.repo_name,
            canonical_url: meta.canonical_url,
            note_count: notes.len(),
            last_note_at_ms,
        });
    }
    // ピン留めを先頭に、あとは最終ノート日時の新しい順
    repos.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.last_note_at_ms.cmp(&a.last_note_at_ms))
    });
    Ok(repos)
}

fn list_note_files(root: &DataRoot, notes_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match root.dir.read_dir(notes_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(storage_error(e)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(storage_error)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') || note_kind(name).is_err() {
            continue;
        }
        let path = notes_dir.join(name);
        if root.dir.metadata(&path).map_err(storage_error)?.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

pub fn list_notes(
    root: &DataRoot,
    repo_key: &str,
    query: Option<&str>,
) -> Result<Vec<NoteSummary>, String> {
    let dir = notes_dir(root, repo_key)?;
    let pins = load_pins(root, repo_key)?;
    let query = query
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_lowercase);
    let mut notes: Vec<NoteSummary> = list_note_files(root, &dir)?
        .into_iter()
        .map(|path| {
            let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
            let kind = note_kind(&file_name)?;
            let content = root.dir.read_to_string(&path).map_err(storage_error)?;
            let (meta, title) = resolve_meta(kind, &content, &file_name);
            if let Some(q) = &query {
                let matches = title.to_lowercase().contains(q)
                    || file_name.to_lowercase().contains(q)
                    || meta.tags.iter().any(|t| t.to_lowercase().contains(q))
                    || content.to_lowercase().contains(q);
                if !matches {
                    return Ok(None);
                }
            }
            Ok(Some(NoteSummary {
                pinned: pins.contains(&file_name),
                file_name,
                kind,
                title,
                created: meta.created,
                updated_ms: mtime_ms(root, &path),
                tags: meta.tags,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .flatten()
        .collect();
    // ピン留めを先頭に、あとはファイル名の YYYY-MM-DD プレフィックスで新しい順
    notes.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.file_name.cmp(&a.file_name))
    });
    Ok(notes)
}

/// アプリが管理するピン留め状態（repos/<key>/pins.json）。
/// meta.json と異なりアプリ側が読み書きする。
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Pins {
    pinned: Vec<String>,
}

fn pins_path(repo_key: &str) -> PathBuf {
    Path::new("repos").join(repo_key).join("pins.json")
}

fn read_pins_file(
    root: &DataRoot,
    path: &Path,
) -> Result<std::collections::HashSet<String>, String> {
    let raw = match root.dir.read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(e) => return Err(storage_error(e)),
    };
    serde_json::from_str::<Pins>(&raw)
        .map(|p| p.pinned.into_iter().collect())
        .map_err(|e| format!("ピン状態が破損しています: {e}"))
}

/// pins に entry を追加（pinned=true）または除去して保存する。
fn write_pins_file(
    root: &DataRoot,
    path: &Path,
    pins: std::collections::HashSet<String>,
    entry: &str,
    pinned: bool,
) -> Result<(), String> {
    let mut pins: Vec<String> = pins.into_iter().collect();
    pins.retain(|e| e != entry);
    if pinned {
        pins.push(entry.to_string());
    }
    pins.sort();
    let json = serde_json::to_string_pretty(&Pins { pinned: pins })
        .map_err(|e| format!("ピン状態のシリアライズに失敗: {e}"))?;
    let target = match root
        .dir
        .open_with(path, OpenOptions::new().read(true).write(true))
    {
        Ok(file) => (
            root.dir.canonicalize(path).map_err(storage_error)?,
            Some(file.metadata().map_err(storage_error)?.permissions()),
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (path.to_path_buf(), None),
        Err(error) => return Err(format!("ピン状態の保存に失敗: {error}")),
    };
    atomic_write(
        root,
        &target.0,
        json.as_bytes(),
        target.1.is_none(),
        target.1,
    )
    .map(|outcome| {
        for warning in outcome.cleanup_warnings {
            eprintln!("{warning}");
        }
    })
    .map_err(|e| format!("ピン状態の保存に失敗: {e}"))
}

fn load_pins(root: &DataRoot, repo_key: &str) -> Result<std::collections::HashSet<String>, String> {
    read_pins_file(root, &pins_path(repo_key))
}

pub fn set_pinned(
    root: &DataRoot,
    repo_key: &str,
    file_name: &str,
    pinned: bool,
) -> Result<(), String> {
    let _lock = root.lock()?;
    // 実在するノートのみピン留め可能（パス検証込み）
    let path = note_path(root, repo_key, file_name)?;
    if pinned && !root.dir.metadata(&path).map_err(storage_error)?.is_file() {
        return Err(format!("ノートが存在しません: {file_name}"));
    }
    write_pins_file(
        root,
        &pins_path(repo_key),
        load_pins(root, repo_key)?,
        file_name,
        pinned,
    )
}

/// アプリが管理するリポジトリのピン留め状態（<root>/pinned-repos.json）。
fn repo_pins_path() -> PathBuf {
    PathBuf::from("pinned-repos.json")
}

fn load_repo_pins(root: &DataRoot) -> Result<std::collections::HashSet<String>, String> {
    read_pins_file(root, &repo_pins_path())
}

pub fn set_repo_pinned(root: &DataRoot, repo_key: &str, pinned: bool) -> Result<(), String> {
    let _lock = root.lock()?;
    validate_segment(repo_key)?;
    if pinned
        && !root
            .dir
            .metadata(Path::new("repos").join(repo_key).join("meta.json"))
            .map_err(storage_error)?
            .is_file()
    {
        return Err(format!("リポジトリが存在しません: {repo_key}"));
    }
    write_pins_file(
        root,
        &repo_pins_path(),
        load_repo_pins(root)?,
        repo_key,
        pinned,
    )
}

/// メタデータとタイトル（フォールバック解決済み）を返す。
fn resolve_meta(kind: NoteKind, content: &str, file_name: &str) -> (NoteMeta, String) {
    let (meta, fallback) = match kind {
        NoteKind::Md => {
            let (meta, body) = parse::parse_markdown(content);
            let fallback = parse::markdown_title_fallback(&body);
            (meta, fallback)
        }
        NoteKind::Html => {
            let meta = parse::parse_html_meta(content);
            let fallback = parse::html_title_fallback(content);
            (meta, fallback)
        }
    };
    let title = meta
        .title
        .clone()
        .or(fallback)
        .unwrap_or_else(|| file_stem(file_name).to_string());
    (meta, title)
}

fn file_stem(file_name: &str) -> &str {
    file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name)
}

pub fn read_note(root: &DataRoot, repo_key: &str, file_name: &str) -> Result<NoteContent, String> {
    let path = note_path(root, repo_key, file_name)?;
    let kind = note_kind(file_name)?;
    let raw = root
        .dir
        .read_to_string(&path)
        .map_err(|e| format!("ノートを読めません: {e}"))?;
    let body = match kind {
        NoteKind::Md => parse::parse_markdown(&raw).1,
        NoteKind::Html => raw.clone(),
    };
    let (meta, title) = resolve_meta(kind, &raw, file_name);
    let meta = NoteMeta {
        title: Some(title),
        ..meta
    };
    Ok(NoteContent {
        file_name: file_name.to_string(),
        kind,
        meta,
        body,
        raw,
        abs_path: root.path.join(&path).to_string_lossy().into_owned(),
        updated_ms: mtime_ms(root, &path),
    })
}

pub fn write_note(
    root: &DataRoot,
    repo_key: &str,
    file_name: &str,
    content: &str,
    expected_raw: &str,
) -> Result<SaveOutcome, SaveError> {
    if note_kind(file_name)? != NoteKind::Md {
        return Err(SaveError::Failure(
            "編集できるのはMarkdownノートのみです".into(),
        ));
    }
    save_note(root, repo_key, file_name, Some(expected_raw), content)
}

pub fn save_note(
    root: &DataRoot,
    repo_key: &str,
    file_name: &str,
    expected_raw: Option<&str>,
    content: &str,
) -> Result<SaveOutcome, SaveError> {
    let _lock = root.lock()?;
    let (target, permissions) = prepare_save(root, repo_key, file_name, expected_raw)?;
    atomic_write(
        root,
        &target,
        content.as_bytes(),
        expected_raw.is_none(),
        permissions,
    )
    .map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            SaveError::Conflict
        } else {
            storage_error(error).into()
        }
    })
}

pub fn check_note_save(
    root: &DataRoot,
    repo_key: &str,
    file_name: &str,
    expected_raw: Option<&str>,
) -> Result<(), SaveError> {
    prepare_save(root, repo_key, file_name, expected_raw).map(|_| ())
}

fn prepare_save(
    root: &DataRoot,
    repo_key: &str,
    file_name: &str,
    expected_raw: Option<&str>,
) -> Result<(PathBuf, Option<cap_std::fs::Permissions>), SaveError> {
    let path = note_path(root, repo_key, file_name)?;
    let opened = root
        .dir
        .open_with(&path, OpenOptions::new().read(true).write(true));
    let (target, permissions) = match (opened, expected_raw) {
        (Ok(mut file), Some(expected)) => {
            let metadata = file.metadata().map_err(storage_error)?;
            if !metadata.is_file() {
                return Err(SaveError::Failure("通常ファイルではありません".into()));
            }
            let mut current = String::new();
            file.read_to_string(&mut current).map_err(storage_error)?;
            if current != expected {
                return Err(SaveError::Conflict);
            }
            (
                root.dir
                    .canonicalize(&path)
                    .map_err(|error| format!("保存対象の実体を解決できません: {error}"))?,
                Some(metadata.permissions()),
            )
        }
        (Ok(_), None) => return Err(SaveError::Conflict),
        (Err(error), None) if error.kind() == io::ErrorKind::NotFound => (path, None),
        (Err(error), Some(_)) if error.kind() == io::ErrorKind::NotFound => {
            return Err(SaveError::Conflict)
        }
        (Err(error), _) => return Err(storage_error(error).into()),
    };
    Ok((target, permissions))
}

pub fn create_note(root: &DataRoot, repo_key: &str, title: &str) -> Result<String, String> {
    let _lock = root.lock()?;
    let dir = notes_dir(root, repo_key)?;
    let title = title.trim();
    if title.is_empty() {
        return Err("タイトルを入力してください".into());
    }
    let now = chrono::Local::now();
    let base = format!("{}-{}", now.format("%Y-%m-%d"), slugify(title));
    let content = format!(
        "---\ntitle: {}\ncreated: {}\ntags: []\n---\n\n",
        yaml_quote(title),
        now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
    );
    for n in 1..100 {
        let file_name = if n == 1 {
            format!("{base}.md")
        } else {
            format!("{base}-{n}.md")
        };
        match atomic_write(root, &dir.join(&file_name), content.as_bytes(), true, None) {
            Ok(outcome) => {
                for warning in outcome.cleanup_warnings {
                    eprintln!("{warning}");
                }
                return Ok(file_name);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(storage_error(e)),
        }
    }
    Err("同名のノートが多すぎます".into())
}

fn yaml_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn slugify(title: &str) -> String {
    let slug: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug: String = slug.chars().take(50).collect();
    if slug.is_empty() {
        "note".to_string()
    } else {
        slug
    }
}

pub fn delete_note(
    root: &DataRoot,
    repo_key: &str,
    file_name: &str,
) -> Result<DeleteOutcome, String> {
    let _lock = root.lock()?;
    let path = note_path(root, repo_key, file_name)?;
    if !root.dir.metadata(&path).map_err(storage_error)?.is_file() {
        return Err(format!("ノートが存在しません: {file_name}"));
    }
    let pins = load_pins(root, repo_key)?;
    let assets = match path.parent() {
        Some(parent) => {
            let assets = parent.join("assets").join(file_stem(file_name));
            match root.dir.open_dir(&assets) {
                Ok(_) => Some(assets),
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(storage_error(e)),
            }
        }
        None => None,
    };
    root.dir.remove_file(&path).map_err(storage_error)?;
    let mut cleanup_warnings = Vec::new();
    if let Some(assets) = assets {
        if let Err(error) = root.dir.remove_dir_all(&assets) {
            cleanup_warnings.push(format!("アセットの削除に失敗しました: {error}"));
        }
    }
    if let Err(error) = write_pins_file(root, &pins_path(repo_key), pins, file_name, false) {
        cleanup_warnings.push(error);
    }
    Ok(DeleteOutcome { cleanup_warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_repo(root: &Path, repo_key: &str) {
        let repo_dir = root.join("repos").join(repo_key);
        fs::create_dir_all(repo_dir.join("notes")).unwrap();
        fs::write(
            repo_dir.join("meta.json"),
            format!(
                r#"{{"version":1,"repoKey":"{repo_key}","repoName":"demo","canonicalUrl":"github.com/o/demo","createdAt":"2026-07-02T10:00:00+09:00"}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn stale_save_is_rejected_without_losing_external_update() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        fs::write(
            tmp.path().join("repos").join(key).join("notes/a.md"),
            "external update",
        )
        .unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        assert!(matches!(
            save_note(&root, key, "a.md", Some("original"), "draft"),
            Err(SaveError::Conflict)
        ));
        assert_eq!(
            read_note(&root, key, "a.md").unwrap().raw,
            "external update"
        );
    }

    #[test]
    fn shared_writer_creates_new_html_without_overwriting_existing_note() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let root = DataRoot::open(tmp.path()).unwrap();
        save_note(&root, key, "a.html", None, "<title>new</title>").unwrap();
        assert!(matches!(
            save_note(&root, key, "a.html", None, "replacement"),
            Err(SaveError::Conflict)
        ));
        assert_eq!(
            read_note(&root, key, "a.html").unwrap().raw,
            "<title>new</title>"
        );
    }

    #[test]
    fn saving_a_deleted_note_does_not_recreate_it() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let root = DataRoot::open(tmp.path()).unwrap();
        assert!(matches!(
            save_note(&root, key, "gone.md", Some("original"), "draft"),
            Err(SaveError::Conflict)
        ));
        assert!(list_notes(&root, key, None).unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn failed_atomic_save_preserves_original_content() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let notes = tmp.path().join("repos").join(key).join("notes");
        fs::write(notes.join("a.md"), "original").unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        fs::set_permissions(&notes, fs::Permissions::from_mode(0o555)).unwrap();
        let result = save_note(&root, key, "a.md", Some("original"), "replacement");
        fs::set_permissions(&notes, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(result, Err(SaveError::Failure(_))));
        assert_eq!(read_note(&root, key, "a.md").unwrap().raw, "original");
    }

    #[test]
    fn concurrent_pin_updates_preserve_both_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let root = DataRoot::open(tmp.path()).unwrap();
        save_note(&root, key, "a.md", None, "a").unwrap();
        save_note(&root, key, "b.md", None, "b").unwrap();
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            for name in ["a.md", "b.md"] {
                let barrier = &barrier;
                let root = &root;
                scope.spawn(move || {
                    barrier.wait();
                    set_pinned(root, key, name, true).unwrap();
                });
            }
        });
        assert!(list_notes(&root, key, None)
            .unwrap()
            .iter()
            .all(|note| note.pinned));
    }

    #[test]
    fn rejects_path_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        for (key, file) in [
            ("../etc", "a.md"),
            ("..", "a.md"),
            ("demo--12345678", "../meta.json"),
            ("demo--12345678", "..\\evil.md"),
            ("demo--12345678", ".hidden.md"),
            ("demo--12345678", "note.txt"),
            ("demo--12345678", "dir/note.md"),
        ] {
            assert!(
                read_note(&DataRoot::open(tmp.path()).unwrap(), key, file).is_err(),
                "許可されるべきでない: {key}/{file}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_note_symlink_outside_storage() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let secret = outside.path().join("private.md");
        fs::write(&secret, "outside data").unwrap();
        std::os::unix::fs::symlink(
            &secret,
            tmp.path().join("repos").join(key).join("notes/linked.md"),
        )
        .unwrap();
        assert!(read_note(&DataRoot::open(tmp.path()).unwrap(), key, "linked.md").is_err());
        assert!(write_note(
            &DataRoot::open(tmp.path()).unwrap(),
            key,
            "linked.md",
            "overwritten",
            "outside data"
        )
        .is_err());
        assert_eq!(fs::read_to_string(&secret).unwrap(), "outside data");
    }

    #[cfg(unix)]
    #[test]
    fn asset_directory_symlink_cannot_delete_outside_storage() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let notes = tmp.path().join("repos").join(key).join("notes");
        fs::write(notes.join("a.md"), "note").unwrap();
        fs::create_dir(outside.path().join("a")).unwrap();
        fs::write(outside.path().join("a/image.png"), "keep").unwrap();
        std::os::unix::fs::symlink(outside.path(), notes.join("assets")).unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        assert!(delete_note(&root, key, "a.md").is_err());
        assert_eq!(
            fs::read_to_string(outside.path().join("a/image.png")).unwrap(),
            "keep"
        );
        assert_eq!(read_note(&root, key, "a.md").unwrap().raw, "note");
    }

    #[test]
    fn damaged_pins_are_reported_and_preserved() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let repo = tmp.path().join("repos").join(key);
        fs::write(repo.join("notes/a.md"), "note").unwrap();
        fs::write(repo.join("pins.json"), "invalid json").unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        assert!(list_notes(&root, key, None).is_err());
        assert!(set_pinned(&root, key, "a.md", true).is_err());
        assert_eq!(
            fs::read_to_string(repo.join("pins.json")).unwrap(),
            "invalid json"
        );
    }

    #[test]
    fn deletion_rejected_by_damaged_pins_preserves_note_and_assets() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let repo = tmp.path().join("repos").join(key);
        let assets = repo.join("notes/assets/a");
        fs::create_dir_all(&assets).unwrap();
        fs::write(repo.join("notes/a.md"), "note").unwrap();
        fs::write(assets.join("image.png"), "image data").unwrap();
        fs::write(repo.join("pins.json"), "invalid json").unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();

        assert!(delete_note(&root, key, "a.md").is_err());
        assert_eq!(read_note(&root, key, "a.md").unwrap().raw, "note");
        assert_eq!(
            root.read(Path::new("repos/demo--12345678/notes/assets/a/image.png"))
                .unwrap(),
            b"image data"
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_note_removal_preserves_assets() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let notes = tmp.path().join("repos").join(key).join("notes");
        let assets = notes.join("assets/a");
        fs::create_dir_all(&assets).unwrap();
        fs::write(notes.join("a.md"), "note").unwrap();
        fs::write(assets.join("image.png"), "image data").unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        fs::set_permissions(&notes, fs::Permissions::from_mode(0o555)).unwrap();

        let result = delete_note(&root, key, "a.md");
        fs::set_permissions(&notes, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err());
        assert_eq!(read_note(&root, key, "a.md").unwrap().raw, "note");
        assert_eq!(fs::read(assets.join("image.png")).unwrap(), b"image data");
    }

    #[cfg(unix)]
    #[test]
    fn pin_cleanup_failure_does_not_report_deleted_note_as_failure() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let repo = tmp.path().join("repos").join(key);
        fs::write(repo.join("notes/a.md"), "note").unwrap();
        fs::write(repo.join("pins.json"), r#"{"pinned":["a.md"]}"#).unwrap();
        fs::set_permissions(repo.join("pins.json"), fs::Permissions::from_mode(0o444)).unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();

        let result = delete_note(&root, key, "a.md");
        assert!(read_note(&root, key, "a.md").is_err());
        assert!(
            result.is_ok(),
            "削除済みのノートを失敗扱いしない: {result:?}"
        );
        let outcome = result.unwrap();
        assert_eq!(outcome.cleanup_warnings.len(), 1);
        assert!(outcome.cleanup_warnings[0].contains("ピン状態の保存に失敗"));
        assert_eq!(
            fs::read_to_string(repo.join("pins.json")).unwrap(),
            r#"{"pinned":["a.md"]}"#
        );
    }

    #[test]
    fn missing_repos_are_empty_but_invalid_storage_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        assert!(list_repos(&root).unwrap().is_empty());
        fs::write(tmp.path().join("repos"), "not a directory").unwrap();
        assert!(list_repos(&root).is_err());
        assert!(DataRoot::open(&tmp.path().join("repos")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn opened_root_survives_alias_replacement_and_accepts_internal_relative_links() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        let storage = tmp.path().join("storage");
        setup_repo(&storage, key);
        let notes = storage.join("repos").join(key).join("notes");
        fs::write(notes.join("actual.md"), "original").unwrap();
        std::os::unix::fs::symlink("actual.md", notes.join("alias.md")).unwrap();
        let alias = tmp.path().join("alias");
        std::os::unix::fs::symlink(&storage, &alias).unwrap();
        let root = DataRoot::open(&alias).unwrap();
        fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(outside.path(), &alias).unwrap();
        assert_eq!(read_note(&root, key, "alias.md").unwrap().raw, "original");
        write_note(&root, key, "alias.md", "updated", "original").unwrap();
        assert_eq!(read_note(&root, key, "actual.md").unwrap().raw, "updated");
    }

    #[cfg(unix)]
    #[test]
    fn intermediate_symlink_with_trailing_slash_cannot_escape_storage() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("private.md"), "private").unwrap();
        std::os::unix::fs::symlink(
            format!("{}/", outside.path().display()),
            tmp.path().join("linked"),
        )
        .unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        assert!(root.read(Path::new("linked/private.md")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn external_editor_rejects_executable_targets_disguised_as_notes() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "demo--12345678";
        setup_repo(tmp.path(), key);
        let notes = tmp.path().join("repos").join(key).join("notes");
        fs::write(notes.join("evil.command"), "executable").unwrap();
        std::os::unix::fs::symlink("evil.command", notes.join("alias.md")).unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        assert!(note_external_path(&root, key, "alias.md").is_err());
    }

    #[test]
    fn crud_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";

        let file_name =
            create_note(&DataRoot::open(tmp.path()).unwrap(), key, "テスト メモ").unwrap();
        assert!(file_name.ends_with(".md"));

        let note = read_note(&DataRoot::open(tmp.path()).unwrap(), key, &file_name).unwrap();
        assert_eq!(note.meta.title.as_deref(), Some("テスト メモ"));

        write_note(
            &DataRoot::open(tmp.path()).unwrap(),
            key,
            &file_name,
            "---\ntitle: 更新後\ncreated: 2026-07-02\n---\n\n# 中身\n",
            &note.raw,
        )
        .unwrap();
        let note = read_note(&DataRoot::open(tmp.path()).unwrap(), key, &file_name).unwrap();
        assert_eq!(note.meta.title.as_deref(), Some("更新後"));
        assert_eq!(note.body, "\n# 中身\n");

        let notes = list_notes(&DataRoot::open(tmp.path()).unwrap(), key, None).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "更新後");

        delete_note(&DataRoot::open(tmp.path()).unwrap(), key, &file_name).unwrap();
        assert!(list_notes(&DataRoot::open(tmp.path()).unwrap(), key, None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn create_note_avoids_collision() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        let first = create_note(&DataRoot::open(tmp.path()).unwrap(), key, "同じタイトル").unwrap();
        let second =
            create_note(&DataRoot::open(tmp.path()).unwrap(), key, "同じタイトル").unwrap();
        assert_ne!(first, second);
        assert!(second.ends_with("-2.md"));
    }

    #[test]
    fn write_rejects_html_and_missing() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        fs::write(
            tmp.path()
                .join("repos")
                .join(key)
                .join("notes")
                .join("v.html"),
            "<html></html>",
        )
        .unwrap();
        assert!(write_note(&DataRoot::open(tmp.path()).unwrap(), key, "v.html", "x", "").is_err());
        assert!(write_note(
            &DataRoot::open(tmp.path()).unwrap(),
            key,
            "missing.md",
            "x",
            ""
        )
        .is_err());
    }

    #[test]
    fn list_repos_reports_broken_meta() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "ok--12345678");
        let broken = tmp.path().join("repos").join("broken--00000000");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("meta.json"), "{not json").unwrap();
        assert!(list_repos(&DataRoot::open(tmp.path()).unwrap()).is_err());
    }

    #[test]
    fn html_note_metadata_in_listing() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        fs::write(
            tmp.path().join("repos").join(key).join("notes").join("2026-07-02-viz.html"),
            r#"<!-- repo-notes-meta {"title":"グラフ","created":"2026-07-02T09:00:00+09:00"} --><html></html>"#,
        )
        .unwrap();
        let notes = list_notes(&DataRoot::open(tmp.path()).unwrap(), key, None).unwrap();
        assert_eq!(notes[0].title, "グラフ");
        assert_eq!(notes[0].kind, NoteKind::Html);
    }

    #[test]
    fn search_matches_title_body_and_tags() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        let dir = tmp.path().join("repos").join(key).join("notes");
        fs::write(
            dir.join("2026-07-01-auth.md"),
            "---\ntitle: 認証フロー\ncreated: 2026-07-01\ntags: [Auth]\n---\n\nOAuthの仕組み\n",
        )
        .unwrap();
        fs::write(
            dir.join("2026-07-02-db.md"),
            "---\ntitle: DB設計\ncreated: 2026-07-02\ntags: [database]\n---\n\nテーブル一覧\n",
        )
        .unwrap();

        // タイトル一致
        let hit = list_notes(&DataRoot::open(tmp.path()).unwrap(), key, Some("認証")).unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].file_name, "2026-07-01-auth.md");
        // 本文一致（大文字小文字を無視）
        assert_eq!(
            list_notes(&DataRoot::open(tmp.path()).unwrap(), key, Some("oauth"))
                .unwrap()
                .len(),
            1
        );
        // タグ一致
        assert_eq!(
            list_notes(&DataRoot::open(tmp.path()).unwrap(), key, Some("auth"))
                .unwrap()
                .len(),
            1
        );
        // 不一致
        assert!(list_notes(
            &DataRoot::open(tmp.path()).unwrap(),
            key,
            Some("存在しない語")
        )
        .unwrap()
        .is_empty());
        // 空クエリは全件
        assert_eq!(
            list_notes(&DataRoot::open(tmp.path()).unwrap(), key, Some("  "))
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn pin_sorts_first_and_survives_reload() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        let dir = tmp.path().join("repos").join(key).join("notes");
        fs::write(dir.join("2026-07-01-old.md"), "old\n").unwrap();
        fs::write(dir.join("2026-07-02-new.md"), "new\n").unwrap();

        set_pinned(
            &DataRoot::open(tmp.path()).unwrap(),
            key,
            "2026-07-01-old.md",
            true,
        )
        .unwrap();
        let notes = list_notes(&DataRoot::open(tmp.path()).unwrap(), key, None).unwrap();
        assert_eq!(notes[0].file_name, "2026-07-01-old.md");
        assert!(notes[0].pinned);
        assert!(!notes[1].pinned);

        // 解除で通常の並びに戻る
        set_pinned(
            &DataRoot::open(tmp.path()).unwrap(),
            key,
            "2026-07-01-old.md",
            false,
        )
        .unwrap();
        let notes = list_notes(&DataRoot::open(tmp.path()).unwrap(), key, None).unwrap();
        assert_eq!(notes[0].file_name, "2026-07-02-new.md");

        // 存在しないノートはピン留め不可
        assert!(set_pinned(
            &DataRoot::open(tmp.path()).unwrap(),
            key,
            "missing.md",
            true
        )
        .is_err());
    }

    #[test]
    fn repo_pin_sorts_first_and_survives_reload() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "with-note--11111111");
        setup_repo(tmp.path(), "no-note--22222222");
        fs::write(
            tmp.path()
                .join("repos")
                .join("with-note--11111111")
                .join("notes")
                .join("2026-07-01-a.md"),
            "a\n",
        )
        .unwrap();

        // ピンなしではノートがあるリポジトリ（last_note_at_ms あり）が先頭
        let repos = list_repos(&DataRoot::open(tmp.path()).unwrap()).unwrap();
        assert_eq!(repos[0].repo_key, "with-note--11111111");
        assert!(repos.iter().all(|r| !r.pinned));

        // ピン留めすると末尾のリポジトリが先頭に来る
        set_repo_pinned(
            &DataRoot::open(tmp.path()).unwrap(),
            "no-note--22222222",
            true,
        )
        .unwrap();
        let repos = list_repos(&DataRoot::open(tmp.path()).unwrap()).unwrap();
        assert_eq!(repos[0].repo_key, "no-note--22222222");
        assert!(repos[0].pinned);
        assert!(!repos[1].pinned);

        // 解除で通常の並びに戻る
        set_repo_pinned(
            &DataRoot::open(tmp.path()).unwrap(),
            "no-note--22222222",
            false,
        )
        .unwrap();
        let repos = list_repos(&DataRoot::open(tmp.path()).unwrap()).unwrap();
        assert_eq!(repos[0].repo_key, "with-note--11111111");

        // 存在しないリポジトリはピン留め不可
        assert!(set_repo_pinned(
            &DataRoot::open(tmp.path()).unwrap(),
            "missing--00000000",
            true
        )
        .is_err());
        // パストラバーサルも拒否
        assert!(set_repo_pinned(&DataRoot::open(tmp.path()).unwrap(), "../etc", true).is_err());
    }

    #[test]
    fn delete_note_removes_pin() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        let dir = tmp.path().join("repos").join(key).join("notes");
        fs::write(dir.join("2026-07-01-a.md"), "a\n").unwrap();
        set_pinned(
            &DataRoot::open(tmp.path()).unwrap(),
            key,
            "2026-07-01-a.md",
            true,
        )
        .unwrap();
        delete_note(&DataRoot::open(tmp.path()).unwrap(), key, "2026-07-01-a.md").unwrap();
        assert!(load_pins(&DataRoot::open(tmp.path()).unwrap(), key)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn slugify_handles_japanese_and_symbols() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("日本語タイトル"), "日本語タイトル");
        assert_eq!(slugify("!!!"), "note");
    }
}
