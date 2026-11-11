# ライセンスと第三者表記

## 適用範囲

repo-notes のコード・ドキュメント・同梱スキルには [MIT License](../LICENSE) を適用する。著作権者の表記はリポジトリの公開名 `Gild-shogi` とする。アプリで作成・閲覧するユーザーのノートに、このライセンスを適用するものではない。

依存ライブラリには元のライセンスを適用する。配布時に元の著作権表示・ライセンス文を保持するため、[第三者表記](../THIRD-PARTY-NOTICES.md)を同梱する。

## 依存ライブラリの配布条件

| 対象 | 適用する条件 | 配布時の対応 |
| --- | --- | --- |
| MIT・BSD・ISC・Apache-2.0 など | 各ライセンスの著作権表示・本文・NOTICE の保持 | 第三者表記に収録 |
| Rust の `cssparser`・`cssparser-macros`・`dtoa-short`・`option-ext`・`selectors` | MPL-2.0 | 元のライセンスと、同じバージョンのソースアーカイブへのリンクを収録 |
| 開発依存の `lightningcss` とネイティブパッケージ | MPL-2.0 | 開発依存も含む一覧に収録。ツール自体を再配布する場合もソースの入手先を保持 |
| Mermaid 経由の `elkjs` | EPL-2.0 を選択 | ライセンス本文と elkjs・ELK の対応するソースの入手先を収録 |
| `dompurify` | Apache-2.0 を選択 | 元の配布物のライセンス文を収録 |
| `khroma` | MIT | package.json のライセンス欄が無いため、同梱の `license` 本文で確認 |

MPL の対象ファイルや EPL の対象プログラムを改変して配布する場合は、改変後の対応するソースも提供する。repo-notes 独自のコードには MIT を適用し、依存先を MIT に変更しない。

根拠: [Mozilla の MPL 配布 FAQ](https://www.mozilla.org/MPL/2.0/FAQ/)・[EPL-2.0 本文](https://www.eclipse.org/legal/epl-2.0/)・[elkjs 0.12.1 のリリース情報](https://github.com/kieler/elkjs/releases/tag/0.12.1)。

## 一覧の生成と確認

Node 24 と Rust を用意し、ロックファイルどおりに依存を導入して実行する。

```bash
pnpm install --frozen-lockfile
pnpm licenses:generate
pnpm licenses:check
```

`scripts/generate-licenses.ts` は、導入済み npm の直接・推移・開発・peer 依存を収録する。Cargo はロック済みの全ターゲット・ビルド・開発依存を取得する。npm の他 OS 向けなど未導入の optional dependencies は対象外。一覧は配布物の実際の構成を断定するものではなく、対象環境の依存を広めに収録したもの。

本文が配布パッケージに無い依存先は、公式リポジトリから取得した原文と固定リビジョンの出典を `scripts/license-supplements.json` に保存する。バージョン情報に VCS リビジョンが無い `tao-macros` などは、取得時の公式ソースのリビジョンを記録する。`natural-compare` は README が指定する著作者のライセンス URL の原文を保存する。ネイティブパッケージは同じバージョンの親パッケージ、`@humanfs/types` は同じ公式リポジトリの `@humanfs/core` の Apache-2.0 本文を収録する。更新時に不足があれば生成を失敗させるので、原文を確認して補足データを更新する。

依存更新後は一覧を再生成し、ライセンスの変更、追加の NOTICE、ソース提供条件を確認する。新たな配布先 OS の依存・システムライブラリや、依存に内包された別製品・フォント・画像などは配布物に応じて確認する。

## 配布物への同梱

Tauri の `bundle.resources` で `LICENSE` と `THIRD-PARTY-NOTICES.md` を同梱する。macOS のアプリでは `repo-notes.app/Contents/Resources/` に配置される。`make install-skill` でも、バイナリとともに両方のファイルをスキルの配置先へコピーする。

単体バイナリを別の経路で配布する場合も、両方のファイルを添付する。
