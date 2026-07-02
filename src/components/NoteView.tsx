import { useQuery } from "@tanstack/react-query";
import { readNote } from "../api";
import { formatDateTime } from "../lib/date";
import { MarkdownView } from "./MarkdownView";

interface Props {
  repoKey: string;
  fileName: string;
}

export function NoteView({ repoKey, fileName }: Props) {
  const { data: note, isLoading, error } = useQuery({
    queryKey: ["note", repoKey, fileName],
    queryFn: () => readNote(repoKey, fileName),
  });

  if (isLoading) return <div className="pane-status">読み込み中…</div>;
  if (error || !note) {
    return <div className="pane-status error">ノートを読み込めません: {String(error)}</div>;
  }

  const notesDirAbs = note.absPath.slice(0, note.absPath.length - note.fileName.length - 1);

  return (
    <article className="note-view">
      <header className="note-header">
        <h1>{note.meta.title}</h1>
        <div className="note-header-meta">
          {note.meta.created && <span>{formatDateTime(note.meta.created)}</span>}
          {note.meta.agent && <span className="badge">{note.meta.agent}</span>}
          {note.meta.tags.map((tag) => (
            <span key={tag} className="badge tag">
              {tag}
            </span>
          ))}
        </div>
      </header>
      {note.kind === "md" ? (
        <MarkdownView body={note.body} notesDirAbs={notesDirAbs} />
      ) : (
        <div className="pane-status">HTMLノートの表示は未対応です。</div>
      )}
    </article>
  );
}
