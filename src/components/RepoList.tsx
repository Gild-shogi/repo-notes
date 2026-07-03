import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listRepos, setRepoPinned } from "../api";
import { formatDateTime } from "../lib/date";
import { PinIcon } from "./icons";

interface Props {
  selectedRepoKey: string | null;
  onSelect: (repoKey: string) => void;
}

export function RepoList({ selectedRepoKey, onSelect }: Props) {
  const queryClient = useQueryClient();
  const { data: repos, isLoading } = useQuery({
    queryKey: ["repos"],
    queryFn: listRepos,
  });

  const pin = useMutation({
    mutationFn: ({ repoKey, pinned }: { repoKey: string; pinned: boolean }) =>
      setRepoPinned(repoKey, pinned),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["repos"] });
    },
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
          <button
            type="button"
            className={`pin-btn ${repo.pinned ? "pinned" : ""}`}
            title={repo.pinned ? "ピン留めを解除" : "ピン留め"}
            disabled={pin.isPending}
            onClick={() =>
              pin.mutate({ repoKey: repo.repoKey, pinned: !repo.pinned })
            }
          >
            <PinIcon filled={repo.pinned} />
          </button>
        </li>
      ))}
    </ul>
  );
}
