# 導入ガイド

macOS でアプリとエージェント用スキルをソースから導入し、最初のノートを作成する手順。

[アプリ紹介](../README.md) · [開発ガイド](development.md) · [インストーラーの利用手順](installation.md)

## 必要な環境

- Node.js 24 / pnpm 11（`mise.toml` で指定）
- Rust ツールチェーン
- Xcode Command Line Tools と macOS のビルド環境
- エージェント用スキルを使う場合は Claude Code または Codex

## アプリの起動

このリポジトリを取得し、リポジトリ直下で実行する。

```bash
mise install
pnpm install
pnpm tauri dev
```

開発モードでアプリが起動する。アプリの配布形式でビルドする場合は `pnpm tauri build` を実行する。macOS のアプリバンドルは `src-tauri/target/release/bundle/macos/repo-notes.app` に出力される。

## エージェント用スキルのインストール

```bash
make install-skill    # ~/.claude/skills/repo-notes と ~/.agents/skills/repo-notes にコピー
```

`install-skill` は実行内容を表示して `yes` を1回確認した後、既存アプリバイナリをビルドしてスキルの `bin/repo-notes` に同梱する。CIでは `make YES=1 install-skill` を使う。ビルド済み成果物を配る場合は `BUILD_CLI=true APP_BINARY=/path/to/repo-notes` を指定できる。macOS ではアプリのバイナリを使う。Windows/WSL では Windows の GUI バイナリを CLI にせず、接続時に配備した Linux バックエンドを使う。ビルド済み backend を渡せば Node.js/Rust の再ビルドは不要。

WSL の配備済み backend をスキルへコピーする場合は、アプリに表示される版のパスを指定する（以下は `1.0.0`）。カスタム保存先を使う場合は、エージェントの `REPO_NOTES_DIR` と UI の接続先を同じにする。

```bash
make BUILD_CLI=true APP_BINARY="$HOME/.local/share/repo-notes/backend/1.0.0/repo-notes-backend" install-skill
```

共通設定をrulesyncで管理する場合、直接インストールしたスキルは `rulesync import --targets claudecode --global --features skills` でソースへ取り込み、`~/claude-dotfiles/` と `setup.sh` による同期を維持する。

## 最初のノート

1. メモを残したい Git リポジトリで、スキルを導入したエージェントを開く。
2. 「このプロジェクトの構成を調べて、repo-notes にメモして」と依頼する。
3. アプリのリポジトリ一覧から対象を選び、作成されたノートを開く。

ノートは既定で `~/.repo-notes/` に保存される。プロジェクトの Git リポジトリには追加されず、同じリポジトリのワークツリー間で共有される。

## 閲覧と編集

- リポジトリとノートを選ぶと本文が表示される。検索欄ではタイトル・タグ・本文から探せる。
- Markdown ノートの「編集」から本文を変更し、保存ボタンまたは `⌘S` で保存できる。
- よく使うリポジトリやノートはピン留めできる。サイドペインは折りたたみ可能。
- HTML ノートは閲覧用。変更する場合は「エディタで開く」から外部エディタを使う。
- 別の書き込みと競合すると保存を拒否し、下書きを保持する。[保存競合の詳細](storage.md#保存競合の扱い)を確認してから再読込する。

保存先の変更とファイル構成は[保存と安全性](storage.md)、エージェントが作成するノートの規約は[ノート形式](note-format.md)を参照する。
