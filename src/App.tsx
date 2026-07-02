import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RepoList } from "./components/RepoList";
import { NoteList } from "./components/NoteList";
import { NoteView } from "./components/NoteView";
import "./App.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 5_000, retry: 1 },
  },
});

function Main() {
  const [selectedRepoKey, setSelectedRepoKey] = useState<string | null>(null);
  const [selectedFileName, setSelectedFileName] = useState<string | null>(null);

  const selectRepo = (repoKey: string) => {
    setSelectedRepoKey(repoKey);
    setSelectedFileName(null);
  };

  return (
    <div className="layout">
      <aside className="pane pane-repos">
        <h2 className="pane-title">リポジトリ</h2>
        <RepoList selectedRepoKey={selectedRepoKey} onSelect={selectRepo} />
      </aside>
      <aside className="pane pane-notes">
        <h2 className="pane-title">ノート</h2>
        {selectedRepoKey ? (
          <NoteList
            repoKey={selectedRepoKey}
            selectedFileName={selectedFileName}
            onSelect={setSelectedFileName}
          />
        ) : (
          <div className="pane-status">リポジトリを選択してください。</div>
        )}
      </aside>
      <main className="pane pane-content">
        {selectedRepoKey && selectedFileName ? (
          <NoteView repoKey={selectedRepoKey} fileName={selectedFileName} />
        ) : (
          <div className="pane-status">ノートを選択してください。</div>
        )}
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
