#!/usr/bin/env bash
# resolve-note-dir.sh の repo-key 導出テスト
set -u

script_dir="$(cd "$(dirname "$0")" && pwd)"
resolve="$script_dir/../../skill/scripts/resolve-note-dir.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export REPO_NOTES_DIR="$work/data"

failures=0
tests=0

assert_eq() {
  local desc="$1" expected="$2" actual="$3"
  tests=$((tests + 1))
  if [[ "$expected" == "$actual" ]]; then
    echo "ok: $desc"
  else
    echo "FAIL: $desc"
    echo "  expected: $expected"
    echo "  actual:   $actual"
    failures=$((failures + 1))
  fi
}

assert_ne() {
  local desc="$1" a="$2" b="$3"
  tests=$((tests + 1))
  if [[ "$a" != "$b" ]]; then
    echo "ok: $desc"
  else
    echo "FAIL: $desc（一致してはいけない: $a）"
    failures=$((failures + 1))
  fi
}

# origin URL を指定して git リポジトリを作り、resolve 結果の notesDir を返す
make_repo() {
  local dir="$1" url="${2:-}"
  git init -q "$dir"
  if [[ -n "$url" ]]; then
    git -C "$dir" remote add origin "$url"
  fi
}

resolve_dir() {
  "$resolve" "$1"
}

# --- 1. remote URL の正規化: 各形式が同一キーに解決される ---
base="$work/repos"
make_repo "$base/r1" "https://user:token@github.com/Owner/Repo.git"
make_repo "$base/r2" "git@github.com:Owner/Repo.git"
make_repo "$base/r3" "ssh://git@github.com:2222/owner/repo"
make_repo "$base/r4" "git://github.com/owner/repo"
make_repo "$base/r5" "https://github.com/owner/repo/"

expected="$(resolve_dir "$base/r1")"
assert_eq "credentials付きhttpsが解決される" "$REPO_NOTES_DIR/repos/owner-repo--$(printf '%s' 'github.com/owner/repo' | shasum -a 256 | cut -c1-8)/notes" "$expected"
assert_eq "scp形式が同一キーになる" "$expected" "$(resolve_dir "$base/r2")"
assert_eq "ssh+ポート番号が同一キーになる" "$expected" "$(resolve_dir "$base/r3")"
assert_eq "git://（.gitなし）が同一キーになる" "$expected" "$(resolve_dir "$base/r4")"
assert_eq "末尾スラッシュが同一キーになる" "$expected" "$(resolve_dir "$base/r5")"

# --- 2. origin無しフォールバック: 再実行で安定 ---
make_repo "$base/local-only"
first="$(resolve_dir "$base/local-only")"
second="$(resolve_dir "$base/local-only")"
assert_eq "origin無しでも同一キーに解決される" "$first" "$second"
assert_ne "origin無しはリモート系と別キー" "$first" "$expected"

# --- 3. ワークツリーからの実行が main と同一キーになる ---
make_repo "$base/wt-main" "git@github.com:Team/Project.git"
git -C "$base/wt-main" commit -q --allow-empty -m init
git -C "$base/wt-main" worktree add -q "$base/wt-branch" -b feature
main_dir="$(resolve_dir "$base/wt-main")"
wt_dir="$(resolve_dir "$base/wt-branch")"
assert_eq "ワークツリーがmainと同一キーになる" "$main_dir" "$wt_dir"

# --- 4. meta.json が生成され canonicalUrl を持つ ---
meta="$(dirname "$expected")/meta.json"
tests=$((tests + 1))
if grep -q '"canonicalUrl": "github.com/owner/repo"' "$meta"; then
  echo "ok: meta.jsonにcanonicalUrlが記録される"
else
  echo "FAIL: meta.jsonにcanonicalUrlが記録される"
  cat "$meta"
  failures=$((failures + 1))
fi

# --- 5. hash8 衝突時は hash12 に伸長される ---
make_repo "$base/collision" "https://github.com/other/project"
key_dir="$(dirname "$(resolve_dir "$base/collision")")"
# 別リポジトリのmeta.jsonを同じキー位置に偽装配置して衝突させる
sed -i '' 's|"canonicalUrl": "github.com/other/project"|"canonicalUrl": "github.com/different/repo"|' "$key_dir/meta.json"
long_dir="$(resolve_dir "$base/collision")"
assert_ne "衝突時は別キーに解決される" "$key_dir" "$(dirname "$long_dir")"
hash_part="$(basename "$(dirname "$long_dir")" | sed 's/.*--//')"
assert_eq "衝突時のhashは12桁" "12" "${#hash_part}"

# --- 6. Gitリポジトリ外は exit 2 ---
mkdir -p "$work/not-a-repo"
"$resolve" "$work/not-a-repo" >/dev/null 2>&1
assert_eq "Git管理外はexit 2" "2" "$?"

# --- 7. --json 出力 ---
json="$("$resolve" --json "$base/r1")"
tests=$((tests + 1))
if [[ "$json" == *'"repoKey":"owner-repo--'* && "$json" == *'"created":false'* ]]; then
  echo "ok: --jsonがrepoKeyとcreatedを含む"
else
  echo "FAIL: --json出力: $json"
  failures=$((failures + 1))
fi

echo
echo "$tests tests, $failures failures"
[[ "$failures" -eq 0 ]]
