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
cargo test --manifest-path src-tauri/Cargo.toml --features backend   # Rust（パーサ・パス検証）
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

## WSL バックエンド

GUI ライブラリを使わないバックエンドは、同じ Rust crate の `desktop` feature を外し、`backend` feature を指定してビルドする。Windows 版はインストーラーに同梱した backend を接続時に配置する。以下はバックエンド単体の開発用コマンド。

```bash
cargo build --manifest-path src-tauri/Cargo.toml --locked --no-default-features --features backend --bin repo-notes-backend
cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features --features backend
src-tauri/target/debug/repo-notes-backend --help
src-tauri/target/debug/repo-notes-backend --stdio
```

`--stdio` は1行1メッセージの JSON 接続。最初に `hello` を送り、UI とバックエンドのアプリ版・プロトコル版が一致した後に操作を受け付ける。不一致なら保存先を開く前に拒否して接続を終了する。`dataRoot` は接続ごとに固定し、省略時は既存の `REPO_NOTES_DIR` または Linux home の `.repo-notes` を使う。シェルの profile は実行しない。

```json
{"id":1,"method":"hello","appVersion":"0.1.0","protocolVersion":1,"dataRoot":"/tmp/repo-notes-fixture"}
{"id":2,"method":"list_repos"}
{"id":3,"method":"shutdown"}
```

正常応答は `{"type":"response","id":2,"result":...}`、失敗は同じ `id` と `error.kind` を返す。ノートの引数と結果は既存の Tauri コマンドと共通で、引数のキーは `repoKey`・`fileName`・`expectedRaw` などの camelCase。`read_asset` は検証済みアセットの HTTP status・headers・base64 body を返し、Windows 側で WebView の配信応答へ戻す。変更通知は `{"type":"event","event":"changed","repoKeys":[...]}`、監視障害は `event:"watch_error"` と `message` で通知する。初期化時の空 `repoKeys` は全一覧の再取得を表す。

stdout は応答とイベント専用、診断は stderr に出力する。要求の形式不正は `invalidRequest` を返して接続を終了する。保存競合は既存の `conflict`、I/O 失敗は `failure` で通知し、接続は維持する。標準入力の EOF または `shutdown` で所有するバックエンドと監視を終了し、WSL 全体や他のプロセスを停止しない。結果不明の保存は呼び出し側で自動再送しない。

既存の `note read/write` も `repo-notes-backend` から利用でき、GUI と同じ保存元比較・排他・原子的保存を使う。接続の wire 型は `src-tauri/src/backend.rs` に置き、保存処理をプロトコルへ複製しない。新規ノートのファイル日付と `created` はバックエンドが動く端末の時刻設定に従い、画面の日時は閲覧している Windows／macOS のタイムゾーンで表示する。WSL は既定の `useWindowsTimezone=true` で Windows のタイムゾーンへ追従するが、同期設定やプロセスの `TZ` 上書きによって異なる。[WSL の時刻設定](https://learn.microsoft.com/en-us/windows/wsl/wsl-config#time-settings)

## インストーラーの配布

導入・接続・復旧と確認済みの範囲は [インストーラーの利用手順](installation.md) を参照する。

`v<package.json の version>` タグを起点に `Installer builds` が動く。`package.json`、Cargo package、Tauri config、同梱 backend の版と protocol を照合し、WSL backend・macOS DMG・Windows NSIS の全ビルドが成功してから同じ GitHub Release へ添付する。PR では同じビルドを行って Actions artifact に保存し、Release は作らない。

| ビルド対象 | 成果物 | 実行時の構成 |
|---|---|---|
| macOS 13 以降 / Intel・Apple Silicon | universal `.dmg` | ローカルのノート処理 |
| Windows 11 x64 + 既存 Ubuntu WSL2 | NSIS `.exe` | Windows UI と同梱 Linux backend |
| WSL Linux x64 | static musl backend `.tar.gz` | 同じ版のエージェント用 CLI / backend |

ビルド対象と実機検証済みの範囲は別であり、実機確認の範囲は #22 の証跡を参照する。異なる版へのアップデートの事前検証は今回の完了条件から外し、不具合発生時に Bug Issue で追跡する。Windows ARM64、Ubuntu 以外の distro、完全オフライン導入は初回対象外。WebView2 がない環境では Tauri インストーラーが runtime を取得するためネット接続が必要。WSL2 自体は利用者が導入し、アプリは権限昇格・OS 再起動・distro の作成を自動実行しない。

初回の workflow は未署名の検証用成果物を作り、Release を **Draft のまま** にする。macOS は ad-hoc 署名であり Developer ID 署名・公証済みではない。Windows の署名 provider / HSM / 証明書運用は未設定・未検証で、CI に汎用の署名コマンドや新しいクラウド連携を追加していない。署名設定の存在だけでは公開せず、macOS の Developer ID 署名・公証・staple と Windows のアプリ/インストーラー両方の実署名検証が成功するまで一般公開しない。署名方式と CI 設定は別途決定する必要がある。[macOS 署名](https://v2.tauri.app/distribute/sign/macos/)、[Windows 署名](https://v2.tauri.app/distribute/sign/windows/)

Draft の利用には OS の警告やポリシー上の制限がある。Windows Smart App Control 等で未署名アプリが拒否される環境について、保護設定の無効化は案内しない。署名済みの公開版が必要な環境は準備完了まで利用対象にしない。

Windows の同梱資源は `backend/repo-notes-backend` と `backend/manifest.json`。manifest は `version`・`protocolVersion`・`sha256` を持ち、resource と WSL への配備後の双方で照合する。WSL の配置先は選択した Linux ユーザーの `~/.local/share/repo-notes/backend/<version>/` で、ノートとは別に保持する。上書き導入では同じ版の UI/backend を起動し、ノート形式の移行やユーザーノートの削除を行わない。

`SHA256SUMS` と `release-manifest.json` を各配布物と一緒に添付する。公開済み Release の成果物は再実行で置換せず、Draft の再実行だけを更新する。失敗した OS ビルドや版不一致の成果物は一般公開しない。

配布のロジックは Node 24 標準ライブラリの `scripts/release.ts` にまとめている。

```bash
node --test tests/release/release.node-test.ts
node scripts/release.ts check v0.1.0
cargo build --manifest-path src-tauri/Cargo.toml --locked --no-default-features --features backend --bin repo-notes-backend --release --target x86_64-unknown-linux-musl
node scripts/release.ts stage-backend src-tauri/target/x86_64-unknown-linux-musl/release/repo-notes-backend
node scripts/release.ts package-backend /tmp/repo-notes-backend-release
```

OS 別配布は CI に閉じ、Makefile は `release`・`install-skill`・`test` の3ターゲットを維持する。ローカルの macOS 再配置は `.app` だけをビルドし、GitHub Release の作成とは分ける。今回増えるローカル処理は配布スクリプトの直接実行で足りるため、Just は導入しない。今後ローカルの複数行・OS 別タスクが増えた場合に再比較する。
