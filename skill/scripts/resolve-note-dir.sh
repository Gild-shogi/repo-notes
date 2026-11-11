#!/usr/bin/env bash
# repo-notes: Gitリポジトリからメモ書き込み先ディレクトリを解決する。
#
# usage: resolve-note-dir.sh [--json] [--probe] [repo-path]
#   repo-path 省略時はカレントディレクトリ。
#   標準出力: notesディレクトリの絶対パス1行（--json 時はJSONオブジェクト）
#   exit 2: Gitリポジトリではない
#
# 副作用: repos/<key>/notes/assets の mkdir -p、meta.json の初回作成、
#         および origin 追加前のローカルキーから origin キーへのメモ移行。
# --probe 指定時は副作用なしでパス解決のみ行う（読み取り用途）。
#         未移行のレガシーキーがある場合はそのディレクトリを返す。
# repo-key 導出仕様は docs/note-format.md を参照。このスクリプトが唯一の実装。
set -euo pipefail

json_output=false
probe=false
repo_path=""
for arg in "$@"; do
  case "$arg" in
    --json) json_output=true ;;
    --probe) probe=true ;;
    -h|--help)
      sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) repo_path="$arg" ;;
  esac
done
repo_path="${repo_path:-$PWD}"

root="${REPO_NOTES_DIR:-$HOME/.repo-notes}"

if ! git -C "$repo_path" rev-parse --git-common-dir >/dev/null 2>&1; then
  echo "error: Gitリポジトリではありません: $repo_path" >&2
  exit 2
fi

lower() { tr '[:upper:]' '[:lower:]'; }

# remote URL を canonical form（github.com/owner/repo）に正規化する
normalize_url() {
  local url="$1"
  url="$(printf '%s' "$url" | tr -d '[:space:]')"

  # file:// または絶対パスのリモートはローカル扱い
  if [[ "$url" == file://* ]]; then
    printf 'local:%s' "${url#file://}"
    return
  fi
  if [[ "$url" == /* ]]; then
    printf 'local:%s' "$url"
    return
  fi

  if [[ "$url" == *"://"* ]]; then
    url="${url#*://}"
  elif [[ "$url" == *:* ]]; then
    # scp形式 git@host:path → host/path
    local host="${url%%:*}"
    local path="${url#*:}"
    url="${host}/${path}"
  fi

  # authority 部から userinfo とポート番号を除去
  local authority="${url%%/*}"
  local rest=""
  if [[ "$url" == */* ]]; then
    rest="${url#*/}"
  fi
  authority="${authority##*@}"
  authority="${authority%%:*}"
  url="${authority}/${rest}"

  url="${url%/}"
  url="${url%.git}"
  printf '%s' "$url" | lower
}

# canonical form の決定（local: 形式は origin 追加前のレガシーキー検出にも使うため常に算出する）
common_dir="$(git -C "$repo_path" rev-parse --git-common-dir)"
if [[ "$common_dir" != /* ]]; then
  common_dir="$repo_path/$common_dir"
fi
common_dir="$(cd "$common_dir" && pwd -P)"
local_canonical="local:$common_dir"

remote_url="$(git -C "$repo_path" remote get-url origin 2>/dev/null || true)"
if [[ -n "$remote_url" ]]; then
  canonical="$(normalize_url "$remote_url")"
else
  canonical="$local_canonical"
fi

# slug（可読性用・一意性はhashで担保）
kebab() { lower | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//' | cut -c1-50; }
local_repo_dir="${local_canonical#local:}"
local_repo_dir="${local_repo_dir%/.git}"
local_slug="$(basename "$local_repo_dir" .git | kebab)"
[[ -n "$local_slug" ]] || local_slug="repo"

if [[ "$canonical" == "$local_canonical" ]]; then
  repo_name="$(basename "$local_repo_dir" .git)"
  slug="$local_slug"
else
  repo_name="$(basename "$canonical")"
  slug="$(printf '%s' "$canonical" | awk -F/ '{ n=NF; if (n>=2) printf "%s/%s", $(n-1), $n; else print $n }' | kebab)"
  [[ -n "$slug" ]] || slug="repo"
fi

hash_of() { printf '%s' "$1" | shasum -a 256 | cut -c1-"$2"; }

json_escape() { sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }

meta_canonical_url() {
  # このスクリプトが書いた meta.json から canonicalUrl を取り出す（jq非依存）
  sed -n 's/.*"canonicalUrl": *"\([^"]*\)".*/\1/p' "$1" | head -1
}

resolve_key() {
  local hash_len="$1"
  printf '%s--%s' "$slug" "$(hash_of "$canonical" "$hash_len")"
}

key="$(resolve_key 8)"
meta_file="$root/repos/$key/meta.json"
if [[ -f "$meta_file" ]]; then
  existing="$(meta_canonical_url "$meta_file")"
  if [[ -n "$existing" && "$existing" != "$canonical" ]]; then
    # slug+hash8 衝突 → hash12 で再試行
    key="$(resolve_key 12)"
    meta_file="$root/repos/$key/meta.json"
    if [[ -f "$meta_file" ]]; then
      existing="$(meta_canonical_url "$meta_file")"
      if [[ -n "$existing" && "$existing" != "$canonical" ]]; then
        echo "error: repo-keyが衝突しています: $key" >&2
        exit 1
      fi
    fi
  fi
fi

# origin 追加前にローカルフォールバックキーで作られたメモの検出
# （レガシーは hash8 キーのみ対象。local: の hash12 伸長は実運用では起きない前提）
legacy_dir=""
if [[ "$canonical" != "$local_canonical" ]]; then
  legacy_key="${local_slug}--$(hash_of "$local_canonical" 8)"
  legacy_meta="$root/repos/$legacy_key/meta.json"
  if [[ "$legacy_key" != "$key" && -f "$legacy_meta" && "$(meta_canonical_url "$legacy_meta")" == "$local_canonical" ]]; then
    legacy_dir="$root/repos/$legacy_key"
  fi
fi

notes_dir="$root/repos/$key/notes"
assets_dir="$notes_dir/assets"

if [[ -n "$legacy_dir" ]]; then
  if [[ -f "$meta_file" || -d "$notes_dir" ]]; then
    # 新旧両方に実体がある場合は自動では触らない（手動マージが必要）
    echo "warning: 旧キーのメモが残っています: ${legacy_dir}（移行先 $root/repos/$key は既に存在）" >&2
  elif $probe; then
    # 副作用なしで読めるよう、移行前はレガシー側を返す
    notes_dir="$legacy_dir/notes"
    assets_dir="$notes_dir/assets"
  else
    # メモ・pins.json ごと新キーへ移行し、meta.json は新キーの内容で作り直す
    mv "$legacy_dir" "$root/repos/$key"
    rm -f "$meta_file"
    repo_pins="$root/pinned-repos.json"
    if [[ -f "$repo_pins" ]]; then
      pin_update="$(mktemp "$root/.pinned-repos.XXXXXX")"
      sed "s|\"$legacy_key\"|\"$key\"|g" "$repo_pins" > "$pin_update"
      mv "$pin_update" "$repo_pins"
    fi
  fi
fi

$probe || mkdir -p "$assets_dir"

created=false
if ! $probe && [[ ! -f "$meta_file" ]]; then
  created=true
  now="$(date '+%Y-%m-%dT%H:%M:%S%z' | sed 's/\([0-9][0-9]\)$/:\1/')"
  esc_name="$(printf '%s' "$repo_name" | json_escape)"
  esc_canonical="$(printf '%s' "$canonical" | json_escape)"
  esc_remote="$(printf '%s' "$remote_url" | json_escape)"
  {
    printf '{\n'
    printf '  "version": 1,\n'
    printf '  "repoKey": "%s",\n' "$key"
    printf '  "repoName": "%s",\n' "$esc_name"
    printf '  "canonicalUrl": "%s",\n' "$esc_canonical"
    if [[ -n "$remote_url" ]]; then
      printf '  "remoteUrl": "%s",\n' "$esc_remote"
    fi
    printf '  "createdAt": "%s"\n' "$now"
    printf '}\n'
  } > "$meta_file"
fi

if $json_output; then
  printf '{"repoKey":"%s","repoName":"%s","notesDir":"%s","assetsDir":"%s","created":%s}\n' \
    "$key" \
    "$(printf '%s' "$repo_name" | json_escape)" \
    "$(printf '%s' "$notes_dir" | json_escape)" \
    "$(printf '%s' "$assets_dir" | json_escape)" \
    "$created"
else
  printf '%s\n' "$notes_dir"
fi
