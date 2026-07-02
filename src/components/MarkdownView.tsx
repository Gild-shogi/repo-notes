import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { convertFileSrc } from "@tauri-apps/api/core";
import { rewriteAssetUrl } from "../lib/assetUrl";
import "highlight.js/styles/github.css";

interface Props {
  body: string;
  notesDirAbs: string;
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
      >
        {body}
      </ReactMarkdown>
    </div>
  );
}
