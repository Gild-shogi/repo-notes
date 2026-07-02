use crate::model::NoteMeta;

/// MDノートをパースし、(メタデータ, frontmatter除去済み本文) を返す。
/// frontmatterが不正なYAMLでもブロック自体は除去し、メタデータは空で返す。
pub fn parse_markdown(content: &str) -> (NoteMeta, String) {
    match split_frontmatter(content) {
        Some((yaml, body)) => {
            let meta = serde_yaml_ng::from_str::<NoteMeta>(yaml).unwrap_or_default();
            (meta, body.to_string())
        }
        None => (NoteMeta::default(), content.to_string()),
    }
}

/// 先頭の `---` ブロックを (yaml, 残り本文) に分割する。閉じ `---` が無ければ None。
fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = content.strip_prefix("\u{feff}").unwrap_or(content);
    let first_line_end = rest.find('\n')?;
    if rest[..first_line_end].trim_end_matches('\r') != "---" {
        return None;
    }
    let after_open = &rest[first_line_end + 1..];
    let mut offset = 0;
    for line in after_open.split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']) == "---" {
            let yaml = &after_open[..offset];
            let body = &after_open[offset + line.len()..];
            return Some((yaml, body));
        }
        offset += line.len();
    }
    None
}

/// MD本文の最初のH1見出しをタイトルとして拾う。
pub fn markdown_title_fallback(body: &str) -> Option<String> {
    body.lines()
        .map(|l| l.trim_end_matches('\r'))
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// HTMLノート先頭2KB以内の `<!-- repo-notes-meta {...} -->` コメントからメタデータを読む。
pub fn parse_html_meta(content: &str) -> NoteMeta {
    const MARKER: &str = "repo-notes-meta";
    const SCAN_LIMIT: usize = 2048;
    let head: String = content.chars().take(SCAN_LIMIT).collect();
    if let Some(marker_pos) = head.find(MARKER) {
        let after = &head[marker_pos + MARKER.len()..];
        if let Some(end) = after.find("-->") {
            let json = after[..end].trim();
            if let Ok(meta) = serde_json::from_str::<NoteMeta>(json) {
                return meta;
            }
        }
    }
    NoteMeta::default()
}

/// HTMLの `<title>` をタイトルとして拾う。
pub fn html_title_fallback(content: &str) -> Option<String> {
    let lower = content.to_lowercase();
    let start = lower.find("<title")?;
    let open_end = content[start..].find('>')? + start + 1;
    let close = lower[open_end..].find("</title>")? + open_end;
    let title = content[open_end..close].trim();
    (!title.is_empty()).then(|| title.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_with_frontmatter() {
        let content = "---\ntitle: テストメモ\ncreated: 2026-07-02T10:00:00+09:00\ntags: [a, b]\nagent: claude-code\n---\n\n# 本文\n";
        let (meta, body) = parse_markdown(content);
        assert_eq!(meta.title.as_deref(), Some("テストメモ"));
        assert_eq!(meta.created.as_deref(), Some("2026-07-02T10:00:00+09:00"));
        assert_eq!(meta.tags, vec!["a", "b"]);
        assert_eq!(meta.agent.as_deref(), Some("claude-code"));
        assert_eq!(body, "\n# 本文\n");
    }

    #[test]
    fn markdown_without_frontmatter() {
        let content = "# 見出しだけ\n本文\n";
        let (meta, body) = parse_markdown(content);
        assert!(meta.title.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn markdown_unclosed_frontmatter_is_treated_as_body() {
        let content = "---\ntitle: 閉じない\n\n本文のつもり\n";
        let (meta, body) = parse_markdown(content);
        assert!(meta.title.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn markdown_invalid_yaml_still_strips_block() {
        let content = "---\ntitle: [unclosed\n---\n本文\n";
        let (meta, body) = parse_markdown(content);
        assert!(meta.title.is_none());
        assert_eq!(body, "本文\n");
    }

    #[test]
    fn markdown_crlf_frontmatter() {
        let content = "---\r\ntitle: CRLF\r\n---\r\n本文\r\n";
        let (meta, body) = parse_markdown(content);
        assert_eq!(meta.title.as_deref(), Some("CRLF"));
        assert_eq!(body, "本文\r\n");
    }

    #[test]
    fn markdown_title_fallback_finds_first_h1() {
        assert_eq!(
            markdown_title_fallback("前置き\n# タイトル \n# 二個目\n"),
            Some("タイトル".to_string())
        );
        assert_eq!(markdown_title_fallback("H1なし\n"), None);
    }

    #[test]
    fn html_meta_comment() {
        let content = r#"<!-- repo-notes-meta {"title":"依存グラフ","created":"2026-07-02T10:00:00+09:00","tags":["viz"]} -->
<!DOCTYPE html><html><body></body></html>"#;
        let meta = parse_html_meta(content);
        assert_eq!(meta.title.as_deref(), Some("依存グラフ"));
        assert_eq!(meta.tags, vec!["viz"]);
    }

    #[test]
    fn html_meta_missing_or_broken_json() {
        assert!(parse_html_meta("<html></html>").title.is_none());
        assert!(parse_html_meta("<!-- repo-notes-meta {broken -->").title.is_none());
    }

    #[test]
    fn html_title_tag_fallback() {
        assert_eq!(
            html_title_fallback("<html><head><TITLE>ページ名</TITLE></head></html>"),
            Some("ページ名".to_string())
        );
        assert_eq!(html_title_fallback("<html></html>"), None);
    }
}
