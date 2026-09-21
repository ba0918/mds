---
$schema: ../../.mds/schemas/context.yaml
---
# 用語集

mds の仕様 IR で使う用語を置く。意味が一般の用法と違うものだけを載せる。

| 用語 | 意味 | 出典 |
|---|---|---|
| スキーマ | 文書の書式（構造と制約）と抽出規則を宣言する YAML ファイル | docs/decision/records/2026-09-21-mds-spec.md#A1 |
| 文書 | mds が検査と抽出の対象にする Markdown ファイル | docs/decision/records/2026-09-21-mds-spec.md#A1 |
| frontmatter | 文書の先頭にある "---" で挟んだ YAML ブロック | docs/decision/records/2026-09-21-mds-spec.md#A1 |
| 閉じた世界 | スキーマに宣言していない見出しと行を誤りとする検証の流儀 | docs/decision/records/2026-09-21-mds-spec.md#A2 |
| 開いた世界 | 宣言していない構造とその内側の行を許す検証の流儀 | docs/decision/records/2026-09-21-mds-spec.md#A2 |
| 規則種別 | スキーマが文書の構造を記述するノードの種類 | docs/decision/records/2026-09-21-mds-spec.md#A3 |
| ノード | スキーマの木の1要素。検証と抽出の単位 | docs/decision/records/2026-09-21-mds-spec.md#A3 |
| 題名 | 文書の深さ1の見出し | docs/decision/records/2026-09-21-mds-spec.md#A5 |
| 前置部 | 題名の後、最初の節より前の部分 | docs/decision/records/2026-09-21-mds-spec.md#A5 |
| 節 | 深さ2の見出し | docs/decision/records/2026-09-21-mds-spec.md#A5 |
| 項目 | ID と名前を持つ深さ3の見出し | docs/decision/records/2026-09-21-mds-spec.md#A5 |
| フィールド行 | 一覧のマーカーに続く「名前と値」の形の行で、名前がスキーマの宣言と一致するもの | docs/decision/records/2026-09-21-mds-spec.md#A10 |
| 箇条書き | 一覧のマーカーで始まり、フィールド行でない一覧の行 | docs/decision/records/2026-09-21-mds-spec.md#A10 |
| 文 | 見出しや前置部の下にある、一覧でも表でもない空でない行 | docs/decision/records/2026-09-21-mds-spec.md#A3 |
| 継続段落 | 一覧の行の後に空行で区切って続く、その行の子である段落 | docs/decision/records/2026-09-21-mds-spec.md#A11 |
| 表 | Markdown の表。ヘッダのセル列と列数を指定できる | docs/decision/records/2026-09-21-mds-spec.md#A13 |
| コードブロック | フェンスで囲んだブロック。言語と行ごとの規則を指定できる | docs/decision/records/2026-09-21-mds-spec.md#A12 |
| 抽出 | スキーマが各ノードに宣言する、値を取り出す規則 | docs/decision/records/2026-09-21-mds-spec.md#A4 |
| 配置パス | 抽出した値を置く場所を指すドット区切りの名前 | docs/decision/records/2026-09-21-mds-spec.md#A4 |
| 指摘 | 検査が出す1件の結果 | docs/decision/records/2026-09-21-mds-spec.md#A17 |
| 停止 | 検査を行えないときに終了コード 2 で終わること | docs/decision/records/2026-09-21-mds-spec.md#A15 |
| 基準のディレクトリ | カレントディレクトリから上に向かって探し、最初に見つかった ".mds/" のあるディレクトリ | docs/decision/records/2026-09-21-mds-spec.md#A14 |
| 出現回数 | ノードが現れてよい個数の下限と上限 | docs/decision/records/2026-09-21-mds-spec.md#A3 |
| 条件付き規則 | あるフィールド行の値に応じて、別の規則の適用を切り替える条件節 | docs/decision/records/2026-09-21-mds-spec.md#A3 |
