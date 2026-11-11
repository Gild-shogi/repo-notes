import type { SaveError } from "../types";

export function canAutoSave(error: SaveError | null): boolean {
  return error?.kind !== "conflict" && error?.kind !== "indeterminate";
}
