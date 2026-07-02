import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listNotes, setPinned } from "../api";
import { formatDateTime } from "../lib/date";
import { useDebouncedValue } from "../lib/useDebouncedValue";
import { PinIcon } from "./icons";

interface Props {
  repoKey: string;
  selectedFileName: string | null;
  onSelect: (fileName: string) => void;
}

export function NoteList({ repoKey, selectedFileName, onSelect }: Props) {
  const [search, setSearch] = useState("");
  const query = useDebouncedValue(search.trim(), 200);
  const queryClient = useQueryClient();

  const { data: notes, isLoading } = useQuery({
    queryKey: ["notes", repoKey, query],
    queryFn: () => listNotes(repoKey, query || undefined),
  });

  const pin = useMutation({
    mutationFn: ({ fileName, pinned }: { fileName: string; pinned: boolean }) =>
      setPinned(repoKey, fileName, pinned),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["notes", repoKey] });
    },
  });

  return (
    <>
      <div className="note-search">
        <input
          type="search"
          value={search}
          placeholder="検索（タイトル・タグ・本文）"
          onChange={(e) => setSearch(e.currentTarget.value)}
        />
      </div>
      {isLoading ? (
        <div className="pane-status">読み込み中…</div>
      ) : !notes || notes.length === 0 ? (
        <div className="pane-status">
          {query ? `「${query}」に一致するノートがありません。` : "ノートがありません。"}
        </div>
      ) : (
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
              <button
                type="button"
                className={`pin-btn ${note.pinned ? "pinned" : ""}`}
                title={note.pinned ? "ピン留めを解除" : "ピン留め"}
                disabled={pin.isPending}
                onClick={() =>
                  pin.mutate({ fileName: note.fileName, pinned: !note.pinned })
                }
              >
                <PinIcon filled={note.pinned} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
