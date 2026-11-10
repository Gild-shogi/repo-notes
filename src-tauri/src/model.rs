use serde::{Deserialize, Serialize};

/// repos/<repo-key>/meta.json の内容。スキル側スクリプトのみが書き込む。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoMeta {
    pub version: u32,
    pub repo_key: String,
    pub repo_name: String,
    #[serde(default)]
    pub canonical_url: Option<String>,
    #[serde(default)]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Md,
    Html,
}

/// MDのfrontmatter / HTMLのrepo-notes-metaコメントから読むメタデータ。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub repo_key: String,
    pub repo_name: String,
    pub canonical_url: Option<String>,
    pub note_count: usize,
    pub last_note_at_ms: Option<i64>,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub file_name: String,
    pub kind: NoteKind,
    pub title: String,
    pub created: Option<String>,
    pub updated_ms: i64,
    pub tags: Vec<String>,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteContent {
    pub file_name: String,
    pub kind: NoteKind,
    pub meta: NoteMeta,
    /// レンダリング用本文（MDはfrontmatter除去済み。HTMLは全文）
    pub body: String,
    /// エディタ用の生ファイル内容
    pub raw: String,
    pub abs_path: String,
    pub updated_ms: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    pub cleanup_warnings: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveOutcome {
    pub cleanup_warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum SaveError {
    Conflict,
    Failure(String),
}

impl From<String> for SaveError {
    fn from(message: String) -> Self {
        Self::Failure(message)
    }
}
