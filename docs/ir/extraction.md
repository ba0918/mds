---
$schema: ../../.mds/schemas/ir.yaml
---
# 抽出と素の構文木

この文書は、スキーマが宣言した抽出規則から値を組み立てる振る舞いと、検査と別に素の構文木を出す振る舞いを扱う。

## 要求

### REQ-035: 抽出の書式

- 種類: algorithm
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A4
- 定義: TBL-008
- 検証: unit

### REQ-036: 配置パス

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A4
- 検証: unit

mds は常に、`抽出`した値を`配置パス`のドット区切りの名前に沿って入れ子にして置く。

### REQ-037: 値の型は文字列

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A4
- 検証: property

mds は常に、`抽出`した値を文字列として出し、日付や数値への型変換をしない。

### REQ-038: 欠けた値はキーを出さない

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A4
- 検証: unit

`抽出`の対象の`ノード`が`文書`に無いとき、mds はその`配置パス`のキーを出力に出さない。

### REQ-039: 項目の内側には抽出を宣言できない

- 種類: prohibition
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A4, docs/decision/records/2026-09-21-mds-spec.md#A12
- 検証: unit

mds は、`項目`の内側の`フィールド行`、`文`、`箇条書き`、`表`、`コードブロック`に`抽出`を宣言した`スキーマ`を受け付けず、`停止`する。

### REQ-040: 素の構文木

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A19
- 検証: review
- 確かめ方: `mds ast` の出力を mdast（unist）の仕様と突き合わせ、ノードの "type" の名前、"children" の入れ子、インライン要素の種別が準拠していることを確認する。準拠は外部の仕様との一致なので、自分のテストでは見られない

mds は常に、素の構文木を mdast に沿った JSON で出し、インライン要素まで含め、位置情報は含めない。

## 決定表

### TBL-008: 抽出の形

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A4, docs/decision/records/2026-09-21-mds-spec.md#A13

| `ノード` | `抽出`の形 |
|---|---|
| `題名` | 見出しの文字列、または正規表現の名前付きキャプチャ |
| `フィールド行` | 値の文字列。区切り文字を宣言すれば文字列の配列 |
| `文` | 本文の文字列 |
| `節` | `文`と`箇条書き`だけをつないだ本文の文字列 |
| `項目` | 見出しと本文をつないだ1つの文字列。`表`と`コードブロック`は含めない |
| `箇条書き` | 元の行を保った文字列の配列 |
| `表` | 文書のヘッダ行をキーにしたオブジェクトの配列 |
| `コードブロック` | ブロック全体の文字列 |

## 性質

### PROP-007: 抽出は閉じた世界の設定に依らない

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A2, docs/decision/records/2026-09-21-mds-spec.md#A4

同じ`文書`と同じ`スキーマ`であれば、`抽出`の結果は`閉じた世界`と`開いた世界`のどちらで検査しても変わらない。

## 具体例

```gherkin
@id=EX-013 @about=REQ-036 @source=docs/decision/records/2026-09-21-mds-spec.md#A4
Scenario: 配置パスに沿って入れ子の JSON を出す
  Given ドットを含む`配置パス`を宣言した`スキーマ`がある
  When "mds values --format json" を実行する
  Then 値はドットで区切った名前の入れ子として出る

@id=EX-014 @about=REQ-039 @source=docs/decision/records/2026-09-21-mds-spec.md#A12
Scenario: 項目の内側の抽出は停止する
  Given `項目`の中の`表`に`抽出`を宣言した`スキーマ`がある
  When "mds check" を実行する
  Then 終了コードは 2 である
```
