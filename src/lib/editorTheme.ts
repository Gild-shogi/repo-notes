import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";
import { RangeSetBuilder } from "@codemirror/state";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";

const SANS =
  '-apple-system, BlinkMacSystemFont, "Hiragino Sans", "Noto Sans JP", sans-serif';
const MONO = '"SF Mono", ui-monospace, Menlo, monospace';

/** アプリのトーンに合わせたエディタ本体のテーマ */
const theme = EditorView.theme({
  "&": {
    fontSize: "14px",
    backgroundColor: "transparent",
  },
  ".cm-content": {
    fontFamily: SANS,
    lineHeight: "1.9",
    padding: "20px 4px 40vh",
    caretColor: "#2563eb",
  },
  ".cm-line": {
    padding: "0 20px",
  },
  ".cm-cursor": {
    borderLeftColor: "#2563eb",
    borderLeftWidth: "2px",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "#dbeafe !important",
  },
  ".cm-activeLine": {
    backgroundColor: "transparent",
  },
  ".cm-scroller": {
    fontFamily: SANS,
  },
});

/** Markdown構文の装飾（見出しの大きさ・強調・コード等をWYSIWYG風に） */
const markdownHighlight = HighlightStyle.define([
  { tag: tags.heading1, fontSize: "1.5em", fontWeight: "700", color: "#1f2328" },
  { tag: tags.heading2, fontSize: "1.3em", fontWeight: "700", color: "#1f2328" },
  { tag: tags.heading3, fontSize: "1.15em", fontWeight: "600", color: "#1f2328" },
  { tag: tags.heading4, fontWeight: "600", color: "#1f2328" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through", color: "#656d76" },
  { tag: tags.link, color: "#2563eb" },
  { tag: tags.url, color: "#2563eb", textDecoration: "underline" },
  { tag: tags.quote, color: "#57606a", fontStyle: "italic" },
  { tag: tags.monospace, fontFamily: MONO, fontSize: "13px", color: "#0550ae" },
  // リストマーカー・見出しの#・コードフェンス等の記号は控えめに
  { tag: tags.processingInstruction, color: "#9ca3af" },
  { tag: tags.punctuation, color: "#9ca3af" },
  { tag: tags.meta, color: "#9ca3af" },
  { tag: tags.contentSeparator, color: "#d1d5db", fontWeight: "700" },
  // コードブロック内の埋め込み言語ハイライト（GitHub風）
  { tag: tags.keyword, color: "#cf222e" },
  { tag: tags.string, color: "#0a3069" },
  { tag: tags.comment, color: "#6e7781", fontStyle: "italic" },
  { tag: tags.number, color: "#0550ae" },
  { tag: tags.bool, color: "#0550ae" },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], color: "#8250df" },
  { tag: tags.typeName, color: "#953800" },
  { tag: tags.propertyName, color: "#0550ae" },
  { tag: tags.operator, color: "#cf222e" },
]);

/**
 * 先頭のfrontmatterブロック（--- 〜 ---）をメタ情報として控えめに表示する。
 * Markdownパーサはfrontmatterを知らずsetext見出し等に誤解釈するため、
 * 行デコレーションでスタイルを上書きする（App.cssの .cm-frontmatter 参照）。
 */
const frontmatterLine = Decoration.line({ class: "cm-frontmatter" });

function frontmatterDecorations(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const doc = view.state.doc;
  if (doc.lines >= 2 && doc.line(1).text.trim() === "---") {
    let end = 0;
    for (let i = 2; i <= Math.min(doc.lines, 50); i++) {
      if (doc.line(i).text.trim() === "---") {
        end = i;
        break;
      }
    }
    for (let i = 1; i <= end; i++) {
      const line = doc.line(i);
      builder.add(line.from, line.from, frontmatterLine);
    }
  }
  return builder.finish();
}

const frontmatterPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = frontmatterDecorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged) {
        this.decorations = frontmatterDecorations(update.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

export const editorExtensions = [
  theme,
  syntaxHighlighting(markdownHighlight),
  frontmatterPlugin,
];
