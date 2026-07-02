/**
 * ブラウザ単体でUIを確認するための Tauri API モック。
 * `pnpm dev` で起動した vite をブラウザで直接開いたときのみ有効になる
 * （Tauri 上では __TAURI_INTERNALS__ が既に存在するため何もしない）。
 *
 * 制限: convertFileSrc はパスをそのまま返すため、画像・HTMLノートの
 * iframe は実際には読み込めない。レイアウト・Markdown・Mermaid の確認用。
 */

const MD_FIXTURE = `---
title: モック: アーキテクチャ概要
created: 2026-07-02T12:40:00+09:00
tags: [architecture, mock]
agent: claude-code
---

## 全体像

ブラウザ単体モードのフィクスチャです。

\`\`\`mermaid
flowchart LR
    A[エージェント] -->|resolve-note-dir.sh| B[(~/.repo-notes)]
    C[Tauriアプリ] -->|read/write| B
\`\`\`

| レイヤ | 責務 |
|---|---|
| skill | repo-key導出 |
| Rust | CRUD・パース |

\`\`\`ts
const x: number = 42;
\`\`\`

- [x] チェックリスト
- [ ] 未完了項目
`;

const HTML_FIXTURE = `<!-- repo-notes-meta {"title":"モック: HTMLノート","created":"2026-07-02T12:50:00+09:00","tags":["viz"]} -->
<!DOCTYPE html><html><body><h1>HTML note</h1></body></html>`;

interface MockNote {
  raw: string;
  kind: "md" | "html";
}

export function installDevMock() {
  if ("__TAURI_INTERNALS__" in window) return;

  const repoKey = "mock-repo--12345678";
  const notes = new Map<string, MockNote>([
    ["2026-07-02-architecture.md", { raw: MD_FIXTURE, kind: "md" }],
    ["2026-07-02-viz.html", { raw: HTML_FIXTURE, kind: "html" }],
  ]);
  const pins = new Set<string>();

  const parseFrontmatter = (raw: string) => {
    const m = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
    if (!m) return { meta: {}, body: raw };
    const yaml = m[1];
    const title =
      yaml
        .match(/^title:\s*(.+)$/m)?.[1]
        ?.trim()
        .replace(/^"(.*)"$/, "$1") ?? null;
    const created = yaml.match(/^created:\s*(.+)$/m)?.[1]?.trim() ?? null;
    const tags =
      yaml
        .match(/^tags:\s*\[(.*)\]$/m)?.[1]
        ?.split(",")
        .map((t) => t.trim())
        .filter(Boolean) ?? [];
    const agent = yaml.match(/^agent:\s*(.+)$/m)?.[1]?.trim() ?? null;
    return { meta: { title, created, tags, agent }, body: raw.slice(m[0].length) };
  };

  const htmlMeta = (raw: string) => {
    const m = raw.match(/repo-notes-meta\s*(\{[\s\S]*?\})\s*-->/);
    try {
      return m ? JSON.parse(m[1]) : {};
    } catch {
      return {};
    }
  };

  const summarize = (fileName: string, note: MockNote) => {
    const meta =
      note.kind === "md" ? parseFrontmatter(note.raw).meta : htmlMeta(note.raw);
    return {
      fileName,
      kind: note.kind,
      title: meta.title ?? fileName,
      created: meta.created ?? null,
      updatedMs: Date.now(),
      tags: meta.tags ?? [],
      pinned: pins.has(fileName),
    };
  };

  const handle = (cmd: string, args: Record<string, unknown> = {}) => {
    switch (cmd) {
      case "get_data_root":
        return "/mock/.repo-notes";
      case "list_repos":
        return [
          {
            repoKey,
            repoName: "mock-repo",
            canonicalUrl: "github.com/mock/repo",
            noteCount: notes.size,
            lastNoteAtMs: Date.now(),
          },
        ];
      case "list_notes": {
        const q = ((args.query as string | null) ?? "").trim().toLowerCase();
        return [...notes.entries()]
          .filter(
            ([name, note]) =>
              !q ||
              name.toLowerCase().includes(q) ||
              note.raw.toLowerCase().includes(q) ||
              summarize(name, note).title.toLowerCase().includes(q),
          )
          .map(([name, note]) => summarize(name, note))
          .sort(
            (a, b) =>
              Number(b.pinned) - Number(a.pinned) ||
              b.fileName.localeCompare(a.fileName),
          );
      }
      case "set_pinned": {
        const fileName = args.fileName as string;
        if (args.pinned) pins.add(fileName);
        else pins.delete(fileName);
        return null;
      }
      case "read_note": {
        const fileName = args.fileName as string;
        const note = notes.get(fileName);
        if (!note) throw new Error(`ノートが存在しません: ${fileName}`);
        const { meta, body } =
          note.kind === "md"
            ? { meta: parseFrontmatter(note.raw).meta, body: parseFrontmatter(note.raw).body }
            : { meta: htmlMeta(note.raw), body: note.raw };
        const summary = summarize(fileName, note);
        return {
          fileName,
          kind: note.kind,
          meta: { title: summary.title, created: summary.created, tags: summary.tags, agent: (meta as { agent?: string }).agent ?? null },
          body,
          raw: note.raw,
          absPath: `/mock/.repo-notes/repos/${repoKey}/notes/${fileName}`,
          updatedMs: Date.now(),
        };
      }
      case "write_note": {
        const fileName = args.fileName as string;
        if (!notes.has(fileName)) throw new Error(`ノートが存在しません: ${fileName}`);
        notes.set(fileName, { raw: args.content as string, kind: "md" });
        return null;
      }
      case "create_note": {
        const title = (args.title as string).trim();
        const date = new Date().toISOString().slice(0, 10);
        let fileName = `${date}-${title.toLowerCase().replace(/[^a-z0-9]+/g, "-") || "note"}.md`;
        let n = 2;
        while (notes.has(fileName)) fileName = fileName.replace(/(-\d+)?\.md$/, `-${n++}.md`);
        notes.set(fileName, {
          raw: `---\ntitle: "${title}"\ncreated: ${new Date().toISOString()}\ntags: []\n---\n\n`,
          kind: "md",
        });
        return fileName;
      }
      case "delete_note":
        notes.delete(args.fileName as string);
        return null;
      default:
        if (cmd.startsWith("plugin:event|")) return 0;
        if (cmd.startsWith("plugin:opener|")) {
          console.log("[devMock] opener:", args);
          return null;
        }
        throw new Error(`unknown command: ${cmd}`);
    }
  };

  let callbackId = 0;
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    invoke: (cmd: string, args?: Record<string, unknown>) => {
      try {
        return Promise.resolve(handle(cmd, args));
      } catch (e) {
        return Promise.reject(e instanceof Error ? e.message : String(e));
      }
    },
    transformCallback: (cb: (payload: unknown) => void) => {
      const id = ++callbackId;
      (window as unknown as Record<string, unknown>)[`_${id}`] = cb;
      return id;
    },
    convertFileSrc: (path: string) => path,
  };
  (window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: () => {},
  };
  console.log("[devMock] Tauri APIモックを有効化しました（ブラウザ単体モード）");
}
