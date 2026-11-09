import { useEffect, useId, useState } from "react";
import DOMPurify from "dompurify";

// mermaid本体はバンドルが大きいため dynamic import し、モジュール単位で1度だけ初期化する
let mermaidPromise: Promise<typeof import("mermaid").default> | null = null;

function loadMermaid() {
  mermaidPromise ??= import("mermaid").then(({ default: mermaid }) => {
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: "strict",
      theme: "neutral",
      // foreignObjectによるHTMLラベルはDOMPurifyやWebKitと相性が悪いためSVGテキストで描画する
      // （v11はノード形状がトップレベルのhtmlLabelsを参照するため両方に指定する）
      htmlLabels: false,
      flowchart: { htmlLabels: false },
    } as Parameters<typeof mermaid.initialize>[0]);
    return mermaid;
  });
  return mermaidPromise;
}

interface Props {
  code: string;
}

export function Mermaid({ code }: Props) {
  const id = useId().replace(/[^a-zA-Z0-9]/g, "");
  const [result, setResult] = useState<
    { code: string; svg: string } | { code: string; error: string } | null
  >(null);

  useEffect(() => {
    let cancelled = false;
    loadMermaid()
      .then((mermaid) => mermaid.render(`mermaid-${id}`, code))
      .then(({ svg }) => {
        if (!cancelled) {
          setResult({
            code,
            svg: DOMPurify.sanitize(svg, {
              USE_PROFILES: { svg: true, svgFilters: true, html: true },
              ADD_TAGS: ["foreignObject"],
            }),
          });
        }
      })
      .catch((e) => {
        if (!cancelled) setResult({ code, error: String(e) });
        // renderが失敗すると一時要素が残ることがあるため掃除する
        document.getElementById(`dmermaid-${id}`)?.remove();
      });
    return () => {
      cancelled = true;
    };
  }, [code, id]);

  const current = result?.code === code ? result : null;
  if (current && "error" in current) {
    return (
      <div className="mermaid-block mermaid-error">
        <p>Mermaidのレンダリングに失敗しました</p>
        <pre>
          <code>{code}</code>
        </pre>
      </div>
    );
  }
  if (current === null) {
    return <div className="mermaid-block loading">図を生成中…</div>;
  }
  return (
    <div className="mermaid-block" dangerouslySetInnerHTML={{ __html: current.svg }} />
  );
}
