---
$schema: ../.mds/schemas/decision.yaml
---
# 判断の記録

この文書は、判断の記録（決定の行 + 子の superseded_by）の書式を mds で検査するための抜粋である。

## Agreements

- 決定1 これは実在しないダミーの決定である。判断の記録の書式を検査するためにだけ存在する
  - superseded_by: [example-log.md#S1](./example-log.md#S1)（新しい記録は書かず、既存は残す）
- 決定2 これは実在しないダミーの決定である。既定の置き場所をこの書式で表す