# インストーラーの導入・接続・復旧

ビルド対象は Windows 11 x64 + 既存 Ubuntu WSL2、macOS 13 以降の Intel / Apple Silicon。現在は未署名の検証用配布で、macOS は ad-hoc 署名、Developer ID 署名・公証と Windows 署名は未設定。一般公開前に必要な署名方式・運用が残っている。OS が未署名アプリを拒否する環境では、保護設定を無効化せず署名済み公開版を待つ。

## 成果物の取得

1. Actions の `Installer builds` で、取得する版・コミットと全ビルドの成功を確認する。
2. Windows は `release-windows`、macOS は `release-macos` の artifact を取得して展開する。Windows インストーラーには WSL バックエンドを同梱している。
3. タグ起点の実行では全成果物の成功後に Draft Release へ添付する経路がある。今回の検証ではタグ push・実際の Draft Release 作成は行っていない。

## Windows の導入と接続

1. 既存 Ubuntu が WSL2 で動き、ユーザーの初期設定が完了していることを確認する。未導入の場合は先に [Microsoft の WSL 導入手順](https://learn.microsoft.com/en-us/windows/wsl/install) に従う。
2. `repo-notes_<版>_windows_x64_setup.exe` で導入し、アプリを起動する。WebView2 がない場合の runtime 取得にはネット接続が必要。[Windows インストーラーの前提](https://v2.tauri.app/distribute/windows-installer/)
3. 「WSL 接続設定」に以下を入力し、「保存して接続」を選ぶ。

| 入力欄 | 指定内容 |
|---|---|
| ディストリビューション | エージェントが使う既存 Ubuntu の名前 |
| Linux ユーザー | 同じ Ubuntu でエージェントを実行するユーザー。Ubuntu ターミナルの `id -un` で確認する |
| ノート保存先 | 既定の `~/.repo-notes` なら空欄。カスタム保存先は Linux の絶対パス |

4. 「WSL 接続済み」と、意図した distro・ユーザー・保存先の表示を確認する。成功した設定は次回起動にも使われる。

空欄は選択ユーザーのホーム直下の `.repo-notes` を意味する。シェルの profile やプロジェクトの `.envrc` にある `REPO_NOTES_DIR` は UI へ自動取り込みされないため、カスタム保存先は明示する。ノートは WSL 内に置き、バックエンドは `~/.local/share/repo-notes/backend/<版>/repo-notes-backend` に配置する。

## 保存先とエージェント CLI

UI とエージェントには同じ保存先を指定する。一覧が空なら、WSL の対象 Git リポジトリで既存スキルのメモ作成手順を実行して保存先を初期化し、UI の指定先と照合する。

アプリの導入には Node.js / Rust での再ビルドは不要。エージェント用スキルの導入・更新には、既存の [スキルのインストール手順](../README.md#スキルのインストール) を使う。ソース配布の Makefile がある場所では、配備済み backend を渡して再ビルドせず更新できる（以下は `0.1.0` の例）。

```bash
make BUILD_CLI=true APP_BINARY="$HOME/.local/share/repo-notes/backend/0.1.0/repo-notes-backend" install-skill
```

表示された操作を確認して `yes` を1回入力する。rulesync 管理下では、既存の `rulesync import --targets claudecode --global --features skills` による取り込みと、管理元 `~/claude-dotfiles/` の `./setup.sh` による同期を維持する。生成先を直接編集しない。

保存はスキルの `bin/repo-notes note read/write` を使う。既存ノートは `--output raw` で保存元を取得し、別の下書きを `--expected-file` と `--content-file` で保存する。新規作成は `--new`、事前確認は `--dry-run`。終了コード3は競合なので、保存元と下書きを保持して最新を確認する。詳細は [repo-notes スキル](../skill/SKILL.md) を参照する。

## Windows の外部操作

| 操作 | 利用するパス・アプリ |
|---|---|
| パスをコピー / Claude用にコピー | Linux の絶対パス |
| エディタで開く | Windows 版 VS Code + WSL 拡張。`code` の PATH 登録と `ms-vscode-remote.remote-wsl` が必要 |
| 場所を表示 | Explorer から WSL の場所を表示 |
| クリップボード・印刷 | Windows 側の機能 |

VS Code での編集と Explorer での場所表示には、選択した Linux ユーザーと distro の既定ユーザーの一致が必要。不一致の案内が出てもアプリ内の閲覧・編集は利用できる。[VS Code の WSL 接続](https://code.visualstudio.com/docs/remote/wsl#_from-the-windows-command-prompt)、[Windows からの WSL ファイルアクセス権限](https://learn.microsoft.com/en-us/windows/wsl/file-permissions#accessing-linux-files-from-windows-using-wsl)

## 切断・保存競合・接続先変更

| 状況 | 操作 |
|---|---|
| WSL 未導入・対象 distro なし | WSL2/Ubuntu の導入と初期設定、入力した distro 名を確認して接続する |
| WSL/バックエンド停止・接続失敗 | 下書きを控え、対象 WSL が利用可能になった後に「再接続」を選ぶ |
| UI/backend の版・検証値不一致 | 同じリリースのインストーラーを再導入して接続する |
| 「保存結果を確認できません」 | 下書きを控えて再接続し、保存済み内容を確認する。結果不明の保存は自動再送されない |
| 別の書き込みによる競合 | 下書きと最新内容を比較して採用する内容を決める。強制保存は行わない |
| 別の distro・ユーザー・保存先へ変更 | 未保存の下書きを保存するか、対象を確認して明示的に破棄してから変更する |

同じ接続先への再接続では下書きを保持する。「下書きを破棄して最新を読込」は確認後に下書きを失う操作なので、必要な内容を先に控える。下書きはメモリ上だけにあり、アプリ終了・再起動・再導入の前に保存または退避する。アプリ終了時は所有バックエンドだけを終了し、他の WSL 環境は停止しない。

新規ノートのファイル日付・`created` はバックエンドが動く端末、画面の日時は閲覧端末のタイムゾーンに従う。WSL は既定で Windows のタイムゾーンへ追従するが、同期無効化やプロセスの `TZ` 上書きがある場合は異なる。[WSL の時刻設定](https://learn.microsoft.com/en-us/windows/wsl/wsl-config#time-settings)

## 再導入とアンインストール

更新前に未保存の下書きを保存・退避し、同じ版の UI/backend がそろったインストーラーを使う。再導入後に接続先を確認する。設定が失われた場合も、既存の同じ WSL 保存先を指定する。

Windows UI のアンインストールは Windows の「設定」→「アプリ」から行う。今回のサイレントアンインストールでは接続設定が残り、WSL の fixture ノートも保持された。WSL のノートと `~/.local/share/repo-notes/backend/<版>/` は Windows UI と別の場所にあり、この手順には Linux 側データの削除・distro 登録解除を含めない。

macOS は DMG 内の `repo-notes.app` を `/Applications` に配置して起動し、ローカルの既存保存先を使う。macOS 実機での導入・起動・更新は未確認。

## 検証用 CI の設定

現行の検証ビルドに、利用者が追加登録する Secrets はない。タグ起点の Release 書き込みは、自動発行の `github.token` を `GH_TOKEN` に割り当てて行う。macOS の `APPLE_SIGNING_IDENTITY` は ad-hoc 署名を指定しており、Developer ID・公証と Windows 署名の設定キーは署名方式の決定後に追加する。[配布 workflow](../.github/workflows/release.yml) を参照する。

## 確認済みの範囲と未確認事項

Linux backend・Windows NSIS・macOS universal DMG のビルドと、全成果物の組み立て検証に成功した。Windows `0.1.0` は元の identifier を維持した未変更のインストーラーを使い、Vite を起動せず embedded runtime で実導入を確認した。対象コミット・成果物・合成 fixture・検証条件は [PR #28 の検証証跡](https://github.com/Gild-shogi/repo-notes/pull/28#issuecomment-6776966827) を参照する。

| 対象 | 確認済み | 未確認 |
|---|---|---|
| Windows インストーラー・UI | 元 identifier の導入、embedded 起動、CRUD・検索・pin、エージェント書込の監視反映、画像・静的 HTML・Mermaid | 開発ツールが未導入の OS への導入、実際の VS Code / Explorer / 印刷 / クリップボード操作 |
| 日時表示 | 同じ `created` を WebView の LA 表示 emulation と端末の Asia/Tokyo で表示。OS/WSL 設定は変更していない | 別タイムゾーンの OS 実機 |
| 再導入・アンインストール | 同版再導入の設定保持、サイレント削除後の WSL fixture 8ファイルの hash 保持。QA 設定だけを限定清掃し、所有プロセス残存0 | 別バージョンへのアップグレード、他の環境でのデータ保持 |
| WSL 障害・接続先変更 | 版照合・切断・保存再送抑止などの実装と自動検証 | WSL 未導入・再起動からの一連の実復旧、Ubuntu 以外の distro、完全オフライン導入 |
| macOS | universal DMG のビルド | macOS 13 / Intel / Apple Silicon 実機の導入・起動・CRUD・更新 |
| 署名・Release | 未署名の検証用配布に限定する公開 gate | Developer ID / 公証、Windows 署名、タグ起点の実 Draft Release 作成・一般公開 |
