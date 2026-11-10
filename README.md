# repo-notes

Gitリポジトリ単位で管理する、AIエージェント用キャッチアップメモアプリ。

チーム開発中に Claude Code / Codex などのエージェントが書くキャッチアップメモの置き場所を提供する。メモはプロジェクトリポジトリの外（ローカルのデータディレクトリ）に保存され、Tauri 製デスクトップアプリで閲覧・編集できる。

## 構成

- **スキル** (`skill/`) — エージェントがメモを書き込むための Claude Code / Codex 用スキル。`resolve-note-dir.sh` が repo-key を導出し、書き込み先ディレクトリを解決する
- **ビューアアプリ** (`src/`, `src-tauri/`) — Tauri v2 + React。MD/HTML ノートの閲覧、MD ノートの編集・作成・削除、Mermaid・画像・SVG 表示に対応

## データディレクトリ

`~/.repo-notes/`（環境変数 `REPO_NOTES_DIR` で上書き可）

保存先は起動時に作成・正規化し、CRUD・監視・アセット配信・ノートの外部アプリ操作で共有する。相対パスや保存先自体のシンボリックリンクも起動時に解決する。ノート・メタデータ・pins・アセットの I/O はディレクトリ handle を基準に行い、操作途中のリンクすり替えによる領域外への逸脱を拒否する。保存領域内を指す相対シンボリックリンクは利用できる。絶対シンボリックリンクは領域内を指していても利用できない。

未作成の一覧・pins は空として扱い、I/O 障害や JSON 破損は画面にエラーを表示する。破損した pins をピン操作で初期化・上書きしない。アセットはアプリ独自の `asset` protocol から同じ保存先の handle で読み込む。設定の asset scope は空で、標準ハンドラーによる別の読み込み経路は許可しない。

ノート本体を削除できなかった場合は削除失敗を返す。本体の削除後にアセットやピン情報の後処理だけが失敗した場合は、ノートを閉じて後処理の警告を表示する。警告は別のノートを選ぶか、警告の「閉じる」で解除できる。

```
~/.repo-notes/
└── repos/<repo-key>/
    ├── meta.json               # リポジトリ情報（作成時のみ書き込み・以後不変）
    └── notes/
        ├── YYYY-MM-DD-<slug>.md    # 通常のメモ（frontmatter付き）
        ├── YYYY-MM-DD-<slug>.html  # ビジュアライズ用メモ（自己完結HTML）
        └── assets/<note-slug>/     # 画像・SVG等のアセット
```

## repo-key 仕様

ワークツリーを区別せず「Gitリポジトリ」単位でメモを集約するための識別子。

1. `git remote get-url origin` を正規化する
   - scp 形式（`git@host:path`）→ `host/path`
   - scheme（`https://` `ssh://` `git://` 等）・userinfo・ポート番号を除去
   - 末尾の `/` と `.git` を除去、全体を小文字化
   - 例: `git@github.com:Owner/Repo.git` → `github.com/owner/repo`
2. origin が無い場合は `git rev-parse --git-common-dir` の実体パスを使う（`local:/abs/path/.git`）。ワークツリーから実行しても共通 git ディレクトリに解決されるため、同一リポジトリに集約される
3. key は `<slug>--<hash8>`。slug はパス末尾2セグメントの kebab 化（可読性用）、hash8 は正規化文字列の SHA-256 先頭8桁（一意性担保）
4. origin が後から追加された場合、次回の解決時に `local:` キー配下のメモ（pins.json 含む）が origin ベースの新キーへ自動移行され、meta.json は新キーの内容で再生成される。`--probe` は移行せず、未移行のレガシーディレクトリがあればそちらを返す。新旧両方に実体がある場合は自動では触らない（警告のみ）

導出ロジックは `skill/scripts/resolve-note-dir.sh` に一元化されている。

## メタデータ規約

### MD ノート（frontmatter）

```yaml
---
title: プロジェクト構成の概要   # 必須
created: 2026-07-02T10:00:00+09:00  # 必須
tags: [architecture, onboarding]    # 任意
agent: claude-code                  # 任意
---
```

パース失敗時はビューアが H1 → ファイル名の順でタイトルにフォールバックする。

### HTML ノート（先頭コメント）

```html
<!-- repo-notes-meta {"title":"依存グラフ","created":"2026-07-02T10:00:00+09:00","tags":["viz"]} -->
```

先頭 2KB 以内に無い場合は `<title>` → ファイル名にフォールバック。HTML ノートは**単一ファイル自己完結の静的 HTML** として、インライン CSS と data 画像を利用できる。表示前に DOMPurify で script・link・meta・base・子フレーム等の要素と、リンク・フォーム送信先等の属性を除去する。アプリ所有の文書用 CSP を先頭に置き、権限を付与しない sandbox 付き iframe の srcDoc として表示する。JavaScript はインライン・外部とも実行せず、外部リソースの読み込み・接続の先行確立・外部遷移を禁止する。既存の JavaScript に依存するグラフやフィルタ UI は動作しない。Markdown の Mermaid は引き続き利用できる。

## 開発

```bash
mise install          # Node 24 / pnpm 11
pnpm install
pnpm tauri dev        # 開発起動
pnpm tauri build      # ビルド
make release          # 起動中アプリを終了 → ビルド → /Applications へ再配置 → 起動
```

### テスト

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # Rust（パーサ・パス検証）
pnpm test                                          # vitest（URL書き換え）
pnpm lint                                          # React Hooks recommended
pnpm test:skill                                    # repo-key導出のshellテスト
```

### WSL バックエンド

GUI ライブラリを使わないバックエンドは、同じ Rust crate の `desktop` feature を外してビルドする。Windows の接続・配置 UI とインストーラーは後続作業で、現段階では手動の開発用コマンドを提供する。

```bash
cargo build --manifest-path src-tauri/Cargo.toml --locked --no-default-features --bin repo-notes-backend
cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features
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

既存の `note read/write` も `repo-notes-backend` から利用でき、GUI と同じ保存元比較・排他・原子的保存を使う。接続の wire 型は `src-tauri/src/backend.rs` に置き、保存処理をプロトコルへ複製しない。

### スキルのインストール

```bash
make install-skill    # ~/.claude/skills/repo-notes と ~/.agents/skills/repo-notes にコピー
```

`install-skill` は実行内容を表示して `yes` を1回確認した後、既存アプリバイナリをビルドしてスキルの `bin/repo-notes` に同梱する。CIでは `make YES=1 install-skill` を使う。ビルド済み成果物を配る場合は `BUILD_CLI=true APP_BINARY=/path/to/repo-notes` を指定できる。macOSの配布経路を対象とし、WindowsのGUI subsystemを持つバイナリのコンソール対応は別途扱う。

共通設定をrulesyncで管理する場合、直接インストールしたスキルは `rulesync import --targets claudecode --global --features skills` でソースへ取り込み、`~/claude-dotfiles/` と `setup.sh` による同期を維持する。

## 保存競合の扱い

アプリは編集開始時のraw内容を保存元として送り、現在のファイルと異なる場合は保存を拒否する。下書きと保存結果はアプリ起動中の画面切替後も保持し、破棄と再読込は利用者が明示して行う。保存中の追加入力も未保存として保持する。未保存の下書きはメモリ上にあり、アプリ終了・再起動からの復元は行わない。

エージェントも同じアプリバイナリのheadless `note read/write` を使う。GUIは初期化せず、readのraw出力は改行を加えない。再編集はスナップショットと下書きを別ファイルに保持し、`--expected-file` と `--content-file` を渡す。新規作成は `--new`、事前確認は `--dry-run`、詳細は `repo-notes note write --help` を参照する。競合は終了コード3で通知し、強制上書き・自動マージは行わない。

保存に参加する全mutatorは、保存先の永続lockfileでプロセス間排他を取り、版比較から同じディレクトリでの原子的な公開までを一緒に行う。lockfile自体は削除・置換しない。pinsも同じ排他と原子的保存を使う。公開前の一時ファイルは同期するが、rename後のディレクトリ同期による電源断耐性までは保証しない。

排他を使わない外部エディタ等も、次の保存時のraw比較で既存更新を検出する。ただし比較後から公開までに非参加writerが書き込む競合や、保存先・lockfileを非参加プロセスが入れ替える操作は排他の保証外となる。アプリとエージェントの双方が同じ保存先と保存方法を使う必要がある。

## ファイルI/Oの実行先

一覧・全文検索・ノートCRUD・ピン操作・ローカルファイルの外部起動は非同期のTauriコマンドを入口とし、同期I/Oとロック待機を `spawn_blocking` の実行先で行う。アセットは非同期schemeで要求を受け、同じblocking実行先へ読込を渡し、既存の保存先・配信レスポンスを維持する。起動時に保持した保存先を共有し、各呼び出しで環境変数から解決し直さない。

保存競合と通常の失敗、削除後のcleanup警告は従来の戻り値に保持する。workerが異常終了した場合もコマンドはエラー、assetは500を返す。印刷のようなネイティブUI操作と、メモリ上の保存先取得は同期の入口を維持する。

### デバッグ

- **ブラウザ単体モード**: `pnpm dev` で起動した vite（http://localhost:1420）をブラウザで直接開くと、Tauri APIモック（`src/lib/devMock.ts`、devビルド限定）によりフィクスチャデータでUIを確認できる。レイアウト・Markdown・Mermaid・静的HTML・編集フローの確認に使う。Markdownの画像アセットはasset protocolが無いため表示されない
- **tauri-mcp**: `cargo install --git https://github.com/dirvine/tauri-mcp` で入る MCP サーバー/CLI。`tauri-mcp tool launch_app '{"app_path":"..."}'` でアプリ起動、`take_screenshot` で画面キャプチャができる（キャプチャには端末アプリへの「画面収録」権限が必要）。macOS では `execute_js` は使えない（WKWebViewがDevToolsのHTTPエンドポイントを持たないため）

## 制限事項

- 外部アプリ起動はノートのパスを検証して OS に渡す。受け取った外部アプリがファイルを開くまでの間の変更は、アプリ内 I/O のディレクトリ handle では制御できない
- HTML ノートはアプリ内では編集不可（閲覧・削除のみ）。編集は外部エディタで行う
- 画像アセットのアプリ内アップロードは非対応（エージェントがスキル経由で配置する想定）
