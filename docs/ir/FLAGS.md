---
$schema: ../../.mds/schemas/flags.yaml
---
# 問題の記録

"docs/spec/mds.md" から IR へ写す過程で、写しきれていないと分かっている箇所を記録する。

## 問題の記録

### FLAG-001: フィールド行の並び順を写していない

- 種類: gap
- 関係: REQ-028, REQ-029
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A10

"docs/spec/mds.md" の R8 は、フィールド行の一覧に並び順の強制を宣言でき、違反を指摘にすると定めている。IR にはこの規則を写していない。

### FLAG-002: 停止の理由を個別の要求に落としていない

- 種類: gap
- 関係: REQ-009, REQ-014, REQ-018
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A15

"docs/spec/mds.md" の R19 は停止の理由を5つに分けているが、IR では停止という振る舞いだけを写し、理由ごとの要求にはしていない。

### FLAG-003: ディレクトリ検査の除外を写していない

- 種類: gap
- 関係: REQ-010
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A1

"docs/spec/mds.md" の R20 は、ディレクトリを辿るときに隠しディレクトリ、スキーマの置き場、シンボリックリンク、Markdown 以外の拡張子を除くと定めている。IR にはこの除外を写していない。

### FLAG-004: 抽出の細部を決定表に畳んでいる

- 種類: gap
- 関係: REQ-035, TBL-008
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A4

"docs/spec/mds.md" の R16 は、区切り文字と出現回数の併用、継続段落の連結の順序まで定めている。IR の決定表は規則種別ごとの形までしか写していない。
