import { useCallback, useMemo, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { keymap, EditorView } from "@codemirror/view";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { readNote, writeNote } from "../api";
import type { SaveError, SaveOutcome } from "../types";
import { editorExtensions } from "../lib/editorTheme";

interface Props {
  repoKey: string;
  fileName: string;
  initialContent: string;
}

export function NoteEditor({ repoKey, fileName, initialContent }: Props) {
  const [content, setContent] = useState(initialContent);
  const [savedContent, setSavedContent] = useState(initialContent);
  const dirty = content !== savedContent;
  const queryClient = useQueryClient();

  const save = useMutation<SaveOutcome, SaveError, { content: string; expectedRaw: string }>({
    mutationFn: (input) => writeNote(repoKey, fileName, input.content, input.expectedRaw),
    onSuccess: (_, input) => {
      setSavedContent(input.content);
      queryClient.invalidateQueries({ queryKey: ["note", repoKey, fileName] });
      queryClient.invalidateQueries({ queryKey: ["notes", repoKey] });
      queryClient.invalidateQueries({ queryKey: ["repos"] });
    },
  });

  const reload = useMutation({
    mutationFn: () => readNote(repoKey, fileName),
    onSuccess: (note) => {
      setContent(note.raw);
      setSavedContent(note.raw);
      save.reset();
    },
  });
  const conflict = save.error?.kind === "conflict";

  const { isPending, mutate } = save;
  const saveContent = useCallback((value: string) => {
    if (!isPending && !reload.isPending && !conflict) mutate({ content: value, expectedRaw: savedContent });
  }, [isPending, mutate, reload.isPending, conflict, savedContent]);

  const extensions = useMemo(
    () => [
      markdown({ base: markdownLanguage, codeLanguages: languages }),
      EditorView.lineWrapping,
      ...editorExtensions,
      keymap.of([
        {
          key: "Mod-s",
          run: (view: EditorView) => {
            saveContent(view.state.doc.toString());
            return true;
          },
        },
      ]),
    ],
    [saveContent],
  );

  return (
    <div className="note-editor">
      <div className="editor-toolbar">
        <span className={`editor-status ${dirty ? "dirty" : ""}`}>
          {save.isPending
            ? "保存中…"
            : dirty
              ? "未保存の変更あり"
              : "保存済み"}
        </span>
        {save.isError && (
          <span className="editor-status error">
            {conflict
              ? "別の書き込みで更新されています。保存を拒否し、下書きを保持しました。必要な内容を控えてから最新を読み込んでください。"
              : `保存に失敗しました: ${save.error?.kind === "failure" ? save.error.message : String(save.error)}`}
          </span>
        )}
        {save.data?.cleanupWarnings.map((warning) => <span className="editor-status error" key={warning}>{warning}</span>)}
        {reload.isError && <span className="editor-status error">再読込に失敗しました。下書きは保持しています: {String(reload.error)}</span>}
        {conflict && (
          <button type="button" className="btn" disabled={reload.isPending} onClick={() => {
            if (window.confirm("編集中の下書きを破棄し、ファイルの最新内容を読み込みますか？")) reload.mutate();
          }}>下書きを破棄して最新を読込</button>
        )}
        <button
          type="button"
          className="btn primary"
          disabled={!dirty || save.isPending || reload.isPending || conflict}
          onClick={() => saveContent(content)}
        >
          保存 (⌘S)
        </button>
      </div>
      <CodeMirror
        value={content}
        editable={!reload.isPending}
        extensions={extensions}
        onChange={(value) => {
          setContent(value);
        }}
        onBlur={() => {
          if (dirty) saveContent(content);
        }}
        autoFocus
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          highlightSelectionMatches: false,
        }}
      />
    </div>
  );
}
