# Windows/WSL の配置・接続・配布方式

状態: 比較中。分割と下記3点は承認済み。通信方式・接続保護・外部アプリ操作・プロトコルの合意が済むまで、製品コードへ実装しない。対象は [Epic #2](https://github.com/Gild-shogi/repo-notes/issues/2)、設計作業は [#18](https://github.com/Gild-shogi/repo-notes/issues/18)。

## 目的と現行の境界

Windows ネイティブの Tauri UI で、WSL 内のエージェントと同じノートを扱う。macOS はローカル動作を維持し、両経路の保存元比較・プロセス間排他・原子的保存、安全な保存領域、静的 HTML の隔離を共有する。開発ツールのない利用者へタグ単位で同じ版の成果物を配布する。

調査対象は main の `d3ac7748b80f4c1e9b8bf5a45f0ffd6c81dfe248`。

| 現在の場所 | 確認済みの境界 | 今回必要になる変更 |
|---|---|---|
| `src-tauri/src/store.rs:14`、`:25`、`:51` | 保存先解決、開いたディレクトリ、プロセス間排他を所有 | WSL と macOS が同じ処理を利用 |
| `src-tauri/src/commands.rs:18` | Tauri State から保存先を取得し blocking worker で実行 | Windows の要求を選択した WSL へ中継 |
| `src-tauri/src/watch.rs:12` | 監視と Tauri event の送信が同じ関数にある | 監視結果と OS ごとの通知を分離 |
| `src-tauri/src/asset.rs:11` | Tauri HTTP 型、保存領域検査、MIME/CSP を所有 | 検証した内容を Windows の asset protocol へ中継 |
| `src/components/NoteView.tsx:91`、`src/lib/links.ts:37` | 同一 OS の絶対パスと macOS home を前提 | Linux パスの所有者と Windows 操作を明示 |
| `src-tauri/src/open_link.rs:29` | macOS の実行系拡張子を除外 | Windows 側の外部起動契約を先に確定 |
| `src-tauri/src/main.rs:2`、`src-tauri/Cargo.toml` | CLI も GUI と同じバイナリ・依存を持つ | WSL 用成果物が GUI ライブラリを要求しない構成 |
| `.github/workflows/checks.yml` | main/Epic/PR の既存検証のみ | タグ・OS 別配布・全成果物成功後の公開判定 |

品質の優先順はデータ保全とアクセス境界、起動/復旧、共有処理の保守性。LAN 接続、双方向同期、自動更新、WSLg は対象外。

## 承認済みの方針

1. エージェントが利用する既存の WSL2 ディストリビューションで実行する。ディストリビューション、Linux ユーザー、保存先を明示し、接続設定だけ Windows 側に保存する。ノートは既定の `~/.repo-notes` または明示した `REPO_NOTES_DIR` に置く。専用環境の作成・データ移行を追加しない。
2. 初回検証対象は Windows 11 x64 と既存 Ubuntu WSL2、macOS 13 以降の Intel/Apple Silicon。WSL バックエンドは Windows インストーラーへ同梱する。Windows ARM64、他ディストリビューション、完全オフライン導入は初回対象外。実証前に対応済みとは表示しない。
3. 初回は未署名の検証用成果物で導入確認を進める。一般公開の Release は macOS の署名・公証と Windows 署名が整ってから行う。署名不足時は Draft とし、認証情報の値を取得しない。

## 通信方式の比較

比較スクリプトと生データの確認後に、測定条件・結果・限界をここへ記録する。標準入出力と loopback HTTP の試作は製品コードではなく、一時データだけを扱う。Windows→WSL と Linux 内の測定は別に集計する。

公式資料上、`wsl.exe --distribution <name> --user <name> --exec <command>` で対象と実行先を固定できる。`--exec` は既定シェルを使わないため、シェルの profile に設定した `REPO_NOTES_DIR` の読込を前提にしない。[WSL 基本コマンド](https://learn.microsoft.com/en-us/windows/wsl/basic-commands)、[WSL release notes](https://learn.microsoft.com/en-us/windows/wsl/release-notes)、[Bash startup files](https://www.gnu.org/s/bash/manual/bash.html#Bash-Startup-Files)

HTTP は NAT の Windows→WSL localhost、mirrored networking でも利用できるが、WSL2 ディストリビューションは network namespace を共有する。したがって loopback や動的ポートだけで呼出元・対象 distro を認証できるとは扱わない。後半は資料からの推論。[WSL networking](https://learn.microsoft.com/en-us/windows/wsl/networking)、[WSL2 の構成](https://learn.microsoft.com/en-us/windows/wsl/about)

## 配置方式の比較

| 候補 | 利点 | 負担・残る課題 | 状態 |
|---|---|---|---|
| エージェントが使う既存 WSL2 | 同じユーザー/保存先でノートと排他ファイルを共有できる | distro/ユーザー/保存先の明示と、再起動後の再接続が必要 | 採用承認済み |
| アプリ専用 distro | アプリが環境構成を管理できる | import、一般ユーザー設定、環境更新が必要。既存 distro のノートとの共有問題は残る | 初回対象外 |

専用 distro は `wsl --import` で作れるが、その作成を実験として行ってはいない。既定 root ユーザーの扱いも必要になる。[custom distro import](https://learn.microsoft.com/en-us/windows/wsl/use-custom-distro)

## 配布とローカル操作

配布形式の検討案は `v<semver>` タグ、macOS `.dmg`、Windows NSIS `.exe`。Tauri の universal macOS build と Linux backend を別に生成し、Windows の通常 resource として backend を同梱する。製品の最小版・動作保証を Tauri 自体の既定値だけで決めない。[macOS bundle](https://v2.tauri.app/distribute/macos-application-bundle/)、[Windows installer](https://v2.tauri.app/distribute/windows-installer/)、[resources](https://v2.tauri.app/develop/resources/)

WebView2 の既定方式は未導入時の bootstrapper ダウンロード。完全オフライン導入は初回対象外。署名・公証は未確認で、未署名の実行可否は OS ポリシーにも依存する。[WebView2](https://v2.tauri.app/distribute/windows-installer/#webview2-installation-options)、[macOS signing](https://v2.tauri.app/distribute/sign/macos/)、[Windows signing](https://v2.tauri.app/distribute/sign/windows/)

Makefile は46行、`release`・`install-skill`・`test` の3ターゲット。現在の CI は pnpm/cargo を直接実行しており、タグ配布の OS 別処理をローカル Makefile へ集約する必要はない。実際に増えるローカル処理を #21 で比較し、複数行/OS 別処理が複雑になる場合は Just 移行も選ぶ。移行時は新たな開発ツール指定、README 更新、操作一覧と1回の確認、非対話の明示スキップが必要。ランナーの置換だけでは Bash/OS 依存は解消しない。

## 実行順と未確認事項

[#18 設計](https://github.com/Gild-shogi/repo-notes/issues/18) → [#19 共通処理と backend](https://github.com/Gild-shogi/repo-notes/issues/19) → [#20 Windows UI](https://github.com/Gild-shogi/repo-notes/issues/20) → [#21 配布](https://github.com/Gild-shogi/repo-notes/issues/21) → [#22 導入検証](https://github.com/Gild-shogi/repo-notes/issues/22)。変更範囲の重なる後続は直列に進める。

未決定: 通信方式・接続保護、パスコピーと外部起動の対象、プロトコル/版不一致/再接続の契約。未確認: Rust/Tauri の Windows/macOS 製品ビルド、対象 OS の下限での導入・更新、署名設定の準備、Windows の実 WebView 表示。試作成功はこれらの検証の代用にならない。
