# mds 入れ子リスト実装計画

## Goal

箇条書きの入れ子（任意の深さ）をサポートする。判断の記録（決定の行 + 子の superseded_by）を mds のスキーマで検査できるようにする。

## Specification

`docs/spec/mds.md`（改訂版）。この計画は仕様の見出しを参照し、本文を写さない。実装者は各ステップで示す見出しを読む。

## Approach and why

- 現行実装は入れ子リストを「トップレベルの箇条書きとして平坦化」している（`crates/mds-core/src/document.rs` の `blocks_from_list_item`）。これを「親の箇条書きに子ブロックを持たせる」ツリー構造に変える
- 子ブロックは、入れ子の箇条書き（さらに深い `-`、`*`、`+` の行）と、コードブロック・表などの子を保持する。継続段落（インデントされた散文）は親の箇条書きの一部として付く（R10）
- 子の行の元のインデントを保持する必要がある（仕様 R10 は「子の箇条書きの行は、そのままのインデントで抽出要素に含める」と要求）。`Block::Bullet` に子の行の生テキストを保持する
- 仕様 R10 の `children`（fields / bullets を再帰的に宣言）を、スキーマの `Bullets` 構造体に追加する
- 検証は、各レベルの `bullets` 規則に照合し、子は親の `children` の宣言に照合する（閉じた世界では undeclared_line。open でも宣言済みの構造の中の未宣言の子は undeclared）
- 抽出は、親の抽出要素に子の箇条書きの行をインデント付きで含める（子フィールド行は含めない）
- 各ステップは RED → GREEN → REFACTOR。既存テストのうち「入れ子は平坦化」を固定しているものを新挙動に合わせて更新する

## Reuse decisions

新規の依存は要らない。既存の `markdown` クレートが入れ子を既に構造化するので、それを活かす。スキーマの再帰的な `children` は既存の型システムで表現できる（`Bullets` に再帰フィールドを足す）。

## Scope of change

- 変更する: `crates/mds-core/src/document.rs`、`crates/mds-core/src/schema.rs`、`crates/mds-core/src/validate.rs`、`crates/mds-core/src/extract.rs`、`crates/mds-core/src/finding.rs`（必要な場合）、`tests/`、`fixtures/`（判断の記録のスキーマ例を追加）
- 変更しない: `docs/spec/mds.md`、`CONTEXT.md`、`AGENTS.md`、`PROJECT.md`、`CLAUDE.md`
- 仕様の変更が必要になったら、実装で黙って決めずに brainstorm へ戻す

## Step order and prerequisites

1. スキーマモデル: `Bullets` に `children`（fields / bullets を再帰的に）を追加
2. 文書モデル: `Block::Bullet` に子ブロックを持たせ、入れ子をツリーにする（インデント保持を含む）
3. 検証: 子を親の `children` 宣言に照合（閉じた世界の undeclared と open の扱いを含む）
4. 抽出: 親の抽出要素に子の箇条書きの行をインデント付きで含める
5. 判断の記録のスキーマとフィクスチャ、端から端までの確認

## Verification map

| 仕様の見出し | 確かめるステップ |
|---|---|
| R8 フィールド行 | 1, 3, 4, 5 |
| R9 文 | 2 |
| R10 箇条書き | 1, 2, 3, 4, 5 |
| R13 閉じた世界と `open` | 2, 3, 5 |
| R14 出現回数 | 3 |
| R16 抽出と `values` | 4, 5 |
| R19 停止 | 1 |

## Left to the implementer

- モジュール内の関数分割、命名、ヘルパーの抽出
- `Block::Bullet` の子ブロックの型（子に持てるもの: 入れ子の箇条書き・コードブロック・表・その他。`Vec<Block>` で表現し、`Block` の既存 variant を再利用する）
- 指摘の `detail` の文言（英語。仕様は kind と severity を固定し、文言は固定しない）
- テストのファイル配置（core は各モジュールの `#[cfg(test)]`、CLI は `tests/`）

## Stop conditions

- 仕様に無い入力・境界・エラー挙動が必要になったとき
- 入れ子の深さが任意なのに、スキーマの再帰的な `children` が型で表現できないとき
- 子の元のインデントを保持できないとき（仕様 R10 の「そのままのインデントで含める」を満たせないとき）
- 仕様の節を参照先として見つけられないとき（仕様の欠落なので brainstorm へ戻す）

## Test command

`cargo test`（ルートで）。lint は `cargo clippy --all-targets -- -D warnings`。

## Out of scope

仕様の「作らないもの」節のすべて。加えて、順序付きリストの入れ子、抽出値の型付け、外部プロジェクト本体の変更。

---

## Step 1 — スキーマモデル: `Bullets` に `children` を追加

Purpose: スキーマの `bullets` 規則に、子のフィールド行と箇条書きを宣言できる `children` を追加し、`children.bullets` の `extract` を禁止する。Specification: `docs/spec/mds.md#R10`、`docs/spec/mds.md#R8`、`docs/spec/mds.md#R19`。
Prerequisites: なし。
May change: `crates/mds-core/src/schema.rs`、`crates/mds-core/src/finding.rs`（必要な場合）。
Done when: `bullets` に `children: { fields: [...], bullets: {...} }` を書けて、`bullets` が再帰的に `children` を持てる。項目（item）の中の箇条書きも同様に `children` を持てる。未知のキー・型違反は `schema_invalid`。`children.bullets` に `extract` を宣言すると `schema_invalid`（A14）。
Shown by: test — `children` を持つスキーマが読めること、再帰的な `bullets` が読めること、項目内の `bullets` の `children` が読めること、未知のキーが `schema_invalid` になること、`children.bullets` の `extract` 宣言が `schema_invalid` になることを確認するテスト。
Left to the implementer: `Bullets` に再帰フィールドを足す型の設計（`children: Option<Children>`、`Children` が `fields` と `bullets` を持つ）、`children.bullets` の `extract` 禁止の検証。
Stop and hand back if: 再帰的な `children` が Rust の型で表現できないとき。

## Step 2 — 文書モデル: `Block::Bullet` に子ブロックを持たせる

Purpose: 入れ子リストを平坦化せず、親の箇条書きに子ブロックとして保持する。仕様 R10 の「任意の深さで入れ子にできる」を実現する。Specification: `docs/spec/mds.md#R10`、`docs/spec/mds.md#R9`、`docs/spec/mds.md#R13`。
Prerequisites: なし（スキーマモデルとは独立）。
May change: `crates/mds-core/src/document.rs`、`crates/mds-core/src/validate.rs`、`crates/mds-core/src/extract.rs`、`crates/mds-core/src/finding.rs`。
Done when: `Block::Bullet` が子ブロック（`Vec<Block>`）を持ち、`blocks_from_list_item` が入れ子リストをトップレベルに平坦化せず、親の子として保持する。子の行の元のインデントを保持する（仕様 R10 の「そのままのインデントで抽出要素に含める」を満たすため）。継続段落はこれまでどおり親に付く。コードブロック・表などの子はブロックとして残し、閉じた世界の undeclared の対象にする。リスト項目の先頭がコードブロック・表のとき、後続の段落は文として扱う（R9 のまま）。
Shown by: test — 2段・3段の入れ子が親の子として構造化されること、子のインデントが保持されること、継続段落が親に付くこと、コードブロック・表の子がブロックとして残ること、先頭がコードブロック・表の項目の後続段落が文として扱われることを確認するテスト。既存の平坦化テスト（`nested_list_items_are_each_a_top_level_bullet`）を新挙動に合わせて更新する。
Left to the implementer: `Block::Bullet` の子ブロックの型（`Vec<Block>`）、`blocks_from_list_item` の再帰の実装、子の行の生テキストとインデントの保持。
Stop and hand back if: 入れ子リストと継続段落の区別が mdast でつかないとき。子の元のインデントを保持できないとき。

## Step 3 — 検証: 子を親の `children` 宣言に照合

Purpose: 入れ子の検証を、仕様 R13 の閉じた世界と open に合わせる。Specification: `docs/spec/mds.md#R13`、`docs/spec/mds.md#R10`、`docs/spec/mds.md#R8`、`docs/spec/mds.md#R14`。
Prerequisites: Step 1, Step 2。
May change: `crates/mds-core/src/validate.rs`。
Done when: 親の `bullets` 規則が宣言されたとき、子は親の `children` の宣言に照合する。`children.fields` で宣言された名前と一致する子の `- 名前: 値` はフィールド行として検証し、一致しない `- 名前: 値` は箇条書きとして検証する。`children` に宣言が無い子、または親が `children` を持たないのに子リストがある場合は `undeclared_line`。`children.bullets` の再帰的な照合も同じ規則。子は親の `bullets.repeat` の本数には数えず、`children.bullets` の規則で別に数える（A11）。open でも、宣言済みの構造の中の未宣言の子は undeclared_line（R13 の「宣言済みの構造の中の未宣言の行」と同じ扱い）。
Shown by: test — 宣言された子フィールドが通ること、宣言されていない子が undeclared_line になること、`children` を持たない親の下の子リストが undeclared_line になること、再帰的な `bullets` の照合、子が親の repeat に数えられないこと、open でも宣言済みの構造の中の未宣言の子が undeclared_line になることを確認するテスト。既存の閉じた世界のテスト（`nested_list_items_are_each_reported_in_closed_world`）を新挙動に合わせて更新する。
Left to the implementer: 検証の再帰の実装、子フィールドの判定（R8 と同じ `is_declared_field` の再利用）、`children.bullets` の本数カウント。
Stop and hand back if: 閉じた世界での入れ子の undeclared の判定が仕様で決まらないとき。

## Step 4 — 抽出: 親の抽出要素に子の箇条書きを含める

Purpose: 入れ子の抽出を、仕様 R16 に合わせる。Specification: `docs/spec/mds.md#R16`、`docs/spec/mds.md#R10`、`docs/spec/mds.md#R8`。
Prerequisites: Step 1, Step 2。
May change: `crates/mds-core/src/extract.rs`。
Done when: 親の箇条書きの抽出要素は「元の行、継続段落、子の箇条書きの行を改行でつないだ文字列」になる。子の箇条書きの行はそのままのインデントで含める（A4）。子フィールド行は親の抽出要素に含めない（A12）。子フィールドは自身の `extract` を持てば、その配置パスに値を出す（A15）。配列の入れ子にはしない。節の本文の抽出・項目の本文の抽出も、子の箇条書きの行を含む形になる（R16 の改訂）。
Shown by: test — 親の抽出要素に子の箇条書きの行が含まれること、子フィールド行が含まれないこと、子フィールド自身の `extract` が配置パスに値を出すこと、再帰的な入れ子が含まれること、節の本文・項目の本文が子を含むことを確認するテスト。既存の抽出テスト（`section_body_includes_continuation_paragraphs_and_nested_bullets`）を新挙動に合わせて更新する。
Left to the implementer: 抽出要素の組み立ての再帰の実装、子フィールド行の除外、子フィールド自身の `extract` の配置。
Stop and hand back if: 抽出の要素の形が仕様で決まらないとき。

## Step 5 — 判断の記録のスキーマとフィクスチャ

Purpose: 判断の記録（決定の行 + 子フィールド）をスキーマで表現し、端から端まで通す。Specification: `docs/spec/mds.md#R10`、`docs/spec/mds.md#R13`、`docs/spec/mds.md#R16`、`docs/spec/mds.md#R8`。
Prerequisites: Step 1〜4。
May change: `fixtures/`、`tests/`。
Done when: 判断の記録のフィクスチャ（決定の行 `- 決定1 ...` + 子の `superseded_by`。決定の行は継続段落を持たない）が `children` で宣言したスキーマで `check` に合格し、`values` が期待の形（配列の各要素が「決定の行の抽出要素 = 元の行の文字列」で、子フィールド `superseded_by` は自身の `extract` で別の配置パスに値が出る）を返す。フィクスチャの形は判断の記録の実際の形（決定の行 + superseded_by、決定の行は継続段落を持たない）に沿う。
Shown by: check — `cargo test` が通り、`mds check fixtures/` が終了コード0。`mds values fixtures/decision/records.md` の出力が期待の形（決定の行の要素は元の行の文字列、子フィールド `superseded_by` の値が配置パスに出る）になることを確認する。フィクスチャとスキーマは `fixtures/` に作る（May change の成果物）。
Left to the implementer: フィクスチャの内容（判断の記録の実際の形に沿う範囲で）。
Stop and hand back if: 判断の記録の入れ子（決定の行 + 子フィールド）がスキーマで表現できないとき（仕様 R10 の前提が崩れるので brainstorm へ戻す）。判断の記録の実際の形を参照できないとき。