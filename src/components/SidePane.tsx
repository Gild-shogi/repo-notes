import type { ReactNode } from "react";

interface Props {
  title: string;
  icon: ReactNode;
  className: string;
  open: boolean;
  onToggle: () => void;
  headerExtra?: ReactNode;
  children: ReactNode;
}

/** 折りたたみ可能なサイドバーペイン。閉じるとアイコンだけの細いストリップになる。 */
export function SidePane({
  title,
  icon,
  className,
  open,
  onToggle,
  headerExtra,
  children,
}: Props) {
  if (!open) {
    return (
      <aside className={`pane ${className} collapsed`} onClick={onToggle}>
        <button
          type="button"
          className="icon-btn"
          title={`${title}を開く`}
          aria-expanded={false}
          onClick={(e) => {
            e.stopPropagation();
            onToggle();
          }}
        >
          {icon}
        </button>
      </aside>
    );
  }
  return (
    <aside className={`pane ${className}`}>
      <div className="pane-header">
        <h2 className="pane-title">{title}</h2>
        <div className="pane-header-actions">
          {headerExtra}
          <button
            type="button"
            className="icon-btn"
            title={`${title}を畳む`}
            aria-expanded={true}
            onClick={onToggle}
          >
            «
          </button>
        </div>
      </div>
      {children}
    </aside>
  );
}
