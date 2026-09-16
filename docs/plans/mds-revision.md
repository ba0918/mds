# mds 仕様改訂の実装修正計画

## Goal

改訂された仕様（`docs/spec/mds.md`、実装サイクルの findings を反映）に実装を合わせる。既存の実装（ブランチ `mds`、24コミット）を、仕様改訂で確定した挙動へ修正する。

## Specification

`docs/spec/mds.md`（改訂版）。この計画は仕様の見出しを参照し、本文を写さない。実装者は各ステップで示す見出しを読む。

## Approach and why

- 既存実装は改訂前の仕様で実装済み・テスト済み（103テスト成功）。修正は「改訂仕様で変わった挙動」だけに絞る
- ただし、改訂仕様と一致するが未確認の挙動（R17 の root など）は、テストで固定して検証対象にする
- 各修正は RED → GREEN → REFACTOR。改訂仕様が契約を宣言しているので、失敗テストは Evidence conditions を満たせる
- 修正は1つの関心事ごとにコミットする
- 実装・レビュー・修正は既存のワークツリー（`.agents/worktrees/mds`）で、ブランチ `mds` で行う

## Reuse decisions

改訂は既存実装の挙動修正であり、新規の依存は要らない。既存の依存（`markdown`、`serde-saphyr`、`gray_matter`、`clap`、`regex`、`serde_json`、`walkdir`、`ureq`、`sha2`）をそのまま使う。

## Scope of change

- 変更する: `crates/mds-core/src/`（frontmatter、document、validate、extract、schema）、`src/main.rs`、`tests/`、`fixtures/`（必要な場合）
- 変更しない: `docs/spec/mds.md`、`CONTEXT.md`、`AGENTS.md`、`PROJECT.md`、`CLAUDE.md`
- 仕様の変更が必要になったら、実装で黙って決めずに brainstorm へ戻す

## Step order and prerequisites

1. frontmatter の `$schema` の null・空・非文字列と、URL キャッシュの再取得（独立）
2. 継続段落の分類と箇条書きの扱い（独立）
3. 停止理由の整理（`argument_error`、`frontmatter_invalid` のコマンド横断の確認）（1 を含む）
4. 閉じた世界の範囲（title 未宣言の `#`、宣言外構造の内側、open の範囲）（独立）
5. 抽出の形と連結規則（repeat 0件・separator・箇条書き・表・項目の `required`・項目内部 extract 禁止・本文の連結・`:` 無し見出しの invalid_id）（独立）
6. `check --format json` の空出力（ファイルとディレクトリ）（独立）
7. ブロック引用・水平線・画像の行の扱い（独立）
8. 改訂仕様と一致する既存挙動の確認と固定（R17 の root、R14 の min 省略、R16 の text 継続行、ast --schema の type、R20 の全体停止、R12 のコードブロック行照合）
9. テストの総確認と fixtures の更新

## Verification map

| 仕様の見出し | 確かめるステップ |
|---|---|
| R1 CLI と終了コード | 3, 6, 9 |
| R2 frontmatter と `$schema` の解決 | 1, 3 |
| R3 スキーマ言語の全体像 | 5, 9 |
| R7 項目 | 5 |
| R8 フィールド行 | 5 |
| R9 文 | 2, 7 |
| R10 箇条書き | 2, 5 |
| R12 コードブロック | 8 |
| R13 閉じた世界と `open` | 4, 7 |
| R14 出現回数 | 5, 8 |
| R16 抽出と `values` | 2, 5, 8 |
| R17 素の AST | 8 |
| R18 `check` の出力と指摘の種類 | 6, 9 |
| R19 停止 | 1, 3 |
| R20 ディレクトリ検査 | 3, 6, 8 |

## Left to the implementer

- モジュール内の関数分割、命名、ヘルパーの抽出
- 指摘の `detail` の文言（英語。仕様は kind と severity を固定し、文言は固定しない）
- テストのファイル配置（core は各モジュールの `#[cfg(test)]`、CLI は `tests/`）

## Stop conditions

- 仕様に無い入力・境界・エラー挙動が必要になったとき
- 改訂仕様の文言と既存実装の構造が噛み合わず、仕様の再解釈が必要になったとき
- 仕様の節を参照先として見つけられないとき（仕様の欠落なので brainstorm へ戻す）

## Test command

`cargo test`（ルートで）。lint は `cargo clippy --all-targets -- -D warnings`。

## Out of scope

仕様の「作らないもの」節のすべて。加えて、抽出値の型付け、kotowari 本体の変更、配布。既に実装済みで改訂仕様と一致する挙動の再実装（一致の確認とテスト固定は含む）。

---

## Step 1 — frontmatter の `$schema` と URL キャッシュの再取得

Purpose: `$schema` の null・空・非文字列を `frontmatter_invalid` にし、破損した URL キャッシュを再取得する。Specification: `docs/spec/mds.md#R2`、`docs/spec/mds.md#R19`。
Prerequisites: なし。
May change: `crates/mds-core/src/frontmatter.rs`、`crates/mds-core/src/`（エラー型）、`src/main.rs`。
Done when: `$schema:` が null、空、数値、配列の文書が `frontmatter_invalid` で停止し、`$schema` キーが無い文書は「スキーマを持たない」のまま。破損・空の URL キャッシュを読んだら内容を検証して再取得し、再取得も失敗したら `schema_not_found` で停止する。URL の取得は10秒のタイムアウト、応答は 4 MiB 上限。
Shown by: test — null・空・非文字列・キー無しの4ケースのテスト。破損キャッシュを置いて再取得で回復することを確認するテスト。URL のタイムアウト（10秒）と 4 MiB 上限のテスト（現実装にはこの2つのテストが無いので、挙動を固定するテストを新規に書く）。
Left to the implementer: null と空を区別するための serde の扱い方、キャッシュの検証方法（YAML として読めるか）。
Stop and hand back if: serde で YAML の null とキー無しを区別できないとき。破損キャッシュの再取得の回数や停止条件が仕様で決まらないとき。

## Step 2 — 継続段落の分類と箇条書きの扱い

Purpose: リスト項目の継続段落を文ではなく箇条書きの一部にする。Specification: `docs/spec/mds.md#R10`、`docs/spec/mds.md#R9`、`docs/spec/mds.md#R16`。
Prerequisites: なし。
May change: `crates/mds-core/src/document.rs`、`crates/mds-core/src/validate.rs`、`crates/mds-core/src/extract.rs`、`tests/`。
Done when: 継続段落が文の `required`/`pattern`/`repeat` に数えられない。`bullet` の `pattern` は元の `- ` 行にだけ適用される。抽出要素は「元の `- ` 行（`- ` マーカー付き）と継続段落を改行でつないだ文字列」。継続段落が複数のときは継続段落どうしを空行でつないでから元の行と改行でつなぐ。継続段落は閉じた世界でも `undeclared_line` にならない。
Shown by: test — 継続段落を含む文書で `statement.required: true` が `missing_statement` になること、継続段落が `bullet` の `pattern` に照合されないこと、抽出要素が「`- ` マーカー付き行＋継続段落」になること、継続段落が undeclared_line にならないことを確認するテスト。箇条書きの入れ子（インデント付き `- `）がトップレベルの箇条書きとして扱われることを確認するテスト。既存の継続段落テスト（旧挙動を固定）を更新する。
Left to the implementer: 継続段落の検出（リスト項目の子である段落）の実装。
Stop and hand back if: mdast で継続段落と文の区別がつかないとき。

## Step 3 — 停止理由の整理

Purpose: `argument_error` を追加し、`frontmatter_invalid` のコマンド横断を確認する。Specification: `docs/spec/mds.md#R19`、`docs/spec/mds.md#R1`。
Prerequisites: Step 1。
May change: `src/main.rs`、`crates/mds-core/src/`（停止理由の型）。
Done when: `ast --format text` と知らないフラグが `argument_error` で停止する。停止理由の出力は標準エラーに `mds: <kind>: <説明>` の1行。`values` / `ast --schema` でも `frontmatter_invalid` で停止する（共有の読み込み経路で既に成立している場合は、成立をテストで固定する）。ディレクトリ検査で `frontmatter_invalid` の文書と `schema_not_found` の文書に出会ったとき全体停止（終了コード2）する。
Shown by: test — `argument_error` の2ケース（`--format text` と知らないフラグ）と、`values` / `ast --schema` での `frontmatter_invalid` を確認するテスト。停止理由の出力形を確認するテスト。ディレクトリ検査で `frontmatter_invalid` の文書・`schema_not_found` の文書を含むディレクトリで全体停止になることを確認するテスト。
Left to the implementer: 停止理由の説明文の文言（英語）。
Stop and hand back if: 停止理由の型が仕様の5種に合わないとき。`values` / `ast --schema` の `frontmatter_invalid` が共有経路で成立せず、仕様の再解釈が必要なとき。

## Step 4 — 閉じた世界の範囲

Purpose: title 未宣言の `#`、宣言外構造の内側の行を undeclared にする。open の範囲を仕様どおりにする。Specification: `docs/spec/mds.md#R13`。
Prerequisites: なし。
May change: `crates/mds-core/src/validate.rs`。
Done when: title 規則が無い `#` が `undeclared_heading`。宣言していない構造（前置部・節・項目）の内側の行が `undeclared_line`。宣言済みの構造の中に足された未宣言の構造は `undeclared_heading` + `undeclared_line`。宣言済みの構造の中の未宣言の行も `undeclared_line`。open では、宣言していない構造とその内側の行を許す（title 規則の無い `#` を含む）。ただし宣言済み構造の中の未宣言の構造と行は open でも誤り。
Shown by: test — 各ケースのテスト。open の有無で結果が変わることを確認するテスト（未宣言の節の内側が open で許され、宣言済みの節の中の未宣言の行は open でも誤りになること）。
Left to the implementer: 閉じた世界の走査の実装。
Stop and hand back if: 仕様 R13 の「宣言済み構造の中の未宣言の構造」の判定が実装で決まらないとき。

## Step 5 — 抽出の形・連結規則・項目

Purpose: 抽出の形と本文の連結規則を仕様どおりにし、項目の `required` を許し、項目内部の extract と `:` 無し見出しを不正にする。Specification: `docs/spec/mds.md#R16`、`docs/spec/mds.md#R14`、`docs/spec/mds.md#R8`、`docs/spec/mds.md#R7`、`docs/spec/mds.md#R10`。
Prerequisites: なし。
May change: `crates/mds-core/src/extract.rs`、`crates/mds-core/src/schema.rs`、`crates/mds-core/src/validate.rs`、`crates/mds-core/src/document.rs`、`tests/`。
Done when: repeat 0件はキー省略。`separator` は repeat の有無に関わらず配列、repeat と `separator` の併用は配列の配列。要素は前後の空白を取り除いて照合・抽出、空要素は照合対象で空文字列の要素として残す。箇条書きと表は常に配列、複数の表・箇条書きは文書に現れた順で連結、表に repeat を宣言しても行オブジェクトの配列のまま（入れ子にしない）。箇条書きと表が0件のときはキーを省略する。項目に `required` が使える（`required: false` は単一値の任意、`repeat` は配列）。項目内部の extract 宣言は `schema_invalid`。項目見出しに `:` が無いとき `invalid_id`。節本文の連結は文どうし空行・それ以外の隣接は改行・箇条書きは R10 の抽出要素。節本文にはフィールド行・表・コードブロック・項目を含めない。項目本文はフィールド行を含み、見出しは `### ` を付けず ID と名前だけ、本文内は文どうし空行・それ以外の隣接は改行。
Shown by: test — 各抽出形・連結・項目のテスト。項目内部の extract を宣言したスキーマが `schema_invalid` になるテスト。`:` 無し見出しの `invalid_id` のテスト。既存の本文連結テスト（旧挙動を固定）を更新する。
Left to the implementer: 抽出の配列組み立てと本文連結の実装。
Stop and hand back if: 仕様の「欠落時はキー省略」と「常に配列」の境界、または本文の連結規則が実装で決まらないとき。

## Step 6 — `check --format json` の空出力

Purpose: 指摘が無いときも `{"files": []}` を出力する。Specification: `docs/spec/mds.md#R18`。
Prerequisites: なし。
May change: `src/main.rs`。
Done when: 合格の単一ファイルと、合格のみのディレクトリの両方で `check --format json` が `{"files": []}` を出力し、text は何も出さない。
Shown by: test — 単一ファイルとディレクトリの両方の json 空出力を確認するテスト。
Left to the implementer: 出力の組み立て。
Stop and hand back if: なし。

## Step 7 — ブロック引用・水平線・画像の行の扱い

Purpose: ブロック引用・水平線・画像などの行を文の対象外とし、閉じた世界でも無視する。Specification: `docs/spec/mds.md#R9`、`docs/spec/mds.md#R13`。
Prerequisites: なし。
May change: `crates/mds-core/src/document.rs`、`crates/mds-core/src/validate.rs`、`tests/`。
Done when: ブロック引用・水平線・画像の行が文に数えられず、`undeclared_line` にもならない。
Shown by: test — 各行種別を含む文書のテスト。
Left to the implementer: 行種別の判定の実装。
Stop and hand back if: mdast でブロック引用・水平線・画像と文の区別がつかないとき。

## Step 8 — 既存挙動の確認と固定

Purpose: 改訂仕様と一致するが未確認の既存挙動をテストで固定し、検証対象にする。Specification: `docs/spec/mds.md#R17`、`docs/spec/mds.md#R14`、`docs/spec/mds.md#R16`、`docs/spec/mds.md#R20`、`docs/spec/mds.md#R12`。
Prerequisites: なし。
May change: `tests/`。
Done when: `mds ast` のルート型が `root`。`repeat: { max: 2 }` の min の省略が 0。`values --format text` の複数行値がキーより2文字深くインデントされる。`ast --schema` が name の type を添え、name 無しでは type を出さない。ディレクトリ検査で全体停止のとき指摘を出力しない。コードブロックの行照合は、行頭の空白を取り除いて行い、trim 後に空の行だけを対象外にする（インデント付きの非空行は照合対象）。
Shown by: test — 各挙動を固定するテスト（既に成立していれば GREEN で固定）。コードブロックの照合では、trim 後に空の行は対象外・インデント付きの非空行は照合対象であることを確認するテスト。
Left to the implementer: テストの配置。
Stop and hand back if: 既存挙動が改訂仕様と食い違うことが確認されたとき（その場合は該当ステップへ戻す）。

## Step 9 — テストの総確認と fixtures の更新

Purpose: 全体の整合を確認し、改訂仕様の成功条件が fixtures で満たされることを確かめる。Specification: `docs/spec/mds.md#R3`、`docs/spec/mds.md#R16`、`docs/spec/mds.md#R18`。
Prerequisites: Step 1〜8。
May change: `fixtures/`、`tests/`。
Done when: `cargo test` がすべて通り、`cargo clippy` が警告なし、`mds check fixtures/` が終了コード0。
Shown by: check — `cargo test`、`cargo clippy --all-targets -- -D warnings`、`mds check fixtures/`。
Left to the implementer: fixtures の内容（仕様の例に沿う範囲で）。
Stop and hand back if: 改訂仕様の成功条件を fixtures で満たせないとき（仕様の欠落として brainstorm へ戻す）。