import { useState } from "react";
import type { TocItem } from "../lib/toc";

interface Props {
  items: TocItem[];
}

const STORAGE_KEY = "repo-notes:toc";

/** ノート上部に表示する折りたたみ可能な目次 */
export function Toc({ items }: Props) {
  const [open, setOpen] = useState(() => localStorage.getItem(STORAGE_KEY) !== "closed");

  if (items.length < 2) return null;

  const toggle = () =>
    setOpen((v) => {
      localStorage.setItem(STORAGE_KEY, v ? "closed" : "open");
      return !v;
    });

  const minLevel = Math.min(...items.map((i) => i.level));

  const jumpTo = (id: string) => {
    document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  return (
    <nav className="toc" aria-label="目次">
      <button type="button" className="toc-toggle" aria-expanded={open} onClick={toggle}>
        <span className={`toc-chevron ${open ? "open" : ""}`}>▸</span>
        目次
        <span className="toc-count">{items.length}</span>
      </button>
      {open && (
        <ul className="toc-list">
          {items.map((item, index) => (
            <li
              key={`${item.id}-${index}`}
              style={{ paddingLeft: `${(item.level - minLevel) * 16}px` }}
            >
              <button type="button" onClick={() => jumpTo(item.id)}>
                {item.text}
              </button>
            </li>
          ))}
        </ul>
      )}
    </nav>
  );
}
