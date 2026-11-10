import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import type { SaveError } from "../types";

type DraftOperation =
  | { kind: "idle" | "saving" | "reloading" }
  | { kind: "save-error"; error: SaveError }
  | { kind: "reload-error"; message: string };

interface NoteDraft {
  content: string;
  expectedRaw: string;
  operation: DraftOperation;
  cleanupWarnings: string[];
}

type Transition = (draft: NoteDraft) => NoteDraft;
type Drafts = Readonly<Record<string, NoteDraft>>;
const DraftContext = createContext<{
  drafts: Drafts;
  update: (key: string, initialContent: string, transition: Transition) => void;
  release: (key: string) => void;
  discard: () => void;
} | null>(null);

function initialDraft(content: string): NoteDraft {
  return { content, expectedRaw: content, operation: { kind: "idle" }, cleanupWarnings: [] };
}

export function NoteDraftProvider({ children }: { children: ReactNode }) {
  const [drafts, setDrafts] = useState<Drafts>({});
  const update = useCallback((key: string, initialContent: string, transition: Transition) => {
    setDrafts((current) => ({ ...current, [key]: transition(current[key] ?? initialDraft(initialContent)) }));
  }, []);
  const release = useCallback((key: string) => {
    setDrafts((current) => {
      const draft = current[key];
      if (!draft || draft.content !== draft.expectedRaw || draft.operation.kind !== "idle" || draft.cleanupWarnings.length) return current;
      const next = { ...current };
      delete next[key];
      return next;
    });
  }, []);
  const discard = useCallback(() => setDrafts({}), []);
  const value = useMemo(() => ({ drafts, update, release, discard }), [drafts, update, release, discard]);
  return <DraftContext.Provider value={value}>{children}</DraftContext.Provider>;
}

export function useNoteDraft(repoKey: string, fileName: string, initialContent: string) {
  const [openedContent] = useState(initialContent);
  const context = useContext(DraftContext);
  if (!context) throw new Error("NoteDraftProvider is required");
  const { drafts, update, release } = context;
  const key = JSON.stringify([repoKey, fileName]);
  const transition = useCallback((change: Transition) => update(key, openedContent, change), [update, key, openedContent]);
  useEffect(() => {
    release(key);
    return () => release(key);
  }, [release, key]);
  return [drafts[key] ?? initialDraft(openedContent), transition] as const;
}

export function useDraftChanges() {
  const context = useContext(DraftContext);
  if (!context) throw new Error("NoteDraftProvider is required");
  const unsaved = Object.entries(context.drafts).filter(([,draft]) =>
    draft.content !== draft.expectedRaw || draft.operation.kind === "saving" || draft.operation.kind === "reloading" || (draft.operation.kind === "save-error" && draft.operation.error.kind === "indeterminate")
  ).map(([key]) => {
    const [repoKey, fileName] = JSON.parse(key) as [string, string];
    return { repoKey, fileName };
  });
  return { unsaved, discard: context.discard };
}
