import type { ComponentProps } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import rehypeSlug from "rehype-slug";
import { assetSource, useConnection } from "../lib/connection";
import { openUrl } from "@tauri-apps/plugin-opener";
import { openLocalPath } from "../api";
import { rewriteAssetUrl } from "../lib/assetUrl";
import { classifyLink } from "../lib/links";
import { Mermaid } from "./Mermaid";
import "highlight.js/styles/github.css";

interface Props {
  body: string;
  notesDirAbs: string;
  onNavigateNote?: (fileName: string) => void;
}

function extractText(node: React.ReactNode): string {
  if (typeof node === "string") return node;
  if (Array.isArray(node)) return node.map(extractText).join("");
  return "";
}

function Code({ className, children, ...rest }: ComponentProps<"code">) {
  if (className?.split(" ").includes("language-mermaid")) {
    return <Mermaid code={extractText(children).trim()} />;
  }
  return (
    <code className={className} {...rest}>
      {children}
    </code>
  );
}

function Pre({ children, ...rest }: ComponentProps<"pre">) {
  // mermaidブロックは <pre> の装飾を外して図として表示する
  const child = Array.isArray(children) ? children[0] : children;
  if (
    child &&
    typeof child === "object" &&
    "props" in child &&
    typeof (child.props as { className?: string }).className === "string" &&
    (child.props as { className: string }).className
      .split(" ")
      .includes("language-mermaid")
  ) {
    return <>{children}</>;
  }
  return <pre {...rest}>{children}</pre>;
}

export function MarkdownView({ body, notesDirAbs, onNavigateNote }: Props) {
  const { view } = useConnection();
  const urlTransform = (url: string, key: string) => {
    if (key === "src") {
      return rewriteAssetUrl(url, notesDirAbs, (path) => assetSource(path, view));
    }
    // hrefはクリック時にインターセプトして自前で開くため元の値を保持する。
    // ただしblocked対象（javascript:等）はhref自体を無効化する
    return classifyLink(url, notesDirAbs).type === "blocked" ? "" : url;
  };

  const handleLinkClick = (href: string) => {
    const action = classifyLink(href, notesDirAbs);
    switch (action.type) {
      case "url":
        void openUrl(action.target);
        break;
      case "path":
        // Rust側でhome配下・実行系拡張子でない・実在することを検証して開く
        void openLocalPath(action.target);
        break;
      case "note":
        onNavigateNote?.(action.target);
        break;
      case "blocked":
        break;
    }
  };

  const Anchor = ({ href, children, ...rest }: ComponentProps<"a">) => (
    <a
      {...rest}
      href={href}
      title={href}
      onClick={(e) => {
        e.preventDefault();
        if (href) handleLinkClick(href);
      }}
    >
      {children}
    </a>
  );

  return (
    <div className="markdown-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight, rehypeSlug]}
        urlTransform={urlTransform}
        components={{ code: Code, pre: Pre, a: Anchor }}
      >
        {body}
      </ReactMarkdown>
    </div>
  );
}
