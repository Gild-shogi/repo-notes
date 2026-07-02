import { useQuery } from "@tanstack/react-query";
import { listNotes } from "../api";
import { formatDateTime } from "../lib/date";

interface Props {
  repoKey: string;
  selectedFileName: string | null;
  onSelect: (fileName: string) => void;
}

export function NoteList({ repoKey, selectedFileName, onSelect }: Props) {
  const { data: notes, isLoading } = useQuery({
    queryKey: ["notes", repoKey],
    queryFn: () => listNotes(repoKey),
  });

  if (isLoading) return <div className="pane-status">読み込み中…</div>;
  if (!notes || notes.length === 0) {
    return <div className="pane-status">ノートがありません。</div>;
  }

  return (
    <ul className="note-list">
      {notes.map((note) => (
        <li key={note.fileName}>
          <button
            type="button"
            className={note.fileName === selectedFileName ? "selected" : ""}
            onClick={() => onSelect(note.fileName)}
          >
            <span className="note-title">
              {note.kind === "html" && <span className="badge html">HTML</span>}
              {note.title}
            </span>
            <span className="note-meta">
              {formatDateTime(note.created ?? note.updatedMs)}
              {note.tags.map((tag) => (
                <span key={tag} className="badge tag">
                  {tag}
                </span>
              ))}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
