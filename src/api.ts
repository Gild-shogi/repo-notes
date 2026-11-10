import { invoke } from "@tauri-apps/api/core";
import type { ConnectionView, WslConfig } from "./lib/connectionTypes";
import type { DeleteOutcome, NoteContent, NoteSummary, RepoSummary, SaveOutcome } from "./types";

let connectionId: number | null = null;

export function updateConnection(view: ConnectionView) {
  connectionId = view.connectionId;
}

const call = <T>(command: string, args: Record<string, unknown> = {}) =>
  invoke<T>(command, { ...args, connectionId });

export async function getConnection(): Promise<ConnectionView> {
  const view = await invoke<ConnectionView>("get_connection");
  updateConnection(view);
  return view;
}

export async function connectBackend(config: WslConfig): Promise<ConnectionView> {
  const view = await invoke<ConnectionView>("connect_backend", { config });
  updateConnection(view);
  return view;
}

export const getDataRoot = () => call<string>("get_data_root");

export const listRepos = () => call<RepoSummary[]>("list_repos");

export const listNotes = (repoKey: string, query?: string) =>
  call<NoteSummary[]>("list_notes", { repoKey, query: query ?? null });

export const setPinned = (repoKey: string, fileName: string, pinned: boolean) =>
  call<void>("set_pinned", { repoKey, fileName, pinned });

export const setRepoPinned = (repoKey: string, pinned: boolean) =>
  call<void>("set_repo_pinned", { repoKey, pinned });

/** ノート内リンクのローカルパスをRust側の検証付きで開く */
export const openLocalPath = (path: string) =>
  call<void>("open_local_path", { path });

export const openNote = (repoKey: string, fileName: string, reveal = false) =>
  call<void>("open_note", { repoKey, fileName, reveal });

export const readNote = (repoKey: string, fileName: string) =>
  call<NoteContent>("read_note", { repoKey, fileName });

export const writeNote = (repoKey: string, fileName: string, content: string, expectedRaw: string) =>
  call<SaveOutcome>("write_note", { repoKey, fileName, content, expectedRaw });

export const createNote = (repoKey: string, title: string) =>
  call<string>("create_note", { repoKey, title });

export const deleteNote = (repoKey: string, fileName: string) =>
  call<DeleteOutcome>("delete_note", { repoKey, fileName });

/** ネイティブ印刷ダイアログを開く（PDF保存もここから可能） */
export const printView = () => call<void>("print_view");
