---
$schema: ../../.mds/schemas/ir.yaml
---
# 文書の骨格

この文書は、題名、前置部、節、項目という文書の骨格を、スキーマがどう検証するかを扱う。

## 要求

### REQ-022: 題名は1つ

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A5
- 検証: unit

mds は常に、`文書`が`題名`をちょうど1つ持つことを求め、無いときと2つ以上あるときを`指摘`にする。

### REQ-023: 前置部の範囲

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A5
- 検証: unit

mds は常に、`題名`の後から最初の`節`の前までを`前置部`として読み、そこに`フィールド行`、`文`、`箇条書き`を宣言させる。

### REQ-024: 節の名前

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A5
- 検証: unit

mds は常に、`節`を見出しの文字で見分け、`スキーマ`が宣言した名前と一致しない`節`を`指摘`にする。

### REQ-025: 項目の見出しの形

- 種類: algorithm
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A5
- 定義: TBL-006
- 検証: unit

### REQ-026: 深すぎる見出し

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A5
- 検証: unit

深さ4以上の見出しがあるとき、mds はその見出しを`指摘`にする。

### REQ-027: 宣言していない項目

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A2, docs/decision/records/2026-09-21-mds-spec.md#A5
- 検証: unit

`項目`を宣言していない`節`の中に深さ3の見出しがあるとき、mds はその見出しと、その内側の行を`指摘`にする。`開いた世界`でも同じである。

## 決定表

### TBL-006: 項目の見出しの読み方

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A5

| 順 | 見出しの形 | 読み方 |
|---|---|---|
| 1 | 区切りのコロンが無い | ID の形に合わない`指摘` |
| 2 | コロンがあり、その前が`スキーマ`の正規表現に合う | ID と名前として読む |
| 3 | コロンがあり、その前が正規表現に合わない | ID の形に合わない`指摘` |

## 性質

### PROP-005: 骨格の深さは3段で閉じている

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A5

`スキーマ`が宣言できる見出しの深さは、`題名`、`節`、`項目`の3段だけであり、`項目`の下にさらに見出しの`ノード`を宣言する手段は無い。

## 具体例

```gherkin
@id=EX-009 @about=REQ-025 @source=docs/decision/records/2026-09-21-mds-spec.md#A5
Scenario: 形に合わない項目の見出しは誤りになる
  Given `項目`の ID の正規表現を宣言した`スキーマ`がある
  When 正規表現に合わない ID を持つ`文書`で "mds check" を実行する
  Then ID の形の`指摘`が出る

@id=EX-010 @about=REQ-022 @source=docs/decision/records/2026-09-21-mds-spec.md#A5
Scenario: 題名が2つある文書は誤りになる
  Given 深さ1の見出しを2つ持つ`文書`がある
  When "mds check" を実行する
  Then `題名`が複数ある`指摘`が出る
```
