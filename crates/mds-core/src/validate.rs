//! 文書の構造と閉じた世界の検証。

use crate::document::{Block, Document, Heading, Item};
use crate::finding::{Finding, FindingKind};
use crate::schema::{
    Bullets, CodeBlock, Field, Item as ItemRule, Preamble, Repeat, Schema, Section, Statement,
    Table, Title, When,
};
use std::collections::HashMap;

/// スキーマと文書の木から指摘を集める。`open` は閉じた世界を緩めるか（スキーマの `open` と CLI の `--open` を合わせた値）。
pub fn validate(schema: &Schema, document: &Document, open: bool) -> Vec<Finding> {
    let mut findings = Vec::new();
    let doc_rule = &schema.document;

    if let Some(title) = &doc_rule.title {
        validate_title(title, &document.titles, &mut findings);
    } else if !open {
        // title 規則が無い `#` 見出しは閉じた世界で undeclared_heading（R13）
        for heading in &document.titles {
            findings.push(Finding {
                kind: FindingKind::UndeclaredHeading,
                line: Some(heading.line),
                detail: format!("undeclared title heading \"{}\"", heading.text),
            });
        }
    }

    // 宣言済みの前置部は open でも未宣言の行を誤りにする。宣言していない
    // 前置部の内側の行は閉じた世界だけで誤りにする。
    let effective_open = if doc_rule.preamble.is_some() { false } else { open };
    let rules = ContainerRules::for_preamble(doc_rule.preamble.as_ref());
    validate_container(&rules, &document.preamble, effective_open, &mut findings);

    let mut section_counts: HashMap<&str, usize> = HashMap::new();
    for section in &document.sections {
        *section_counts.entry(section.name.as_str()).or_insert(0) += 1;
    }

    for section in &document.sections {
        match doc_rule.sections.iter().find(|def| def.name == section.name) {
            Some(def) => {
                let rules = ContainerRules::for_section(def);
                validate_container(&rules, &section.blocks, false, &mut findings);
                validate_items(def, &section.items, &mut findings);
            }
            None => {
                // 宣言していない節は、閉じた世界で見出しと内側の行を誤りにする
                if !open {
                    findings.push(Finding {
                        kind: FindingKind::UndeclaredHeading,
                        line: Some(section.line),
                        detail: format!("undeclared section heading \"{}\"", section.name),
                    });
                    for block in &section.blocks {
                        push_undeclared_line(&mut findings, block);
                    }
                    for item in &section.items {
                        findings.push(Finding {
                            kind: FindingKind::UndeclaredHeading,
                            line: Some(item.line),
                            detail: format!("undeclared item heading \"{}\"", item.id),
                        });
                        for block in &item.blocks {
                            push_undeclared_line(&mut findings, block);
                        }
                    }
                }
            }
        }
    }

    for def in &doc_rule.sections {
        let count = section_counts
            .get(def.name.as_str())
            .copied()
            .unwrap_or(0) as u64;
        let (min, max) = bounds(def.required, def.repeat.as_ref());
        check_occurrence(
            count,
            min,
            max,
            def.repeat.is_some(),
            FindingKind::MissingRequiredSection,
            &format!("section \"{}\"", def.name),
            &mut findings,
        );
    }

    for heading in &document.stray_headings {
        validate_stray_heading(heading, open, &mut findings);
    }

    findings
}

fn validate_title(title: &Title, headings: &[Heading], findings: &mut Vec<Finding>) {
    match headings.len() {
        0 => findings.push(Finding {
            kind: FindingKind::MissingTitle,
            line: None,
            detail: "the document has no level-1 heading".into(),
        }),
        n if n > 1 => findings.push(Finding {
            kind: FindingKind::MultipleTitles,
            line: Some(headings[1].line),
            detail: "the document has more than one level-1 heading".into(),
        }),
        _ => {
            let heading = &headings[0];
            if let Some(pattern) = &title.pattern {
                if !pattern.is_match(&heading.text) {
                    findings.push(Finding {
                        kind: FindingKind::TitlePatternMismatch,
                        line: Some(heading.line),
                        detail: format!(
                            "title \"{}\" does not match pattern \"{}\"",
                            heading.text,
                            pattern.source()
                        ),
                    });
                }
            }
        }
    }
}

fn validate_items(section: &Section, items: &[Item], findings: &mut Vec<Finding>) {
    let Some(item_rule) = &section.item else {
        // 宣言済みの節の中に足された未宣言の項目。open でも見出しと内側の行を誤りにする（R13）
        for item in items {
            findings.push(Finding {
                kind: FindingKind::UndeclaredHeading,
                line: Some(item.line),
                detail: format!("undeclared item heading \"{}\"", item.id),
            });
            for block in &item.blocks {
                push_undeclared_line(findings, block);
            }
        }
        return;
    };

    for item in items {
        if !item.has_id_separator {
            findings.push(Finding {
                kind: FindingKind::InvalidId,
                line: Some(item.line),
                detail: format!("item heading \"{}\" has no \":\" separator", item.id),
            });
        } else if let Some(id_pattern) = &item_rule.id {
            if !id_pattern.is_match(&item.id) {
                findings.push(Finding {
                    kind: FindingKind::InvalidId,
                    line: Some(item.line),
                    detail: format!(
                        "item id \"{}\" does not match pattern \"{}\"",
                        item.id,
                        id_pattern.source()
                    ),
                });
            }
        }
        let rules = ContainerRules::for_item(item_rule);
        validate_container(&rules, &item.blocks, false, findings);
    }

    let count = items.len() as u64;
    let (min, max) = bounds(item_rule.required, item_rule.repeat.as_ref());
    check_occurrence(
        count,
        min,
        max,
        true,
        FindingKind::MissingRequiredSection,
        "item",
        findings,
    );
}

fn validate_stray_heading(heading: &Heading, open: bool, findings: &mut Vec<Finding>) {
    if heading.depth >= 4 {
        findings.push(Finding {
            kind: FindingKind::HeadingLevelMismatch,
            line: Some(heading.line),
            detail: format!("heading at depth {} is not allowed", heading.depth),
        });
    } else if !open {
        findings.push(Finding {
            kind: FindingKind::UndeclaredHeading,
            line: Some(heading.line),
            detail: format!("undeclared item heading \"{}\"", heading.text),
        });
    }
}

/// 1つの前置部・節・項目の中の行の規則。規則種別ごとの判定に使う。
struct ContainerRules<'a> {
    fields: &'a [Field],
    ordered: bool,
    statement: Option<&'a Statement>,
    bullets: Option<&'a Bullets>,
    table: Option<&'a Table>,
    codeblock: Option<&'a CodeBlock>,
}

impl<'a> ContainerRules<'a> {
    fn for_preamble(preamble: Option<&'a Preamble>) -> Self {
        match preamble {
            Some(p) => ContainerRules {
                fields: &p.fields,
                ordered: p.ordered,
                statement: p.statement.as_ref(),
                bullets: p.bullets.as_ref(),
                table: None,
                codeblock: None,
            },
            None => Self::empty(),
        }
    }

    fn for_section(section: &'a Section) -> Self {
        ContainerRules {
            fields: &section.fields,
            ordered: section.ordered,
            statement: section.statement.as_ref(),
            bullets: section.bullets.as_ref(),
            table: section.table.as_ref(),
            codeblock: section.codeblock.as_ref(),
        }
    }

    fn for_item(item: &'a ItemRule) -> Self {
        ContainerRules {
            fields: &item.fields,
            ordered: item.ordered,
            statement: item.statement.as_ref(),
            bullets: item.bullets.as_ref(),
            table: None,
            codeblock: None,
        }
    }

    fn empty() -> Self {
        ContainerRules {
            fields: &[],
            ordered: false,
            statement: None,
            bullets: None,
            table: None,
            codeblock: None,
        }
    }
}

/// 宣言されていない行の指摘をブロックの種別に応じた文言で作る。
/// 文の対象外の行種別（ブロック引用・水平線・画像など）は None を返す（R13）。
fn undeclared_line_for_block(block: &Block) -> Option<Finding> {
    let (detail, line) = match block {
        Block::Field { name, line, .. } => (format!("undeclared field line \"{name}\""), *line),
        Block::Bullet { text, line, .. } => (format!("undeclared bullet \"{text}\""), *line),
        Block::OrderedList { text, line, .. } => {
            (format!("undeclared ordered list \"{text}\""), *line)
        }
        Block::Statement { text, line, .. } => {
            (format!("undeclared statement \"{text}\""), *line)
        }
        Block::Table { line, .. } => ("undeclared table".to_string(), *line),
        Block::Code { line, .. } => ("undeclared code block".to_string(), *line),
        Block::Other { .. } => return None,
    };
    Some(Finding {
        kind: FindingKind::UndeclaredLine,
        line: Some(line),
        detail,
    })
}

fn push_undeclared_line(findings: &mut Vec<Finding>, block: &Block) {
    if let Some(finding) = undeclared_line_for_block(block) {
        findings.push(finding);
    }
}

fn validate_container(
    rules: &ContainerRules,
    blocks: &[Block],
    open: bool,
    findings: &mut Vec<Finding>,
) {
    let mut field_counts: HashMap<&str, usize> = HashMap::new();
    let mut statement_count = 0u64;
    let mut bullet_count = 0u64;
    let mut table_count = 0u64;
    let mut code_count = 0u64;
    let mut ordered_seen: Vec<(usize, usize)> = Vec::new();

    for block in blocks {
        match block {
            Block::Field { name, value, line, .. } => {
                match rules.fields.iter().position(|f| f.name == *name) {
                    Some(idx) => {
                        *field_counts.entry(name.as_str()).or_insert(0) += 1;
                        ordered_seen.push((idx, *line));
                        let field = &rules.fields[idx];
                        if when_allows(field.when.as_ref(), blocks) {
                            // 区切った要素は前後の空白を取り除いてから照合する（R8）
                            let values: Vec<String> = match field.effective_separator() {
                                Some(sep) => split_trimmed(value, sep),
                                None => vec![value.clone()],
                            };
                            for v in &values {
                                if let Some(pattern) = &field.pattern {
                                    if !pattern.is_match(v) {
                                        findings.push(Finding {
                                            kind: FindingKind::FieldPatternMismatch,
                                            line: Some(*line),
                                            detail: format!(
                                                "value \"{v}\" does not match pattern \"{}\"",
                                                pattern.source()
                                            ),
                                        });
                                    }
                                }
                                if let Some(allowed) = &field.r#enum {
                                    if !allowed.iter().any(|e| e == v) {
                                        findings.push(Finding {
                                            kind: FindingKind::FieldEnumInvalid,
                                            line: Some(*line),
                                            detail: format!(
                                                "value \"{v}\" is not one of {allowed:?}"
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    None => {
                        if !open {
                            push_undeclared_line(findings, block);
                        }
                    }
                }
            }
            Block::Bullet { text, line, .. } => match rules.bullets {
                Some(bullets) => {
                    bullet_count += 1;
                    if when_allows(bullets.when.as_ref(), blocks) {
                        if let Some(pattern) = &bullets.pattern {
                            if !pattern.is_match(text) {
                                findings.push(Finding {
                                    kind: FindingKind::BulletPatternMismatch,
                                    line: Some(*line),
                                    detail: format!(
                                        "bullet \"{text}\" does not match pattern \"{}\"",
                                        pattern.source()
                                    ),
                                });
                            }
                        }
                    }
                }
                None => {
                    if !open {
                        push_undeclared_line(findings, block);
                    }
                }
            },
            Block::Statement { text, line } => match rules.statement {
                Some(statement) => {
                    statement_count += 1;
                    if when_allows(statement.when.as_ref(), blocks) {
                        if let Some(pattern) = &statement.pattern {
                            if !pattern.is_match(text) {
                                findings.push(Finding {
                                    kind: FindingKind::StatementPatternMismatch,
                                    line: Some(*line),
                                    detail: format!(
                                        "statement \"{text}\" does not match pattern \"{}\"",
                                        pattern.source()
                                    ),
                                });
                            }
                        }
                        if let Some(allowed) = &statement.r#enum {
                            if !allowed.iter().any(|e| e == text) {
                                findings.push(Finding {
                                    kind: FindingKind::StatementEnumInvalid,
                                    line: Some(*line),
                                    detail: format!(
                                        "statement \"{text}\" is not one of {allowed:?}"
                                    ),
                                });
                            }
                        }
                    }
                }
                None => {
                    if !open {
                        push_undeclared_line(findings, block);
                    }
                }
            },
            Block::Table { header, rows, line } => match rules.table {
                Some(table) => {
                    table_count += 1;
                    if header != &table.header {
                        findings.push(Finding {
                            kind: FindingKind::TableHeaderMismatch,
                            line: Some(*line),
                            detail: format!(
                                "table header {header:?} does not match expected {:?}",
                                table.header
                            ),
                        });
                    }
                    for row in rows {
                        if row.len() != table.header.len() {
                            findings.push(Finding {
                                kind: FindingKind::TableHeaderMismatch,
                                line: Some(*line),
                                detail: format!(
                                    "row has {} columns but the header has {}",
                                    row.len(),
                                    table.header.len()
                                ),
                            });
                        }
                    }
                }
                None => {
                    if !open {
                        push_undeclared_line(findings, block);
                    }
                }
            },
            Block::Code { lang, value, line } => match rules.codeblock {
                Some(codeblock) => {
                    code_count += 1;
                    if let Some(expected) = &codeblock.lang {
                        if lang.as_deref() != Some(expected.as_str()) {
                            findings.push(Finding {
                                kind: FindingKind::CodeblockLangMismatch,
                                line: Some(*line),
                                detail: format!(
                                    "code block language {:?} does not match \"{expected}\"",
                                    lang
                                ),
                            });
                        }
                    }
                    if let Some(patterns) = &codeblock.lines {
                        for (i, code_line) in value.lines().enumerate() {
                            let code_line = code_line.trim();
                            if code_line.is_empty() {
                                continue;
                            }
                            if !patterns.iter().any(|p| p.is_match(code_line)) {
                                findings.push(Finding {
                                    kind: FindingKind::CodeblockLineMismatch,
                                    line: Some(*line),
                                    detail: format!(
                                        "line {} does not match any allowed pattern",
                                        i + 1
                                    ),
                                });
                            }
                        }
                    }
                }
                None => {
                    if !open {
                        push_undeclared_line(findings, block);
                    }
                }
            },
            // ブロック引用・水平線・画像などの文の対象外の行種別は閉じた世界でも無視する（R13）
            Block::Other { .. } => {}
            // 順序付きリストはどの規則種別にも属さない。閉じた世界では undeclared_line にする（R10）
            Block::OrderedList { .. } => {
                if !open {
                    push_undeclared_line(findings, block);
                }
            }
        }
    }

    for field in rules.fields {
        if when_allows(field.when.as_ref(), blocks) {
            let count = field_counts.get(field.name.as_str()).copied().unwrap_or(0) as u64;
            let (min, max) = bounds(field.required, field.repeat.as_ref());
            check_occurrence(
                count,
                min,
                max,
                field.repeat.is_some(),
                FindingKind::MissingRequiredField,
                &format!("field \"{}\"", field.name),
                findings,
            );
        }
    }
    if let Some(statement) = rules.statement {
        if when_allows(statement.when.as_ref(), blocks) {
            let (min, max) = bounds(statement.required, statement.repeat.as_ref());
            check_occurrence(
                statement_count,
                min,
                max,
                statement.repeat.is_some(),
                FindingKind::MissingStatement,
                "statement",
                findings,
            );
        }
    }
    if let Some(bullets) = rules.bullets {
        if when_allows(bullets.when.as_ref(), blocks) {
            let (min, max) = bounds(bullets.required, bullets.repeat.as_ref());
            check_occurrence(
                bullet_count,
                min,
                max,
                bullets.repeat.is_some(),
                FindingKind::MissingBullets,
                "bullets",
                findings,
            );
        }
    }
    if let Some(table) = rules.table {
        let (min, max) = bounds(table.required, table.repeat.as_ref());
        check_occurrence(
            table_count,
            min,
            max,
            table.repeat.is_some(),
            FindingKind::MissingTable,
            "table",
            findings,
        );
    }
    if let Some(codeblock) = rules.codeblock {
        let (min, max) = bounds(codeblock.required, codeblock.repeat.as_ref());
        check_occurrence(
            code_count,
            min,
            max,
            codeblock.repeat.is_some(),
            FindingKind::MissingCodeblock,
            "code block",
            findings,
        );
    }

    if rules.ordered {
        let mut prev: Option<usize> = None;
        for (idx, line) in &ordered_seen {
            if let Some(prev_idx) = prev {
                if *idx < prev_idx {
                    findings.push(Finding {
                        kind: FindingKind::FieldOrderMismatch,
                        line: Some(*line),
                        detail: "fields are not in the declared order".into(),
                    });
                    break;
                }
            }
            prev = Some(*idx);
        }
    }
}

/// `separator` で分けた要素を、前後の空白を取り除いて返す。空の要素は
/// 空文字列として残す（R8）。
fn split_trimmed(value: &str, sep: &str) -> Vec<String> {
    value.split(sep).map(|s| s.trim().to_string()).collect()
}

/// `when` の条件を評価する。参照フィールドが無いとき eq は偽、ne は真。
fn when_allows(when: Option<&When>, blocks: &[Block]) -> bool {
    let Some(when) = when else {
        return true;
    };
    let value = blocks.iter().find_map(|b| match b {
        Block::Field { name, value, .. } if name == &when.field => Some(value.as_str()),
        _ => None,
    });
    match value {
        Some(value) => match (&when.eq, &when.ne) {
            (Some(eq), _) => value == eq,
            (_, Some(ne)) => value != ne,
            _ => unreachable!("parse_schema が when の演算子を保証する"),
        },
        None => when.eq.is_none(),
    }
}

fn bounds(required: Option<bool>, repeat: Option<&Repeat>) -> (u64, Option<u64>) {
    if let Some(repeat) = repeat {
        (repeat.min.unwrap_or(0), repeat.max)
    } else if required == Some(false) {
        (0, Some(1))
    } else {
        (1, Some(1))
    }
}

fn check_occurrence(
    count: u64,
    min: u64,
    max: Option<u64>,
    repeat_style: bool,
    missing_kind: FindingKind,
    what: &str,
    findings: &mut Vec<Finding>,
) {
    if count < min {
        if repeat_style {
            findings.push(Finding {
                kind: FindingKind::RepeatMinNotMet,
                line: None,
                detail: format!("{what} appears {count} time(s), minimum is {min}"),
            });
        } else {
            findings.push(Finding {
                kind: missing_kind,
                line: None,
                detail: format!("{what} is required but missing"),
            });
        }
    } else if let Some(max) = max {
        if count > max {
            findings.push(Finding {
                kind: FindingKind::RepeatMaxExceeded,
                line: None,
                detail: format!("{what} appears {count} time(s), maximum is {max}"),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::schema::parse_schema;

    fn validate_src(schema_yaml: &str, doc: &str, open: bool) -> Vec<Finding> {
        let schema = parse_schema(schema_yaml).unwrap();
        let document = Document::parse(doc).unwrap();
        validate(&schema, &document, open)
    }

    fn kinds(findings: &[Finding]) -> Vec<FindingKind> {
        findings.iter().map(|f| f.kind).collect()
    }

    const SCHEMA: &str = r#"
name: adr
document:
  title:
    pattern: "^ADR-\\d{4}:"
  preamble:
    fields:
      - name: 状態
  sections:
    - name: 状況
      statement:
        required: false
    - name: 決定
      statement:
        required: false
"#;

    fn ok_body() -> &'static str {
        "# ADR-0001: 印\n\n- 状態: 承認済み\n\n## 状況\n\n背景。\n\n## 決定\n\n判断。\n"
    }

    #[test]
    fn missing_title_is_found() {
        let doc = "## 状況\n\n背景。\n\n## 決定\n\n判断。\n";
        let findings = validate_src(SCHEMA, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingTitle));
    }

    #[test]
    fn multiple_titles_is_found() {
        let doc = "# ADR-0001: a\n\n# ADR-0002: b\n\n## 状況\n\n背景。\n\n## 決定\n\n判断。\n";
        let findings = validate_src(SCHEMA, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MultipleTitles));
    }

    #[test]
    fn title_pattern_mismatch_is_found() {
        let doc = "# テストの印\n\n- 状態: 承認済み\n\n## 状況\n\n背景。\n\n## 決定\n\n判断。\n";
        let findings = validate_src(SCHEMA, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::TitlePatternMismatch));
    }

    #[test]
    fn undeclared_heading_is_found() {
        let doc = "# ADR-0001: a\n\n## 状況\n\n背景。\n\n## 決定\n\n判断。\n\n## 補足\n\n余計な節。\n";
        let findings = validate_src(SCHEMA, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::UndeclaredHeading));
    }

    #[test]
    fn undeclared_line_is_found() {
        let doc = "# ADR-0001: a\n\n## 状況\n\n- 宣言外の箇条書き\n\n## 決定\n\n判断。\n";
        let findings = validate_src(SCHEMA, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    #[test]
    fn continuation_paragraph_is_part_of_the_bullet_not_undeclared() {
        let schema = "document:\n  sections:\n    - name: 理由\n      bullets:\n        repeat: { min: 0 }\n";
        let doc = "## 理由\n\n- 親\n\n  続きの段落\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    #[test]
    fn continuation_paragraph_does_not_count_as_statement() {
        let schema = "document:\n  sections:\n    - name: 理由\n      statement:\n        required: true\n      bullets:\n        repeat: { min: 0 }\n";
        let doc = "## 理由\n\n- 親\n\n  続きの段落\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingStatement));
    }

    #[test]
    fn bullet_pattern_is_not_applied_to_continuation_paragraphs() {
        let schema = "document:\n  sections:\n    - name: 理由\n      bullets:\n        repeat: { min: 0 }\n        pattern: \"^親\"\n";
        let doc = "## 理由\n\n- 親\n\n  続きの段落はパターンに合わない\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::BulletPatternMismatch));
    }

    #[test]
    fn code_block_child_of_list_item_is_undeclared_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 理由\n";
        let doc = "## 理由\n\n- 親\n\n  ```\n  x = 1\n  ```\n";
        let findings = validate_src(schema, doc, false);
        let details: Vec<&str> = findings
            .iter()
            .filter(|f| f.kind == FindingKind::UndeclaredLine)
            .map(|f| f.detail.as_str())
            .collect();
        assert!(
            details.contains(&"undeclared code block"),
            "リスト項目の中のコードブロックが undeclared_line になる: {details:?}"
        );
    }

    #[test]
    fn code_block_as_first_child_of_list_item_is_undeclared_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 理由\n      bullets:\n        repeat: { min: 0 }\n";
        let doc = "## 理由\n\n- ```python\n  x = 1\n  ```\n";
        let findings = validate_src(schema, doc, false);
        assert!(
            kinds(&findings).contains(&FindingKind::UndeclaredLine),
            "先頭がコードブロックのリスト項目は undeclared_line になる: {:?}",
            kinds(&findings)
        );
    }

    #[test]
    fn table_as_first_child_of_list_item_is_undeclared_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 理由\n      bullets:\n        repeat: { min: 0 }\n";
        let doc = "## 理由\n\n- | a | b |\n  |---|---|\n  | 1 | 2 |\n";
        let findings = validate_src(schema, doc, false);
        assert!(
            kinds(&findings).contains(&FindingKind::UndeclaredLine),
            "先頭が表のリスト項目は undeclared_line になる: {:?}",
            kinds(&findings)
        );
    }

    #[test]
    fn table_child_of_list_item_is_undeclared_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 理由\n";
        let doc = "## 理由\n\n- 親\n\n  | a | b |\n  |---|---|\n  | 1 | 2 |\n";
        let findings = validate_src(schema, doc, false);
        let details: Vec<&str> = findings
            .iter()
            .filter(|f| f.kind == FindingKind::UndeclaredLine)
            .map(|f| f.detail.as_str())
            .collect();
        assert!(
            details.contains(&"undeclared table"),
            "リスト項目の中の表が undeclared_line になる: {details:?}"
        );
    }

    #[test]
    fn paragraph_after_code_block_lead_is_undeclared_line_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 理由\n      bullets:\n        repeat: { min: 0 }\n      codeblock:\n        required: false\n        lang: python\n";
        let doc = "## 理由\n\n- ```python\n  x = 1\n  ```\n\n  後続の段落\n";
        let findings = validate_src(schema, doc, false);
        assert!(
            kinds(&findings).contains(&FindingKind::UndeclaredLine),
            "先頭がコードブロックのリスト項目の後続段落は undeclared_line になる: {:?}",
            kinds(&findings)
        );
    }

    #[test]
    fn paragraph_after_table_lead_is_undeclared_line_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 理由\n      table:\n        required: false\n        header: [a, b]\n      bullets:\n        repeat: { min: 0 }\n";
        let doc = "## 理由\n\n- | a | b |\n  |---|---|\n  | 1 | 2 |\n\n  後続の段落\n";
        let findings = validate_src(schema, doc, false);
        assert!(
            kinds(&findings).contains(&FindingKind::UndeclaredLine),
            "先頭が表のリスト項目の後続段落は undeclared_line になる: {:?}",
            kinds(&findings)
        );
    }

    #[test]
    fn paragraph_after_code_block_lead_counts_as_a_statement() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: true\n      codeblock:\n        required: false\n        lang: python\n      bullets:\n        repeat: { min: 0 }\n";
        let doc = "## 状況\n\n- ```python\n  x = 1\n  ```\n\n  後続の段落\n";
        let findings = validate_src(schema, doc, false);
        assert!(
            !kinds(&findings).contains(&FindingKind::MissingStatement),
            "先頭がコードブロックのリスト項目の後続段落は文として数える: {:?}",
            kinds(&findings)
        );
    }

    #[test]
    fn paragraph_after_code_block_lead_is_subject_to_statement_pattern() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        pattern: \"^状況\"\n      codeblock:\n        required: false\n        lang: python\n      bullets:\n        repeat: { min: 0 }\n";
        let doc = "## 状況\n\n- ```python\n  x = 1\n  ```\n\n  後続の段落\n";
        let findings = validate_src(schema, doc, false);
        assert!(
            kinds(&findings).contains(&FindingKind::StatementPatternMismatch),
            "先頭がコードブロックのリスト項目の後続段落に文の pattern を適用する: {:?}",
            kinds(&findings)
        );
    }

    #[test]
    fn image_only_paragraph_after_code_block_lead_is_not_a_statement() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: true\n      codeblock:\n        required: false\n        lang: python\n      bullets:\n        repeat: { min: 0 }\n";
        let doc = "## 状況\n\n- ```python\n  x = 1\n  ```\n\n  ![alt](img.png)\n";
        let findings = validate_src(schema, doc, false);
        assert!(
            kinds(&findings).contains(&FindingKind::MissingStatement),
            "先頭がコードブロックのリスト項目の画像だけの段落は文に数えない（R9）: {:?}",
            kinds(&findings)
        );
        assert!(
            !kinds(&findings).contains(&FindingKind::UndeclaredLine),
            "先頭がコードブロックのリスト項目の画像だけの段落は undeclared_line にしない（R13）: {:?}",
            kinds(&findings)
        );
    }

    #[test]
    fn nested_list_items_are_each_a_top_level_bullet() {
        let schema = "document:\n  sections:\n    - name: 理由\n      bullets:\n        repeat: { min: 2 }\n";
        let doc = "## 理由\n\n- 親\n  - 子\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::RepeatMinNotMet));
    }

    #[test]
    fn nested_list_items_are_each_reported_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 理由\n";
        let doc = "## 理由\n\n- 親\n  - 子\n";
        let findings = validate_src(schema, doc, false);
        let undeclared = findings
            .iter()
            .filter(|f| f.kind == FindingKind::UndeclaredLine)
            .count();
        assert_eq!(undeclared, 2);
    }

    #[test]
    fn ordered_list_is_not_a_bullet_and_is_undeclared_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 理由\n      bullets:\n        pattern: \"^親\"\n";
        let doc = "## 理由\n\n1. 子\n";
        let findings = validate_src(schema, doc, false);
        assert!(
            !kinds(&findings).contains(&FindingKind::BulletPatternMismatch),
            "順序付きリストは箇条書きの対象外なので pattern を適用しない（R10）"
        );
        assert!(kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    #[test]
    fn ordered_list_does_not_satisfy_required_bullets() {
        let schema = "document:\n  sections:\n    - name: 理由\n      bullets:\n        required: true\n";
        let doc = "## 理由\n\n1. 順序付き\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingBullets));
    }

    #[test]
    fn field_continuation_is_part_of_the_field_not_undeclared_or_statement() {
        let schema = "document:\n  preamble:\n    fields:\n      - name: 状態\n    statement:\n      required: true\n";
        let doc = "# 題名\n\n- 状態: 承認済み\n\n  継続の段落\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingStatement));
        assert!(
            !kinds(&findings).contains(&FindingKind::UndeclaredLine),
            "継続段落はフィールド行の一部で undeclared_line にしない（R8）"
        );
    }

    #[test]
    fn missing_required_section_is_found() {
        let doc = "# ADR-0001: a\n\n- 状態: 承認済み\n\n## 状況\n\n背景。\n";
        let findings = validate_src(SCHEMA, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingRequiredSection));
    }

    #[test]
    fn heading_level_mismatch_is_found() {
        let doc = "# ADR-0001: a\n\n## 状況\n\n#### 深すぎ\n\n## 決定\n\n判断。\n";
        let findings = validate_src(SCHEMA, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::HeadingLevelMismatch));
    }

    #[test]
    fn invalid_item_id_is_found() {
        let schema = r#"
document:
  sections:
    - name: 要求
      item:
        id: "REQ-\\d{3,}"
        statement:
          required: false
"#;
        let doc = "## 要求\n\n### XYZ-001: 名前\n\n本文。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::InvalidId));
    }

    #[test]
    fn declared_field_line_passes() {
        let findings = validate_src(SCHEMA, ok_body(), false);
        assert!(!kinds(&findings).contains(&FindingKind::UndeclaredLine));
        assert!(!kinds(&findings).contains(&FindingKind::MissingRequiredField));
    }

    #[test]
    fn open_allows_undeclared_heading_and_line_but_not_missing_required() {
        let schema = r#"
open: true
document:
  title:
    pattern: "^ADR-\\d{4}:"
  sections:
    - name: 状況
      statement:
        required: false
    - name: 決定
      statement:
        required: false
"#;
        let doc = "# ADR-0001: a\n\n## 状況\n\n背景。\n\n## 補足\n\n- 宣言外\n";
        let findings = validate_src(schema, doc, true);
        let ks = kinds(&findings);
        assert!(!ks.contains(&FindingKind::UndeclaredHeading));
        assert!(!ks.contains(&FindingKind::UndeclaredLine));
        assert!(ks.contains(&FindingKind::MissingRequiredSection));
    }

    #[test]
    fn title_without_rule_is_undeclared_heading_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: false\n";
        let doc = "# 題名\n\n## 状況\n\n背景。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::UndeclaredHeading));
    }

    #[test]
    fn title_without_rule_is_allowed_when_open() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: false\n";
        let doc = "# 題名\n\n## 状況\n\n背景。\n";
        let findings = validate_src(schema, doc, true);
        assert!(!kinds(&findings).contains(&FindingKind::UndeclaredHeading));
    }

    #[test]
    fn undeclared_section_lines_are_flagged_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: false\n";
        let doc = "# 題名\n\n## 状況\n\n背景。\n\n## 補足\n\n- 余計な箇条書き\n\n### 補足の項目\n\n中身。\n";
        let findings = validate_src(schema, doc, false);
        let headings = findings
            .iter()
            .filter(|f| f.kind == FindingKind::UndeclaredHeading)
            .count();
        let lines = findings
            .iter()
            .filter(|f| f.kind == FindingKind::UndeclaredLine)
            .count();
        assert!(headings >= 2, "節と項目の見出しが undeclared_heading になる");
        assert!(lines >= 2, "節の中の箇条書きと項目の中の行が undeclared_line になる");
    }

    #[test]
    fn undeclared_preamble_lines_are_flagged_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: false\n";
        let doc = "# 題名\n\n- 状態: 承認済み\n\n## 状況\n\n背景。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    #[test]
    fn undeclared_preamble_lines_are_allowed_when_open() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: false\n";
        let doc = "# 題名\n\n- 状態: 承認済み\n\n## 状況\n\n背景。\n";
        let findings = validate_src(schema, doc, true);
        assert!(!kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    #[test]
    fn undeclared_section_lines_are_allowed_when_open() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: false\n";
        let doc = "# 題名\n\n## 状況\n\n背景。\n\n## 補足\n\n- 余計な箇条書き\n";
        let findings = validate_src(schema, doc, true);
        assert!(!kinds(&findings).contains(&FindingKind::UndeclaredHeading));
        assert!(!kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    #[test]
    fn undeclared_item_inside_declared_section_is_flagged_even_when_open() {
        let schema = r#"
document:
  sections:
    - name: 要求
      statement:
        required: false
"#;
        let doc = "## 要求\n\n### REQ-001: 名前\n\n本文。\n";
        let findings = validate_src(schema, doc, true);
        assert!(kinds(&findings).contains(&FindingKind::UndeclaredHeading));
        assert!(kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    #[test]
    fn undeclared_line_in_declared_section_is_flagged_even_when_open() {
        let schema = r#"
document:
  sections:
    - name: 状況
      statement:
        required: false
"#;
        let doc = "## 状況\n\n本文。\n\n- 宣言外の箇条書き\n";
        let findings = validate_src(schema, doc, true);
        assert!(kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    // ---- R8〜R12: 行の規則 ----

    #[test]
    fn missing_required_field_is_found() {
        let schema = "document:\n  preamble:\n    fields:\n      - name: 状態\n";
        let doc = "# 題名\n\n## 状況\n\n本文。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingRequiredField));
    }

    #[test]
    fn field_pattern_mismatch_is_found() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: 日付
        pattern: "^\\d{4}-\\d{2}-\\d{2}"
"#;
        let doc = "# 題名\n\n- 日付: yesterday\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::FieldPatternMismatch));
    }

    #[test]
    fn field_enum_invalid_is_found() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: 状態
        enum: [承認済み, 却下]
"#;
        let doc = "# 題名\n\n- 状態: 保留\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::FieldEnumInvalid));
    }

    #[test]
    fn separator_splits_value_before_enum_check() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        separator: ","
        enum: [a, b]
"#;
        let doc = "# 題名\n\n- タグ: a,c\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::FieldEnumInvalid));
    }

    #[test]
    fn separator_elements_are_trimmed_before_enum_check() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        separator: ","
        enum: [a, b, c]
"#;
        let doc = "# 題名\n\n- タグ: a, c\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::FieldEnumInvalid));
    }

    #[test]
    fn missing_statement_is_found() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement: {}\n";
        let doc = "## 状況\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingStatement));
    }

    #[test]
    fn statement_pattern_mismatch_is_found() {
        let schema = r#"
document:
  sections:
    - name: 状況
      statement:
        pattern: "^状況"
"#;
        let doc = "## 状況\n\n背景。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::StatementPatternMismatch));
    }

    #[test]
    fn statement_enum_invalid_is_found() {
        let schema = r#"
document:
  sections:
    - name: 状況
      statement:
        enum: [a, b]
"#;
        let doc = "## 状況\n\n本文。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::StatementEnumInvalid));
    }

    #[test]
    fn missing_bullets_is_found() {
        let schema = "document:\n  sections:\n    - name: 理由\n      bullets: {}\n";
        let doc = "## 理由\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingBullets));
    }

    #[test]
    fn bullet_pattern_mismatch_is_found() {
        let schema = r#"
document:
  sections:
    - name: 理由
      bullets:
        pattern: "^理由"
"#;
        let doc = "## 理由\n\n- その他\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::BulletPatternMismatch));
    }

    #[test]
    fn missing_table_is_found() {
        let schema = r#"
document:
  sections:
    - name: 用語集
      table:
        header: [用語, 意味]
"#;
        let doc = "## 用語集\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingTable));
    }

    #[test]
    fn table_header_mismatch_is_found() {
        let schema = r#"
document:
  sections:
    - name: 用語集
      table:
        header: [用語, 意味]
"#;
        let doc = "## 用語集\n\n| 用語 |\n|---|\n| a |\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::TableHeaderMismatch));
    }

    #[test]
    fn missing_codeblock_is_found() {
        let schema = r#"
document:
  sections:
    - name: 具体例
      codeblock:
        lang: gherkin
"#;
        let doc = "## 具体例\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingCodeblock));
    }

    #[test]
    fn codeblock_lang_mismatch_is_found() {
        let schema = r#"
document:
  sections:
    - name: 具体例
      codeblock:
        lang: gherkin
"#;
        let doc = "## 具体例\n\n```python\nx = 1\n```\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::CodeblockLangMismatch));
    }

    #[test]
    fn codeblock_line_mismatch_is_found() {
        let schema = r#"
document:
  sections:
    - name: 具体例
      codeblock:
        lang: gherkin
        lines: ["^Scenario:", "^Given "]
"#;
        let doc = "## 具体例\n\n```gherkin\nScenario: 印を書く\nBad line\n```\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::CodeblockLineMismatch));
    }

    #[test]
    fn codeblock_line_matching_trims_leading_whitespace_and_skips_blank_lines() {
        let schema = r#"
document:
  sections:
    - name: 具体例
      codeblock:
        lang: gherkin
        lines: ["^Scenario:", "^Given "]
"#;
        let doc = "## 具体例\n\n```gherkin\n  Scenario: 印を書く\n\n   Given 文\n```\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::CodeblockLineMismatch));
    }

    #[test]
    fn blockquote_and_thematic_break_lines_are_ignored_in_closed_world() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: false\n";
        let doc = "## 状況\n\n> 引用\n\n---\n\n本文。\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    #[test]
    fn image_only_line_is_not_counted_as_statement() {
        let schema = "document:\n  sections:\n    - name: 状況\n      statement:\n        required: true\n";
        let doc = "## 状況\n\n![alt](img.png)\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingStatement));
        assert!(!kinds(&findings).contains(&FindingKind::UndeclaredLine));
    }

    // ---- R14 / R15 / R8(ordered): 出現回数・条件付き・順序 ----

    #[test]
    fn repeat_min_not_met_for_section_is_found() {
        let schema = r#"
document:
  sections:
    - name: 理由
      repeat: { min: 2 }
"#;
        let doc = "## 理由\n\n本文。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::RepeatMinNotMet));
    }

    #[test]
    fn repeat_min_not_met_for_field_is_found() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        repeat: { min: 2 }
"#;
        let doc = "# 題名\n\n- タグ: a\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::RepeatMinNotMet));
    }

    #[test]
    fn repeat_max_exceeded_for_optional_section_is_found() {
        let schema = r#"
document:
  sections:
    - name: 補足
      required: false
      statement:
        required: false
"#;
        let doc = "## 補足\n\n1つ目。\n\n## 補足\n\n2つ目。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::RepeatMaxExceeded));
    }

    #[test]
    fn repeat_max_exceeded_for_field_is_found() {
        let schema = r#"
document:
  preamble:
    fields:
      - name: タグ
        repeat: { max: 2 }
"#;
        let doc = "# 題名\n\n- タグ: a\n- タグ: b\n- タグ: c\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::RepeatMaxExceeded));
    }

    #[test]
    fn repeat_with_max_only_omits_min_as_zero() {
        let schema = "document:\n  preamble:\n    fields:\n      - name: タグ\n        repeat: { max: 2 }\n";
        let doc = "# 題名\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::RepeatMinNotMet));
    }

    #[test]
    fn item_with_no_repeat_requires_exactly_one() {
        let schema = r#"
document:
  sections:
    - name: 要求
      item:
        id: "REQ-\\d{3,}"
        statement:
          required: false
"#;
        let doc = "## 要求\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::RepeatMinNotMet));
    }

    #[test]
    fn item_repeat_max_exceeded_is_found() {
        let schema = r#"
document:
  sections:
    - name: 要求
      item:
        id: "REQ-\\d{3,}"
        repeat: { max: 1 }
        statement:
          required: false
"#;
        let doc = "## 要求\n\n### REQ-001: a\n\n本文。\n\n### REQ-002: b\n\n本文。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::RepeatMaxExceeded));
    }

    #[test]
    fn item_with_required_false_is_optional() {
        let schema = r#"
document:
  sections:
    - name: 要求
      item:
        id: "REQ-\\d{3,}"
        required: false
        statement:
          required: false
"#;
        let doc = "## 要求\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::RepeatMinNotMet));
        assert!(!kinds(&findings).contains(&FindingKind::MissingRequiredSection));
    }

    #[test]
    fn item_heading_without_colon_is_invalid_id() {
        let schema = r#"
document:
  sections:
    - name: 要求
      item:
        statement:
          required: false
"#;
        let doc = "## 要求\n\n### REQ-001\n\n本文。\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::InvalidId));
    }

    #[test]
    fn field_order_mismatch_is_found() {
        let schema = r#"
document:
  sections:
    - name: 要求
      ordered: true
      fields:
        - name: 種類
        - name: 定義
"#;
        let doc = "## 要求\n\n- 定義: REQ-001\n- 種類: algorithm\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::FieldOrderMismatch));
    }

    #[test]
    fn fields_in_declared_order_pass() {
        let schema = r#"
document:
  sections:
    - name: 要求
      ordered: true
      fields:
        - name: 種類
        - name: 定義
"#;
        let doc = "## 要求\n\n- 種類: algorithm\n- 定義: REQ-001\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::FieldOrderMismatch));
    }

    #[test]
    fn when_eq_true_requires_the_field() {
        let schema = r#"
document:
  sections:
    - name: 要求
      fields:
        - name: 種類
        - name: 定義
          when: { field: 種類, eq: algorithm }
"#;
        let doc = "## 要求\n\n- 種類: algorithm\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingRequiredField));
    }

    #[test]
    fn when_eq_false_skips_the_requirement() {
        let schema = r#"
document:
  sections:
    - name: 要求
      fields:
        - name: 種類
        - name: 定義
          when: { field: 種類, eq: algorithm }
"#;
        let doc = "## 要求\n\n- 種類: ubiquitous\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::MissingRequiredField));
    }

    #[test]
    fn when_with_missing_reference_makes_eq_false() {
        let schema = r#"
document:
  sections:
    - name: 要求
      fields:
        - name: 定義
          when: { field: 種類, eq: algorithm }
"#;
        let doc = "## 要求\n";
        let findings = validate_src(schema, doc, false);
        assert!(!kinds(&findings).contains(&FindingKind::MissingRequiredField));
    }

    #[test]
    fn when_with_missing_reference_makes_ne_true() {
        let schema = r#"
document:
  sections:
    - name: 要求
      fields:
        - name: 定義
          when: { field: 種類, ne: algorithm }
"#;
        let doc = "## 要求\n";
        let findings = validate_src(schema, doc, false);
        assert!(kinds(&findings).contains(&FindingKind::MissingRequiredField));
    }
}