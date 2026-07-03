use std::fs;
use std::path::{Path, PathBuf};

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

fn notes_dir(root: &Path, repo_key: &str) -> Result<PathBuf, String> {
    validate_segment(repo_key)?;
    let dir = root.join("repos").join(repo_key).join("notes");
    // canonicalize による多層防御: 実在するディレクトリがルート配下であることを確認
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("データディレクトリを解決できません: {e}"))?;
    let canonical_dir = dir
        .canonicalize()
        .map_err(|_| format!("リポジトリが見つかりません: {repo_key}"))?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err(format!("データディレクトリ外へのアクセスです: {repo_key}"));
    }
    Ok(canonical_dir)
}

fn note_path(root: &Path, repo_key: &str, file_name: &str) -> Result<PathBuf, String> {
    validate_segment(file_name)?;
    note_kind(file_name)?;
    Ok(notes_dir(root, repo_key)?.join(file_name))
}

fn mtime_ms(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn list_repos(root: &Path) -> Vec<RepoSummary> {
    let repos_dir = root.join("repos");
    let Ok(entries) = fs::read_dir(&repos_dir) else {
        return Vec::new();
    };
    let pins = load_repo_pins(root);
    let mut repos: Vec<RepoSummary> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let meta_path = entry.path().join("meta.json");
            let meta: RepoMeta = serde_json::from_str(&fs::read_to_string(meta_path).ok()?)
                .inspect_err(|e| eprintln!("meta.json の読み込みに失敗: {:?}: {e}", entry.path()))
                .ok()?;
            let notes = list_note_files(&entry.path().join("notes"));
            let last_note_at_ms = notes.iter().map(|p| mtime_ms(p)).max();
            Some(RepoSummary {
                pinned: pins.contains(&meta.repo_key),
                repo_key: meta.repo_key,
                repo_name: meta.repo_name,
                canonical_url: meta.canonical_url,
                note_count: notes.len(),
                last_note_at_ms,
            })
        })
        .collect();
    // ピン留めを先頭に、あとは最終ノート日時の新しい順
    repos.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.last_note_at_ms.cmp(&a.last_note_at_ms))
    });
    repos
}

fn list_note_files(notes_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(notes_dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| !n.starts_with('.') && note_kind(n).is_ok())
        })
        .collect()
}

pub fn list_notes(
    root: &Path,
    repo_key: &str,
    query: Option<&str>,
) -> Result<Vec<NoteSummary>, String> {
    let dir = notes_dir(root, repo_key)?;
    let pins = load_pins(root, repo_key);
    let query = query.map(str::trim).filter(|q| !q.is_empty()).map(str::to_lowercase);
    let mut notes: Vec<NoteSummary> = list_note_files(&dir)
        .into_iter()
        .filter_map(|path| {
            let file_name = path.file_name()?.to_str()?.to_string();
            let kind = note_kind(&file_name).ok()?;
            let content = fs::read_to_string(&path).ok()?;
            let (meta, title) = resolve_meta(kind, &content, &file_name);
            if let Some(q) = &query {
                let matches = title.to_lowercase().contains(q)
                    || file_name.to_lowercase().contains(q)
                    || meta.tags.iter().any(|t| t.to_lowercase().contains(q))
                    || content.to_lowercase().contains(q);
                if !matches {
                    return None;
                }
            }
            Some(NoteSummary {
                pinned: pins.contains(&file_name),
                file_name,
                kind,
                title,
                created: meta.created,
                updated_ms: mtime_ms(&path),
                tags: meta.tags,
            })
        })
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

fn pins_path(root: &Path, repo_key: &str) -> PathBuf {
    root.join("repos").join(repo_key).join("pins.json")
}

fn read_pins_file(path: &Path) -> std::collections::HashSet<String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Pins>(&s).ok())
        .map(|p| p.pinned.into_iter().collect())
        .unwrap_or_default()
}

/// pins に entry を追加（pinned=true）または除去して保存する。
fn write_pins_file(
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
    fs::write(path, json).map_err(|e| format!("ピン状態の保存に失敗: {e}"))
}

fn load_pins(root: &Path, repo_key: &str) -> std::collections::HashSet<String> {
    read_pins_file(&pins_path(root, repo_key))
}

pub fn set_pinned(
    root: &Path,
    repo_key: &str,
    file_name: &str,
    pinned: bool,
) -> Result<(), String> {
    // 実在するノートのみピン留め可能（パス検証込み）
    let path = note_path(root, repo_key, file_name)?;
    if pinned && !path.is_file() {
        return Err(format!("ノートが存在しません: {file_name}"));
    }
    write_pins_file(
        &pins_path(root, repo_key),
        load_pins(root, repo_key),
        file_name,
        pinned,
    )
}

/// アプリが管理するリポジトリのピン留め状態（<root>/pinned-repos.json）。
fn repo_pins_path(root: &Path) -> PathBuf {
    root.join("pinned-repos.json")
}

fn load_repo_pins(root: &Path) -> std::collections::HashSet<String> {
    read_pins_file(&repo_pins_path(root))
}

pub fn set_repo_pinned(root: &Path, repo_key: &str, pinned: bool) -> Result<(), String> {
    validate_segment(repo_key)?;
    if pinned && !root.join("repos").join(repo_key).join("meta.json").is_file() {
        return Err(format!("リポジトリが存在しません: {repo_key}"));
    }
    write_pins_file(&repo_pins_path(root), load_repo_pins(root), repo_key, pinned)
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

pub fn read_note(root: &Path, repo_key: &str, file_name: &str) -> Result<NoteContent, String> {
    let path = note_path(root, repo_key, file_name)?;
    let kind = note_kind(file_name)?;
    let raw = fs::read_to_string(&path).map_err(|e| format!("ノートを読めません: {e}"))?;
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
        abs_path: path.to_string_lossy().into_owned(),
        updated_ms: mtime_ms(&path),
    })
}

pub fn write_note(
    root: &Path,
    repo_key: &str,
    file_name: &str,
    content: &str,
) -> Result<(), String> {
    let path = note_path(root, repo_key, file_name)?;
    if note_kind(file_name)? != NoteKind::Md {
        return Err("編集できるのはMarkdownノートのみです".into());
    }
    if !path.is_file() {
        return Err(format!("ノートが存在しません: {file_name}"));
    }
    fs::write(&path, content).map_err(|e| format!("保存に失敗しました: {e}"))
}

pub fn create_note(root: &Path, repo_key: &str, title: &str) -> Result<String, String> {
    let dir = notes_dir(root, repo_key)?;
    let title = title.trim();
    if title.is_empty() {
        return Err("タイトルを入力してください".into());
    }
    let now = chrono::Local::now();
    let base = format!("{}-{}", now.format("%Y-%m-%d"), slugify(title));
    let file_name = available_file_name(&dir, &base, "md")?;
    let content = format!(
        "---\ntitle: {}\ncreated: {}\ntags: []\n---\n\n",
        yaml_quote(title),
        now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
    );
    fs::write(dir.join(&file_name), content).map_err(|e| format!("作成に失敗しました: {e}"))?;
    Ok(file_name)
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

fn available_file_name(dir: &Path, base: &str, ext: &str) -> Result<String, String> {
    let candidate = format!("{base}.{ext}");
    if !dir.join(&candidate).exists() {
        return Ok(candidate);
    }
    for n in 2..100 {
        let candidate = format!("{base}-{n}.{ext}");
        if !dir.join(&candidate).exists() {
            return Ok(candidate);
        }
    }
    Err("同名のノートが多すぎます".into())
}

pub fn delete_note(root: &Path, repo_key: &str, file_name: &str) -> Result<(), String> {
    let path = note_path(root, repo_key, file_name)?;
    if !path.is_file() {
        return Err(format!("ノートが存在しません: {file_name}"));
    }
    fs::remove_file(&path).map_err(|e| format!("削除に失敗しました: {e}"))?;
    // ノート専用のアセットディレクトリがあれば一緒に削除
    let assets = path
        .parent()
        .map(|p| p.join("assets").join(file_stem(file_name)));
    if let Some(assets) = assets {
        if assets.is_dir() {
            let _ = fs::remove_dir_all(&assets);
        }
    }
    // ピン留めされていれば解除しておく
    let _ = set_pinned(root, repo_key, file_name, false);
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
                read_note(tmp.path(), key, file).is_err(),
                "許可されるべきでない: {key}/{file}"
            );
        }
    }

    #[test]
    fn crud_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";

        let file_name = create_note(tmp.path(), key, "テスト メモ").unwrap();
        assert!(file_name.ends_with(".md"));

        let note = read_note(tmp.path(), key, &file_name).unwrap();
        assert_eq!(note.meta.title.as_deref(), Some("テスト メモ"));

        write_note(tmp.path(), key, &file_name, "---\ntitle: 更新後\ncreated: 2026-07-02\n---\n\n# 中身\n").unwrap();
        let note = read_note(tmp.path(), key, &file_name).unwrap();
        assert_eq!(note.meta.title.as_deref(), Some("更新後"));
        assert_eq!(note.body, "\n# 中身\n");

        let notes = list_notes(tmp.path(), key, None).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "更新後");

        delete_note(tmp.path(), key, &file_name).unwrap();
        assert!(list_notes(tmp.path(), key, None).unwrap().is_empty());
    }

    #[test]
    fn create_note_avoids_collision() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        let first = create_note(tmp.path(), key, "同じタイトル").unwrap();
        let second = create_note(tmp.path(), key, "同じタイトル").unwrap();
        assert_ne!(first, second);
        assert!(second.ends_with("-2.md"));
    }

    #[test]
    fn write_rejects_html_and_missing() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        fs::write(
            tmp.path().join("repos").join(key).join("notes").join("v.html"),
            "<html></html>",
        )
        .unwrap();
        assert!(write_note(tmp.path(), key, "v.html", "x").is_err());
        assert!(write_note(tmp.path(), key, "missing.md", "x").is_err());
    }

    #[test]
    fn list_repos_skips_broken_meta() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "ok--12345678");
        let broken = tmp.path().join("repos").join("broken--00000000");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("meta.json"), "{not json").unwrap();
        let repos = list_repos(tmp.path());
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].repo_key, "ok--12345678");
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
        let notes = list_notes(tmp.path(), key, None).unwrap();
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
        let hit = list_notes(tmp.path(), key, Some("認証")).unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].file_name, "2026-07-01-auth.md");
        // 本文一致（大文字小文字を無視）
        assert_eq!(list_notes(tmp.path(), key, Some("oauth")).unwrap().len(), 1);
        // タグ一致
        assert_eq!(list_notes(tmp.path(), key, Some("auth")).unwrap().len(), 1);
        // 不一致
        assert!(list_notes(tmp.path(), key, Some("存在しない語")).unwrap().is_empty());
        // 空クエリは全件
        assert_eq!(list_notes(tmp.path(), key, Some("  ")).unwrap().len(), 2);
    }

    #[test]
    fn pin_sorts_first_and_survives_reload() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        let dir = tmp.path().join("repos").join(key).join("notes");
        fs::write(dir.join("2026-07-01-old.md"), "old\n").unwrap();
        fs::write(dir.join("2026-07-02-new.md"), "new\n").unwrap();

        set_pinned(tmp.path(), key, "2026-07-01-old.md", true).unwrap();
        let notes = list_notes(tmp.path(), key, None).unwrap();
        assert_eq!(notes[0].file_name, "2026-07-01-old.md");
        assert!(notes[0].pinned);
        assert!(!notes[1].pinned);

        // 解除で通常の並びに戻る
        set_pinned(tmp.path(), key, "2026-07-01-old.md", false).unwrap();
        let notes = list_notes(tmp.path(), key, None).unwrap();
        assert_eq!(notes[0].file_name, "2026-07-02-new.md");

        // 存在しないノートはピン留め不可
        assert!(set_pinned(tmp.path(), key, "missing.md", true).is_err());
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
        let repos = list_repos(tmp.path());
        assert_eq!(repos[0].repo_key, "with-note--11111111");
        assert!(repos.iter().all(|r| !r.pinned));

        // ピン留めすると末尾のリポジトリが先頭に来る
        set_repo_pinned(tmp.path(), "no-note--22222222", true).unwrap();
        let repos = list_repos(tmp.path());
        assert_eq!(repos[0].repo_key, "no-note--22222222");
        assert!(repos[0].pinned);
        assert!(!repos[1].pinned);

        // 解除で通常の並びに戻る
        set_repo_pinned(tmp.path(), "no-note--22222222", false).unwrap();
        let repos = list_repos(tmp.path());
        assert_eq!(repos[0].repo_key, "with-note--11111111");

        // 存在しないリポジトリはピン留め不可
        assert!(set_repo_pinned(tmp.path(), "missing--00000000", true).is_err());
        // パストラバーサルも拒否
        assert!(set_repo_pinned(tmp.path(), "../etc", true).is_err());
    }

    #[test]
    fn delete_note_removes_pin() {
        let tmp = tempfile::tempdir().unwrap();
        setup_repo(tmp.path(), "demo--12345678");
        let key = "demo--12345678";
        let dir = tmp.path().join("repos").join(key).join("notes");
        fs::write(dir.join("2026-07-01-a.md"), "a\n").unwrap();
        set_pinned(tmp.path(), key, "2026-07-01-a.md", true).unwrap();
        delete_note(tmp.path(), key, "2026-07-01-a.md").unwrap();
        assert!(load_pins(tmp.path(), key).is_empty());
    }

    #[test]
    fn slugify_handles_japanese_and_symbols() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("日本語タイトル"), "日本語タイトル");
        assert_eq!(slugify("!!!"), "note");
    }
}
