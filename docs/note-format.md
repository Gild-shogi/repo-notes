# ノート形式とリポジトリ識別子

エージェントや外部ツールからノートを扱うための、識別子・ファイル名・メタデータの仕様。

[アプリ紹介](../README.md) · [導入ガイド](getting-started.md) · [開発ガイド](development.md) · [ノート形式](note-format.md) · [保存と安全性](storage.md)

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
created: 2026-07-02T01:00:00Z  # 必須
tags: [architecture, onboarding]    # 任意
agent: claude-code                  # 任意
---
```

パース失敗時はビューアが H1 → ファイル名の順でタイトルにフォールバックする。

### HTML ノート（先頭コメント）

```html
<!-- repo-notes-meta {"title":"依存グラフ","created":"2026-07-02T01:00:00Z","tags":["viz"]} -->
```

先頭 2KB 以内に無い場合は `<title>` → ファイル名にフォールバック。HTML ノートは**単一ファイル自己完結の静的 HTML** として、インライン CSS と data 画像を利用できる。表示前に DOMPurify で script・link・meta・base・子フレーム等の要素と、リンク・フォーム送信先等の属性を除去する。アプリ所有の文書用 CSP を先頭に置き、権限を付与しない sandbox 付き iframe の srcDoc として表示する。JavaScript はインライン・外部とも実行せず、外部リソースの読み込み・接続の先行確立・外部遷移を禁止する。既存の JavaScript に依存するグラフやフィルタ UI は動作しない。Markdown の Mermaid は引き続き利用できる。

## 制限事項

- 外部アプリ起動はノートのパスを検証して OS に渡す。受け取った外部アプリがファイルを開くまでの間の変更は、アプリ内 I/O のディレクトリ handle では制御できない
- HTML ノートはアプリ内では編集不可（閲覧・削除のみ）。編集は外部エディタで行う
- 画像アセットのアプリ内アップロードは非対応（エージェントがスキル経由で配置する想定）
