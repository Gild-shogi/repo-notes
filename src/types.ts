export type NoteKind = "md" | "html";

export interface RepoSummary {
  repoKey: string;
  repoName: string;
  canonicalUrl: string | null;
  noteCount: number;
  lastNoteAtMs: number | null;
}

export interface NoteSummary {
  fileName: string;
  kind: NoteKind;
  title: string;
  created: string | null;
  updatedMs: number;
  tags: string[];
  pinned: boolean;
}

export interface NoteMeta {
  title: string | null;
  created: string | null;
  tags: string[];
  agent: string | null;
}

export interface NoteContent {
  fileName: string;
  kind: NoteKind;
  meta: NoteMeta;
  /** レンダリング用本文（MDはfrontmatter除去済み） */
  body: string;
  /** エディタ用の生ファイル内容 */
  raw: string;
  absPath: string;
  updatedMs: number;
}
