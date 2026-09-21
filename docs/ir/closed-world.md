---
$schema: ../../.mds/schemas/ir.yaml
---
# 閉じた世界と開いた世界

この文書は、宣言していない見出しと行をどう扱うか、どこまで緩められるかを扱う。

## 要求

### REQ-001: 閉じた世界が既定

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A2
- 検証: unit

mds は常に、`スキーマ`に宣言していない見出しと行を`指摘`にする。

### REQ-002: 開いた世界に緩める

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A2
- 検証: unit

`スキーマ`が "open: true" を宣言したとき、または検査に "--open" を付けたとき、mds は宣言していない構造と、その内側のすべての行を許す。

### REQ-003: 宣言済みの構造の中は緩めない

- 種類: prohibition
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A2
- 検証: unit

mds は、`開いた世界`でも、宣言済みの`前置部`、`節`、`項目`の中に足された未宣言の構造と行を許さない。

### REQ-004: 欠落と形の違反は緩めない

- 種類: prohibition
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A2
- 検証: unit

mds は、`開いた世界`でも、必須の`ノード`の欠落と、`出現回数`や形の違反を許さない。

## 性質

### PROP-001: 緩める方向にしか働かない

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A2

`開いた世界`で出る`指摘`の集まりは、同じ`文書`を`閉じた世界`で検査したときに出る`指摘`の集まりに含まれる。

## 具体例

```gherkin
@id=EX-001 @about=REQ-002 @source=docs/decision/records/2026-09-21-mds-spec.md#A2
Scenario: 開いた世界では未宣言の節を許す
  Given `スキーマ`に宣言していない`節`を持つ`文書`がある
  When "mds check --open" を実行する
  Then その`節`の`指摘`は出ない

@id=EX-002 @about=REQ-003 @source=docs/decision/records/2026-09-21-mds-spec.md#A2
Scenario: 開いた世界でも宣言済みの節の中の未宣言の行は誤りになる
  Given 宣言済みの`節`の中に、宣言していない行を持つ`文書`がある
  When "mds check --open" を実行する
  Then その行の`指摘`が出る
```
