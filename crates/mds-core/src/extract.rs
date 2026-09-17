//! スキーマの `extract` に沿って値を組み立てる。

use crate::document::{
    join_continuation, Block, Document, Item as DocItem, Section as DocSection,
};
use crate::schema::{Bullets, CodeBlock, Extract, Field, Schema, Statement, Table};
use serde_json::{Map, Value};

/// `values` の出力。配置パスに沿った入れ子の JSON。
pub fn extract_values(schema: &Schema, document: &Document) -> Value {
    let mut root = Map::new();
    extract_title(schema, document, &mut root);
    if let Some(preamble) = &schema.document.preamble {
        let blocks: Vec<&Block> = document.preamble.iter().collect();
        extract_fields(&preamble.fields, &blocks, &mut root);
        extract_statement(preamble.statement.as_ref(), &blocks, &mut root);
        extract_bullets(preamble.bullets.as_ref(), &blocks, &mut root);
    }
    for def in &schema.document.sections {
        extract_section(def, document, &mut root);
    }
    Value::Object(root)
}

/// `ast --schema` の出力。抽出値に、スキーマの `name` を `type` として添える。
pub fn extract_typed(schema: &Schema, document: &Document) -> Value {
    let values = extract_values(schema, document);
    let Value::Object(values) = values else {
        return values;
    };
    let mut out = Map::new();
    if let Some(name) = &schema.name {
        out.insert("type".to_string(), Value::String(name.clone()));
    }
    for (key, value) in values {
        out.insert(key, value);
    }
    Value::Object(out)
}

/// `values` の text 出力。ネストは2文字のインデント、配列は番号付き。
pub fn render_values_text(value: &Value) -> String {
    let mut out = String::new();
    render_text(value, &mut out, 0);
    out
}

fn extract_title(schema: &Schema, document: &Document, root: &mut Map<String, Value>) {
    let Some(title) = &schema.document.title else {
        return;
    };
    let Some(extract) = &title.extract else {
        return;
    };
    let Some(heading) = document.titles.first() else {
        return;
    };
    let value = match extract {
        Extract::Path(_) => Value::String(heading.text.clone()),
        Extract::Capture { group, .. } => {
            let Some(pattern) = &title.pattern else {
                return;
            };
            let Some(caps) = pattern.captures(&heading.text) else {
                return;
            };
            let Some(m) = caps.name(group) else {
                return;
            };
            Value::String(m.as_str().to_string())
        }
    };
    place(root, extract, value);
}

fn extract_section(def: &crate::schema::Section, document: &Document, root: &mut Map<String, Value>) {
    let occurrences: Vec<&DocSection> = document
        .sections
        .iter()
        .filter(|s| s.name == def.name)
        .collect();

    if let Some(extract) = &def.extract {
        if def.repeat.is_some() {
            if !occurrences.is_empty() {
                let bodies: Vec<Value> = occurrences
                    .iter()
                    .map(|s| Value::String(section_body(s)))
                    .collect();
                place(root, extract, Value::Array(bodies));
            }
        } else if let Some(section) = occurrences.first() {
            place(root, extract, Value::String(section_body(section)));
        }
    }

    let blocks: Vec<&Block> = occurrences
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();
    extract_fields(&def.fields, &blocks, root);
    extract_statement(def.statement.as_ref(), &blocks, root);
    extract_bullets(def.bullets.as_ref(), &blocks, root);
    extract_table(def.table.as_ref(), &blocks, root);
    extract_codeblock(def.codeblock.as_ref(), &blocks, root);

    if let Some(item) = &def.item {
        if let Some(extract) = &item.extract {
            let items: Vec<&DocItem> = occurrences
                .iter()
                .flat_map(|s| s.items.iter())
                .collect();
            if items.is_empty() {
                // 0件のときはキーを省略する（R16）
            } else if item.repeat.is_some() {
                let values: Vec<Value> = items.iter().map(|i| item_value(i)).collect();
                place(root, extract, Value::Array(values));
            } else {
                place(root, extract, item_value(items[0]));
            }
        }
    }
}

fn extract_fields(fields: &[Field], blocks: &[&Block], root: &mut Map<String, Value>) {
    for field in fields {
        if let Some(extract) = &field.extract {
            let occurrences: Vec<&Block> = blocks
                .iter()
                .copied()
                .filter(|b| matches!(b, Block::Field { name, .. } if name == &field.name))
                .collect();
            if occurrences.is_empty() {
                // 0件のときはキーを省略する（R16）
                continue;
            }
            let value = if field.repeat.is_some() {
                let values: Vec<Value> = occurrences.iter().map(|b| field_single(field, b)).collect();
                Value::Array(values)
            } else {
                field_single(field, occurrences[0])
            };
            place(root, extract, value);
        }
    }
}

fn field_single(field: &Field, block: &Block) -> Value {
    let (value, continuation) = match block {
        Block::Field { value, continuation, .. } => (value.as_str(), continuation.as_slice()),
        _ => ("", &[][..]),
    };
    match field.effective_separator() {
        // 区切った要素は前後の空白を取り除いて抽出し、空の要素は空文字列として残す（R8）。
        // 分割は継続段落を含めない値だけを対象にし、継続段落は末尾の要素に改行で付ける（R8・R16）
        Some(sep) => {
            let mut elements: Vec<String> =
                value.split(sep).map(|s| s.trim().to_string()).collect();
            if let Some(last) = elements.last_mut() {
                join_continuation(last, continuation);
            }
            Value::Array(elements.into_iter().map(Value::String).collect())
        }
        None => {
            // 継続段落はフィールド行の一部で、値と改行でつなぐ（R8・R16）。複数あるときは
            // 箇条書きと同じく空行でつなぐ（R10）
            let mut full = value.to_string();
            join_continuation(&mut full, continuation);
            Value::String(full)
        }
    }
}

fn extract_statement(
    statement: Option<&Statement>,
    blocks: &[&Block],
    root: &mut Map<String, Value>,
) {
    let Some(statement) = statement else {
        return;
    };
    let Some(extract) = &statement.extract else {
        return;
    };
    let texts: Vec<&str> = blocks
        .iter()
        .copied()
        .filter_map(|b| match b {
            Block::Statement { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    if texts.is_empty() {
        // 0件のときはキーを省略する（R16）
        return;
    }
    let value = if statement.repeat.is_some() {
        Value::Array(texts.iter().map(|t| Value::String(t.to_string())).collect())
    } else {
        Value::String(texts.join("\n\n"))
    };
    place(root, extract, value);
}

fn extract_bullets(
    bullets: Option<&Bullets>,
    blocks: &[&Block],
    root: &mut Map<String, Value>,
) {
    let Some(bullets) = bullets else {
        return;
    };
    let Some(extract) = &bullets.extract else {
        return;
    };
    let texts: Vec<Value> = blocks
        .iter()
        .copied()
        .filter_map(|b| {
            if matches!(b, Block::Bullet { .. }) {
                Some(Value::String(b.bullet_element()))
            } else {
                None
            }
        })
        .collect();
    if texts.is_empty() {
        // 0件のときはキーを省略する（R16）
        return;
    }
    place(root, extract, Value::Array(texts));
}

fn extract_table(
    table: Option<&Table>,
    blocks: &[&Block],
    root: &mut Map<String, Value>,
) {
    let Some(table) = table else {
        return;
    };
    let Some(extract) = &table.extract else {
        return;
    };
    let tables: Vec<&Block> = blocks
        .iter()
        .copied()
        .filter(|b| matches!(b, Block::Table { .. }))
        .collect();
    if tables.is_empty() {
        // 0件のときはキーを省略する（R16）
        return;
    }
    // 表は repeat の宣言に関わらず常に配列。複数の表は現れた順に1つの配列へ連結する（R16）
    let rows: Vec<Value> = tables.iter().flat_map(|b| table_objects(b)).collect();
    place(root, extract, Value::Array(rows));
}

fn table_objects(block: &Block) -> Vec<Value> {
    match block {
        Block::Table { header, rows, .. } => rows
            .iter()
            .map(|row| {
                let mut map = Map::new();
                for (i, cell) in row.iter().enumerate() {
                    if let Some(key) = header.get(i) {
                        map.insert(key.clone(), Value::String(cell.clone()));
                    }
                }
                Value::Object(map)
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn extract_codeblock(
    codeblock: Option<&CodeBlock>,
    blocks: &[&Block],
    root: &mut Map<String, Value>,
) {
    let Some(codeblock) = codeblock else {
        return;
    };
    let Some(extract) = &codeblock.extract else {
        return;
    };
    let codes: Vec<&Block> = blocks
        .iter()
        .copied()
        .filter(|b| matches!(b, Block::Code { .. }))
        .collect();
    if codes.is_empty() {
        // 0件のときはキーを省略する（R16）
        return;
    }
    let values: Vec<Value> = codes
        .iter()
        .map(|b| match b {
            Block::Code { value, .. } => Value::String(value.clone()),
            _ => Value::Null,
        })
        .collect();
    let value = if codeblock.repeat.is_some() {
        Value::Array(values)
    } else {
        values.into_iter().next().unwrap()
    };
    place(root, extract, value);
}

fn section_body(section: &DocSection) -> String {
    body_from_blocks(&section.blocks, false)
}

fn item_value(item: &DocItem) -> Value {
    let heading = if item.title.is_empty() {
        item.id.clone()
    } else {
        format!("{}: {}", item.id, item.title)
    };
    let body = body_from_blocks(&item.blocks, true);
    if body.is_empty() {
        Value::String(heading)
    } else {
        Value::String(format!("{heading}\n{body}"))
    }
}

/// 本文を組み立てる。文と箇条書き（必要ならフィールド行も）を含み、
/// 表・コードブロック・項目は含めない。文どうしは空行、それ以外の
/// 隣接（文と箇条書き・文とフィールド行・箇条書きどうしなど）は改行で
/// つなぐ。箇条書きは R10 の抽出要素（継続段落を含む文字列）を使う。
fn body_from_blocks(blocks: &[Block], include_fields: bool) -> String {
    let mut out = String::new();
    let mut last_was_statement = false;
    for block in blocks {
        let (text, is_statement): (String, bool) = match block {
            Block::Statement { text, .. } => (text.clone(), true),
            Block::Bullet { .. } => (block.bullet_element(), false),
            Block::Field { .. } if include_fields => (block.field_element(), false),
            _ => continue,
        };
        if !out.is_empty() {
            out.push_str(if is_statement && last_was_statement {
                "\n\n"
            } else {
                "\n"
            });
        }
        out.push_str(&text);
        last_was_statement = is_statement;
    }
    out
}

fn place(root: &mut Map<String, Value>, extract: &Extract, value: Value) {
    let path = match extract {
        Extract::Path(p) => p.clone(),
        Extract::Capture { path, .. } => path.clone(),
    };
    let keys: Vec<&str> = path.split('.').collect();
    if keys.is_empty() {
        return;
    }
    let mut current = root;
    for key in &keys[..keys.len() - 1] {
        let entry = current
            .entry(key.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        current = entry.as_object_mut().unwrap();
    }
    current.insert(keys[keys.len() - 1].to_string(), value);
}

fn render_text(value: &Value, out: &mut String, indent: usize) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                match val {
                    Value::Object(_) | Value::Array(_) => {
                        push_indent(out, indent);
                        out.push_str(&format!("{key}:\n"));
                        render_text(val, out, indent + 2);
                    }
                    Value::String(s) => {
                        push_indent(out, indent);
                        out.push_str(key);
                        out.push_str(": ");
                        push_string(out, indent + 2, s);
                        out.push('\n');
                    }
                    other => {
                        push_indent(out, indent);
                        out.push_str(&format!("{key}: {other}\n"));
                    }
                }
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                match item {
                    Value::Object(_) | Value::Array(_) => {
                        push_indent(out, indent);
                        out.push_str(&format!("{}.\n", i + 1));
                        render_text(item, out, indent + 2);
                    }
                    Value::String(s) => {
                        push_indent(out, indent);
                        out.push_str(&format!("{}. ", i + 1));
                        push_string(out, indent + 2, s);
                        out.push('\n');
                    }
                    other => {
                        push_indent(out, indent);
                        out.push_str(&format!("{}. {other}\n", i + 1));
                    }
                }
            }
        }
        _ => {}
    }
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push(' ');
    }
}

/// 値の文字列を出力する。改行を含む値は続きの行をインデントし、1件の値として
/// 読めるようにする。末尾の改行は表示の邪魔なので落とす（json の値は変えない）。
fn push_string(out: &mut String, indent: usize, s: &str) {
    for (i, line) in s.trim_end_matches('\n').split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            push_indent(out, indent);
        }
        out.push_str(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::schema::parse_schema;
    use serde_json::json;

    fn values(schema_yaml: &str, doc: &str) -> serde_json::Value {
        let schema = parse_schema(schema_yaml).unwrap();
        let document = Document::parse(doc).unwrap();
        extract_values(&schema, &document)
    }

    const ADR: &str = r#"
name: adr
document:
  title:
    pattern: "^ADR-\\d{4}:"
    extract: title
  preamble:
    fields:
      - name: ID
        extract: id
      - name: 状態
        extract: status
      - name: 日付
        extract: date
  sections:
    - name: 状況
      extract: sections.context
      statement:
        required: false
    - name: 決定
      extract: sections.decision
      statement:
        required: false
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: sections.reasons
"#;

    fn adr_doc() -> &'static str {
        "# ADR-0001: 印の話\n\n- ID: ADR-0001\n- 状態: 承認済み\n- 日付: 2026-09-16\n\n## 状況\n\n背景。\n\n## 決定\n\n判断。\n\n## 理由\n\n- 理由1\n- 理由2\n"
    }

    #[test]
    fn adr_values_are_nested_by_path() {
        let v = values(ADR, adr_doc());
        assert_eq!(v["id"], "ADR-0001");
        assert_eq!(v["title"], "ADR-0001: 印の話");
        assert_eq!(v["status"], "承認済み");
        assert_eq!(v["date"], "2026-09-16");
        assert_eq!(v["sections"]["context"], "背景。");
        assert_eq!(v["sections"]["decision"], "判断。");
        assert_eq!(v["sections"]["reasons"], json!(["- 理由1", "- 理由2"]));
    }

    #[test]
    fn title_named_group_capture_is_extracted() {
        let schema = r#"
document:
  title:
    pattern: "^ADR-(?<id>\\d{4}):"
    extract: { path: id, group: id }
"#;
        let v = values(schema, "# ADR-0042: 話\n");
        assert_eq!(v["id"], "0042");
    }

    #[test]
    fn repeated_node_is_an_array_when_present_and_omits_key_when_absent() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        repeat: { min: 0, max: 1 }
        extract: tags
"#;
        let present = values(schema, "# 題名\n\n- タグ: a\n");
        assert_eq!(present["tags"], json!(["a"]));
        let absent = values(schema, "# 題名\n");
        assert!(absent.get("tags").is_none(), "0件のときはキーを省略する");
    }

    #[test]
    fn non_repeated_node_is_single_and_omits_key_when_missing() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: 状態
        extract: status
"#;
        let present = values(schema, "# 題名\n\n- 状態: 承認済み\n");
        assert_eq!(present["status"], "承認済み");
        let absent = values(schema, "# 題名\n");
        assert!(absent.get("status").is_none());
    }

    #[test]
    fn non_repeated_bullets_omit_key_when_missing() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        extract: reasons
"#;
        let v = values(schema, "## 理由\n");
        assert!(v.get("reasons").is_none());
    }

    #[test]
    fn repeated_bullets_omit_key_when_missing() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let v = values(schema, "## 理由\n");
        assert!(v.get("reasons").is_none(), "箇条書き0件のときはキーを省略する");
    }

    #[test]
    fn separator_trims_elements_and_keeps_empty_ones() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        separator: ","
        extract: tags
"#;
        let v = values(schema, "# 題名\n\n- タグ: a, b,,c,\n");
        assert_eq!(v["tags"], json!(["a", "b", "", "c", ""]));
    }

    #[test]
    fn repeated_field_with_separator_extracts_array_of_arrays() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        repeat: { min: 0 }
        separator: ","
        extract: tags
"#;
        let v = values(schema, "# 題名\n\n- タグ: a,b\n- タグ: c\n");
        assert_eq!(v["tags"], json!([["a", "b"], ["c"]]));
    }

    #[test]
    fn separator_split_excludes_continuation_paragraphs() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        separator: ","
        extract: tags
"#;
        let doc = "# 題名\n\n- タグ: a,b\n\n  継続,の段落\n";
        let v = values(schema, doc);
        assert_eq!(
            v["tags"],
            json!(["a", "b\n継続,の段落"]),
            "継続段落は値の分割に含めず、末尾の要素に改行で付ける（R8・R16）"
        );
    }

    #[test]
    fn wrapped_line_is_part_of_value_and_subject_to_separator_split() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        separator: ","
        extract: tags
"#;
        let doc = "# 題名\n\n- タグ: a,b\n折り返し,c\n";
        let v = values(schema, doc);
        assert_eq!(
            v["tags"],
            json!(["a", "b\n折り返し", "c"]),
            "空行なしの折り返し行は値の一部で、separator の分割対象になる（R8）。継続段落のように末尾要素に付かない"
        );
    }

    #[test]
    fn separator_splits_value_into_string_elements() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        separator: ","
        extract: tags
"#;
        let v = values(schema, "# 題名\n\n- タグ: a,b,c\n");
        assert_eq!(v["tags"], json!(["a", "b", "c"]));
    }

    #[test]
    fn paragraph_after_code_block_lead_is_extracted_as_a_statement() {
        let schema = r#"
document:
  sections:
    - name: 状況
      statement:
        required: false
        extract: note
      codeblock:
        required: false
        lang: python
      bullets:
        repeat: { min: 0 }
"#;
        let doc = "## 状況\n\n- ```python\n  x = 1\n  ```\n\n  後続の段落\n";
        let v = values(schema, doc);
        assert_eq!(
            v["note"], "後続の段落",
            "先頭がコードブロックのリスト項目の後続段落は文として抽出する"
        );
    }

    #[test]
    fn section_body_includes_statements_and_bullets_only() {
        let schema = r#"
document:
  sections:
    - name: 状況
      extract: sections.body
      statement:
        required: false
      bullets:
        repeat: { min: 0 }
      table:
        required: false
        header: [a]
      codeblock:
        required: false
        lang: gherkin
"#;
        let doc = "## 状況\n\n背景。\n\n- 箇条1\n- 箇条2\n\n| a |\n|---|\n| x |\n\n```gherkin\nScenario: 例\n```\n";
        let v = values(schema, doc);
        // 文と箇条書きの間は改行、文どうしだけ空行でつなぐ
        assert_eq!(v["sections"]["body"], "背景。\n- 箇条1\n- 箇条2");
    }

    #[test]
    fn section_body_includes_continuation_paragraphs_and_nested_bullets() {
        let schema = r#"
document:
  sections:
    - name: 状況
      extract: sections.body
      statement:
        required: false
      bullets:
        repeat: { min: 0 }
"#;
        let doc = "## 状況\n\n- 理由1\n\n  続きの段落\n  - 入れ子\n";
        let v = values(schema, doc);
        assert_eq!(v["sections"]["body"], "- 理由1\n続きの段落\n- 入れ子");
    }

    #[test]
    fn bullet_extract_preserves_original_markers() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n* 星\n\n+ プラス\n\n- ハイフン\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["* 星", "+ プラス", "- ハイフン"]),
            "箇条書きの抽出は元のマーカーを保つ（R10）"
        );
    }

    #[test]
    fn bullet_extract_preserves_marker_with_continuation() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n* 親\n\n  続きの段落\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["* 親\n続きの段落"]),
            "継続段落を付けるときも元のマーカーを保つ（R10）"
        );
    }

    #[test]
    fn bullet_extract_includes_marker_and_continuation() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n- 親\n\n  続きの段落\n\n- 次\n";
        let v = values(schema, doc);
        assert_eq!(v["reasons"], json!(["- 親\n続きの段落", "- 次"]));
    }

    #[test]
    fn bullet_with_multiple_continuations_joins_them_with_blank_line() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n- 親\n\n  続き1\n\n  続き2\n";
        let v = values(schema, doc);
        assert_eq!(v["reasons"], json!(["- 親\n続き1\n\n続き2"]));
    }

    #[test]
    fn ordered_list_is_not_extracted_as_a_bullet() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n1. 順序付き\n\n- 箇条書き\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["- 箇条書き"]),
            "順序付きリストは箇条書きの対象外で抽出に含めない（R10）"
        );
    }

    #[test]
    fn field_extract_includes_continuation_paragraph() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: 状態
        extract: status
"#;
        let doc = "# 題名\n\n- 状態: 承認済み\n\n  継続の段落\n";
        let v = values(schema, doc);
        assert_eq!(
            v["status"],
            "承認済み\n継続の段落",
            "フィールド行の継続段落は値に改行でつなぐ（R8・R16）"
        );
    }

    #[test]
    fn item_body_field_line_includes_continuation_paragraph() {
        let schema = r#"
document:
  sections:
    - name: 要求
      item:
        repeat: { min: 0 }
        extract: items
        fields:
          - name: 種類
        statement:
          required: false
"#;
        let doc = "## 要求\n\n### REQ-001: 名前\n\n- 種類: algorithm\n\n  継続の説明\n";
        let v = values(schema, doc);
        // 継続段落はフィールド行の一部なので、項目の本文のフィールド行にも含める（R8・R16）
        assert_eq!(v["items"], json!(["REQ-001: 名前\n- 種類: algorithm\n継続の説明"]));
    }

    #[test]
    fn item_extract_is_heading_plus_body() {
        let schema = r#"
document:
  sections:
    - name: 要求
      item:
        repeat: { min: 0 }
        extract: items
        fields:
          - name: 種類
        statement:
          required: false
"#;
        let doc = "## 要求\n\n### REQ-001: 名前\n\n- 種類: algorithm\n\n本文。\n";
        let v = values(schema, doc);
        // 見出しと本文は改行でつなぎ、本文内は文どうしだけ空行でつなぐ
        assert_eq!(v["items"], json!(["REQ-001: 名前\n- 種類: algorithm\n本文。"]));
    }

    #[test]
    fn table_extract_concatenates_multiple_tables_in_document_order() {
        let schema = r#"
document:
  sections:
    - name: 用語集
      table:
        required: false
        header: [a, b]
        extract: glossary
"#;
        let doc = "## 用語集\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n| a | b |\n|---|---|\n| 3 | 4 |\n";
        let v = values(schema, doc);
        assert_eq!(v["glossary"], json!([{ "a": "1", "b": "2" }, { "a": "3", "b": "4" }]));
    }

    #[test]
    fn repeated_table_extracts_flat_row_objects_without_nesting() {
        let schema = r#"
document:
  sections:
    - name: 用語集
      table:
        repeat: { min: 0 }
        header: [a, b]
        extract: glossary
"#;
        let doc = "## 用語集\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let v = values(schema, doc);
        assert_eq!(v["glossary"], json!([{ "a": "1", "b": "2" }]));
    }

    #[test]
    fn table_omits_key_when_no_tables() {
        let schema = r#"
document:
  sections:
    - name: 用語集
      table:
        repeat: { min: 0 }
        header: [a, b]
        extract: glossary
"#;
        let v = values(schema, "## 用語集\n");
        assert!(v.get("glossary").is_none(), "表0件のときはキーを省略する");
    }

    #[test]
    fn table_extract_keys_by_header_and_later_column_overrides() {
        let schema = r#"
document:
  sections:
    - name: 用語集
      table:
        header: [a, b, a]
        extract: glossary
"#;
        let doc = "## 用語集\n\n| a | b | a |\n|---|---|---|\n| 1 | 2 | 3 |\n";
        let v = values(schema, doc);
        assert_eq!(v["glossary"], json!([{ "a": "3", "b": "2" }]));
    }

    #[test]
    fn repeated_section_extracts_array_of_bodies() {
        let schema = r#"
document:
  sections:
    - name: 状況
      repeat: { min: 0 }
      extract: contexts
      statement:
        required: false
"#;
        let doc = "## 状況\n\n1つ目。\n\n## 状況\n\n2つ目。\n";
        let v = values(schema, doc);
        assert_eq!(v["contexts"], json!(["1つ目。", "2つ目。"]));
    }

    fn typed(schema_yaml: &str, doc: &str) -> serde_json::Value {
        let schema = parse_schema(schema_yaml).unwrap();
        let document = Document::parse(doc).unwrap();
        extract_typed(&schema, &document)
    }

    #[test]
    fn typed_ast_includes_type_when_schema_is_named() {
        let schema = "name: adr\ndocument:\n  title:\n    extract: title\n";
        let v = typed(schema, "# 題名\n");
        assert_eq!(v["type"], "adr");
        assert_eq!(v["title"], "題名");
    }

    #[test]
    fn typed_ast_omits_type_when_schema_is_unnamed() {
        let schema = "document:\n  title:\n    extract: title\n";
        let v = typed(schema, "# 題名\n");
        assert!(v.get("type").is_none());
        assert_eq!(v["title"], "題名");
    }
}