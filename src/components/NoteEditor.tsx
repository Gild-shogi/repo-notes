import { useMemo, useRef, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { keymap, EditorView } from "@codemirror/view";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { writeNote } from "../api";
import { editorExtensions } from "../lib/editorTheme";

interface Props {
  repoKey: string;
  fileName: string;
  initialContent: string;
}

export function NoteEditor({ repoKey, fileName, initialContent }: Props) {
  const [content, setContent] = useState(initialContent);
  const [dirty, setDirty] = useState(false);
  const queryClient = useQueryClient();

  const save = useMutation({
    mutationFn: (value: string) => writeNote(repoKey, fileName, value),
    onSuccess: () => {
      setDirty(false);
      queryClient.invalidateQueries({ queryKey: ["note", repoKey, fileName] });
      queryClient.invalidateQueries({ queryKey: ["notes", repoKey] });
      queryClient.invalidateQueries({ queryKey: ["repos"] });
    },
  });

  // keymap から常に最新値を保存できるよう ref に保持する
  const latest = useRef(content);
  latest.current = content;
  const saveRef = useRef(() => {});
  saveRef.current = () => {
    if (!save.isPending) save.mutate(latest.current);
  };

  const extensions = useMemo(
    () => [
      markdown({ base: markdownLanguage, codeLanguages: languages }),
      EditorView.lineWrapping,
      ...editorExtensions,
      keymap.of([
        {
          key: "Mod-s",
          run: () => {
            saveRef.current();
            return true;
          },
        },
      ]),
    ],
    [],
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
            保存に失敗しました: {String(save.error)}
          </span>
        )}
        <button
          type="button"
          className="btn primary"
          disabled={!dirty || save.isPending}
          onClick={() => saveRef.current()}
        >
          保存 (⌘S)
        </button>
      </div>
      <CodeMirror
        value={content}
        extensions={extensions}
        onChange={(value) => {
          setContent(value);
          setDirty(true);
        }}
        onBlur={() => {
          if (dirty) saveRef.current();
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
