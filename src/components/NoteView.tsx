import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { deleteNote, readNote } from "../api";
import { formatDateTime } from "../lib/date";
import { extractToc } from "../lib/toc";
import { MarkdownView } from "./MarkdownView";
import { NoteEditor } from "./NoteEditor";
import { Toc } from "./Toc";

interface Props {
  repoKey: string;
  fileName: string;
  startInEdit?: boolean;
  onDeleted: () => void;
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
  const queryClient = useQueryClient();

  const { data: note, isLoading, error } = useQuery({
    queryKey: ["note", repoKey, fileName],
    queryFn: () => readNote(repoKey, fileName),
  });

  const remove = useMutation({
    mutationFn: () => deleteNote(repoKey, fileName),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["notes", repoKey] });
      queryClient.invalidateQueries({ queryKey: ["repos"] });
      onDeleted();
    },
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
              className="btn"
              title="デフォルトのアプリで開く"
              onClick={() => openPath(note.absPath)}
            >
              エディタで開く
            </button>
            <button
              type="button"
              className="btn"
              title="Finderで表示"
              onClick={() => revealItemInDir(note.absPath)}
            >
              Finder
            </button>
            {note.kind === "md" && (
              <button
                type="button"
                className="btn"
                onClick={() => setEditing((v) => !v)}
              >
                {editing ? "プレビュー" : "編集"}
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
                className="btn"
                onClick={() => setConfirmingDelete(true)}
              >
                削除
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
        // allow-same-origin を付けないことで opaque origin となり、
        // ノート内JSから親DOM・Tauri IPCへは到達できない
        <iframe
          className="html-note-frame"
          src={convertFileSrc(note.absPath)}
          sandbox="allow-scripts"
          title={note.meta.title ?? note.fileName}
        />
      )}
    </article>
  );
}
