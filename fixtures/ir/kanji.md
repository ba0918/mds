---
$schema: ../.mds/schemas/ir.yaml
---
# 印の仕様

この文書は、印の構文と意味を正規化した仕様を扱う。

## 要求

### REQ-001: 印の構文

- 種類: algorithm
- 出典: docs/decision/2026-09-01-mark.md
- 検証: unit
- 定義: REQ-002

印は `@kotowari[ID, ...]` の形で書く。

### REQ-002: 印の意味

- 種類: ubiquitous
- 出典: docs/decision/2026-09-01-mark.md
- 検証: review

印が指す先は ID で解決される。

## 決定表

### TBL-001: 印と検査の対応

- 出典: docs/decision/2026-09-01-mark.md

印は検査の種類に対応する。

## 性質

### PROP-001: 印は検査に結び付く

- 出典: docs/decision/2026-09-01-mark.md

印は常に検査の対象を指す。

## 用語集

| 用語 | 意味 | 出典 |
|---|---|---|
| 印 | テストの印 | docs/decision/2026-09-01-mark.md |
| スキーマ | 文書の書式 | docs/spec/mds.md |

## 具体例

```gherkin
@id=EX-001
@about=REQ-001
@source=docs/decision/2026-09-01-mark.md
Scenario: 印を書く
Given 文書に印を書く
Then 印が ID に解決される
```