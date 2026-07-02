import { useQuery } from "@tanstack/react-query";
import { listRepos } from "../api";
import { formatDateTime } from "../lib/date";

interface Props {
  selectedRepoKey: string | null;
  onSelect: (repoKey: string) => void;
}

export function RepoList({ selectedRepoKey, onSelect }: Props) {
  const { data: repos, isLoading } = useQuery({
    queryKey: ["repos"],
    queryFn: listRepos,
  });

  if (isLoading) return <div className="pane-status">読み込み中…</div>;
  if (!repos || repos.length === 0) {
    return (
      <div className="pane-status">
        <p>リポジトリがまだありません。</p>
        <p className="hint">
          エージェントの repo-notes
          スキルでメモを作成すると、ここに表示されます。
        </p>
      </div>
    );
  }

  return (
    <ul className="repo-list">
      {repos.map((repo) => (
        <li key={repo.repoKey}>
          <button
            type="button"
            className={repo.repoKey === selectedRepoKey ? "selected" : ""}
            onClick={() => onSelect(repo.repoKey)}
          >
            <span className="repo-name">{repo.repoName}</span>
            {repo.canonicalUrl && (
              <span className="repo-url">{repo.canonicalUrl}</span>
            )}
            <span className="repo-meta">
              {repo.noteCount}件
              {repo.lastNoteAtMs != null &&
                ` · ${formatDateTime(repo.lastNoteAtMs)}`}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
