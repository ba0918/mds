use assert_cmd::Command;
use std::path::Path;

const T_SCHEMA: &str = r#"
document:
  title:
    pattern: "^T-\\d+:"
  sections:
    - name: 状況
      statement:
        required: false
"#;

fn write_file(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

fn mds() -> Command {
    Command::cargo_bin("mds").unwrap()
}

#[test]
fn ast_outputs_mdast_json_for_the_adr_fixture() {
    let mut cmd = Command::cargo_bin("mds").unwrap();
    let output = cmd
        .args(["ast", "fixtures/adr/0001.md"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["type"], "root");
    let types: Vec<&str> = json["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["type"].as_str().unwrap())
        .collect();
    assert!(types.contains(&"heading"));
    assert!(types.contains(&"list"));
    assert!(types.contains(&"paragraph"));
    assert!(json.get("position").is_none());
    assert!(json["children"][0].get("position").is_none());
}

#[test]
fn ast_with_format_text_stops_with_exit_code_2() {
    let mut cmd = Command::cargo_bin("mds").unwrap();
    cmd.args(["ast", "fixtures/adr/0001.md", "--format", "text"]);
    cmd.assert().code(2);
}

#[test]
fn ast_schema_outputs_typed_tree() {
    let output = mds()
        .args(["ast", "fixtures/adr/0001.md", "--schema"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["type"], "adr");
    assert_eq!(json["id"], "ADR-0001");
    assert_eq!(json["sections"]["reasons"], serde_json::json!(["理由その1", "理由その2"]));
}

#[test]
fn ast_schema_without_schema_stops() {
    let dir = tempfile::tempdir().unwrap();
    let doc = write_file(dir.path(), "doc.md", "# 題名\n");
    let output = mds()
        .args(["ast", doc.to_str().unwrap(), "--schema"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema_not_found"));
}

#[test]
fn check_clean_document_exits_zero_with_no_output() {
    let output = mds()
        .args(["check", "fixtures/adr/0001.md"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
}

#[test]
fn check_document_with_findings_exits_one_with_text() {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", T_SCHEMA);
    let doc = write_file(
        dir.path(),
        "doc.md",
        "---\n$schema: ./schema.yaml\n---\n# T-1: 例\n\n## 状況\n\n本文。\n\n## 補足\n\n本文。\n",
    );
    let output = mds().args(["check", doc.to_str().unwrap()]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("{}:10: undeclared_heading:", doc.display())));
}

#[test]
fn check_json_output_has_files_with_findings() {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", T_SCHEMA);
    let doc = write_file(
        dir.path(),
        "doc.md",
        "---\n$schema: ./schema.yaml\n---\n# T-1: 例\n\n## 状況\n\n本文。\n\n## 補足\n\n本文。\n",
    );
    let output = mds()
        .args(["check", doc.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    let finding = &files[0]["findings"][0];
    assert_eq!(finding["kind"], "undeclared_heading");
    assert_eq!(finding["severity"], "error");
    assert_eq!(finding["path"], doc.to_str().unwrap());
    assert_eq!(finding["line"], 10);
    assert!(finding.get("detail").is_some());
}

#[test]
fn check_format_defaults_to_text() {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", T_SCHEMA);
    let doc = write_file(
        dir.path(),
        "doc.md",
        "---\n$schema: ./schema.yaml\n---\n# T-1: 例\n\n## 状況\n\n本文。\n\n## 補足\n\n本文。\n",
    );
    let output = mds().args(["check", doc.to_str().unwrap()]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("{}:10: undeclared_heading:", doc.display())));
    assert!(!stdout.trim_start().starts_with('{'));
}

#[test]
fn check_open_relaxes_undeclared_headings() {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", T_SCHEMA);
    let doc = write_file(
        dir.path(),
        "doc.md",
        "---\n$schema: ./schema.yaml\n---\n# T-1: 例\n\n## 状況\n\n本文。\n\n## 補足\n\n本文。\n",
    );
    let closed = mds().args(["check", doc.to_str().unwrap()]).output().unwrap();
    assert_eq!(closed.status.code(), Some(1));
    let opened = mds()
        .args(["check", doc.to_str().unwrap(), "--open"])
        .output()
        .unwrap();
    assert_eq!(opened.status.code(), Some(0));
    assert!(opened.stdout.is_empty());
}

#[test]
fn check_skips_bom_and_counts_crlf_lines() {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", T_SCHEMA);
    let doc = write_file(
        dir.path(),
        "doc.md",
        "\u{FEFF}---\r\n$schema: ./schema.yaml\r\n---\r\n# T-1: 例\r\n\r\n## 状況\r\n\r\n## 補足",
    );
    let output = mds().args(["check", doc.to_str().unwrap()]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("{}:8: undeclared_heading:", doc.display())));
}

#[test]
fn check_missing_title_has_no_line_number() {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", T_SCHEMA);
    let doc = write_file(
        dir.path(),
        "doc.md",
        "---\n$schema: ./schema.yaml\n---\n## 状況\n\n本文。\n",
    );
    let output = mds().args(["check", doc.to_str().unwrap()]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("{}: missing_title:", doc.display())));
    assert!(!stdout.contains(":1: missing_title:"));
}

#[test]
fn check_missing_schema_stops_with_schema_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let doc = write_file(dir.path(), "doc.md", "# 題名\n");
    let output = mds().args(["check", doc.to_str().unwrap()]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema_not_found"));
}

#[test]
fn check_unresolvable_schema_stops_with_schema_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let doc = write_file(
        dir.path(),
        "doc.md",
        "---\n$schema: ./nope.yaml\n---\n# 題名\n",
    );
    let output = mds().args(["check", doc.to_str().unwrap()]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema_not_found"));
}

#[test]
fn check_unreadable_file_stops() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.md");
    let output = mds().args(["check", missing.to_str().unwrap()]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unreadable_file"));
}

#[test]
fn check_broken_frontmatter_stops_with_frontmatter_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let doc = write_file(dir.path(), "doc.md", "---\n$schema: [\n---\n# 題名\n");
    let output = mds().args(["check", doc.to_str().unwrap()]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("frontmatter_invalid"));
}

#[test]
fn values_text_defaults_to_indented_text() {
    let output = mds()
        .args(["values", "fixtures/adr/0001.md"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("id: ADR-0001\n"));
    assert!(stdout.contains("status: 承認済み\n"));
    assert!(stdout.contains("sections:\n  context: 背景の段落。\n"));
    assert!(stdout.contains("  reasons:\n    1. 理由その1\n    2. 理由その2\n"));
}

#[test]
fn values_json_is_nested_by_path() {
    let output = mds()
        .args(["values", "fixtures/adr/0001.md", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["id"], "ADR-0001");
    assert_eq!(json["title"], "ADR-0001: テストの印は @kotowari[ID, ...]");
    assert_eq!(json["status"], "承認済み");
    assert_eq!(json["date"], "2026-09-16");
    assert_eq!(json["sections"]["context"], "背景の段落。");
    assert_eq!(json["sections"]["decision"], "判断の内容。");
    assert_eq!(json["sections"]["reasons"], serde_json::json!(["理由その1", "理由その2"]));
}

#[test]
fn values_without_schema_stops_with_schema_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let doc = write_file(dir.path(), "doc.md", "# 題名\n");
    let output = mds().args(["values", doc.to_str().unwrap()]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema_not_found"));
}

const DIR_SCHEMA: &str = r#"
document:
  title:
    pattern: "^T-\\d+:"
  sections:
    - name: 状況
      statement:
        required: false
"#;

fn good_doc() -> &'static str {
    "---\n$schema: ./schema.yaml\n---\n# T-1: 例\n\n## 状況\n\n本文。\n"
}

fn bad_doc() -> &'static str {
    "---\n$schema: ./schema.yaml\n---\n# T-1: 例\n\n## 状況\n\n本文。\n\n## 補足\n\n本文。\n"
}

#[test]
fn check_directory_reports_all_failing_documents() {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", DIR_SCHEMA);
    write_file(dir.path(), "good.md", good_doc());
    write_file(dir.path(), "bad.md", bad_doc());
    write_file(dir.path(), "noschema.md", "# スキーマなし\n");
    let output = mds()
        .args(["check", dir.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0]["path"].as_str().unwrap().ends_with("bad.md"));
    assert_eq!(files[0]["findings"][0]["kind"], "undeclared_heading");
}

#[test]
fn check_directory_skips_hidden_directories() {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", DIR_SCHEMA);
    write_file(dir.path(), "good.md", good_doc());
    write_file(dir.path(), ".hidden/bad.md", bad_doc());
    let output = mds()
        .args(["check", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
}

#[test]
fn check_directory_does_not_follow_symlinks() {
    let outside = tempfile::tempdir().unwrap();
    let external = write_file(outside.path(), "external.md", bad_doc());
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", DIR_SCHEMA);
    write_file(dir.path(), "good.md", good_doc());
    std::os::unix::fs::symlink(&external, dir.path().join("link.md")).unwrap();
    let output = mds()
        .args(["check", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
}

#[test]
fn check_directory_stops_on_invalid_schema() {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "schema.yaml", "document: [\n");
    write_file(dir.path(), "doc.md", good_doc());
    let output = mds()
        .args(["check", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema_invalid"));
}

#[test]
fn check_directory_stops_on_unreadable_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(dir.path(), "secret.md", good_doc());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let output = mds()
        .args(["check", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unreadable_file"));
}

#[test]
fn check_fixtures_directory_passes() {
    let output = mds().args(["check", "fixtures"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
}

#[test]
fn ir_fixture_passes_check_and_extracts_values() {
    let checked = mds()
        .args(["check", "fixtures/ir/kanji.md"])
        .output()
        .unwrap();
    assert_eq!(checked.status.code(), Some(0));

    let values = mds()
        .args(["values", "fixtures/ir/kanji.md"])
        .output()
        .unwrap();
    assert_eq!(values.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&values.stdout);
    assert!(stdout.contains("title: 印の仕様"));
    assert!(stdout.contains("scope: この文書は、印の構文と意味を正規化した仕様を扱う。"));
    assert!(stdout.contains("用語: 印"));
    assert!(stdout.contains("Scenario: 印を書く"));
}
