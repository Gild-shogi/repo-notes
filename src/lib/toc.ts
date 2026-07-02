import GithubSlugger from "github-slugger";

export interface TocItem {
  level: number;
  text: string;
  /** rehype-slugが見出しに振るidと同じスラグ */
  id: string;
}

/**
 * Markdown本文から目次項目（h1〜h4）を抽出する。
 * フェンスコードブロック内の `#` 行は無視する。
 * idはrehype-slug（github-slugger）と同じアルゴリズムで生成する。
 */
export function extractToc(body: string): TocItem[] {
  const slugger = new GithubSlugger();
  const items: TocItem[] = [];
  let fence: string | null = null;

  for (const line of body.split("\n")) {
    const trimmed = line.trimEnd();
    const fenceMatch = /^\s*(`{3,}|~{3,})/.exec(trimmed);
    if (fenceMatch) {
      if (fence === null) {
        fence = fenceMatch[1][0];
      } else if (fenceMatch[1][0] === fence) {
        fence = null;
      }
      continue;
    }
    if (fence !== null) continue;

    const heading = /^(#{1,4})\s+(.+?)\s*#*\s*$/.exec(trimmed);
    if (!heading) continue;
    // インライン装飾（リンク・強調・コード）を落として表示テキストに揃える
    const text = heading[2]
      .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
      .replace(/[*_`~]/g, "")
      .trim();
    if (!text) continue;
    items.push({ level: heading[1].length, text, id: slugger.slug(text) });
  }
  return items;
}
