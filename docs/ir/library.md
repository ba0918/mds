---
$schema: ../../.mds/schemas/ir.yaml
---
# ライブラリの入口

この文書は、mds をクレートとして使う側に向けて、契約として当てにしてよい入口と、呼び出し側に残る責務を扱う。

## 要求

### REQ-049: ライブラリの入口

- 種類: algorithm
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A18, docs/decision/records/2026-09-21-mds-spec.md#A24
- 定義: TBL-010
- 検証: unit

### REQ-050: 読み書きは呼び出し側の責務

- 種類: prohibition
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A18, docs/decision/records/2026-09-21-mds-spec.md#A24
- 検証: unit

ライブラリは、`文書`と`スキーマ`のファイルを読まず、URL の`スキーマ`も取得しない。`スキーマ`の位置を決めるところまでを行い、読み書きは呼び出し側に残す。

### REQ-051: 列挙に無い公開項目

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A24
- 検証: review
- 確かめ方: TBL-010 に無い公開項目が、依存するクレートから使われていないことを確認する。列挙に無い公開項目は実装の都合であり、契約ではない

mds は常に、TBL-010 が列挙した入口だけを依存するクレートへの契約として約束する。

## 決定表

### TBL-010: ライブラリの入口

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A18, docs/decision/records/2026-09-21-mds-spec.md#A24

| 入口 | 何をするか | 返すもの |
|---|---|---|
| frontmatter_schema | `文書`の文字列から "$schema" の参照を読む | 参照、または無し。`frontmatter`が壊れていれば誤り |
| resolve_schema | `文書`の位置と参照から`スキーマ`の位置を決める | ファイルのパス、または URL |
| parse_schema | `スキーマ`の YAML を読む | `スキーマ`、または形の違反 |
| Document::parse | `文書`の文字列を読む | `文書`の構造 |
| validate | `スキーマ`と`文書`を突き合わせる | `指摘`の並び |
| extract_values | `スキーマ`の`抽出`に沿って値を組み立てる | `配置パス`に沿った JSON |

## 具体例

```gherkin
@id=EX-015 @about=REQ-049 @source=docs/decision/records/2026-09-21-mds-spec.md#A18
Scenario: 依存するクレートが入口だけで一通りを通せる
  Given `スキーマ`を宣言した`文書`の文字列がある
  When TBL-010 の入口を順に呼ぶ
  Then `指摘`の並びと`抽出`の値の両方が得られる

@id=EX-016 @about=REQ-050 @source=docs/decision/records/2026-09-21-mds-spec.md#A24
Scenario: URL のスキーマは位置だけを返す
  Given URL の`スキーマ`を宣言した`文書`の文字列がある
  When resolve_schema を呼ぶ
  Then URL が返り、取得は行われない
```
