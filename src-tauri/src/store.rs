use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use cap_std::fs::{Dir, OpenOptions};

use crate::model::{NoteContent, NoteKind, NoteMeta, NoteSummary, RepoMeta, RepoSummary};
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
}

pub struct StoreState(pub Result<DataRoot, String>);

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
    root.dir
        .write(path, json)
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
) -> Result<(), String> {
    let path = note_path(root, repo_key, file_name)?;
    if note_kind(file_name)? != NoteKind::Md {
        return Err("編集できるのはMarkdownノートのみです".into());
    }
    let mut file = root
        .dir
        .open_with(&path, OpenOptions::new().write(true))
        .map_err(storage_error)?;
    if !file.metadata().map_err(storage_error)?.is_file() {
        return Err(format!("通常ファイルではありません: {file_name}"));
    }
    file.set_len(0).map_err(storage_error)?;
    file.write_all(content.as_bytes()).map_err(storage_error)
}

pub fn create_note(root: &DataRoot, repo_key: &str, title: &str) -> Result<String, String> {
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
        match root.dir.open_with(
            dir.join(&file_name),
            OpenOptions::new().write(true).create_new(true),
        ) {
            Ok(mut file) => {
                file.write_all(content.as_bytes()).map_err(storage_error)?;
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

pub fn delete_note(root: &DataRoot, repo_key: &str, file_name: &str) -> Result<(), String> {
    let path = note_path(root, repo_key, file_name)?;
    if !root.dir.metadata(&path).map_err(storage_error)?.is_file() {
        return Err(format!("ノートが存在しません: {file_name}"));
    }
    // ノート専用のアセットディレクトリがあれば一緒に削除
    let assets = path
        .parent()
        .map(|p| p.join("assets").join(file_stem(file_name)));
    if let Some(assets) = assets {
        match root.dir.metadata(&assets) {
            Ok(metadata) if metadata.is_dir() => {
                root.dir.remove_dir_all(&assets).map_err(storage_error)?
            }
            Ok(_) => return Err("アセットの保存先がディレクトリではありません".into()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(storage_error(e)),
        }
    }
    let pins = load_pins(root, repo_key)?;
    root.dir.remove_file(&path).map_err(storage_error)?;
    write_pins_file(root, &pins_path(repo_key), pins, file_name, false)?;
    Ok(())
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
            "overwritten"
        )
        .is_err());
        assert_eq!(fs::read_to_string(&secret).unwrap(), "outside data");
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
        assert!(write_note(&DataRoot::open(tmp.path()).unwrap(), key, "v.html", "x").is_err());
        assert!(write_note(&DataRoot::open(tmp.path()).unwrap(), key, "missing.md", "x").is_err());
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
