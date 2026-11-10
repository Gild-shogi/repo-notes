import { useCallback, useMemo, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { keymap, EditorView } from "@codemirror/view";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { readNote, writeNote } from "../api";
import type { SaveError, SaveOutcome } from "../types";
import { editorExtensions } from "../lib/editorTheme";
import { useNoteDraft } from "../lib/noteDrafts";
import { canAutoSave } from "../lib/saveState";
import { useConnection } from "../lib/connection";

interface Props {
  repoKey: string;
  fileName: string;
  initialContent: string;
}

export function NoteEditor({ repoKey, fileName, initialContent }: Props) {
  const { switchingTarget } = useConnection();
  const [draft, updateDraft] = useNoteDraft(repoKey, fileName, initialContent);
  const { content, expectedRaw: savedContent, operation } = draft;
  const [confirmingReload, setConfirmingReload] = useState(false);
  const dirty = content !== savedContent;
  const saving = operation.kind === "saving";
  const reloading = operation.kind === "reloading";
  const readOnly = reloading || switchingTarget;
  const saveError = operation.kind === "save-error" ? operation.error : null;
  const conflict = saveError?.kind === "conflict" || operation.kind === "reload-error" || reloading;
  const blocked = conflict || !canAutoSave(saveError);
  const queryClient = useQueryClient();

  const save = useMutation<SaveOutcome, SaveError, { content: string; expectedRaw: string }>({
    mutationFn: (input) => writeNote(repoKey, fileName, input.content, input.expectedRaw),
    onMutate: () => updateDraft((current) => ({ ...current, operation: { kind: "saving" } })),
    onError: (error) => updateDraft((current) => ({ ...current, operation: { kind: "save-error", error } })),
    onSuccess: (outcome, input) => {
      updateDraft((current) => ({ ...current, expectedRaw: input.content, operation: { kind: "idle" }, cleanupWarnings: outcome.cleanupWarnings }));
      queryClient.invalidateQueries({ queryKey: ["note", repoKey, fileName] });
      queryClient.invalidateQueries({ queryKey: ["notes", repoKey] });
      queryClient.invalidateQueries({ queryKey: ["repos"] });
    },
  });

  const reload = useMutation({
    mutationFn: () => readNote(repoKey, fileName),
    onMutate: () => updateDraft((current) => ({ ...current, operation: { kind: "reloading" } })),
    onError: (error) => updateDraft((current) => ({ ...current, operation: { kind: "reload-error", message: String(error) } })),
    onSuccess: (note) => {
      updateDraft(() => ({ content: note.raw, expectedRaw: note.raw, operation: { kind: "idle" }, cleanupWarnings: [] }));
      setConfirmingReload(false);
    },
  });

  const { mutate } = save;
  const saveContent = useCallback((value: string) => {
    if (!saving && !readOnly && !blocked) mutate({ content: value, expectedRaw: savedContent });
  }, [saving, mutate, readOnly, blocked, savedContent]);

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
          {switchingTarget
            ? "接続先を変更中…"
            : saving
            ? "保存中…"
            : dirty
              ? "未保存の変更あり"
              : "保存済み"}
        </span>
        {saveError && (
          <span className="editor-status error">
            {conflict
              ? "別の書き込みで更新されています。保存を拒否し、下書きを保持しました。必要な内容を控えてから最新を読み込んでください。"
              : saveError.kind === "indeterminate"
                ? `保存結果を確認できません。下書きを保持し、自動再送を止めました。再接続後に内容を確認してください: ${saveError.message}`
                : `保存に失敗しました: ${saveError.kind === "failure" ? saveError.message : String(saveError)}`}
          </span>
        )}
        {draft.cleanupWarnings.map((warning) => <span className="editor-status error" key={warning}>{warning}</span>)}
        {operation.kind === "reload-error" && <span className="editor-status error">再読込に失敗しました。下書きは保持しています: {operation.message}</span>}
        {blocked && (confirmingReload ? (
          <>
            <span className="editor-status error">下書きを破棄して最新内容を読み込みます。</span>
            <button type="button" className="btn danger" disabled={readOnly} onClick={() => reload.mutate()}>破棄して読込</button>
            <button type="button" className="btn" disabled={readOnly} onClick={() => setConfirmingReload(false)}>取消</button>
          </>
        ) : (
          <button type="button" className="btn" disabled={switchingTarget} onClick={() => setConfirmingReload(true)}>下書きを破棄して最新を読込</button>
        ))}
        <button
          type="button"
          className="btn primary"
          disabled={!dirty || saving || readOnly || blocked}
          onClick={() => saveContent(content)}
        >
          保存 (⌘S)
        </button>
      </div>
      <CodeMirror
        value={content}
        editable={!readOnly}
        extensions={extensions}
        onChange={(value) => {
          if (readOnly) return;
          updateDraft((current) => ({ ...current, content: value }));
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
