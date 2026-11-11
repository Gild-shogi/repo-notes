# 開発ガイド

Tauri v2・React・Rust で構成されたデスクトップアプリの開発・検証手順。macOS のビルド・配置手順を記載する。

[アプリ紹介](../README.md) · [導入ガイド](getting-started.md)

## 構成

- **スキル** (`skill/`) — エージェントがメモを書き込むための Claude Code / Codex 用スキル。`resolve-note-dir.sh` が repo-key を導出し、書き込み先ディレクトリを解決する
- **ビューアアプリ** (`src/`, `src-tauri/`) — Tauri v2 + React。MD/HTML ノートの閲覧、MD ノートの編集・作成・削除、Mermaid・画像・SVG 表示に対応

## 開発環境

```bash
mise install          # Node 24 / pnpm 11
pnpm install
pnpm tauri dev        # 開発起動
pnpm tauri build      # ビルド
make release          # 起動中アプリを終了 → ビルド → /Applications へ再配置 → 起動
```

## テスト

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # Rust（パーサ・パス検証）
pnpm test                                          # vitest（URL書き換え）
pnpm lint                                          # Oxlint + React Hooks recommended（警告もエラー）
pnpm test:skill                                    # repo-key導出のshellテスト
```

Lint は `oxlint.config.ts` で設定する。Oxlint の標準ルールに加え、既存の React Hooks 推奨ルールを `eslint-plugin-react-hooks` の JS プラグインとして実行する。React Compiler 由来の検査も維持するため、推奨プリセット全体を継承する。[JS プラグイン機能](https://oxc.rs/docs/guide/usage/linter/js-plugins.html)は alpha のため、Oxlint はバージョンを固定している。

## デバッグ

- **ブラウザ単体モード**: `pnpm dev` で起動した vite（http://localhost:1420）をブラウザで直接開くと、Tauri APIモック（`src/lib/devMock.ts`、devビルド限定）によりフィクスチャデータでUIを確認できる。レイアウト・Markdown・Mermaid・静的HTML・編集フローの確認に使う。Markdownの画像アセットはasset protocolが無いため表示されない
- **tauri-mcp**: `cargo install --git https://github.com/dirvine/tauri-mcp` で入る MCP サーバー/CLI。`tauri-mcp tool launch_app '{"app_path":"..."}'` でアプリ起動、`take_screenshot` で画面キャプチャができる（キャプチャには端末アプリへの「画面収録」権限が必要）。macOS では `execute_js` は使えない（WKWebViewがDevToolsのHTTPエンドポイントを持たないため）

## ライセンス一覧の更新

依存更新時は `pnpm licenses:generate` で第三者表記を再生成し、`pnpm licenses:check` で一致を確認する。[配布条件と生成範囲](licenses.md)を参照する。

## README 用画像

[スクリーンショットの撮影条件](screenshots/README.md)を参照する。実際のノートや顧客・業務情報は使わず、ブラウザ単体モードの架空データだけを撮影する。
