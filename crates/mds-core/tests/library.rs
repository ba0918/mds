//! ライブラリの入口の契約。依存するクレートが実際に辿る道筋を、公開 API だけで通す。
//! ファイルの読み書きと URL の取得は呼び出し側の責務で、ここでは行わない。

use std::path::{Path, PathBuf};

use mds_core::document::Document;
use mds_core::extract::extract_values;
use mds_core::finding::FindingKind;
use mds_core::frontmatter::{ResolvedSchema, SchemaRef, frontmatter_schema, resolve_schema};
use mds_core::schema::parse_schema;
use mds_core::validate::validate;

const SCHEMA: &str = r#"
name: ir
open: false
document:
  title:
    pattern: "^.+"
    extract: title
  preamble:
    statement:
      required: true
      extract: scope
  sections:
    - name: 要求
      item:
        id: "REQ-\\d{3,}"
        repeat: { min: 0 }
        extract:
          - requirements
          - { path: id, of: id }
          - { path: line, of: line }
        fields:
          - name: 種類
            enum: [ubiquitous, algorithm]
            extract: kind
        statement:
          required: true
          extract: text
"#;

const DOCUMENT: &str = "---\n$schema: ../.mds/schemas/ir.yaml\n---\n# 題名\n\nこの文書が扱う範囲。\n\n## 要求\n\n### REQ-001: 名前\n\n- 種類: ubiquitous\n\n本文。\n";

// @kotowari[REQ-049, EX-015]
#[test]
fn a_dependent_crate_can_run_the_whole_pipeline() {
    // 1. 文書から "$schema" を読む
    let schema_ref = frontmatter_schema(DOCUMENT).unwrap().unwrap();
    assert!(
        matches!(schema_ref, SchemaRef::Relative(ref p) if p == "../.mds/schemas/ir.yaml"),
        "相対パスの参照をそのまま返す"
    );

    // 2. スキーマの位置を決める。読み込みは呼び出し側が行う
    let resolved = resolve_schema(Path::new("docs/ir/x.md"), &schema_ref);
    assert_eq!(
        resolved,
        ResolvedSchema::File(PathBuf::from("docs/.mds/schemas/ir.yaml")),
        "相対パスは文書の位置から解決する"
    );

    // 3. スキーマを読む
    let schema = parse_schema(SCHEMA).unwrap();

    // 4. 文書を読む
    let document = Document::parse(DOCUMENT).unwrap();

    // 5. 検査する
    let findings = validate(&schema, &document, false);
    assert!(
        findings.is_empty(),
        "合格の文書に指摘は出ない: {findings:?}"
    );

    // 6. 抽出する
    let values = extract_values(&schema, &document);
    assert_eq!(values["requirements"][0]["id"], "REQ-001");
    assert_eq!(values["requirements"][0]["kind"], "ubiquitous");
    assert_eq!(values["requirements"][0]["text"], "本文。");
    assert_eq!(values["requirements"][0]["line"], 10);
}

// @kotowari[REQ-049]
#[test]
fn findings_carry_the_kind_line_and_detail() {
    let broken = DOCUMENT.replace("- 種類: ubiquitous", "- 種類: bogus");
    let schema = parse_schema(SCHEMA).unwrap();
    let document = Document::parse(&broken).unwrap();
    let findings = validate(&schema, &document, false);
    let finding = findings.first().expect("指摘が1件は出る");
    assert_eq!(finding.kind, FindingKind::FieldEnumInvalid);
    assert_eq!(finding.line, Some(12));
    assert!(!finding.detail.is_empty());
}

// @kotowari[REQ-049, REQ-050, EX-016]
#[test]
fn a_url_schema_resolves_to_a_url_for_the_caller_to_fetch() {
    let source = "---\n$schema: https://example.com/ir.yaml\n---\n# 題名\n\n範囲。\n";
    let schema_ref = frontmatter_schema(source).unwrap().unwrap();
    let resolved = resolve_schema(Path::new("docs/ir/x.md"), &schema_ref);
    assert_eq!(
        resolved,
        ResolvedSchema::Url("https://example.com/ir.yaml".to_string()),
        "URL は位置だけを返し、取得は呼び出し側が行う"
    );
}
