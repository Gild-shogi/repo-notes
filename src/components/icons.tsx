/** サイドペイン用のインラインSVGアイコン（currentColorで描画） */

export function RepoIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle cx="7" cy="5" r="2.4" stroke="currentColor" strokeWidth="1.8" />
      <circle cx="7" cy="19" r="2.4" stroke="currentColor" strokeWidth="1.8" />
      <circle cx="17" cy="13" r="2.4" stroke="currentColor" strokeWidth="1.8" />
      <path d="M7 7.5v9" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
      <path
        d="M7 10c0 3 7.6 0.5 7.6 3.5"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        fill="none"
      />
    </svg>
  );
}

export function PinIcon({ filled = false }: { filled?: boolean }) {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M9 4h6l-1 6 3.5 3.5v1.5h-5V21l-.5 1-.5-1v-6H6.5v-1.5L10 10z"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinejoin="round"
        fill={filled ? "currentColor" : "none"}
      />
    </svg>
  );
}

export function NoteIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M6 3.5h8l4 4V19a1.5 1.5 0 0 1-1.5 1.5h-10A1.5 1.5 0 0 1 5 19V5A1.5 1.5 0 0 1 6.5 3.5z"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinejoin="round"
      />
      <path d="M14 3.5V8h4" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
      <path d="M8.5 12h7M8.5 15.5h5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}
