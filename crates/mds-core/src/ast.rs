//! Markdown を mdast に解析し、JSON へ写す。

use markdown::{Constructs, ParseOptions, mdast::Node, to_mdast};
use serde_json::Value;

/// Markdown を mdast の木に解析する。GFM（表）と frontmatter の構文を有効にする。
pub fn parse_mdast(src: &str) -> Result<Node, String> {
    let mut constructs = Constructs::gfm();
    constructs.frontmatter = true;
    let options = ParseOptions {
        constructs,
        ..ParseOptions::default()
    };
    to_mdast(src, &options).map_err(|e| format!("{e:?}"))
}

/// 素の AST を JSON で返す。frontmatter のノードと位置情報は含めない。
pub fn ast_json(src: &str) -> Result<Value, String> {
    let mut root = parse_mdast(src)?;
    strip_frontmatter(&mut root);
    strip_positions(&mut root);
    serde_json::to_value(&root).map_err(|e| e.to_string())
}

/// 文書先頭の frontmatter（YAML / TOML）ノードを取り除く。
fn strip_frontmatter(node: &mut Node) {
    if let Node::Root(root) = node {
        root.children
            .retain(|child| !matches!(child, Node::Yaml(_) | Node::Toml(_)));
    }
}

/// 位置情報を取り除く。R17 は出力に position を含めない。
fn strip_positions(node: &mut Node) {
    node.position_set(None);
    if let Some(children) = node.children_mut() {
        for child in children {
            strip_positions(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // @kotowari[REQ-040]
    #[test]
    fn ast_json_is_mdast_shape_with_inline_and_no_position() {
        let src = "# 題名\n\n段落と `インライン` と *強調* と **太字**。\n\n- 箇条書き\n";
        let json = ast_json(src).unwrap();
        assert_eq!(json["type"], "root");
        let children = json["children"].as_array().unwrap();
        assert_eq!(children[0]["type"], "heading");
        assert_eq!(children[0]["depth"], 1);
        assert_eq!(children[1]["type"], "paragraph");
        assert_eq!(children[2]["type"], "list");
        let inline_types: Vec<&str> = children[1]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["type"].as_str().unwrap())
            .collect();
        assert!(inline_types.contains(&"inlineCode"));
        assert!(inline_types.contains(&"emphasis"));
        assert!(inline_types.contains(&"strong"));
        assert!(json.get("position").is_none());
        assert!(children[0].get("position").is_none());
        assert!(children[0]["children"][0].get("position").is_none());
    }

    // @kotowari[REQ-040]
    #[test]
    fn ast_json_excludes_frontmatter_node() {
        let src = "---\n$schema: ../x.yaml\n---\n# 題名\n";
        let json = ast_json(src).unwrap();
        let children = json["children"].as_array().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["type"], "heading");
    }
}
