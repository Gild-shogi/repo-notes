import { invoke } from "@tauri-apps/api/core";
import type { NoteContent, NoteSummary, RepoSummary } from "./types";

export const getDataRoot = () => invoke<string>("get_data_root");

export const listRepos = () => invoke<RepoSummary[]>("list_repos");

export const listNotes = (repoKey: string, query?: string) =>
  invoke<NoteSummary[]>("list_notes", { repoKey, query: query ?? null });

export const setPinned = (repoKey: string, fileName: string, pinned: boolean) =>
  invoke<void>("set_pinned", { repoKey, fileName, pinned });

export const setRepoPinned = (repoKey: string, pinned: boolean) =>
  invoke<void>("set_repo_pinned", { repoKey, pinned });

/** ノート内リンクのローカルパスをRust側の検証付きで開く */
export const openLocalPath = (path: string) =>
  invoke<void>("open_local_path", { path });

export const openNote = (repoKey: string, fileName: string, reveal = false) =>
  invoke<void>("open_note", { repoKey, fileName, reveal });

export const readNote = (repoKey: string, fileName: string) =>
  invoke<NoteContent>("read_note", { repoKey, fileName });

export const writeNote = (repoKey: string, fileName: string, content: string) =>
  invoke<void>("write_note", { repoKey, fileName, content });

export const createNote = (repoKey: string, title: string) =>
  invoke<string>("create_note", { repoKey, title });

export const deleteNote = (repoKey: string, fileName: string) =>
  invoke<void>("delete_note", { repoKey, fileName });

/** ネイティブ印刷ダイアログを開く（PDF保存もここから可能） */
export const printView = () => invoke<void>("print_view");
