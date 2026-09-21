---
$schema: ../../.mds/schemas/ir.yaml
---
# スキーマの解決

この文書は、文書の frontmatter が指すスキーマをどう見つけ、取得し、キャッシュするかを扱う。

## 要求

### REQ-011: スキーマの指定の解決

- 種類: algorithm
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A14, docs/decision/records/2026-09-21-mds-spec.md#A20
- 定義: TBL-003
- 検証: unit

### REQ-012: 相対パスの基準

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A14
- 検証: unit

mds は常に、`frontmatter`に書いた相対パスを、`文書`の置かれた位置を基準に解決する。`基準のディレクトリ`は相対パスの解決には使わない。

### REQ-013: URL のスキーマのキャッシュ

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A14, docs/decision/records/2026-09-21-mds-spec.md#A20
- 検証: unit

`frontmatter`が URL の`スキーマ`を指したとき、mds は取得した内容を SHA-256 の名前でキャッシュに置き、次からはキャッシュを読む。キャッシュが壊れていれば取得し直して回復する。

### REQ-014: スキーマを指していない文書

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A8, docs/decision/records/2026-09-21-mds-spec.md#P1
- 検証: unit

`frontmatter`が YAML のマッピングでないとき、または "$schema" の値が空か空白だけのとき、mds は`停止`する。

### REQ-015: frontmatter の余分なキー

- 種類: ubiquitous
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A1
- 検証: unit

mds は常に、`frontmatter`の "$schema" 以外のキーを読まず、`指摘`にもしない。

### REQ-052: URL の認証情報を伏せる

- 種類: event_driven
- 出典: docs/decision/records/2026-09-21-mds-spec.md#A14, docs/decision/records/2026-09-21-mds-spec.md#A25
- 検証: unit

`停止`の説明に URL を載せるとき、mds はその authority にある認証情報を伏せる。

## 決定表

### TBL-003: スキーマの指定の解決

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A14, docs/decision/records/2026-09-21-mds-spec.md#A8

| 順 | "$schema" の値 | 解決 |
|---|---|---|
| 1 | 無い、空、空白だけ、`frontmatter`がマッピングでない | `停止` |
| 2 | "http://" か "https://" で始まる | 取得してキャッシュに置く。取得できなければ`停止` |
| 3 | それ以外 | `文書`の位置からの相対パスとして読む。読めなければ`停止` |

## 性質

### PROP-003: コマンドによって解決先が変わらない

- 出典: docs/decision/records/2026-09-21-mds-spec.md#A14

同じ`文書`の "$schema" は、検査、`抽出`、素の構文木のどのコマンドから読んでも同じ`スキーマ`に解決する。

## 具体例

```gherkin
@id=EX-005 @about=REQ-012 @source=docs/decision/records/2026-09-21-mds-spec.md#A14
Scenario: 相対パスは文書の位置から解決する
  Given `文書`から離れた位置の`スキーマ`を相対パスで指した`文書`がある
  When "mds check" を実行する
  Then `スキーマ`は`文書`の位置から解決される
  And 終了コードは 0 である

@id=EX-017 @about=REQ-052 @source=docs/decision/records/2026-09-21-mds-spec.md#A25
Scenario: 認証情報を含む URL は伏せて出す
  Given 認証情報を含む URL の`スキーマ`を指した`文書`があり、取得に失敗する
  When "mds check" を実行する
  Then 標準エラーに認証情報は出ない
  And URL は伏せた形で出る

@id=EX-006 @about=REQ-013 @source=docs/decision/records/2026-09-21-mds-spec.md#A14
Scenario: 壊れたキャッシュは取得し直して回復する
  Given URL の`スキーマ`を指した`文書`と、壊れたキャッシュがある
  When "mds check" を実行する
  Then `スキーマ`を取得し直す
  And 終了コードは 0 である
```
