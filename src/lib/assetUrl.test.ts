import { describe, expect, it } from "vitest";
import { rewriteAssetUrl } from "./assetUrl";

const notesDir = "/Users/me/.repo-notes/repos/demo--12345678/notes";
const convert = (p: string) => `asset://localhost${p}`;

describe("rewriteAssetUrl", () => {
  it("相対パスをasset URLに変換する", () => {
    expect(rewriteAssetUrl("assets/note/foo.png", notesDir, convert)).toBe(
      `asset://localhost${notesDir}/assets/note/foo.png`,
    );
    expect(rewriteAssetUrl("./assets/a.svg", notesDir, convert)).toBe(
      `asset://localhost${notesDir}/assets/a.svg`,
    );
  });

  it("エンコード済みの相対画像を実際のLinuxファイル名へ戻す", () => {
    expect(rewriteAssetUrl("assets/%E7%94%BB%E5%83%8F%201.svg", notesDir, convert)).toBe(`asset://localhost${notesDir}/assets/画像 1.svg`);
  });

  it("http(s)/data URLはそのまま通す", () => {
    expect(rewriteAssetUrl("https://example.com/a.png", notesDir, convert)).toBe(
      "https://example.com/a.png",
    );
    expect(rewriteAssetUrl("data:image/png;base64,xxx", notesDir, convert)).toBe(
      "data:image/png;base64,xxx",
    );
  });

  it("絶対パス・トラバーサル・未知スキームは拒否する", () => {
    expect(rewriteAssetUrl("/etc/passwd", notesDir, convert)).toBe("");
    expect(rewriteAssetUrl("../../../secret.png", notesDir, convert)).toBe("");
    expect(rewriteAssetUrl("assets/../../x.png", notesDir, convert)).toBe("");
    expect(rewriteAssetUrl("file:///etc/passwd", notesDir, convert)).toBe("");
  });
});
