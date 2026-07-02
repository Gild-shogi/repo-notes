import { describe, expect, it } from "vitest";
import { classifyLink } from "./links";

const notesDir = "/Users/me/.repo-notes/repos/demo--12345678/notes";

describe("classifyLink", () => {
  it("http(s)/mailtoはurlとして開く", () => {
    expect(classifyLink("https://example.com/a?b=1", notesDir)).toEqual({
      type: "url",
      target: "https://example.com/a?b=1",
    });
    expect(classifyLink("mailto:a@example.com", notesDir).type).toBe("url");
  });

  it("危険・未知スキームはブロック", () => {
    expect(classifyLink("javascript:alert(1)", notesDir).type).toBe("blocked");
    expect(classifyLink("data:text/html,x", notesDir).type).toBe("blocked");
    expect(classifyLink("vscode://file/x", notesDir).type).toBe("blocked");
    expect(classifyLink("#section", notesDir).type).toBe("blocked");
  });

  it("file://と絶対パスはpath", () => {
    expect(classifyLink("file:///Users/me/doc%20a.pdf", notesDir)).toEqual({
      type: "path",
      target: "/Users/me/doc a.pdf",
    });
    expect(classifyLink("/Users/me/proj/src/main.rs", notesDir)).toEqual({
      type: "path",
      target: "/Users/me/proj/src/main.rs",
    });
  });

  it("~/はホームに展開される", () => {
    expect(classifyLink("~/proj/README.md", notesDir)).toEqual({
      type: "path",
      target: "/Users/me/proj/README.md",
    });
  });

  it("同ディレクトリのノートはnote、その他相対はnotesDir基準のpath", () => {
    expect(classifyLink("2026-07-01-other.md", notesDir)).toEqual({
      type: "note",
      target: "2026-07-01-other.md",
    });
    expect(classifyLink("./2026-07-01-viz.html", notesDir)).toEqual({
      type: "note",
      target: "2026-07-01-viz.html",
    });
    expect(classifyLink("assets/x/report.pdf", notesDir)).toEqual({
      type: "path",
      target: `${notesDir}/assets/x/report.pdf`,
    });
  });

  it("相対パスのトラバーサルはブロック", () => {
    expect(classifyLink("../secret.md", notesDir).type).toBe("blocked");
    expect(classifyLink("assets/../../x.pdf", notesDir).type).toBe("blocked");
  });
});
