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
    });
    return mermaid;
  });
  return mermaidPromise;
}

interface Props {
  code: string;
}

export function Mermaid({ code }: Props) {
  const id = useId().replace(/[^a-zA-Z0-9]/g, "");
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSvg(null);
    setError(null);
    loadMermaid()
      .then((mermaid) => mermaid.render(`mermaid-${id}`, code))
      .then(({ svg }) => {
        if (!cancelled) {
          setSvg(
            DOMPurify.sanitize(svg, {
              USE_PROFILES: { svg: true, svgFilters: true, html: true },
              ADD_TAGS: ["foreignObject"],
            }),
          );
        }
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
        // renderが失敗すると一時要素が残ることがあるため掃除する
        document.getElementById(`dmermaid-${id}`)?.remove();
      });
    return () => {
      cancelled = true;
    };
  }, [code, id]);

  if (error) {
    return (
      <div className="mermaid-block mermaid-error">
        <p>Mermaidのレンダリングに失敗しました</p>
        <pre>
          <code>{code}</code>
        </pre>
      </div>
    );
  }
  if (svg === null) {
    return <div className="mermaid-block loading">図を生成中…</div>;
  }
  return (
    <div className="mermaid-block" dangerouslySetInnerHTML={{ __html: svg }} />
  );
}
