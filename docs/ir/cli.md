---
$schema: ../../.mds/schemas/ir.yaml
---
# CLI と結果の出し方

この文書は、mds が持つコマンド、終了コード、出力の形、指摘の形、検査を行えないときの振る舞いを扱う。

## 要求

### REQ-005: コマンドの一覧

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A1, docs/decision/records/2026-09-21-mds-spec.md#A16
- 検証: unit

mds は常に、検査の "check"、素の構文木の "ast"、抽出の "values"、版の "--version" の4つを受ける。

### REQ-006: 終了コードの決め方

- 種類: algorithm
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A15
- 定義: TBL-001
- 検証: unit

### REQ-007: 出力の形

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A16
- 検証: unit

mds は常に、出力の形を "--format" で受け、人間向けの "text" と機械向けの "json" の2つから選ばせる。

### REQ-008: 指摘の形

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A17
- 検証: unit

mds は常に、1件の`指摘`を、種類、深刻度、`文書`のパス、行番号、詳細の5つで表し、行を持たない`指摘`では行番号を省く。

### REQ-009: 検査を行えないときは停止する

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A8, docs/decision/records/2026-09-21-mds-spec.md#P1
- 検証: unit

`スキーマ`を解決できないとき、または`frontmatter`が壊れているとき、mds は部分的な結果を出さずに`停止`する。

### REQ-010: ディレクトリの検査

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A1
- 検証: unit

検査の対象がディレクトリのとき、mds はその下の`スキーマ`を宣言した`文書`だけを集めて検査する。

### REQ-042: 停止の理由

- 種類: algorithm
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A15, docs/decision/records/2026-09-21-mds-spec.md#P1
- 定義: TBL-009
- 検証: unit

### REQ-043: 停止の知らせ方

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A15, docs/decision/records/2026-09-21-mds-spec.md#A16
- 検証: unit

mds が`停止`するとき、理由の名前と説明を並べた1行だけを標準エラーに出し、`指摘`は1件も出さない。

### REQ-044: ディレクトリ検査で辿らないもの

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A21
- 検証: unit

mds は常に、ディレクトリの検査で、名前が "." で始まるディレクトリ、`スキーマ`とキャッシュの置き場、シンボリックリンク、拡張子が ".md" でないファイルを辿らない。

## 決定表

### TBL-001: 終了コード

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A15

| 順 | 条件 | 終了コード |
|---|---|---|
| 1 | 検査を行えなかった（`スキーマ`を解決できない、`frontmatter`が壊れている、`スキーマ`が形に合わない） | 2 |
| 2 | 1 に当たらず、`指摘`が1件以上ある | 1 |
| 3 | 1 にも 2 にも当たらない | 0 |

### TBL-009: 停止の理由

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A15, docs/decision/records/2026-09-21-mds-spec.md#P1

| 理由 | いつ |
|---|---|
| スキーマが見つからない | 参照先の`スキーマ`が無い、URL の取得に失敗した、または "$schema" の無い`文書`を対象に指定した |
| スキーマが形に合わない | `スキーマ`の YAML が読めない、または`規則種別`の形に反する |
| frontmatter が壊れている | `frontmatter`が壊れた YAML である、または "$schema" の値が文字列でないか空である |
| 文書が読めない | `文書`のファイルを読めない |
| 引数の誤り | 知らないフラグ、または受けない "--format" の値 |

### TBL-002: 指摘の分類

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A17, docs/decision/records/2026-09-21-mds-spec.md#A2

| 分類 | 何を見つけるか |
|---|---|
| 欠落 | 必須の`ノード`が無い |
| 形の違反 | 値が正規表現や許可リストに合わない、`表`のヘッダが合わない |
| 出現回数 | `出現回数`の下限を下回る、上限を超える |
| 閉じた世界 | 宣言していない見出しと行がある |

## 性質

### PROP-002: 抽出は検査の合否から独立している

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A4

`抽出`の結果は、同じ`文書`と同じ`スキーマ`であれば、検査で`指摘`が出たかどうかによって変わらない。

## 具体例

```gherkin
@id=EX-003 @about=REQ-006 @source=docs/decision/records/2026-09-21-mds-spec.md#A15
Scenario: 指摘の無い文書は終了コード 0 で終わる
  Given `スキーマ`をすべて満たす`文書`がある
  When "mds check" を実行する
  Then 終了コードは 0 である
  And 何も出力しない

@id=EX-004 @about=REQ-009 @source=docs/decision/records/2026-09-21-mds-spec.md#A8
Scenario: frontmatter が壊れていれば停止する
  Given `frontmatter`が YAML のマッピングでない`文書`がある
  When "mds check" を実行する
  Then 終了コードは 2 である
  And `指摘`は出力しない
```
