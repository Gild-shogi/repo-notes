import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createNote } from "../api";

interface Props {
  repoKey: string;
  onCreated: (fileName: string) => void;
}

export function NewNoteButton({ repoKey, onCreated }: Props) {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const queryClient = useQueryClient();

  const create = useMutation({
    mutationFn: (value: string) => createNote(repoKey, value),
    onSuccess: (fileName) => {
      queryClient.invalidateQueries({ queryKey: ["notes", repoKey] });
      queryClient.invalidateQueries({ queryKey: ["repos"] });
      setOpen(false);
      setTitle("");
      onCreated(fileName);
    },
  });

  if (!open) {
    return (
      <button
        type="button"
        className="btn new-note"
        title="新規ノート"
        onClick={() => setOpen(true)}
      >
        ＋ 新規
      </button>
    );
  }

  return (
    <form
      className="new-note-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (title.trim() && !create.isPending) create.mutate(title);
      }}
    >
      <input
        autoFocus
        value={title}
        placeholder="ノートのタイトル"
        onChange={(e) => setTitle(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            setOpen(false);
            setTitle("");
          }
        }}
      />
      <button type="submit" className="btn primary" disabled={!title.trim() || create.isPending}>
        作成
      </button>
      {create.isError && (
        <span className="editor-status error">{String(create.error)}</span>
      )}
    </form>
  );
}
