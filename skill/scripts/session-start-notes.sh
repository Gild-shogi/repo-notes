#!/usr/bin/env bash
# repo-notes: Claude Code の SessionStart フック。
# カレントリポジトリに repo-notes メモが存在する場合のみ、
# 件数と最新メモの一覧を標準出力に出してセッションのコンテキストに注入する。
#
# usage: session-start-notes.sh [repo-path]
#   Gitリポジトリ外・メモ無し・解決失敗時は何も出力せず exit 0（フックを妨げない）。
set -uo pipefail

max_shown=5

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
notes_dir="$(bash "$script_dir/resolve-note-dir.sh" --probe "${1:-$PWD}" 2>/dev/null)" || exit 0
[[ -d "$notes_dir" ]] || exit 0

files=()
while IFS= read -r f; do
  files+=("$f")
done < <(cd "$notes_dir" && ls -t -- *.md *.html 2>/dev/null)
count=${#files[@]}
[[ "$count" -gt 0 ]] || exit 0

# frontmatter / repo-notes-meta コメントからタイトルを抽出する（失敗時は空）
title_of() {
  local path="$1"
  case "$path" in
    *.md) sed -n '1,10s/^title:[[:space:]]*//p' "$path" | head -1 | sed 's/^"\(.*\)"$/\1/' ;;
    *.html) sed -n '1,3s/.*repo-notes-meta[^{]*{.*"title":[[:space:]]*"\([^"]*\)".*/\1/p' "$path" | head -1 ;;
  esac
}

echo "このリポジトリには repo-notes のキャッチアップメモが ${count} 件あります: $notes_dir"
echo "最新のメモ:"
shown=0
for f in "${files[@]}"; do
  [[ "$shown" -lt "$max_shown" ]] || break
  title="$(title_of "$notes_dir/$f")"
  echo "- ${f}${title:+ — $title}"
  shown=$((shown + 1))
done
[[ "$count" -le "$max_shown" ]] || echo "（他 $((count - max_shown)) 件）"
echo "「メモを元に」「メモを読んで」「メモを更新して」等の依頼では、まず上記ファイルを Read すること。参照・編集の規約は repo-notes スキルに従う。"
