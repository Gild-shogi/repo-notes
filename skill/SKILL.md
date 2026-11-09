---
name: repo-notes
description: ユーザーのキャッチアップ用メモをGitリポジトリ単位のローカルディレクトリに保存・参照します。「メモして」「キャッチアップメモを残して」「調査結果をメモに」「あとで読めるようにまとめて」などの保存リクエストに加え、「メモを読んで」「前回の調査メモを元に」「メモを更新して」「メモの内容を実装して」など既存メモの参照・再編集・それを起点にした実装や再調査にも使用します。プロジェクトのリポジトリ内にメモファイルを作成してはいけません。
---

# repo-notes スキル

ユーザーのキャッチアップ用メモを、プロジェクトリポジトリの**外**にある専用ディレクトリ（`~/.repo-notes/`）に保存する。メモはGitリポジトリ単位で集約され（ワークツリーは区別されない）、repo-notes デスクトップアプリで閲覧・編集できる。

## 重要な原則

- **プロジェクトのリポジトリ内にメモを書かない**。メモは必ずこのスキルで解決したディレクトリに書く
- メモの読者は**ユーザー（人間）**。エージェント自身の作業ログではなく、ユーザーがプロジェクトをキャッチアップするために役立つ内容を書く
- 1メモ1トピック。長大な1ファイルより、トピックごとに分ける

## 手順

### 1. 書き込み先ディレクトリを解決する

スキルディレクトリ内のスクリプトを実行する（`~/.claude/skills/repo-notes/scripts/resolve-note-dir.sh`）:

```bash
NOTES_DIR=$(bash ~/.claude/skills/repo-notes/scripts/resolve-note-dir.sh)
REPO_KEY=$(basename "$(dirname "$NOTES_DIR")")
NOTES_CLI="$HOME/.claude/skills/repo-notes/bin/repo-notes"
```

- 引数省略時はカレントディレクトリのリポジトリが対象。別リポジトリを対象にする場合はパスを渡す
- `--json` を付けると `{"repoKey":...,"repoName":...,"notesDir":...,"assetsDir":...,"created":...}` が得られる
- Gitリポジトリ外では exit 2 で失敗する。その場合はユーザーにメモの対象リポジトリを確認する
- `bin/repo-notes` が無ければ `make install-skill` による更新をユーザーに案内する。ノート本体の保存はこのコマンドを使う

### 2. ファイル名を決めてメモを作成する

命名規則: `YYYY-MM-DD-<kebab-slug>.md`（日付は今日、slugは英小文字ケバブケース）

```bash
ls "$NOTES_DIR"   # 既存メモの確認（同名があれば -2, -3 を付ける）
NOTE_WORK=$(mktemp -d)
```

本文は `$NOTE_WORK/draft.md`（HTMLなら `draft.html`）に作成する。ノート本体への直接の Write/Edit やシェルリダイレクトは、アプリとの競合検出に参加しないため使わない。

### 3-a. Markdownメモ（デフォルト）

原則Markdownで書く。frontmatter は `title` と `created` が必須:

```markdown
---
title: 認証フローの概要
created: 2026-07-02T15:30:00+09:00
tags: [auth, architecture]
agent: claude-code
---

## 概要
...
```

- **図はまずMermaidで検討する**。フローチャート・シーケンス図・ER図・状態遷移図・ガントはMermaidコードブロックで書ける（ビューアがレンダリングする）
- Mermaidの改行は `<br>` を使う
- 画像・SVGを添付する場合は `$NOTES_DIR/assets/<note-slug>/` に置き、メモからは相対パス `assets/<note-slug>/foo.png` で参照する
- **参照情報は必ずMarkdownのハイパーリンクで書く**（ビューアがクリックで適切なアプリで開く）:
  - 外部ドキュメント・Issue・PR: `[Tauri公式](https://tauri.app/)` → ブラウザで開く
  - プロジェクト内のファイル: **絶対パス**で `[src/main.rs](/Users/.../repo/src/main.rs)` → デフォルトアプリで開く（相対パスはノートディレクトリ基準になるので使わない）
  - 関連する別のメモ: ファイル名だけで `[認証フローの概要](2026-07-01-auth-flow.md)` → アプリ内でそのメモに移動
  - 「どのファイルを読めばよいか」を伝えるメモでは、ファイル名を裸で書かずリンクにすること

### 3-b. HTMLメモ（ビジュアライズ専用）

Mermaidで表現できないカスタム可視化（インタラクティブなグラフ、フィルタUI、タイムライン、大量データの表等）に限りHTMLで書く。ユーザーはHTMLメモを編集しない前提。

規約:
- 拡張子は `.html`、命名規則はMDと同じ
- **単一ファイル自己完結**。CDN読み込み・外部fetch・外部フォントは禁止（sandbox付きiframeで表示され、外部通信はブロックされる）。JS/CSSはすべてインラインで書く
- 先頭にメタデータコメントを必ず入れる:

```html
<!-- repo-notes-meta {"title":"依存関係グラフ","created":"2026-07-02T15:30:00+09:00","tags":["viz"]} -->
<!DOCTYPE html>
...
```

### 4. 新規ノートの保存

```bash
"$NOTES_CLI" note write --repo-key "$REPO_KEY" --file-name "YYYY-MM-DD-topic.md" --new --content-file "$NOTE_WORK/draft.md" --dry-run
"$NOTES_CLI" note write --repo-key "$REPO_KEY" --file-name "YYYY-MM-DD-topic.md" --new --content-file "$NOTE_WORK/draft.md"
```

`--new` は既存ファイルを上書きしない。同名があればファイル名に `-2`、`-3` を付ける。詳細な入出力は `"$NOTES_CLI" note write --help` で確認する。

### 5. 作成後の報告

作成したメモの絶対パスとタイトルを伝え、repo-notes アプリで閲覧できることを添える。

## 既存メモの参照・再編集

「メモを読んで」「前回の調査を元に実装して」「メモを更新して」のような依頼では、まず既存メモを探して読む。

### 1. メモディレクトリを解決して一覧する

読み取りだけなら `--probe` を付ける（ディレクトリや meta.json を新規作成しない）:

```bash
NOTES_DIR=$(bash ~/.claude/skills/repo-notes/scripts/resolve-note-dir.sh --probe)
ls -t "$NOTES_DIR"   # 新しい順に一覧（ファイル名の日付とtitleで目星を付ける）
```

- ディレクトリが存在しない・空の場合、そのリポジトリのメモはまだ無い。ユーザーに伝えて指示を仰ぐ
- どのメモか曖昧な場合は、候補のタイトルを提示して確認する

### 2. メモを元に実装・再調査する場合

- 対象メモを **Read してから** 作業に入る。関連する別メモへのリンク（`[...](YYYY-MM-DD-*.md)`）があればそれも辿る
- メモは書かれた時点のスナップショット。**ファイルパス・コード片・結論は現在のコードと突き合わせて検証**してから使う
- 作業の結果、メモの内容が古くなったら更新を提案する

### 3. メモの再編集

保存元のスナップショットを先に読み、その内容を基に別の下書きを作る。

```bash
REPO_KEY=$(basename "$(dirname "$NOTES_DIR")")
NOTES_CLI="$HOME/.claude/skills/repo-notes/bin/repo-notes"
NOTE_WORK=$(mktemp -d)
"$NOTES_CLI" note read --repo-key "$REPO_KEY" --file-name "YYYY-MM-DD-topic.md" --output raw > "$NOTE_WORK/snapshot.md"
cp "$NOTE_WORK/snapshot.md" "$NOTE_WORK/draft.md"
```

`snapshot.md` を Read し、`draft.md` に編集結果を作る。読み込みが失敗した場合は編集・保存へ進まない。保存時は元のスナップショットを渡す。

```bash
"$NOTES_CLI" note write --repo-key "$REPO_KEY" --file-name "YYYY-MM-DD-topic.md" --expected-file "$NOTE_WORK/snapshot.md" --content-file "$NOTE_WORK/draft.md"
```

終了コード3は、保存元と現在のファイルが異なる競合。下書きとスナップショットを保持して最新を読み、採用する変更をユーザーに確認する。元のスナップショットを差し替えるだけで下書きを強制保存しない。

- frontmatter（`title` / `created` / `tags`）の形式を維持する。`created` は変更しない
- メモは追記ログではなく「現在の理解を表す文書」。古い記述は残さず本文を書き換える
- HTMLメモの編集時も自己完結・CDN禁止の規約を守る

## 判断基準まとめ

| 内容 | 形式 |
|---|---|
| 解説・調査結果・手順・所感 | Markdown |
| フロー図・シーケンス図・ER図・状態遷移図 | Markdown + Mermaid |
| スクリーンショット・既存画像の添付 | Markdown + assets/ |
| インタラクティブ可視化・カスタム図 | HTML（自己完結） |
