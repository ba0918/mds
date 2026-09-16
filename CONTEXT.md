# 用語集

| 用語 | 意味 | 出典 |
|---|---|---|
| スキーマ | 文書の書式（構造と制約）と抽出規則を宣言する YAML ファイル | docs/spec/mds.md#用語 |
| 文書 | mds が検査・抽出の対象にする Markdown ファイル | docs/spec/mds.md#用語 |
| frontmatter | 文書の先頭にある `---` で挟んだ YAML ブロック | docs/spec/mds.md#用語 |
| 閉じた世界 | スキーマに宣言していない見出し・行を誤りとする検証の流儀。mds の既定 | docs/spec/mds.md#用語 |
| 規則種別 | スキーマが文書の構造を記述するノードの種類 | docs/spec/mds.md#用語 |
| ノード | スキーマの木の1要素。検証と抽出の単位 | docs/spec/mds.md#用語 |
| 題名 | 文書の `# ` 見出し。深さ1 | docs/spec/mds.md#R4 |
| 前置部 | 題名の後、最初の `##` より前の部分。フィールド行、文、箇条書きを置ける | docs/spec/mds.md#R5 |
| 節 | `## ` 見出し。深さ2 | docs/spec/mds.md#R6 |
| 項目 | `### ID: 名前` 見出し。深さ3 | docs/spec/mds.md#R7 |
| フィールド行 | `- 名前: 値` の形の行 | docs/spec/mds.md#用語 |
| 箇条書き | `- ` で始まり `名前: 値` の形でない一覧行 | docs/spec/mds.md#用語 |
| 文 | 見出しや前置部の下にある、一覧でも表でもない空でない行 | docs/spec/mds.md#用語 |
| 表 | Markdown の表。ヘッダのセル列と列数を指定できる | docs/spec/mds.md#R11 |
| コードブロック | フェンスで囲んだブロック。言語と行ごとの規則を指定できる | docs/spec/mds.md#R12 |
| 抽出 | スキーマが各ノードに `extract` で宣言する、値を取り出す規則 | docs/spec/mds.md#用語 |
| 配置パス | `values` と `ast --schema` が値を置く場所を指すドット区切りの名前 | docs/spec/mds.md#用語 |
| 指摘 | `check` が出す1件の結果 | docs/spec/mds.md#用語 |
| 停止 | 検査を行えないときに終了コード2で終わること | docs/spec/mds.md#用語 |
| 基準のディレクトリ | カレントディレクトリから上に向かって探し、最初に見つかった `.mds/` のあるディレクトリ | docs/spec/mds.md#用語 |
| IR | 正規化した仕様の Markdown の文書の集まり（kotowari が検査する形式）。mds の対象例の一つ | docs/spec/mds.md#用語 |