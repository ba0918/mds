//! スキーマ YAML のモデルと読み込み。

use regex::Regex;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

/// スキーマに書かれた正規表現。読み込み時に一度だけコンパイルし、以後は再利用する。
#[derive(Debug, Clone)]
pub struct Pattern {
    source: String,
    compiled: Regex,
}

impl Pattern {
    /// 文字列に一致するか。
    pub fn is_match(&self, text: &str) -> bool {
        self.compiled.is_match(text)
    }

    /// 名前付きキャプチャを含む最初の一致を取る。
    pub fn captures<'h>(&self, text: &'h str) -> Option<regex::Captures<'h>> {
        self.compiled.captures(text)
    }

    /// スキーマに書かれた正規表現の文字列。
    pub fn source(&self) -> &str {
        &self.source
    }

    /// 指定の名前の名前付きキャプチャを含むか。Capture 形の抽出が使う（R16）。
    pub fn has_capture_group(&self, name: &str) -> bool {
        self.compiled.capture_names().flatten().any(|n| n == name)
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let source = String::deserialize(deserializer)?;
        let compiled = Regex::new(&source).map_err(D::Error::custom)?;
        Ok(Pattern { source, compiled })
    }
}

/// スキーマを読めなかった理由。R19 の `schema_invalid` に相当する。
#[derive(Debug)]
pub struct SchemaError(pub String);

/// スキーマ言語の最上位。R3。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    /// 型の名前。`ast --schema` の `type` に使う
    pub name: Option<String>,
    /// 閉じた世界を緩めるか。R13
    #[serde(default)]
    pub open: bool,
    /// 文書の構造の木
    pub document: Document,
}

/// `document` の下の規則種別。R3。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub title: Option<Title>,
    pub preamble: Option<Preamble>,
    #[serde(default)]
    pub sections: Vec<Section>,
}

/// 題名。R4。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Title {
    pub pattern: Option<Pattern>,
    pub extract: Option<Extract>,
}

/// 前置部。R5。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preamble {
    #[serde(default)]
    pub fields: Vec<Field>,
    pub statement: Option<Statement>,
    pub bullets: Option<Bullets>,
    /// フィールド行の並び順を強制する。R8
    #[serde(default)]
    pub ordered: bool,
}

/// 節。R6。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    /// 見出しの文字列
    pub name: String,
    pub required: Option<bool>,
    pub repeat: Option<Repeat>,
    pub statement: Option<Statement>,
    #[serde(default)]
    pub fields: Vec<Field>,
    /// フィールド行の並び順を強制する。R8
    #[serde(default)]
    pub ordered: bool,
    pub bullets: Option<Bullets>,
    pub table: Option<Table>,
    pub codeblock: Option<CodeBlock>,
    pub item: Option<Item>,
    pub extract: Option<Extract>,
}

/// 項目。R7。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// 見出しの ID 部分の正規表現
    pub id: Option<Pattern>,
    #[serde(default)]
    pub fields: Vec<Field>,
    /// フィールド行の並び順を強制する。R8
    #[serde(default)]
    pub ordered: bool,
    pub statement: Option<Statement>,
    pub bullets: Option<Bullets>,
    pub required: Option<bool>,
    pub repeat: Option<Repeat>,
    pub extract: Option<Extract>,
}

/// フィールド行。R8。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub required: Option<bool>,
    pub repeat: Option<Repeat>,
    pub pattern: Option<Pattern>,
    #[serde(rename = "enum")]
    pub r#enum: Option<Vec<String>>,
    pub separator: Option<String>,
    /// `csv: true` は `separator: ","` の省略形
    pub csv: Option<bool>,
    pub when: Option<When>,
    pub extract: Option<Extract>,
}

impl Field {
    /// separator と csv を解決した区切り文字。どちらも無ければ None。
    pub fn effective_separator(&self) -> Option<&str> {
        match (&self.separator, self.csv) {
            (Some(sep), _) => Some(sep),
            (None, Some(true)) => Some(","),
            (None, _) => None,
        }
    }
}

/// スキーマが宣言したフィールド行の名前と一致するか。R8 のフィールド行判定。
pub(crate) fn is_declared_field(fields: &[Field], name: &str) -> bool {
    fields.iter().any(|f| f.name == name)
}

/// 文。R9。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Statement {
    pub required: Option<bool>,
    pub repeat: Option<Repeat>,
    pub pattern: Option<Pattern>,
    #[serde(rename = "enum")]
    pub r#enum: Option<Vec<String>>,
    pub when: Option<When>,
    pub extract: Option<Extract>,
}

/// 箇条書き。R10。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bullets {
    pub required: Option<bool>,
    pub repeat: Option<Repeat>,
    pub pattern: Option<Pattern>,
    pub when: Option<When>,
    pub extract: Option<Extract>,
}

/// 表。R11。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub header: Vec<String>,
    pub required: Option<bool>,
    pub repeat: Option<Repeat>,
    pub extract: Option<Extract>,
}

/// コードブロック。R12。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeBlock {
    pub lang: Option<String>,
    /// ブロック内の行ごとの規則。各行がパターンのいずれかに一致すること
    pub lines: Option<Vec<Pattern>>,
    pub required: Option<bool>,
    pub repeat: Option<Repeat>,
    pub extract: Option<Extract>,
}

/// 出現回数。R14。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repeat {
    pub min: Option<u64>,
    pub max: Option<u64>,
}

impl Repeat {
    fn validate(&self) -> Result<(), SchemaError> {
        if let (Some(min), Some(max)) = (self.min, self.max) {
            if min > max {
                return Err(SchemaError("repeat min is greater than max".into()));
            }
        }
        Ok(())
    }
}

/// 条件付き規則。R15。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct When {
    /// 参照するフィールド行の名前。同じノードの下に限定
    pub field: String,
    pub eq: Option<String>,
    pub ne: Option<String>,
}

impl When {
    fn validate(&self) -> Result<(), SchemaError> {
        let operators = self.eq.is_some() as u8 + self.ne.is_some() as u8;
        if operators != 1 {
            return Err(SchemaError("when must have exactly one of eq or ne".into()));
        }
        Ok(())
    }
}

/// 抽出規則。R16 の4書式のうち、文字列は書式1・3・4、Capture は書式2。
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Extract {
    Path(String),
    Capture { path: String, group: String },
}

/// スキーマ YAML を型付きのモデルに読み、形の違反を `SchemaError` にする。
pub fn parse_schema(yaml: &str) -> Result<Schema, SchemaError> {
    let schema: Schema = serde_saphyr::from_str(yaml).map_err(|e| SchemaError(format!("{e}")))?;
    validate_schema(&schema)?;
    Ok(schema)
}

fn validate_schema(schema: &Schema) -> Result<(), SchemaError> {
    if let Some(title) = &schema.document.title {
        validate_title(title)?;
    }
    if let Some(preamble) = &schema.document.preamble {
        validate_preamble(preamble)?;
    }
    for section in &schema.document.sections {
        validate_section(section)?;
    }
    Ok(())
}

fn validate_title(title: &Title) -> Result<(), SchemaError> {
    let Some(Extract::Capture { group, .. }) = &title.extract else {
        return Ok(());
    };
    // 書式2（名前付きキャプチャ）は pattern のキャプチャを取る。pattern が無い、
    // または pattern が指定の名前付きキャプチャを含まない題名は schema_invalid（R16）
    let Some(pattern) = &title.pattern else {
        return Err(SchemaError(
            "title capture extract requires a pattern".into(),
        ));
    };
    if !pattern.has_capture_group(group) {
        return Err(SchemaError(format!(
            "title pattern does not contain a named capture group \"{group}\""
        )));
    }
    Ok(())
}

fn validate_preamble(preamble: &Preamble) -> Result<(), SchemaError> {
    validate_fields(&preamble.fields)?;
    validate_statement(preamble.statement.as_ref())?;
    validate_bullets(preamble.bullets.as_ref())?;
    Ok(())
}

fn validate_section(section: &Section) -> Result<(), SchemaError> {
    if let Some(repeat) = &section.repeat {
        repeat.validate()?;
    }
    reject_capture_extract(section.extract.as_ref(), "section")?;
    validate_fields(&section.fields)?;
    validate_statement(section.statement.as_ref())?;
    validate_bullets(section.bullets.as_ref())?;
    if let Some(table) = &section.table {
        if let Some(repeat) = &table.repeat {
            repeat.validate()?;
        }
        reject_capture_extract(table.extract.as_ref(), "table")?;
    }
    if let Some(codeblock) = &section.codeblock {
        if let Some(repeat) = &codeblock.repeat {
            repeat.validate()?;
        }
        reject_capture_extract(codeblock.extract.as_ref(), "codeblock")?;
    }
    if let Some(item) = &section.item {
        validate_item(item)?;
    }
    Ok(())
}

fn validate_item(item: &Item) -> Result<(), SchemaError> {
    if let Some(repeat) = &item.repeat {
        repeat.validate()?;
    }
    reject_capture_extract(item.extract.as_ref(), "item")?;
    // 項目の内部のフィールド行・文・箇条書きには extract を宣言できない（R16）
    if let Some(field) = item.fields.iter().find(|f| f.extract.is_some()) {
        return Err(SchemaError(format!(
            "item field \"{}\" cannot declare extract",
            field.name
        )));
    }
    if let Some(statement) = &item.statement {
        if statement.extract.is_some() {
            return Err(SchemaError("item statement cannot declare extract".into()));
        }
    }
    if let Some(bullets) = &item.bullets {
        if bullets.extract.is_some() {
            return Err(SchemaError("item bullets cannot declare extract".into()));
        }
    }
    validate_fields(&item.fields)?;
    validate_statement(item.statement.as_ref())?;
    validate_bullets(item.bullets.as_ref())?;
    Ok(())
}

fn validate_fields(fields: &[Field]) -> Result<(), SchemaError> {
    for field in fields {
        if let Some(repeat) = &field.repeat {
            repeat.validate()?;
        }
        if let Some(when) = &field.when {
            when.validate()?;
        }
        reject_capture_extract(field.extract.as_ref(), "field")?;
    }
    Ok(())
}

fn validate_statement(statement: Option<&Statement>) -> Result<(), SchemaError> {
    if let Some(statement) = statement {
        if let Some(repeat) = &statement.repeat {
            repeat.validate()?;
        }
        if let Some(when) = &statement.when {
            when.validate()?;
        }
        reject_capture_extract(statement.extract.as_ref(), "statement")?;
    }
    Ok(())
}

fn validate_bullets(bullets: Option<&Bullets>) -> Result<(), SchemaError> {
    if let Some(bullets) = bullets {
        if let Some(repeat) = &bullets.repeat {
            repeat.validate()?;
        }
        if let Some(when) = &bullets.when {
            when.validate()?;
        }
        reject_capture_extract(bullets.extract.as_ref(), "bullets")?;
    }
    Ok(())
}

/// 題名以外のノードには書式2（名前付きキャプチャ）の抽出を宣言できない（R16）。
/// 宣言すると schema_invalid の停止になる。
fn reject_capture_extract(extract: Option<&Extract>, node: &str) -> Result<(), SchemaError> {
    if let Some(Extract::Capture { .. }) = extract {
        return Err(SchemaError(format!(
            "{node} extract cannot use the named-group capture form"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADR_LIKE: &str = r#"
name: adr
open: false
document:
  title:
    pattern: "^ADR-(?<id>\\d{4}):"
    extract: { path: id, group: id }
  preamble:
    fields:
      - name: 状態
        enum: [承認済み, 却下]
        extract: status
      - name: 日付
        pattern: "^\\d{4}-\\d{2}-\\d{2}"
        separator: ","
    statement:
      required: false
    bullets:
      pattern: "^[^-]"
  sections:
    - name: 状況
      extract: sections.context
      statement:
        required: false
      bullets:
        repeat: { min: 0 }
        extract: sections.reasons
      table:
        header: [用語, 意味]
      codeblock:
        lang: gherkin
        lines: ["^@", "^Scenario:"]
      item:
        id: "REQ-\\d{3,}"
        repeat: { min: 0, max: 5 }
        fields:
          - name: 種類
            enum: [algorithm, ubiquitous]
          - name: 定義
            when: { field: 種類, eq: algorithm }
"#;

    #[test]
    fn loads_a_schema_with_all_rule_kinds() {
        let schema = parse_schema(ADR_LIKE).unwrap();
        assert_eq!(schema.name.as_deref(), Some("adr"));
        assert!(!schema.open);
        assert!(schema.document.title.is_some());
        let preamble = schema.document.preamble.as_ref().unwrap();
        assert_eq!(preamble.fields.len(), 2);
        assert_eq!(preamble.fields[0].name, "状態");
        assert!(preamble.statement.is_some());
        assert!(preamble.bullets.is_some());
        let section = &schema.document.sections[0];
        assert_eq!(section.name, "状況");
        assert!(section.table.is_some());
        assert!(section.codeblock.is_some());
        let item = section.item.as_ref().unwrap();
        assert_eq!(item.id.as_ref().map(|p| p.source()), Some("REQ-\\d{3,}"));
        assert_eq!(item.repeat.as_ref().unwrap().max, Some(5));
        assert_eq!(
            item.fields[1].when.as_ref().unwrap().eq.as_deref(),
            Some("algorithm")
        );
    }

    #[test]
    fn csv_is_a_shorthand_for_comma_separator() {
        let yaml = r#"
document:
  preamble:
    fields:
      - name: タグ
        csv: true
"#;
        let schema = parse_schema(yaml).unwrap();
        let field = &schema.document.preamble.unwrap().fields[0];
        assert_eq!(field.effective_separator(), Some(","));
    }

    #[test]
    fn extract_accepts_string_and_capture_forms() {
        let yaml = r#"
document:
  title:
    pattern: "^ADR-(?<id>\\d{4})"
    extract: { path: id, group: id }
  preamble:
    fields:
      - name: 状態
        extract: status
"#;
        let schema = parse_schema(yaml).unwrap();
        assert!(matches!(
            schema.document.title.unwrap().extract,
            Some(Extract::Capture { ref path, ref group }) if path == "id" && group == "id"
        ));
        assert!(matches!(
            schema.document.preamble.unwrap().fields[0].extract,
            Some(Extract::Path(ref p)) if p == "status"
        ));
    }

    #[test]
    fn unknown_key_is_an_error() {
        let yaml = "document:\n  bogus: 1\n";
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn type_violation_is_an_error() {
        let yaml = "open: not-a-bool\ndocument:\n  title:\n    pattern: x\n";
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn min_greater_than_max_is_an_error() {
        let yaml = "document:\n  sections:\n    - name: x\n      repeat: { min: 3, max: 1 }\n";
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn negative_repeat_is_an_error() {
        let yaml = "document:\n  sections:\n    - name: x\n      repeat: { min: -1 }\n";
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn non_integer_repeat_is_an_error() {
        let yaml = "document:\n  sections:\n    - name: x\n      repeat: { min: 1.5 }\n";
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn when_with_both_operators_is_an_error() {
        let yaml = r#"
document:
  preamble:
    fields:
      - name: 定義
        when: { field: 種類, eq: a, ne: b }
"#;
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn when_with_no_operator_is_an_error() {
        let yaml = r#"
document:
  preamble:
    fields:
      - name: 定義
        when: { field: 種類 }
"#;
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn invalid_regex_pattern_is_an_error() {
        let yaml = "document:\n  title:\n    pattern: \"(\"\n";
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn invalid_item_id_regex_is_an_error() {
        let yaml = "document:\n  sections:\n    - name: x\n      item:\n        id: \"(\"\n";
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn capture_extract_outside_title_is_schema_invalid() {
        // 書式2（名前付きキャプチャ）は題名にだけ使える。題名以外のノードで
        // 宣言したときは schema_invalid の停止になる（R16）。
        let cases: &[(&str, &str)] = &[
            (
                "field",
                "document:\n  preamble:\n    fields:\n      - name: 状態\n        extract: { path: s, group: x }\n",
            ),
            (
                "statement",
                "document:\n  preamble:\n    statement:\n      extract: { path: s, group: x }\n",
            ),
            (
                "bullets",
                "document:\n  preamble:\n    bullets:\n      extract: { path: b, group: x }\n",
            ),
            (
                "section",
                "document:\n  sections:\n    - name: 状況\n      extract: { path: sec, group: x }\n",
            ),
            (
                "item",
                "document:\n  sections:\n    - name: 要求\n      item:\n        extract: { path: item, group: x }\n",
            ),
            (
                "table",
                "document:\n  sections:\n    - name: 用語集\n      table:\n        header: [a]\n        extract: { path: t, group: x }\n",
            ),
            (
                "codeblock",
                "document:\n  sections:\n    - name: コード\n      codeblock:\n        extract: { path: c, group: x }\n",
            ),
        ];
        for (node, yaml) in cases {
            assert!(
                parse_schema(yaml).is_err(),
                "{node} に Capture 形の extract を宣言したのに schema_invalid にならない"
            );
        }
    }

    #[test]
    fn item_internal_extract_is_schema_invalid() {
        let yaml = r#"
document:
  sections:
    - name: 要求
      item:
        fields:
          - name: 種類
            extract: kind
"#;
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn capture_extract_without_pattern_is_schema_invalid() {
        // 書式2の抽出は pattern の名前付きキャプチャを取る。pattern が無い
        // 題名は schema_invalid の停止になる（R16）。
        let yaml = r#"
document:
  title:
    extract: { path: id, group: id }
"#;
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn capture_extract_with_pattern_missing_the_group_is_schema_invalid() {
        // pattern が指定の名前付きキャプチャを含まない題名も schema_invalid（R16）。
        let yaml = r#"
document:
  title:
    pattern: "^ADR-(?<other>\\d{4}):"
    extract: { path: id, group: id }
"#;
        assert!(parse_schema(yaml).is_err());
    }

    #[test]
    fn not_yaml_is_an_error() {
        assert!(parse_schema("not: [valid: yaml").is_err());
    }
}
