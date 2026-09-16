# mds 実装計画

## Goal

`mds` CLI を実装する。Markdown 文書が frontmatter の `$schema` で指す YAML スキーマに従うかを検証し、スキーマが宣言した抽出規則に沿って構造と値を取り出せる。ADR の文書で端から端まで動く。

## Specification

`docs/spec/mds.md`（唯一の正）。この計画は仕様の見出しを参照し、本文を写さない。実装者は各ステップで示す見出しを読む。

## Approach and why

- Cargo workspace の monorepo。ルートの package `mds` が CLI（`src/main.rs`）で、`crates/mds-core` がスキーマ言語・検証・抽出を担うライブラリ。core を CLI から独立させておくと、後で kotowari に組み込むときにライブラリとして持ち込める。
- 層の向きは ba0918-design に従う。core は純粋関数（文字列 → モデル、モデル + 文書 → 指摘 / 値）だけを持ち、ファイル読み書き・URL 取得・ディレクトリ走査・キャッシュの置き場の解決は CLI 側に置く。core は CLI のフレームワークに依存しない。
- 検証と抽出は、スキーマの木と文書の木を1回組み立て、それぞれ別のビジターを当てて導出する（仕様 R16 の「values と ast --schema は検証の合否と独立」を満たすための実装方針。共通走査はこの計画の設計判断であり、仕様が強制するものではない）。
- テストは ba0918-tdd に従い RED → GREEN → REFACTOR。テストの粒度は「1つの振る舞いに1つのテスト」。

## Reuse decisions

実装前に層を分け、それぞれ既存解を探した結果。rung は ba0918-reuse の梯子の段。

| 層 | 判断 | 理由 |
|---|---|---|
| Markdown → mdast | adopt (rung 6): `markdown` クレート | 仕様 R17 が mdast（unist）を要求し、このクレートが mdast を直接出す。自前の CommonMark 実装は無駄 |
| YAML（スキーマと frontmatter） | adopt (rung 6): `serde-saphyr` | Serde で型付きに読める。kotowari でも採用済みで、この環境の実績がある |
| frontmatter の切り出し | adopt (rung 6): `gray_matter` | YAML frontmatter の切り出しは既存解がある。仕様 R2 の「他のキーは無視」も扱える |
| CLI 引数の解析 | adopt (rung 6): `clap` | エコシステムの定番。サブコマンドとフラグを宣言的に書ける |
| 正規表現 | adopt (rung 6): `regex` | 仕様 R4 が方言を Rust regex に固定している。自前は不可 |
| JSON 出力 | adopt (rung 6): `serde_json` | 定番。mdast の出力と values / check の json に使う |
| ディレクトリ走査 | adopt (rung 6): `walkdir` | 仕様 R20 の走査規則を実装できる。kotowari でも採用済み |
| URL 取得 | adopt (rung 6): `ureq` | 小さな同期クライアント。非同期ランタイムを持ち込まない |
| キャッシュのハッシュ | adopt (rung 6): `sha2` | 仕様 R2 の例（SHA-256 の16進）に従い SHA-256 を採用。キャッシュは実行をまたぐ永続化形式なので、採用時に固定する |
| スキーマモデル・検証・抽出 | build (rung 8) | この道具の核。既存解は無い |
| 停止理由の表現 | build (rung 7) | 停止理由は数個の列挙で足りる。エラー用クレートは買うものが無い |
| CLI のテスト補助 | adopt (dev): `assert_cmd`, `tempfile`, `predicates` | CLI の end-to-end テストの定番 |

`markdown` クレートが mdast を出さない場合（rung 6 の前提が崩れた場合）は、実装を進めずに止めて報告する。

## Scope of change

- 新規: ルートの `Cargo.toml`、`crates/mds-core/`、`src/`、`fixtures/`、`tests/`
- 変更しない: `docs/spec/mds.md`、`CONTEXT.md`、`AGENTS.md`、`PROJECT.md`、`CLAUDE.md`
- 仕様の変更が必要になったら、実装で黙って決めずに brainstorm へ戻す

## Step order and prerequisites

1. workspace の骨組み
2. mdast の解析と `mds ast`（1 に依存）
3. frontmatter と `$schema` の解決（1 に依存）
4. スキーマモデルと YAML の読み込み（3 に依存）
5. 検証: 構造（2, 4 に依存）
6. 検証: 行の規則（2, 5 に依存）
7. 検証: 出現回数・条件付き・順序（2, 6 に依存）
8. `check`（ファイル）と出力・終了コード・停止（7 に依存）
9. 抽出と `mds values`（2, 4 に依存。8 とは独立）
10. `mds ast --schema`（9 に依存）
11. `check ./`（ディレクトリ）（8 に依存）
12. URL スキーマとキャッシュ（8 に依存。check 経由で検証するため）
13. IR のフィクスチャと端から端までの確認（全部に依存）

## Verification map

| 仕様の見出し | 確かめるステップ |
|---|---|
| R1 CLI と終了コード | 1, 2, 8 |
| R2 frontmatter と `$schema` の解決 | 3, 12 |
| R3 スキーマ言語の全体像 | 4 |
| R4 題名 | 5 |
| R5 前置部 | 5 |
| R6 節 | 5 |
| R7 項目 | 5, 9 |
| R8 フィールド行 | 6, 7 |
| R9 文 | 6 |
| R10 箇条書き | 6 |
| R11 表 | 6, 9 |
| R12 コードブロック | 6 |
| R13 閉じた世界と `open` | 5, 8 |
| R14 出現回数 | 7 |
| R15 条件付き規則 | 7 |
| R16 抽出と `values` | 9, 10 |
| R17 素の AST | 2, 10 |
| R18 `check` の出力と指摘の種類 | 8 |
| R19 停止 | 3, 8, 12 |
| R20 ディレクトリ検査 | 11 |

## Left to the implementer

- モジュール内の関数分割、命名、ヘルパーの抽出
- mdast を扱う内部表現（`markdown` クレートの型をそのまま使うか、独自の中間型を挟むか）。ただし R17 の出力形式は仕様が固定する
- テストのファイル配置（core は各モジュールの `#[cfg(test)]`、CLI は `tests/`）
- 指摘の `detail` の文言（英語。仕様は kind と severity を固定し、文言は固定しない）

## Stop conditions

- 仕様に無い入力・境界・エラー挙動が必要になったとき
- `markdown` クレートが mdast を出さず、R17 が実現できないとき
- URL 取得のテストに外部ネットワークが要るとき（ローカル HTTP サーバで代替できなければ止めて相談する）
- 仕様の節を参照先として見つけられないとき（仕様の欠落なので brainstorm へ戻す）
- 仕様の「決めていないこと」U1（会話メモ）が入手され、仕様を変える内容が含まれるとき。メモの反映はこの計画の対象外で、入手時に仕様へ反映してから計画を見直す

## Test command

`cargo test`（ルートで）。lint は `cargo clippy`。

## Out of scope

仕様の「作らないもの」節のすべて。加えて、抽出値の型付け、kotowari 本体の変更、配布（crates.io・CI・ライセンス）、仕様の「決めていないこと」U1（会話メモ）の反映。

---

## Step 1 — workspace の骨組み

Purpose: ビルドが通り `mds --version` が動く最小の monorepo を作る。Specification: `docs/spec/mds.md#R1`。
Prerequisites: なし。
May change: `Cargo.toml`、`src/main.rs`、`crates/mds-core/Cargo.toml`、`crates/mds-core/src/lib.rs`、`.gitignore`。
Done when: ルートで `cargo build` が成功し、`cargo run -- --version` が `mds 0.1.0` を出力して終了コード0で終わる。
Shown by: check — `cargo build`、`cargo run -- --version`。
Left to the implementer: workspace のメンバー構成（`crates/*`）、core のクレート名（`mds-core`）、package のメタ情報。
Stop and hand back if: ルート package と workspace を同居させると cargo の制約で組めないとき。

## Step 2 — mdast の解析と `mds ast`

Purpose: Markdown を mdast に解析し、`mds ast` が JSON で出し、`--format text` を停止にする。Specification: `docs/spec/mds.md#R17`、`docs/spec/mds.md#R1`。
Prerequisites: Step 1。
May change: `crates/mds-core/src/`（解析モジュール）、`src/main.rs`、`fixtures/adr/0001.md`、`tests/`。
Done when: `fixtures/adr/0001.md` を `mds ast` にかけると、mdast の形（`type` と `children`）の JSON が出て、インライン要素を含み、`position` を含まない。`mds ast --format text` は引数の誤りとして終了コード2で停止する。
Shown by: test — 見出しと段落とインライン要素を含む文書の解析結果が mdast の形になること、`position` が無いことを確認するテスト。`--format text` が停止になることを確認する CLI テスト。テスト用に `fixtures/adr/0001.md`（ADR の適用例）を作る（May change の成果物）。
Left to the implementer: 解析結果の内部表現、JSON への写し方。
Stop and hand back if: `markdown` クレートが mdast を出さないとき。

## Step 3 — frontmatter と `$schema` の解決

Purpose: 文書の frontmatter から `$schema` を読み、相対パスを文書の位置基準で解決する。Specification: `docs/spec/mds.md#R2`、`docs/spec/mds.md#R19`。
Prerequisites: Step 1。
May change: `crates/mds-core/src/`（frontmatter と解決）、`src/main.rs`。
Done when: frontmatter を持たない文書は「スキーマなし」になり、`$schema` の相対パスが文書の位置基準で解決され、`$schema` が URL のときは URL として扱われる（取得は Step 12）、壊れた frontmatter と文字列でない `$schema` が `frontmatter_invalid` の停止になる。
Shown by: test — 相対パスが文書の位置基準で解決されること、`$schema` 以外のキーが無視されること、壊れた frontmatter が停止になることを確認するテスト。
Left to the implementer: frontmatter の切り出しの実装（Reuse decisions のとおり `gray_matter` を使う）。
Stop and hand back if: `gray_matter` で仕様の規則（先頭の `---`、他のキーを無視）を満たせないとき。

## Step 4 — スキーマモデルと YAML の読み込み

Purpose: スキーマ YAML を型付きのモデルに読み、スキーマ言語の形に反するものを `schema_invalid` にする。Specification: `docs/spec/mds.md#R3`、`docs/spec/mds.md#R4`〜`#R12` の各キー、`docs/spec/mds.md#R19`。
Prerequisites: Step 3。
May change: `crates/mds-core/src/`（スキーマモデル）。
Done when: ADR のスキーマ（題名・前置部・節・フィールド行・箇条書き・表・コードブロック・出現回数・条件付き・抽出）が型付きで読め、未知のキーや型違反が `schema_invalid` になる。
Shown by: test — 仕様 R3〜R12 の各キーを持つスキーマが読めること、未知のキーが `schema_invalid` になることを確認するテスト。
Left to the implementer: モデルの型構成、Serde の derive の組み方。
Stop and hand back if: 仕様 R3〜R12 のキーを型で表せないとき。

## Step 5 — 検証: 構造

Purpose: 題名・前置部・節・項目の構造を検証し、閉じた世界の判定（スキーマの `open` 含む）をする。Specification: `docs/spec/mds.md#R4`、`#R5`、`#R6`、`#R7`、`#R13`、`docs/spec/mds.md#R18` の kind のうち構造系。
Prerequisites: Step 2, Step 4。
May change: `crates/mds-core/src/`（検証）、`crates/mds-core/src/`（指摘モデル）。
Done when: 題名の欠落・重複・パターン不一致、未宣言の見出しと行、必須の節の欠落、項目の ID と深さの違反が、対応する kind の指摘になる。スキーマの `open: true` で未宣言の見出しと行が許される。`open` で必須の欠落や形の違反は緩まない。
Shown by: test — `missing_title`、`multiple_titles`、`title_pattern_mismatch`、`undeclared_heading`、`undeclared_line`、`missing_required_section`、`heading_level_mismatch`、`invalid_id` の各テスト。`open: true` の有無で結果が変わるテスト。
Left to the implementer: 文書の木を歩く走査の実装、指摘の集め方、行の分類の基礎（フィールド行・箇条書き・文の見分け。`undeclared_line` の判定に必要。詳細な規則の検証は Step 6）。
Stop and hand back if: 仕様の kind で表現できない構造の違反が出たとき。

## Step 6 — 検証: 行の規則

Purpose: フィールド行・文・箇条書き・表・コードブロックの規則を検証する。Specification: `docs/spec/mds.md#R8`、`#R9`、`#R10`、`#R11`、`#R12`、`docs/spec/mds.md#R18` の kind のうち行系。
Prerequisites: Step 2, Step 5。
May change: `crates/mds-core/src/`（検証）。
Done when: 必須の欠落、パターン不一致、enum 違反、`separator` の分割照合、表のヘッダと列数、コードブロックの lang と行が、対応する kind の指摘になる。
Shown by: test — `missing_required_field`、`field_pattern_mismatch`、`field_enum_invalid`、`missing_statement`、`statement_pattern_mismatch`、`statement_enum_invalid`、`missing_bullets`、`bullet_pattern_mismatch`、`missing_table`、`table_header_mismatch`、`missing_codeblock`、`codeblock_lang_mismatch`、`codeblock_line_mismatch` の各テスト。
Left to the implementer: 行の分類（フィールド行・箇条書き・文の見分け）の実装。
Stop and hand back if: 行の分類が仕様の定義（`- 名前: 値` か、`- ` の箇条書きか、散文か）で決まらないとき。

## Step 7 — 検証: 出現回数・条件付き・順序

Purpose: `repeat` の min / max、`when` の条件、`ordered` の順序を検証する。Specification: `docs/spec/mds.md#R14`、`#R15`、`docs/spec/mds.md#R8` の `ordered`、`docs/spec/mds.md#R18` の `repeat_*` と `field_order_mismatch`。
Prerequisites: Step 2, Step 6。
May change: `crates/mds-core/src/`（検証）。
Done when: `repeat_min_not_met`、`repeat_max_exceeded`、`field_order_mismatch` が正しく出て、`when` の真偽で制約の適用が切り替わり、参照フィールドが無いとき `eq` が偽・`ne` が真になる。`min > max`、負値、非整数は `schema_invalid` になる。
Shown by: test — 各 kind のテストと、`when` の真・偽・参照なしの3ケースのテスト。
Left to the implementer: `when` の評価の実装、`repeat` の数え方。
Stop and hand back if: `when` の探索範囲（同じノードの下）が仕様の構造で決まらないとき。

## Step 8 — `check`（ファイル）と出力・終了コード・停止

Purpose: `mds check <file>` を実装し、text / json の出力、終了コード、停止を仕様どおりにする。Specification: `docs/spec/mds.md#R1`、`#R18`、`#R19`、`#R13` の `--open`。
Prerequisites: Step 7。
May change: `src/main.rs`、`tests/`。
Done when: 合格の文書が終了コード0で何も出さず、指摘のある文書が1で text（`path:line: kind: detail`。行番号を持たない指摘は `path: kind: detail`）または json（`files` 配列）を出し、`--format` の既定は text、`--open` が閉じた世界を緩め、停止が終了コード2で標準エラーに1行出る。指摘の `severity` は `error` のみ。文字コードは UTF-8 で先頭 BOM を読み飛ばし、`\r\n` は1行、末尾改行なしも1行に数える。`$schema` の無い文書を `check <file>` で指定したときと参照先スキーマが無いときは `schema_not_found`、読めないファイルは `unreadable_file` の停止になる。
Shown by: test — 合格・不合格・停止の終了コード、text と json の出力形、`--format` 既定（text）、`--open` の効き、BOM・`\r\n`・末尾改行なしの行数え、行番号なし指摘の書式、`schema_not_found` と `unreadable_file` を確認する CLI テスト（`assert_cmd`）。
Left to the implementer: 出力の組み立て、停止理由の文言（英語）。
Stop and hand back if: 停止と指摘の切り分けが仕様で決まらないとき。

## Step 9 — 抽出と `mds values`

Purpose: スキーマの `extract` に沿って値を組み立て、`mds values` が text / json で出す。Specification: `docs/spec/mds.md#R16`、`docs/spec/mds.md#R7`（項目の抽出）、`docs/spec/mds.md#R11`（ヘッダ重複）、`docs/spec/mds.md#R19`（スキーマ解決失敗の停止）。
Prerequisites: Step 2, Step 4（Step 8 とは独立）。
May change: `crates/mds-core/src/`（抽出）、`src/main.rs`、`tests/`。
Done when: ADR 文書の `values` が `{id, title, status, date, sections: {context, decision, reasons}}` を返す。4つの `extract` 書式が働く。節本文の抽出は文と箇条書きだけを含め、表・コードブロック・項目は含めない。項目の抽出は「見出し＋本文」を1つの値にする。表の抽出がヘッダをキーにしたオブジェクトの配列になり、ヘッダ重複で後の列が先を上書きする。`repeat` を宣言したノードは配列（`{min:0,max:1}` でも配列）、`repeat` が無いノードは単一値で欠落時はキーを省略する。`separator` の分割が要素になる。json は配置パスに沿った入れ子、text はインデントと番号付きで出る。`--format` の既定は text。`$schema` が無い文書やスキーマを解決できない文書を `values` に与えたときは `schema_not_found` で停止する。
Shown by: test — 4つの `extract` 書式、配列の形、節本文の除外規則、項目抽出、ヘッダ重複、json と text の形、`--format` 既定（text）、スキーマ解決失敗の停止のテスト。
Left to the implementer: 配置パスへの値の差し込みの実装。
Stop and hand back if: 抽出の形（配列か単一値か）が仕様の出現回数の規則で決まらないとき。

## Step 10 — `mds ast --schema`

Purpose: スキーマを適用した型付きの AST を出す。Specification: `docs/spec/mds.md#R16`、`#R17`、`docs/spec/mds.md#R19`（スキーマ解決失敗の停止）。
Prerequisites: Step 9。
May change: `crates/mds-core/src/`（型付き AST）、`src/main.rs`、`tests/`。
Done when: ADR 文書の `ast --schema` が `name` の `type` を含む型付きの JSON を返し、`name` が無ければ `type` を出さない。`--format text` は引数の誤りとして終了コード2で停止する。スキーマを解決できない文書では `schema_not_found` で停止する。
Shown by: test — `type` の有無の2ケース、`--format text` の停止、スキーマ解決失敗の停止のテスト。
Left to the implementer: 型付き AST の内部表現。
Stop and hand back if: 型付き AST の形が仕様の抽出パスと一致しないとき。

## Step 11 — `check ./`（ディレクトリ）

Purpose: ディレクトリ配下のスキーマ宣言文書をまとめて検査する。Specification: `docs/spec/mds.md#R20`、`#R1`、`#R19`。
Prerequisites: Step 8。
May change: `src/main.rs`、`tests/`。
Done when: 複数の不合格文書を含むディレクトリで、全文書の指摘が `files` 配列にまとまり終了コード1になり、スキーマを持たない `.md` は対象外になり、隠しディレクトリ・`.mds/`・シンボリックリンクは辿らず、読めないファイルや `schema_invalid` のスキーマに出会ったとき全体を停止する。
Shown by: test — 合格・不合格・スキーマなしを混ぜたディレクトリ、隠しディレクトリ、シンボリックリンク、読めないファイルと壊れたスキーマでの停止のテスト。
Left to the implementer: 走査の実装（`walkdir` の設定）。
Stop and hand back if: 走査の除外規則が仕様で決まらないとき。

## Step 12 — URL スキーマとキャッシュ

Purpose: `$schema` が URL のとき取得してキャッシュし、オフラインで停止する。Specification: `docs/spec/mds.md#R2`、`#R19`。
Prerequisites: Step 8（check 経由で検証するため）。
May change: `src/main.rs`（URL 取得とキャッシュの置き場の解決）、`tests/`。
Done when: URL を宣言した文書の1回目の実行がサーバから取得してキャッシュに保存し、2回目の実行はサーバに到達せずキャッシュで成功する。キャッシュが無いまま取得できないとき `schema_not_found` で停止する。URL 宣言の文書を `check`・`values`・`ast --schema` のどれに渡しても同じ解決結果になる。
Shown by: test — ローカル HTTP サーバ（`http` クレートまたは同等のテスト用サーバ）で取得とキャッシュを確認するテスト。`check`・`values`・`ast --schema` の3コマンドで URL 宣言文書を扱うテスト。外部ネットワークに依存しない。
Left to the implementer: キャッシュファイルの置き場の解決（基準のディレクトリ）、HTTP クライアントの設定。
Stop and hand back if: 外部ネットワークなしでテストできないとき。

## Step 13 — IR のフィクスチャと端から端までの確認

Purpose: IR の文書単体の書式のフィクスチャを置き、ADR と IR の両方で端から端まで通す。Specification: `docs/spec/mds.md#R3`、`#R5`、`#R6`、`#R7`、`#R8`、`#R10`、`#R11`、`#R12`、`#R16`、`#R17`、`#R18`。
Prerequisites: Step 12。
May change: `fixtures/`、`tests/`。
Done when: ADR のフィクスチャが `check` に合格し `values` と `ast --schema` が期待の形を返し、IR のフィクスチャ（前置部の散文、節、項目、フィールド行、表、gherkin のコードブロック）が `check` に合格する。
Shown by: check — 両フィクスチャで `cargo test` が通り、`mds check fixtures/` が終了コード0になる。
Left to the implementer: フィクスチャの内容（仕様の例に沿う範囲で）。
Stop and hand back if: IR の文書単体の書式がスキーマで表現できないとき（仕様 R3 の前提が崩れるので brainstorm へ戻す）。