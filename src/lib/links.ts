/**
 * ノート内ハイパーリンクの分類。
 * - url: ブラウザ等のデフォルトハンドラで開く（http/https/mailto）
 * - path: 関連付けられたアプリで開くローカルパス
 * - note: 同じリポジトリ内の別ノート（アプリ内で開く）
 * - blocked: 開かない（危険スキーム・トラバーサル・アンカー等）
 */
export type LinkAction =
  | { type: "url"; target: string }
  | { type: "path"; target: string }
  | { type: "note"; target: string }
  | { type: "blocked" };

function safeDecode(s: string): string {
  try {
    return decodeURIComponent(s);
  } catch {
    return s;
  }
}

export function classifyLink(href: string, notesDirAbs: string, homeDir?: string | null): LinkAction {
  const trimmed = href.trim();
  if (!trimmed || trimmed.startsWith("#")) return { type: "blocked" };
  if (/^(https?|mailto):/i.test(trimmed)) return { type: "url", target: trimmed };
  if (/^file:\/\//i.test(trimmed)) {
    const path = safeDecode(trimmed.replace(/^file:\/\/(localhost)?/i, ""));
    if (path.split("/").includes("..")) return { type: "blocked" };
    return { type: "path", target: path };
  }
  // http/https/mailto/file 以外のスキームは開かない（javascript:等の排除を含む）
  if (/^[a-z][a-z0-9+.-]*:/i.test(trimmed)) return { type: "blocked" };

  const decoded = safeDecode(trimmed);
  // どの形式でも `..` による遡りは禁止（最終検証はRust側でも行う）
  if (decoded.split("/").includes("..")) return { type: "blocked" };
  if (decoded.startsWith("~/")) {
    const home = homeDir ?? notesDirAbs.match(/^\/Users\/[^/]+/)?.[0];
    if (!home) return { type: "blocked" };
    return { type: "path", target: home + decoded.slice(1) };
  }
  if (decoded.startsWith("/")) return { type: "path", target: decoded };

  const rel = decoded.replace(/^\.\//, "");
  if (!rel.includes("/") && /\.(md|html)$/i.test(rel)) {
    return { type: "note", target: rel };
  }
  const base = notesDirAbs.endsWith("/") ? notesDirAbs.slice(0, -1) : notesDirAbs;
  return { type: "path", target: `${base}/${rel}` };
}
