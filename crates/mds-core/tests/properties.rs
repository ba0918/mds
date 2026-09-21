//! 性質テスト。例を並べても網羅にならない要求を、生成した入力で確かめる。

use mds_core::document::Document;
use mds_core::extract::extract_values;
use mds_core::schema::parse_schema;
use mds_core::validate::validate;
use proptest::prelude::*;
use serde_json::Value;

/// 宣言済みのフィールド行の名前と、宣言していない名前。
const DECLARED: [&str; 2] = ["状態", "日付"];
const UNDECLARED: [&str; 2] = ["備考", "補足"];
const MARKERS: [&str; 3] = ["-", "*", "+"];

/// 箇条書きを宣言しないので、宣言していない名前の行は undeclared_line になる。
/// 宣言した名前の行は値のパターンに照合され、合わなければ field_pattern_mismatch になる。
/// どちらの判定もマーカーに依存してはならない。
const MARKER_SCHEMA: &str = r#"
document:
  sections:
    - name: 記録
      fields:
        - name: 状態
          required: false
          pattern: "^ok"
        - name: 日付
          required: false
          pattern: "^ok"
"#;

fn line_names() -> impl Strategy<Value = Vec<usize>> {
    proptest::collection::vec(0usize..4, 1..6)
}

fn name_at(index: usize) -> &'static str {
    if index < DECLARED.len() {
        DECLARED[index]
    } else {
        UNDECLARED[index - DECLARED.len()]
    }
}

fn document_with(markers: &[usize], names: &[usize], values: &[&str]) -> String {
    let mut out = String::from("## 記録\n\n");
    for (i, (marker, name)) in markers.iter().zip(names).enumerate() {
        out.push_str(&format!(
            "{} {}: {}\n",
            MARKERS[*marker],
            name_at(*name),
            values[i % values.len()]
        ));
    }
    out
}

fn findings_of(
    schema: &mds_core::schema::Schema,
    source: &str,
) -> Vec<(String, Option<usize>, String)> {
    let document = Document::parse(source).unwrap();
    validate(schema, &document, false)
        .into_iter()
        .map(|f| (format!("{:?}", f.kind), f.line, f.detail))
        .collect()
}

proptest! {
    /// REQ-028: 一覧のマーカーが "-"、"*"、"+" のどれであっても読み分けは変わらない。
    // @kotowari[REQ-028]
    #[test]
    fn markers_do_not_change_how_list_lines_are_read(
        names in line_names(),
        values in proptest::collection::vec(prop::sample::select(&["ok", "ng"][..]), 1..4),
    ) {
        let schema = parse_schema(MARKER_SCHEMA).unwrap();
        let hyphens = vec![0usize; names.len()];
        let expected = findings_of(&schema, &document_with(&hyphens, &names, &values));

        // 生成した入力が判定を踏んでいることを確かめる（空振りする性質テストにしない）。
        // values は行ごとに順に使い回すので、実際に使われた値だけを見る
        let used: Vec<&str> = (0..names.len()).map(|i| values[i % values.len()]).collect();
        let touches_classification =
            names.iter().any(|n| *n >= DECLARED.len()) || used.contains(&"ng");
        prop_assert!(
            !touches_classification || !expected.is_empty(),
            "判定を踏む入力なのに指摘が0件だった: {:?} {:?}",
            names,
            values
        );

        for (marker, symbol) in MARKERS.iter().enumerate().skip(1) {
            let markers = vec![marker; names.len()];
            let actual = findings_of(&schema, &document_with(&markers, &names, &values));
            prop_assert_eq!(&actual, &expected, "マーカー {} で読み分けが変わった", symbol);
        }
    }
}

proptest! {
    /// REQ-028: マーカーを1行ごとに混ぜても、すべて "-" のときと読み分けは変わらない。
    // @kotowari[REQ-028]
    #[test]
    fn mixed_markers_read_the_same_as_hyphens(
        names in line_names(),
        values in proptest::collection::vec(prop::sample::select(&["ok", "ng"][..]), 1..4),
        seed in proptest::collection::vec(0usize..MARKERS.len(), 1..6),
    ) {
        let markers: Vec<usize> = (0..names.len()).map(|i| seed[i % seed.len()]).collect();
        let schema = parse_schema(MARKER_SCHEMA).unwrap();
        let hyphens = vec![0usize; names.len()];
        let expected = findings_of(&schema, &document_with(&hyphens, &names, &values));
        let actual = findings_of(&schema, &document_with(&markers, &names, &values));
        prop_assert_eq!(actual, expected);
    }
}

const EXTRACT_SCHEMA: &str = r#"
document:
  title:
    pattern: "^.+"
    extract: title
  sections:
    - name: 記録
      fields:
        - name: 値
          repeat: { min: 0 }
          extract: values
        - name: 並び
          required: false
          separator: ","
          extract: list
      statement:
        required: false
        extract: body
      table:
        required: false
        extract: rows
"#;

/// JSON の葉がすべて文字列であることを確かめる。
fn every_leaf_is_a_string(value: &Value) -> bool {
    match value {
        Value::String(_) => true,
        Value::Array(items) => items.iter().all(every_leaf_is_a_string),
        Value::Object(map) => map.values().all(every_leaf_is_a_string),
        _ => false,
    }
}

proptest! {
    /// REQ-037: 数や日付に見える値でも、抽出した値は文字列のままである。
    // @kotowari[REQ-037]
    #[test]
    fn extracted_leaves_are_always_strings(
        values in proptest::collection::vec("[0-9A-Za-z.:+-]{1,12}", 1..6),
        list in proptest::collection::vec("[0-9A-Za-z.:+-]{0,8}", 1..4),
        cell in "[0-9A-Za-z.:+-]{1,12}",
    ) {
        let mut doc = String::from("# 題名\n\n## 記録\n\n");
        for value in &values {
            doc.push_str(&format!("- 値: {value}\n"));
        }
        doc.push_str(&format!("- 並び: {}\n", list.join(",")));
        doc.push_str("\n本文。\n\n| 列 |\n|---|\n");
        doc.push_str(&format!("| {cell} |\n"));

        let schema = parse_schema(EXTRACT_SCHEMA).unwrap();
        let document = Document::parse(&doc).unwrap();
        let extracted = extract_values(&schema, &document);
        prop_assert!(
            every_leaf_is_a_string(&extracted),
            "文字列でない葉があった: {extracted}"
        );
    }
}
