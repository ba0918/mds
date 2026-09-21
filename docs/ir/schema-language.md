---
$schema: ../../.mds/schemas/ir.yaml
---
# スキーマ言語の骨格

この文書は、スキーマがどの規則種別を持つか、出現回数と条件付き規則をどう書くかを扱う。

## 要求

### REQ-016: スキーマの形

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A3, docs/decision/records/2026-09-21-mds-spec.md#A5
- 検証: unit

mds は常に、`スキーマ`を YAML のマッピングとして読み、`題名`、`前置部`、`節`の3つを根の`ノード`として受ける。

### REQ-017: 規則種別の一覧

- 種類: algorithm
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A3, docs/decision/records/2026-09-21-mds-spec.md#A5
- 定義: TBL-004
- 検証: unit

### REQ-018: 知らないキー

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#P1
- 検証: unit

`スキーマ`に規則種別が受けないキーがあるとき、mds は検査を行わずに`停止`する。

### REQ-019: 出現回数の書き方

- 種類: algorithm
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A3
- 定義: TBL-005
- 検証: unit

### REQ-020: 条件付き規則

- 種類: state_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A3
- 検証: unit

`条件付き規則`の条件が真である間、mds はそれを添えた制約を適用し、偽である間は適用しない。

### REQ-021: 条件が参照するフィールド行の探索

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A3
- 検証: unit

mds は常に、`条件付き規則`が参照する`フィールド行`を同じ`ノード`の下だけから探し、見つからないときは等しい条件を偽、等しくない条件を真として扱う。

## 決定表

### TBL-004: 規則種別

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A3, docs/decision/records/2026-09-21-mds-spec.md#A5

| 規則種別 | 何を検証するか | 置ける場所 |
|---|---|---|
| `題名` | 深さ1の見出し | `スキーマ`の根 |
| `前置部` | `題名`の後、最初の`節`より前の部分 | `スキーマ`の根 |
| `節` | 深さ2の見出し | `スキーマ`の根 |
| `項目` | 深さ3の見出し | `節`の下 |
| `フィールド行` | 名前と値の形の一覧の行 | `前置部`、`節`、`項目`、`箇条書き`の子 |
| `文` | 一覧でも`表`でもない空でない行 | `前置部`、`節`、`項目` |
| `箇条書き` | `フィールド行`でない一覧の行 | `前置部`、`節`、`項目` |
| `表` | Markdown の表 | `節`、`項目` |
| `コードブロック` | フェンスで囲んだブロック | `節`、`項目` |

### TBL-005: 出現回数の書き方

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A3

| 書き方 | 意味 |
|---|---|
| 既定（何も書かない） | ちょうど1個 |
| 必須の宣言を偽にする | 0個か1個 |
| 下限だけを書く | その数以上 |
| 上限だけを書く | 0個からその数まで |
| 下限と上限を書く | その範囲 |

## 性質

### PROP-004: 出現回数の宣言が抽出の形を決める

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A4

`出現回数`の範囲を宣言した`ノード`の`抽出`は、値が1件でも配列になる。範囲を宣言しない`ノード`の`抽出`は単一の値になる。

## 具体例

```gherkin
@id=EX-007 @about=REQ-020 @source=docs/decision/records/2026-09-21-mds-spec.md#A3
Scenario: 条件が真のときだけ必須になる
  Given 別の`フィールド行`の値が特定の値のときだけ必須になる`フィールド行`を宣言した`スキーマ`がある
  When 条件を満たす`文書`から、その`フィールド行`を消して "mds check" を実行する
  Then 欠落の`指摘`が出る

@id=EX-008 @about=REQ-018 @source=docs/decision/records/2026-09-21-mds-spec.md#P1
Scenario: 知らないキーのあるスキーマは停止する
  Given 規則種別が受けないキーを書いた`スキーマ`がある
  When "mds check" を実行する
  Then 終了コードは 2 である
```
