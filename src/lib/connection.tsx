import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";
import { connectBackend, getConnection, updateConnection } from "../api";
import { useDraftChanges } from "./noteDrafts";
import type { ConnectionView, WslConfig } from "./connectionTypes";

const INITIAL: ConnectionView = { platform: "local", state: "connected", config: null, dataRoot: null, home: null, revision: 0, message: null, externalAvailable: true, connectionId: 0, distributions: [] };
const Context = createContext<{
  view: ConnectionView;
  connect: (config: WslConfig) => Promise<void>;
  targetRevision: number;
} | null>(null);

export function ConnectionProvider({ children }: { children: ReactNode }) {
  const [view, setView] = useState(INITIAL);
  const [targetRevision, setTargetRevision] = useState(0);
  const { unsaved, discard } = useDraftChanges();
  const queries = useQueryClient();
  const revision = useRef(-1);
  const connecting = useRef(false);

  useEffect(() => {
    let active = true;
    const apply = (next: ConnectionView) => {
      if (!active || next.revision < revision.current || (connecting.current && next.state === "connected")) return;
      revision.current = next.revision;
      updateConnection(next);
      setView(next);
      if (next.state === "connected") void queries.invalidateQueries();
    };
    void getConnection().then(apply).catch((error: unknown) => {
      if (active) setView((current) => ({ ...current, message: String(error) }));
    });
    const unsubscribe = listen<ConnectionView>("repo-notes://connection", (event) => apply(event.payload));
    return () => { active = false; void unsubscribe.then((remove) => remove()); };
  }, [queries]);

  const connect = useCallback(async (config: WslConfig) => {
    const changed = JSON.stringify(config) !== JSON.stringify(view.config);
    if (changed && unsaved.length) throw new Error("未保存の下書きを保存または明示的に破棄してから接続先を変更してください");
    if (queries.isMutating()) throw new Error("ノートの操作が終わってから接続してください");
    connecting.current = true;
    try {
      await queries.cancelQueries();
      const next = await connectBackend(config);
      if (changed) {
        discard();
        queries.clear();
        setTargetRevision((revision) => revision + 1);
      }
      if (next.revision >= revision.current) {
        revision.current = next.revision;
        updateConnection(next);
        setView(next);
      }
      void queries.invalidateQueries();
    } finally { connecting.current = false; }
  }, [view.config, unsaved.length, queries, discard]);
  const value = useMemo(() => ({ view, connect, targetRevision }), [view, connect, targetRevision]);
  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useConnection() {
  const context = useContext(Context);
  if (!context) throw new Error("ConnectionProvider is required");
  return context;
}

export function assetSource(path: string, connection: ConnectionView): string {
  return connection.platform === "windows"
    ? `http://asset.localhost/wsl/${connection.connectionId}/${encodeURIComponent(path)}`
    : convertFileSrc(path);
}
