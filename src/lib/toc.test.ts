import { describe, expect, it } from "vitest";
import { extractToc } from "./toc";

describe("extractToc", () => {
  it("h1〜h4を抽出しインライン装飾を除去する", () => {
    const body = "# タイトル\n\n## **強調** と `code`\n\n### [リンク](https://x)\n\n##### h5は対象外\n";
    expect(extractToc(body)).toEqual([
      { level: 1, text: "タイトル", id: "タイトル" },
      { level: 2, text: "強調 と code", id: "強調-と-code" },
      { level: 3, text: "リンク", id: "リンク" },
    ]);
  });

  it("コードブロック内の#は無視する", () => {
    const body = "## 本物\n\n```bash\n# コメント\n## これも無視\n```\n\n~~~\n# これも\n~~~\n\n## 本物2\n";
    expect(extractToc(body).map((i) => i.text)).toEqual(["本物", "本物2"]);
  });

  it("重複見出しは連番付きidになる（rehype-slug互換）", () => {
    const body = "## 手順\n\n## 手順\n";
    expect(extractToc(body).map((i) => i.id)).toEqual(["手順", "手順-1"]);
  });
});
