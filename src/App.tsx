import { useRef, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BackToTop } from "./components/BackToTop";
import { RepoList } from "./components/RepoList";
import { NoteList } from "./components/NoteList";
import { NoteView } from "./components/NoteView";
import { NewNoteButton } from "./components/NewNoteButton";
import { SidePane } from "./components/SidePane";
import { NoteIcon, RepoIcon } from "./components/icons";
import { useLiveUpdates } from "./lib/useLiveUpdates";
import "./App.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 5_000, retry: 1 },
  },
});

const PANE_WIDTHS = { repos: "240px", notes: "300px", collapsed: "36px" };

function usePaneOpen(key: string): [boolean, () => void] {
  const storageKey = `repo-notes:pane:${key}`;
  const [open, setOpen] = useState(() => localStorage.getItem(storageKey) !== "closed");
  const toggle = () =>
    setOpen((v) => {
      localStorage.setItem(storageKey, v ? "closed" : "open");
      return !v;
    });
  return [open, toggle];
}

function Main() {
  useLiveUpdates();
  const [selectedRepoKey, setSelectedRepoKey] = useState<string | null>(null);
  const [selectedFileName, setSelectedFileName] = useState<string | null>(null);
  const [justCreatedFile, setJustCreatedFile] = useState<string | null>(null);
  const [deleteWarnings, setDeleteWarnings] = useState<string[]>([]);
  const [repoPaneOpen, toggleRepoPane] = usePaneOpen("repos");
  const [notePaneOpen, toggleNotePane] = usePaneOpen("notes");
  const contentPaneRef = useRef<HTMLElement | null>(null);

  const selectRepo = (repoKey: string) => {
    setSelectedRepoKey(repoKey);
    setSelectedFileName(null);
    setJustCreatedFile(null);
    setDeleteWarnings([]);
  };

  const selectNote = (fileName: string) => {
    setSelectedFileName(fileName);
    setJustCreatedFile(null);
    setDeleteWarnings([]);
  };

  const columns = [
    repoPaneOpen ? PANE_WIDTHS.repos : PANE_WIDTHS.collapsed,
    notePaneOpen ? PANE_WIDTHS.notes : PANE_WIDTHS.collapsed,
    "1fr",
  ].join(" ");

  return (
    <div className="layout" style={{ gridTemplateColumns: columns }}>
      <SidePane
        title="リポジトリ"
        icon={<RepoIcon />}
        className="pane-repos"
        open={repoPaneOpen}
        onToggle={toggleRepoPane}
      >
        <RepoList selectedRepoKey={selectedRepoKey} onSelect={selectRepo} />
      </SidePane>
      <SidePane
        title="ノート"
        icon={<NoteIcon />}
        className="pane-notes"
        open={notePaneOpen}
        onToggle={toggleNotePane}
        headerExtra={
          selectedRepoKey && (
            <NewNoteButton
              repoKey={selectedRepoKey}
              onCreated={(fileName) => {
                setSelectedFileName(fileName);
                setJustCreatedFile(fileName);
                setDeleteWarnings([]);
              }}
            />
          )
        }
      >
        {selectedRepoKey ? (
          <NoteList
            repoKey={selectedRepoKey}
            selectedFileName={selectedFileName}
            onSelect={selectNote}
          />
        ) : (
          <div className="pane-status">リポジトリを選択してください。</div>
        )}
      </SidePane>
      <main className="pane pane-content" ref={contentPaneRef}>
        {deleteWarnings.length > 0 && (
          <div className="pane-status error" role="status">
            <p>ノートは削除しましたが、後処理が完了していません。</p>
            <ul>{deleteWarnings.map((warning) => <li key={warning}>{warning}</li>)}</ul>
            <button type="button" className="btn" onClick={() => setDeleteWarnings([])}>
              閉じる
            </button>
          </div>
        )}
        {selectedRepoKey && selectedFileName ? (
          <NoteView
            key={`${selectedRepoKey}/${selectedFileName}`}
            repoKey={selectedRepoKey}
            fileName={selectedFileName}
            startInEdit={selectedFileName === justCreatedFile}
            onDeleted={(warnings) => {
              setSelectedFileName(null);
              setDeleteWarnings(warnings);
            }}
            onNavigateNote={selectNote}
          />
        ) : (
          <div className="pane-status">ノートを選択してください。</div>
        )}
        <BackToTop
          key={`btt-${selectedRepoKey}/${selectedFileName}`}
          scrollRef={contentPaneRef}
        />
      </main>
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Main />
    </QueryClientProvider>
  );
}
