import { useEffect, useState, type RefObject } from "react";

interface Props {
  scrollRef: RefObject<HTMLElement | null>;
}

/** スクロールコンテナが一定量スクロールされたら表示される「トップに戻る」ボタン */
export function BackToTop({ scrollRef }: Props) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const onScroll = () => setVisible(el.scrollTop > 300);
    onScroll();
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, [scrollRef]);

  if (!visible) return null;

  return (
    <button
      type="button"
      className="back-to-top"
      title="トップに戻る"
      aria-label="トップに戻る"
      onClick={() => scrollRef.current?.scrollTo({ top: 0, behavior: "smooth" })}
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <path
          d="M12 19V6M6 11.5L12 5.5l6 6"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </button>
  );
}
