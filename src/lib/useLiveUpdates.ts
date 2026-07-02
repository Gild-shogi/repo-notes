import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";

interface ChangedPayload {
  repoKeys: string[];
}

/**
 * Rust側のファイル監視イベントを受けて該当クエリを再取得する。
 * 開いているノートのクエリも無効化されるが、編集中のエディタは
 * ローカルstateを持つため上書きされない（NoteEditor参照）。
 */
export function useLiveUpdates() {
  const queryClient = useQueryClient();

  useEffect(() => {
    const unlisten = listen<ChangedPayload>("repo-notes://changed", (event) => {
      queryClient.invalidateQueries({ queryKey: ["repos"] });
      for (const repoKey of event.payload.repoKeys) {
        queryClient.invalidateQueries({ queryKey: ["notes", repoKey] });
        queryClient.invalidateQueries({ queryKey: ["note", repoKey] });
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [queryClient]);
}
