import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import DOMPurify from "dompurify";
import { deleteNote, openNote, printView, readNote } from "../api";
import { formatDateTime } from "../lib/date";
import { extractToc } from "../lib/toc";
import {
  CheckIcon,
  CopyIcon,
  EditIcon,
  EyeIcon,
  FolderIcon,
  OpenExternalIcon,
  PrintIcon,
  PromptIcon,
  TrashIcon,
} from "./icons";
import { MarkdownView } from "./MarkdownView";
import { NoteEditor } from "./NoteEditor";
import { Toc } from "./Toc";

const STATIC_HTML_PREFIX =
  '<!doctype html><meta http-equiv="Content-Security-Policy" content="' +
  "default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; " +
  "img-src data:; base-uri 'none'; form-action 'none'\">";

interface Props {
  repoKey: string;
  fileName: string;
  startInEdit?: boolean;
  onDeleted: (cleanupWarnings: string[]) => void;
  onNavigateNote?: (fileName: string) => void;
}

export function NoteView({
  repoKey,
  fileName,
  startInEdit = false,
  onDeleted,
  onNavigateNote,
}: Props) {
  const [editing, setEditing] = useState(startInEdit);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [copied, setCopied] = useState<"path" | "prompt" | null>(null);
  const copiedTimer = useRef<number | undefined>(undefined);
  const queryClient = useQueryClient();

  useEffect(() => () => window.clearTimeout(copiedTimer.current), []);

  const copyToClipboard = async (kind: "path" | "prompt", text: string) => {
    await writeText(text);
    setCopied(kind);
    window.clearTimeout(copiedTimer.current);
    copiedTimer.current = window.setTimeout(() => setCopied(null), 1500);
  };

  const { data: note, isLoading, error } = useQuery({
    queryKey: ["note", repoKey, fileName],
    queryFn: () => readNote(repoKey, fileName),
  });

  // 印刷/PDF保存時のデフォルトファイル名になるため、表示中ノートのタイトルを反映する
  useEffect(() => {
    if (note) document.title = note.meta.title ?? note.fileName;
    return () => {
      document.title = "repo-notes";
    };
  }, [note]);

  const remove = useMutation({
    mutationFn: () => deleteNote(repoKey, fileName),
    onSuccess: (outcome) => {
      queryClient.invalidateQueries({ queryKey: ["notes", repoKey] });
      queryClient.invalidateQueries({ queryKey: ["repos"] });
      onDeleted(outcome.cleanupWarnings);
    },
  });

  const open = useMutation({
    mutationFn: (reveal: boolean) => openNote(repoKey, fileName, reveal),
  });

  if (isLoading) return <div className="pane-status">読み込み中…</div>;
  if (error || !note) {
    return (
      <div className="pane-status error">ノートを読み込めません: {String(error)}</div>
    );
  }

  const notesDirAbs = note.absPath.slice(
    0,
    note.absPath.length - note.fileName.length - 1,
  );

  return (
    <article className={`note-view ${note.kind === "html" ? "note-view--html" : ""}`}>
      <header className="note-header">
        <div className="note-header-row">
          <h1>{note.meta.title}</h1>
          <div className="note-actions">
            <button
              type="button"
              className="icon-action"
              title={copied === "path" ? "コピーしました" : "パスをコピー"}
              aria-label="パスをコピー"
              onClick={() => copyToClipboard("path", note.absPath)}
            >
              {copied === "path" ? <CheckIcon /> : <CopyIcon />}
            </button>
            <button
              type="button"
              className="icon-action"
              title={copied === "prompt" ? "コピーしました" : "Claude用にコピー"}
              aria-label="Claude用にコピー"
              onClick={() =>
                copyToClipboard(
                  "prompt",
                  `repo-notes のメモ「${note.meta.title ?? note.fileName}」(${note.absPath}) を読んでから、その内容を踏まえて作業を進めてください。`,
                )
              }
            >
              {copied === "prompt" ? <CheckIcon /> : <PromptIcon />}
            </button>
            <button
              type="button"
              className="icon-action"
              title="エディタで開く"
              aria-label="エディタで開く"
              onClick={() => open.mutate(false)}
            >
              <OpenExternalIcon />
            </button>
            <button
              type="button"
              className="icon-action"
              title="Finderで表示"
              aria-label="Finderで表示"
              onClick={() => open.mutate(true)}
            >
              <FolderIcon />
            </button>
            {note.kind === "md" && !editing && (
              <button
                type="button"
                className="icon-action"
                title="印刷 / PDF保存"
                aria-label="印刷 / PDF保存"
                onClick={() => printView()}
              >
                <PrintIcon />
              </button>
            )}
            {note.kind === "md" && (
              <button
                type="button"
                className="icon-action"
                title={editing ? "プレビューに戻る" : "編集"}
                aria-label={editing ? "プレビューに戻る" : "編集"}
                onClick={() => setEditing((v) => !v)}
              >
                {editing ? <EyeIcon /> : <EditIcon />}
              </button>
            )}
            {confirmingDelete ? (
              <>
                <button
                  type="button"
                  className="btn danger"
                  disabled={remove.isPending}
                  onClick={() => remove.mutate()}
                >
                  本当に削除する
                </button>
                <button
                  type="button"
                  className="btn"
                  onClick={() => setConfirmingDelete(false)}
                >
                  やめる
                </button>
              </>
            ) : (
              <button
                type="button"
                className="icon-action danger"
                title="削除"
                aria-label="削除"
                onClick={() => setConfirmingDelete(true)}
              >
                <TrashIcon />
              </button>
            )}
          </div>
        </div>
        <div className="note-header-meta">
          {note.meta.created && <span>{formatDateTime(note.meta.created)}</span>}
          {note.meta.agent && <span className="badge">{note.meta.agent}</span>}
          {note.meta.tags.map((tag) => (
            <span key={tag} className="badge tag">
              {tag}
            </span>
          ))}
        </div>
        {remove.isError && (
          <div className="pane-status error">
            削除に失敗しました: {String(remove.error)}
          </div>
        )}
        {open.isError && (
          <div className="pane-status error">{String(open.error)}</div>
        )}
      </header>
      {note.kind === "md" ? (
        editing ? (
          <NoteEditor
            repoKey={repoKey}
            fileName={fileName}
            initialContent={note.raw}
          />
        ) : (
          <>
            <Toc items={extractToc(note.body)} />
            <MarkdownView
              body={note.body}
              notesDirAbs={notesDirAbs}
              onNavigateNote={onNavigateNote}
            />
          </>
        )
      ) : (
        // WebKitのpreconnectはCSPを通らないため、接続ヒントを除いた文書を表示する。
        <iframe
          className="html-note-frame"
          srcDoc={
            STATIC_HTML_PREFIX +
            DOMPurify.sanitize(note.raw, {
              WHOLE_DOCUMENT: true,
              FORBID_TAGS: [
                "base",
                "embed",
                "frame",
                "frameset",
                "iframe",
                "link",
                "meta",
                "object",
                "script",
              ],
              FORBID_ATTR: [
                "action",
                "formaction",
                "href",
                "ping",
                "srcdoc",
                "srcset",
                "xlink:href",
              ],
              ALLOWED_URI_REGEXP: /^data:image\//i,
            })
          }
          sandbox=""
          title={note.meta.title ?? note.fileName}
        />
      )}
    </article>
  );
}
