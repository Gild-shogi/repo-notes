/**
 * Markdownノート内のURL参照を表示可能なURLへ変換する。
 *
 * - `assets/...` などノートからの相対パスは asset protocol のURLへ書き換える
 *   （変換関数は注入する: Tauri実行時は convertFileSrc、テストでは恒等関数）
 * - http(s) / data / asset URLはそのまま通す
 * - 絶対パスやディレクトリトラバーサルを含む参照は拒否して空文字にする
 */
export function rewriteAssetUrl(
  src: string,
  notesDirAbs: string,
  convert: (absPath: string) => string,
): string {
  if (src === "") return "";
  if (/^(https?:|data:|asset:|blob:)/i.test(src)) return src;
  if (src.startsWith("http://asset.localhost")) return src;
  try { src = decodeURIComponent(src); } catch { return ""; }
  // 絶対パス・スキーム付き・トラバーサルは拒否
  if (src.startsWith("/") || src.includes("..") || /^[a-z][a-z0-9+.-]*:/i.test(src)) {
    return "";
  }
  const rel = src.replace(/^\.\//, "");
  const base = notesDirAbs.endsWith("/") ? notesDirAbs.slice(0, -1) : notesDirAbs;
  return convert(`${base}/${rel}`);
}
