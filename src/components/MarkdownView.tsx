import type { ComponentProps } from "react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { convertFileSrc } from "@tauri-apps/api/core";
import { rewriteAssetUrl } from "../lib/assetUrl";
import { Mermaid } from "./Mermaid";
import "highlight.js/styles/github.css";

interface Props {
  body: string;
  notesDirAbs: string;
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

export function MarkdownView({ body, notesDirAbs }: Props) {
  const urlTransform = (url: string, key: string) => {
    if (key === "src") {
      return rewriteAssetUrl(url, notesDirAbs, convertFileSrc);
    }
    return defaultUrlTransform(url);
  };

  return (
    <div className="markdown-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight]}
        urlTransform={urlTransform}
        components={{ code: Code, pre: Pre }}
      >
        {body}
      </ReactMarkdown>
    </div>
  );
}
