# Windows/WSL の配置・接続・配布方式

状態: 設計合意済み。以下の5点は実装前の確認で承認された。比較の試作と製品の動作確認は区別する。対象は [Epic #2](https://github.com/Gild-shogi/repo-notes/issues/2)、設計作業は [#18](https://github.com/Gild-shogi/repo-notes/issues/18)。

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
4. Windows の外部編集は VS Code + WSL 拡張、場所の表示は Explorer、パスと Claude 用コピーは Linux パス。VS Code 未導入・必要な拡張不足・外部操作と選択ユーザーの権限不一致は必要操作を表示する。アプリ内閲覧/編集は使える。任意ファイルを Windows の既定関連付けへ渡して起動しない。
5. Windows が `wsl.exe` で選択したバックエンドを起動し、専用の標準入出力を常駐接続に使う。ポートや HTTP API は追加しない。画像も同じ接続から Windows の検証済み asset 経路へ渡す。接続時に対象環境と UI/backend の版を照合し、不一致はノート操作を停止する。切断時は下書きを保持して再接続を案内し、結果不明の保存を自動再送しない。終了対象はアプリが所有する backend だけとする。

## 通信方式の比較

試作は [backend.py](../experiments/wsl-transport/backend.py) と [benchmark.py](../experiments/wsl-transport/benchmark.py)。記録は [Linux の全試行](../experiments/wsl-transport/results.json)、[Windows→WSL の全試行](../experiments/wsl-transport/windows-results.json)。実ノートを使わず、一時 fixture を backend 起動時にメモリへ読み込んで測定した。

| 測定対象 | 標準入出力の中央値 / p95 (ms) | HTTP の中央値 / p95 (ms) |
|---|---:|---:|
| Linux 内・約1KiB要求 | 0.0967 / 0.1502 | 0.2202 / 0.3013 |
| Linux 内・1MiB asset | 13.7262 / 14.4151 | 0.3283 / 0.5577 |
| Windows→WSL・約1KiB要求 | 0.9250 / 1.8686 | 1.6325 / 2.3199 |
| Windows→WSL・1MiB asset | 55.8347 / 91.4108 | 14.7367 / 28.3112 |
| Windows→稼働中 WSL の起動 | 172.6105 / 240.2775 | 181.1535 / 228.9666 |

各方式5試行、各試行で要求10回と各 asset 1回をウォームアップ後、要求100回・256KiB/1MiB asset 各10回・通知10回。要求は各方式500件、各サイズ asset は50件。方式の実行順を交互にし、HTTP は持続接続/TCP_NODELAY、Windows HTTP client は proxy 無効。試行別中央値・最小/最大も JSON に残す。

値・ID・asset の長さ/SHA256・通知到着を確認した。要求と画像転送の測定区間は送信から JSON/base64 の復号までで、値/ハッシュ検証は区間外。標準入出力の asset 応答本文は base64/JSON により約1.333倍、HTTP は binary の元サイズ。ヘッダー/TCP と標準入出力の要求サイズは転送量の集計外。

数値は Python/PowerShell/.NET/JSON/base64 の実装を含む試作の観測値。CPU・pipe・VM・encoding の律速資源を分離しておらず、方式そのものや Rust 製品の性能とは扱わない。画像転送は HTTP が有利だったが、常駐の要求/通知/終了と private pipe の管理をまとめられる標準入出力を採用した。実 UI の表示時間や大きな asset 転送中の応答性は #20/#22 で別途確認する。

スクリプトを保存先へ移して Linux/Windows の両方で再実行し、表と JSON はこの再実行の結果を記録した。初回の Windows 要求中央値は標準入出力0.7711ms/HTTP1.2399ms、再実行は0.9250ms/1.6325msで、値に実行時のばらつきがある。Linux は Python 3.14.6、x86_64、Windows から起動した Ubuntu 側は Python 3.12.3。Windows は PowerShell 5.1、Windows x64 build 26200、WSL 3.0.3 の稼働中 Ubuntu WSL2。Linux 内の測定は Windows/WSL の起動や転送を含まない。Windows 通知は別の `wsl.exe touch` の起動も含むため、Linux の5ms polling や通知伝送単体と比較しない。VM の cold boot、実ファイル I/O、Tauri/WebView、製品の保存契約・版照合・署名は試作の対象外。

Linux で所有 child の強制終了を検出し、stdin EOF で両方式の正常終了を確認した。HTTP は未認証/誤 token を403にし、同じ token の別 client は200になることを確認した。Windows は未認証403と明示 shutdown 10回の正常終了を確認した。Windows の強制終了/WSL 再起動からの復旧と、同一 OS ユーザーの悪意あるコードからの隔離はこの結果で証明しない。

初回の Python HTTP は Nagle/delayed ACK の影響で約44msとなったため、TCP_NODELAY を付けて同じ負荷を再測定し、初回値を表に混ぜていない。Windows の試作 launcher は空白のない固定 distro 名/パスに限定する。任意名を扱う製品の引数処理に流用しない。

再実行はこのディレクトリの `benchmark.py` を Python 3 で実行する。Linux で socket を許可し、Windows 試験は WSL 相互運用と PowerShell 5.1 が必要。結果 JSON は上書きされ、一時 fixture と起動 backend は終了/削除する。生成 PS1 とログは Git 対象外。PowerShell の ExecutionPolicy 指定はこの試作プロセスだけに適用し、システム設定を変更しない。

```bash
python3 docs/experiments/wsl-transport/benchmark.py
python3 docs/experiments/wsl-transport/benchmark.py --windows Ubuntu
```

公式資料上、`wsl.exe --distribution <name> --user <name> --exec <command>` で対象と実行先を固定できる。`--exec` は既定シェルを使わないため、シェルの profile に設定した `REPO_NOTES_DIR` の読込を前提にしない。[WSL 基本コマンド](https://learn.microsoft.com/en-us/windows/wsl/basic-commands)、[WSL release notes](https://learn.microsoft.com/en-us/windows/wsl/release-notes)、[Bash startup files](https://www.gnu.org/s/bash/manual/bash.html#Bash-Startup-Files)

HTTP は NAT の Windows→WSL localhost、mirrored networking でも利用できるが、WSL2 ディストリビューションは network namespace を共有する。したがって loopback や動的ポートだけで呼出元・対象 distro を認証できるとは扱わない。後半は資料からの推論。[WSL networking](https://learn.microsoft.com/en-us/windows/wsl/networking)、[WSL2 の構成](https://learn.microsoft.com/en-us/windows/wsl/about)

## 配置方式の比較

| 候補 | 利点 | 負担・残る課題 | 状態 |
|---|---|---|---|
| エージェントが使う既存 WSL2 | 同じユーザー/保存先でノートと排他ファイルを共有できる | distro/ユーザー/保存先の明示と、再起動後の再接続が必要 | 採用承認済み |
| アプリ専用 distro | アプリが環境構成を管理できる | import、一般ユーザー設定、環境更新が必要。既存 distro のノートとの共有問題は残る | 初回対象外 |

専用 distro は `wsl --import` で作れるが、その作成を実験として行ってはいない。既定 root ユーザーの扱いも必要になる。[custom distro import](https://learn.microsoft.com/en-us/windows/wsl/use-custom-distro)

## 外部操作の境界

Linux 側で正規化・許可範囲を検証したファイルを VS Code WSL の編集対象として渡す。場所表示は既存 opener の Windows API を使い、UNC の既定関連付けでファイルを実行しない。Windows からの WSL ファイルアクセスは distro の既定ユーザー権限となるため、接続の選択ユーザーと一致する条件を確認する。不一致時に無断でユーザーを変えない。[VS Code WSL](https://code.visualstudio.com/docs/remote/wsl#_from-the-windows-command-prompt)、[WSL ファイル権限](https://learn.microsoft.com/en-us/windows/wsl/file-permissions#accessing-linux-files-from-windows-using-wsl)、[Explorer の項目表示 API](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shopenfolderandselectitems)

## 共通処理と接続の実装境界

単一 crate と feature/別 binary の構成から始め、store/model/parse の知識と保存契約を共有する。Tauri State/event/HTTP と OS 操作は外側の adapter に置き、WSL backend が GUI ライブラリを要求しないことをビルドで確認する。監視結果の通知先は必要な callback 等で分け、形式的な Repository trait や層を増やさない。

接続の要求 ID・応答・変更通知を型で区別し、stdout を通信専用、診断を stderr とする。起動した同梱成果物と専用 pipe を接続対象にし、対象保存先と版を接続開始時に照合する。同一ユーザーが動かす任意コードの隔離機構は追加しない。版の混在中に操作せず、既存 raw 比較により結果不明の保存後も読み直して状態を確認する。自動マージや無条件の保存再送を足さない。

## 配布とローカル操作

配布形式の検討案は `v<semver>` タグ、macOS `.dmg`、Windows NSIS `.exe`。Tauri の universal macOS build と Linux backend を別に生成し、Windows の通常 resource として backend を同梱する。製品の最小版・動作保証を Tauri 自体の既定値だけで決めない。[macOS bundle](https://v2.tauri.app/distribute/macos-application-bundle/)、[Windows installer](https://v2.tauri.app/distribute/windows-installer/)、[resources](https://v2.tauri.app/develop/resources/)

WebView2 の既定方式は未導入時の bootstrapper ダウンロード。完全オフライン導入は初回対象外。署名・公証は未確認で、未署名の実行可否は OS ポリシーにも依存する。[WebView2](https://v2.tauri.app/distribute/windows-installer/#webview2-installation-options)、[macOS signing](https://v2.tauri.app/distribute/sign/macos/)、[Windows signing](https://v2.tauri.app/distribute/sign/windows/)

Makefile は46行、`release`・`install-skill`・`test` の3ターゲット。現在の CI は pnpm/cargo を直接実行しており、タグ配布の OS 別処理をローカル Makefile へ集約する必要はない。実際に増えるローカル処理を #21 で比較し、複数行/OS 別処理が複雑になる場合は Just 移行も選ぶ。移行時は新たな開発ツール指定、README 更新、操作一覧と1回の確認、非対話の明示スキップが必要。ランナーの置換だけでは Bash/OS 依存は解消しない。

## 実行順と未確認事項

[#18 設計](https://github.com/Gild-shogi/repo-notes/issues/18) → [#19 共通処理と backend](https://github.com/Gild-shogi/repo-notes/issues/19) → [#20 Windows UI](https://github.com/Gild-shogi/repo-notes/issues/20) → [#21 配布](https://github.com/Gild-shogi/repo-notes/issues/21) → [#22 導入検証](https://github.com/Gild-shogi/repo-notes/issues/22)。変更範囲の重なる後続は直列に進める。

設計合意は上記5点。製品の実装・検証は後続の子で行う。未確認: Rust/Tauri の Windows/macOS 製品ビルド、対象 OS の下限での導入・更新、署名設定の準備、Windows の実 WebView 表示。試作成功はこれらの検証の代用にならない。
