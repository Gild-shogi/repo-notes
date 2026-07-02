import { invoke } from "@tauri-apps/api/core";
import type { NoteContent, NoteSummary, RepoSummary } from "./types";

export const getDataRoot = () => invoke<string>("get_data_root");

export const listRepos = () => invoke<RepoSummary[]>("list_repos");

export const listNotes = (repoKey: string) =>
  invoke<NoteSummary[]>("list_notes", { repoKey });

export const readNote = (repoKey: string, fileName: string) =>
  invoke<NoteContent>("read_note", { repoKey, fileName });

export const writeNote = (repoKey: string, fileName: string, content: string) =>
  invoke<void>("write_note", { repoKey, fileName, content });

export const createNote = (repoKey: string, title: string) =>
  invoke<string>("create_note", { repoKey, title });

export const deleteNote = (repoKey: string, fileName: string) =>
  invoke<void>("delete_note", { repoKey, fileName });
