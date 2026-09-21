//! スキーマの `extract` に沿って値を組み立てる。

use crate::document::{join_continuation, Block, Document, Item as DocItem, Section as DocSection};
use crate::schema::{
    is_declared_field, Bullets, Children, CodeBlock, Extract, Field, Schema, Statement, Table,
};
use serde_json::{Map, Value};

/// `values` の出力。配置パスに沿った入れ子の JSON。
pub fn extract_values(schema: &Schema, document: &Document) -> Value {
    let mut root = Map::new();
    extract_title(schema, document, &mut root);
    if let Some(preamble) = &schema.document.preamble {
        let blocks: Vec<&Block> = document.preamble.iter().collect();
        extract_fields(&preamble.fields, &blocks, &mut root);
        extract_statement(preamble.statement.as_ref(), &blocks, &mut root);
        extract_bullets(
            &preamble.fields,
            preamble.bullets.as_ref(),
            &blocks,
            &mut root,
        );
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

fn extract_section(
    def: &crate::schema::Section,
    document: &Document,
    root: &mut Map<String, Value>,
) {
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
                    .map(|s| Value::String(section_body(s, def)))
                    .collect();
                place(root, extract, Value::Array(bodies));
            }
        } else if let Some(section) = occurrences.first() {
            place(root, extract, Value::String(section_body(section, def)));
        }
    }

    let blocks: Vec<&Block> = occurrences.iter().flat_map(|s| s.blocks.iter()).collect();
    extract_fields(&def.fields, &blocks, root);
    extract_statement(def.statement.as_ref(), &blocks, root);
    extract_bullets(&def.fields, def.bullets.as_ref(), &blocks, root);
    extract_table(def.table.as_ref(), &blocks, root);
    extract_codeblock(def.codeblock.as_ref(), &blocks, root);

    if let Some(item) = &def.item {
        if let Some(extract) = &item.extract {
            let items: Vec<&DocItem> = occurrences.iter().flat_map(|s| s.items.iter()).collect();
            if items.is_empty() {
                // 0件のときはキーを省略する（R16）
            } else if item.repeat.is_some() {
                let values: Vec<Value> = items.iter().map(|i| item_value(i, item)).collect();
                place(root, extract, Value::Array(values));
            } else {
                place(root, extract, item_value(items[0], item));
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
                let values: Vec<Value> =
                    occurrences.iter().map(|b| field_single(field, b)).collect();
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
        Block::Field {
            value,
            continuation,
            ..
        } => (value.as_str(), continuation.as_slice()),
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
    fields: &[Field],
    bullets: Option<&Bullets>,
    blocks: &[&Block],
    root: &mut Map<String, Value>,
) {
    let Some(bullets) = bullets else {
        return;
    };
    // 子フィールドの抽出は箇条書きの下からだけ行う。宣言済みフィールド行の下は
    // 未宣言の構造（R13）で、そこにある子フィールド名の行を拾うと正当な子
    // フィールドの値を覆い隠す。宣言された名前と一致しない `- 名前: 値` 行は
    // 箇条書きとして扱う（R8）。
    let bullet_blocks: Vec<&Block> = blocks
        .iter()
        .copied()
        .filter(|b| {
            matches!(b, Block::Bullet { .. })
                || matches!(b, Block::Field { name, .. } if !is_declared_field(fields, name))
        })
        .collect();
    let Some(extract) = &bullets.extract else {
        // 子フィールドの抽出は、親の抽出が無くても子フィールド自身の extract に
        // 沿って行う（A15）
        if let Some(children) = &bullets.children {
            extract_child_fields(children, &bullet_blocks, root);
        }
        return;
    };
    // 抽出要素は元のマーカー行（元のマーカーを保つ）と子の箇条書きの行を
    // そのままのインデントで含める（R10・R16）。
    let texts: Vec<Value> = bullet_blocks
        .iter()
        .copied()
        .map(|b| Value::String(element_with_children(b, bullets.children.as_ref())))
        .collect();
    if texts.is_empty() {
        // 0件のときはキーを省略する（R16）
        return;
    }
    place(root, extract, Value::Array(texts));
    // 子フィールドは自身の extract を持てば、その配置パスに値を出す（A15）
    if let Some(children) = &bullets.children {
        extract_child_fields(children, &bullet_blocks, root);
    }
}

/// 子フィールドの抽出。`children.fields` で宣言された子フィールドのうち、自身の
/// `extract` を持つものをその配置パスに置く（R10・R16）。さらに深い入れ子の
/// 子フィールドは、子の箇条書きの `children` の宣言に沿って再帰する。
fn extract_child_fields(children: &Children, blocks: &[&Block], root: &mut Map<String, Value>) {
    for field in &children.fields {
        if let Some(extract) = &field.extract {
            let occurrences: Vec<&Block> = blocks
                .iter()
                .flat_map(|b| b.children().iter())
                .filter(|c| matches!(c, Block::Field { name, .. } if name == &field.name))
                .collect();
            if occurrences.is_empty() {
                // 0件のときはキーを省略する（R16）
                continue;
            }
            let value = if field.repeat.is_some() {
                let values: Vec<Value> =
                    occurrences.iter().map(|b| field_single(field, b)).collect();
                Value::Array(values)
            } else {
                field_single(field, occurrences[0])
            };
            place(root, extract, value);
        }
    }
    // 子の箇条書きの下の children に再帰する。子の箇条書きは Bullet か、この
    // レベルの宣言と一致しない `- 名前: 値` 行（箇条書きとして扱う行）である（R8）
    if let Some(child_bullets) = children.bullets.as_deref() {
        if let Some(grandchildren) = &child_bullets.children {
            let declared: Vec<&str> = children.fields.iter().map(|f| f.name.as_str()).collect();
            let bullet_blocks: Vec<&Block> = blocks
                .iter()
                .flat_map(|b| b.children().iter())
                .filter(|c| {
                    matches!(c, Block::Bullet { .. })
                        || matches!(
                            c,
                            Block::Field { name, .. } if !declared.contains(&name.as_str())
                        )
                })
                .collect();
            extract_child_fields(grandchildren, &bullet_blocks, root);
        }
    }
}

/// 箇条書きの抽出要素を、子の箇条書きの行を含めて組み立てる（R10・R16）。
/// 元の行、継続段落、子の箇条書きの行を改行でつなぐ。子の箇条書きの行は
/// そのままのインデントで含める。`children` の宣言でフィールド行として宣言された
/// 子の行は含めない（A12）。`children` が無ければ子はすべて箇条書きとして含める。
fn element_with_children(block: &Block, children: Option<&Children>) -> String {
    let base = match block {
        Block::Bullet { .. } => block.bullet_element(),
        Block::Field { .. } => block.field_element(),
        _ => return String::new(),
    };
    let child_blocks = block.children();
    if child_blocks.is_empty() {
        return base;
    }
    let declared: Vec<&str> = children
        .map(|c| c.fields.iter().map(|f| f.name.as_str()).collect())
        .unwrap_or_default();
    let child_rule = children.and_then(|c| c.bullets.as_deref());
    let child_lines: Vec<String> = child_blocks
        .iter()
        .filter_map(|child| match child {
            Block::Bullet { .. } => Some(element_with_children(
                child,
                child_rule.and_then(|b| b.children.as_ref()),
            )),
            Block::Field { name, .. } if !declared.contains(&name.as_str()) => Some(
                element_with_children(child, child_rule.and_then(|b| b.children.as_ref())),
            ),
            _ => None,
        })
        .collect();
    if child_lines.is_empty() {
        base
    } else {
        base + "\n" + &child_lines.join("\n")
    }
}

fn extract_table(table: Option<&Table>, blocks: &[&Block], root: &mut Map<String, Value>) {
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

fn section_body(section: &DocSection, def: &crate::schema::Section) -> String {
    body_from_blocks(&section.blocks, &def.fields, def.bullets.as_ref(), false)
}

fn item_value(item: &DocItem, item_rule: &crate::schema::Item) -> Value {
    let heading = if item.title.is_empty() {
        item.id.clone()
    } else {
        format!("{}: {}", item.id, item.title)
    };
    let body = body_from_blocks(
        &item.blocks,
        &item_rule.fields,
        item_rule.bullets.as_ref(),
        true,
    );
    if body.is_empty() {
        Value::String(heading)
    } else {
        Value::String(format!("{heading}\n{body}"))
    }
}

/// 本文を組み立てる。文と箇条書き（必要ならフィールド行も）を含み、
/// 表・コードブロック・項目は含めない。文どうしは空行、それ以外の
/// 隣接（文と箇条書き・文とフィールド行・箇条書きどうしなど）は改行で
/// つなぐ。箇条書きは R10 の抽出要素（継続段落と子の箇条書きの行を含む
/// 文字列）を使う。宣言された名前と一致しない `- 名前: 値` 行は箇条書きとして
/// 本文に含める（R8）。
fn body_from_blocks(
    blocks: &[Block],
    fields: &[Field],
    bullets: Option<&Bullets>,
    include_fields: bool,
) -> String {
    let mut out = String::new();
    let mut last_was_statement = false;
    for block in blocks {
        let (text, is_statement): (String, bool) = match block {
            Block::Statement { text, .. } => (text.clone(), true),
            Block::Bullet { .. } => (
                element_with_children(block, bullets.and_then(|b| b.children.as_ref())),
                false,
            ),
            Block::Field { name, .. } if include_fields && is_declared_field(fields, name) => {
                (block.field_element(), false)
            }
            Block::Field { name, .. } if include_fields || !is_declared_field(fields, name) => (
                element_with_children(block, bullets.and_then(|b| b.children.as_ref())),
                false,
            ),
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
    fn undeclared_field_name_line_is_extracted_as_a_bullet() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n- 判断の記録かどうかの見分け（A134: 決定の節の見出しを1つ以上持つファイル）\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["- 判断の記録かどうかの見分け（A134: 決定の節の見出しを1つ以上持つファイル）"]),
            "未宣言の名前の `- 名前: 値` 行は箇条書きとして抽出する（R8・R16）"
        );
    }

    #[test]
    fn section_body_includes_undeclared_field_name_line_as_a_bullet() {
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
        let doc = "## 状況\n\n- 判断の記録かどうかの見分け（A134: 決定の節の見出しを1つ以上持つファイル）\n";
        let v = values(schema, doc);
        assert_eq!(
            v["sections"]["body"],
            "- 判断の記録かどうかの見分け（A134: 決定の節の見出しを1つ以上持つファイル）",
            "節の本文はフィールド行を含めず、箇条書きとして含める（R16）"
        );
    }

    #[test]
    fn undeclared_field_name_line_extract_preserves_original_marker() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n* 判断の記録（A134: 決定の節）\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["* 判断の記録（A134: 決定の節）"]),
            "未宣言の名前の `* 名前: 値` 行は元のマーカーを保って箇条書きとして抽出する（R10）"
        );
    }

    #[test]
    fn section_body_includes_undeclared_field_name_line_with_original_marker() {
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
        let doc = "## 状況\n\n* 判断の記録（A134: 決定の節）\n";
        let v = values(schema, doc);
        assert_eq!(
            v["sections"]["body"], "* 判断の記録（A134: 決定の節）",
            "節の本文の箇条書き要素は元のマーカーを保つ（R16）"
        );
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
        assert!(
            v.get("reasons").is_none(),
            "箇条書き0件のときはキーを省略する"
        );
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
        // 子の箇条書きの行は、そのままのインデントで親の抽出要素に含める（R10）
        assert_eq!(v["sections"]["body"], "- 理由1\n続きの段落\n  - 入れ子");
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
    fn bullet_extract_preserves_whitespace_after_marker() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n-  親\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["-  親"]),
            "抽出の1要素は元の行（マーカーとその直後の空白を含む）を使う（R10）"
        );
    }

    #[test]
    fn bullet_extract_preserves_whitespace_after_marker_with_continuation() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n-  親\n\n    続きの段落\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["-  親\n続きの段落"]),
            "継続段落とつなぐときも元の行のマーカー直後の空白を保つ（R10）"
        );
    }

    #[test]
    fn bullet_extract_includes_lead_paragraph_on_a_separate_line_from_the_marker() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n- \n  親\n- 次\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["- \n親", "- 次"]),
            "マーカー行と別の行にある lead 段落の内容を抽出要素が欠落させない（R10）"
        );
    }

    #[test]
    fn bullet_extract_keeps_trailing_whitespace_of_the_marker_line() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n- 親  \n続き\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["- 親  \n続き"]),
            "ハード改行の末尾空白がマーカー行から剥がれない（R10）"
        );
    }

    #[test]
    fn undeclared_field_name_line_extract_preserves_whitespace_after_marker() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        repeat: { min: 0 }
        extract: reasons
"#;
        let doc = "## 理由\n\n-  判断の記録（A134: 決定の節）\n";
        let v = values(schema, doc);
        assert_eq!(
            v["reasons"],
            json!(["-  判断の記録（A134: 決定の節）"]),
            "未宣言の名前の行を箇条書きとして抽出するときも元の行を使う（R8・R10）"
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
            v["status"], "承認済み\n継続の段落",
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
        assert_eq!(
            v["items"],
            json!(["REQ-001: 名前\n- 種類: algorithm\n継続の説明"])
        );
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
        assert_eq!(
            v["items"],
            json!(["REQ-001: 名前\n- 種類: algorithm\n本文。"])
        );
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
        let doc =
            "## 用語集\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n| a | b |\n|---|---|\n| 3 | 4 |\n";
        let v = values(schema, doc);
        assert_eq!(
            v["glossary"],
            json!([{ "a": "1", "b": "2" }, { "a": "3", "b": "4" }])
        );
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

    #[test]
    fn parent_bullet_element_includes_child_bullet_lines_with_indentation() {
        let schema = r#"
document:
  sections:
    - name: 決定
      bullets:
        repeat: { min: 0 }
        extract: decisions
        children:
          bullets:
            repeat: { min: 0 }
"#;
        let doc = "## 決定\n\n- A22 判断の記録\n  - 補足の子\n";
        let v = values(schema, doc);
        assert_eq!(
            v["decisions"],
            json!(["- A22 判断の記録\n  - 補足の子"]),
            "親の抽出要素に子の箇条書きの行をそのままのインデントで含める（R10・R16）"
        );
    }

    #[test]
    fn declared_child_field_is_excluded_from_the_parent_element() {
        let schema = r#"
document:
  sections:
    - name: 決定
      bullets:
        repeat: { min: 0 }
        extract: decisions
        children:
          fields:
            - name: superseded_by
"#;
        let doc = "## 決定\n\n- A22 判断の記録\n  - superseded_by: [A5]\n";
        let v = values(schema, doc);
        assert_eq!(
            v["decisions"],
            json!(["- A22 判断の記録"]),
            "子フィールド行は親の抽出要素に含めない（A12）"
        );
    }

    #[test]
    fn child_field_with_own_extract_is_placed_at_its_path() {
        let schema = r#"
document:
  sections:
    - name: 決定
      bullets:
        repeat: { min: 0 }
        extract: decisions
        children:
          fields:
            - name: superseded_by
              extract: superseded_by
"#;
        let doc = "## 決定\n\n- A22 判断の記録\n  - superseded_by: [A5]\n";
        let v = values(schema, doc);
        assert_eq!(
            v["decisions"],
            json!(["- A22 判断の記録"]),
            "子フィールド行は親の抽出要素に含めない（A12）"
        );
        assert_eq!(
            v["superseded_by"], "[A5]",
            "子フィールドは自身の extract で配置パスに値を出す（A15）"
        );
    }

    #[test]
    fn child_of_declared_field_does_not_shadow_declared_child_field() {
        // 宣言済みフィールド行の下の子は未宣言の構造（R13）なので、子フィールドの
        // 抽出で拾わない。拾うと正当な子フィールドの値を覆い隠す。
        let schema = r#"
document:
  sections:
    - name: 決定
      fields:
        - name: 種類
      bullets:
        repeat: { min: 0 }
        extract: decisions
        children:
          fields:
            - name: superseded_by
              extract: superseded_by
"#;
        let doc = "## 決定\n\n- 種類: algorithm\n  - superseded_by: [余計]\n- A22 判断の記録\n  - superseded_by: [A5]\n";
        let v = values(schema, doc);
        assert_eq!(
            v["superseded_by"], "[A5]",
            "宣言済みフィールド行の下の子は、宣言済みの子フィールドの値を覆い隠さない（R13）"
        );
    }

    #[test]
    fn recursive_nesting_is_included_in_the_parent_element() {
        let schema = r#"
document:
  sections:
    - name: 決定
      bullets:
        repeat: { min: 0 }
        extract: decisions
        children:
          bullets:
            repeat: { min: 0 }
            children:
              bullets:
                repeat: { min: 0 }
"#;
        let doc = "## 決定\n\n- 親\n  - 子\n    - 孫\n";
        let v = values(schema, doc);
        assert_eq!(
            v["decisions"],
            json!(["- 親\n  - 子\n    - 孫"]),
            "再帰的な入れ子が親の抽出要素に含まれる（R16）"
        );
    }

    #[test]
    fn section_body_includes_child_bullet_lines() {
        let schema = r#"
document:
  sections:
    - name: 決定
      extract: sections.body
      bullets:
        repeat: { min: 0 }
        children:
          bullets:
            repeat: { min: 0 }
"#;
        let doc = "## 決定\n\n- 親\n  - 子\n";
        let v = values(schema, doc);
        assert_eq!(
            v["sections"]["body"], "- 親\n  - 子",
            "節の本文は子の箇条書きの行を含む（R16）"
        );
    }

    #[test]
    fn item_body_includes_child_bullet_lines() {
        let schema = r#"
document:
  sections:
    - name: 要求
      item:
        repeat: { min: 0 }
        extract: items
        bullets:
          repeat: { min: 0 }
"#;
        let doc = "## 要求\n\n### REQ-001: 名前\n\n- 親\n  - 子\n";
        let v = values(schema, doc);
        assert_eq!(
            v["items"],
            json!(["REQ-001: 名前\n- 親\n  - 子"]),
            "項目の本文は子の箇条書きの行を含む（R16）"
        );
    }

    #[test]
    fn deep_child_field_extract_is_placed_at_its_path() {
        let schema = r#"
document:
  sections:
    - name: 決定
      bullets:
        repeat: { min: 0 }
        extract: decisions
        children:
          bullets:
            repeat: { min: 0 }
            children:
              fields:
                - name: 補足
                  extract: notes
"#;
        let doc = "## 決定\n\n- 親\n  - 子\n    - 補足: 深い\n";
        let v = values(schema, doc);
        assert_eq!(
            v["decisions"],
            json!(["- 親\n  - 子"]),
            "深いレベルの宣言された子フィールドも親の抽出要素に含めない（R10）"
        );
        assert_eq!(
            v["notes"], "深い",
            "深いレベルの子フィールドも自身の extract で配置パスに値を出す（A15）"
        );
    }

    #[test]
    fn item_body_excludes_table_and_code_block() {
        let schema = r#"
document:
  sections:
    - name: 決定表
      item:
        id: "TBL-\\d{3,}"
        repeat: { min: 0 }
        extract: tables
        fields:
          - name: 出典
        table:
          header: [用語, 意味]
"#;
        let doc = "## 決定表\n\n### TBL-001: 名前\n\n- 出典: docs/a.md\n\n| 用語 | 意味 |\n|---|---|\n| 印 | テストの印 |\n";
        let v = values(schema, doc);
        assert_eq!(
            v["tables"],
            json!(["TBL-001: 名前\n- 出典: docs/a.md"]),
            "項目の抽出の本文に表を含めない（R16）"
        );
    }
}
