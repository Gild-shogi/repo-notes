#!/usr/bin/env bash
# session-start-notes.sh（SessionStartフック）の出力テスト
set -u

script_dir="$(cd "$(dirname "$0")" && pwd)"
hook="$script_dir/../../skill/scripts/session-start-notes.sh"
resolve="$script_dir/../../skill/scripts/resolve-note-dir.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export REPO_NOTES_DIR="$work/data"

failures=0
tests=0

check() {
  local desc="$1" ok="$2"
  tests=$((tests + 1))
  if [[ "$ok" == true ]]; then
    echo "ok: $desc"
  else
    echo "FAIL: $desc"
    failures=$((failures + 1))
  fi
}

# --- 1. Gitリポジトリ外では何も出力せず exit 0 ---
mkdir -p "$work/not-a-repo"
out="$(bash "$hook" "$work/not-a-repo")"
rc=$?
check "Git管理外は無出力でexit 0" "$([[ "$rc" -eq 0 && -z "$out" ]] && echo true || echo false)"

# --- 2. メモが無いリポジトリでは何も出力しない ---
git init -q "$work/empty-repo"
git -C "$work/empty-repo" remote add origin "https://github.com/hook/empty"
out="$(bash "$hook" "$work/empty-repo")"
rc=$?
check "メモ無しは無出力でexit 0" "$([[ "$rc" -eq 0 && -z "$out" ]] && echo true || echo false)"

probe_dir="$("$resolve" --probe "$work/empty-repo")"
check "フック実行がメモディレクトリを作らない" "$([[ ! -e "$probe_dir" ]] && echo true || echo false)"

# --- 3. メモがあるリポジトリでは件数・ファイル名・タイトルを出力する ---
git init -q "$work/repo"
git -C "$work/repo" remote add origin "https://github.com/hook/target"
notes_dir="$("$resolve" "$work/repo")"
cat > "$notes_dir/2026-07-01-auth-flow.md" <<'EOF'
---
title: 認証フローの概要
created: 2026-07-01T10:00:00+09:00
tags: [auth]
---

本文
EOF
printf '<!-- repo-notes-meta {"title":"依存グラフ","created":"2026-07-02T10:00:00+09:00"} -->\n<!DOCTYPE html>\n' \
  > "$notes_dir/2026-07-02-deps.html"

out="$(bash "$hook" "$work/repo")"
check "件数が出力される" "$([[ "$out" == *"2 件"* ]] && echo true || echo false)"
check "notesディレクトリのパスが出力される" "$([[ "$out" == *"$notes_dir"* ]] && echo true || echo false)"
check "mdのタイトルが抽出される" "$([[ "$out" == *"2026-07-01-auth-flow.md — 認証フローの概要"* ]] && echo true || echo false)"
check "htmlのタイトルが抽出される" "$([[ "$out" == *"2026-07-02-deps.html — 依存グラフ"* ]] && echo true || echo false)"

echo
echo "$tests tests, $failures failures"
[[ "$failures" -eq 0 ]]
