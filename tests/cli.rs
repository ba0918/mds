use assert_cmd::Command;

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