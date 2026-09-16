//! frontmatter から `$schema` を読み、参照先を解決する。

use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::path::{Component, Path, PathBuf};

/// `$schema` の参照先。相対パスまたは URL。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaRef {
    Relative(String),
    Url(String),
}

/// frontmatter を読めなかった理由。R19 の `frontmatter_invalid` に相当する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterError(pub String);

/// 解決済みのスキーマの位置。ファイル読み書きは CLI 側の責務なので、ここでは位置だけを決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSchema {
    File(PathBuf),
    Url(String),
}

#[derive(Deserialize)]
struct Frontmatter {
    #[serde(rename = "$schema", default, deserialize_with = "deserialize_schema")]
    schema: SchemaValue,
}

/// `$schema` キーの値。キーの有無と null を区別する（R2 は null を誤りにする）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum SchemaValue {
    /// キーが無い。スキーマを持たない文書
    Missing,
    /// キーはあるが値が null（`$schema:` や `$schema: null`）
    Null,
    Value(String),
}

impl Default for SchemaValue {
    fn default() -> Self {
        SchemaValue::Missing
    }
}

/// `$schema` の値が文字列であるか、null であるかを判別する。null とキー無しを
/// 区別するため、`Option<String>` ではなくこの型で受ける。
fn deserialize_schema<'de, D>(deserializer: D) -> Result<SchemaValue, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = SchemaValue;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or null for $schema")
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(SchemaValue::Null)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(SchemaValue::Null)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(SchemaValue::Value(value.to_string()))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(SchemaValue::Value(value))
        }
    }

    // 数値・配列などは visit_* が無く型エラーになり、R2 の「文字列でない $schema」として扱う
    deserializer.deserialize_any(Visitor)
}

/// 文書の frontmatter から `$schema` を読む。frontmatter が無ければスキーマなし。
/// `$schema` 以外のキーは無視する。壊れた YAML、null や文字列でない `$schema`、
/// 空の `$schema` はエラー。
pub fn frontmatter_schema(src: &str) -> Result<Option<SchemaRef>, FrontmatterError> {
    let matter = Matter::<YAML>::new();
    let result = matter
        .parse::<Frontmatter>(src)
        .map_err(|e| FrontmatterError(format!("{e}")))?;
    match result.data {
        Some(frontmatter) => match frontmatter.schema {
            SchemaValue::Missing => Ok(None),
            SchemaValue::Null => Err(FrontmatterError("$schema is null".into())),
            SchemaValue::Value(value) if value.is_empty() => {
                Err(FrontmatterError("$schema is empty".into()))
            }
            SchemaValue::Value(value) => Ok(Some(classify(value))),
        },
        None => Ok(None),
    }
}

/// `$schema` の参照を、文書の位置を基準に解決する。
pub fn resolve_schema(doc_path: &Path, schema_ref: &SchemaRef) -> ResolvedSchema {
    match schema_ref {
        SchemaRef::Url(url) => ResolvedSchema::Url(url.clone()),
        SchemaRef::Relative(rel) => {
            let base = doc_path.parent().unwrap_or_else(|| Path::new("."));
            ResolvedSchema::File(normalize(&base.join(rel)))
        }
    }
}

/// `..` と `.` を字句的に畳む。ファイルシステムには触れない。
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if out.as_os_str().is_empty() {
                    out.push("..");
                } else {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn classify(schema: String) -> SchemaRef {
    if schema.starts_with("http://") || schema.starts_with("https://") {
        SchemaRef::Url(schema)
    } else {
        SchemaRef::Relative(schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adr() -> &'static str {
        "---\n$schema: ../.mds/schemas/adr.yaml\ntitle: 例\n---\n# 題名\n"
    }

    #[test]
    fn reads_relative_schema_ref_and_ignores_other_keys() {
        let ref_ = frontmatter_schema(adr()).unwrap().unwrap();
        assert_eq!(ref_, SchemaRef::Relative("../.mds/schemas/adr.yaml".into()));
    }

    #[test]
    fn reads_url_schema_ref() {
        let src = "---\n$schema: https://example.com/schema.yaml\n---\n# 題名\n";
        let ref_ = frontmatter_schema(src).unwrap().unwrap();
        assert_eq!(ref_, SchemaRef::Url("https://example.com/schema.yaml".into()));
    }

    #[test]
    fn document_without_frontmatter_has_no_schema() {
        assert_eq!(frontmatter_schema("# 題名\n").unwrap(), None);
    }

    #[test]
    fn frontmatter_without_schema_key_has_no_schema() {
        let src = "---\ntitle: 例\n---\n# 題名\n";
        assert_eq!(frontmatter_schema(src).unwrap(), None);
    }

    #[test]
    fn broken_frontmatter_is_an_error() {
        let src = "---\n$schema: [\n---\n# 題名\n";
        assert!(frontmatter_schema(src).is_err());
    }

    #[test]
    fn non_string_schema_is_an_error() {
        let src = "---\n$schema: 42\n---\n# 題名\n";
        assert!(frontmatter_schema(src).is_err());
    }

    #[test]
    fn null_schema_is_an_error() {
        let src = "---\n$schema:\n---\n# 題名\n";
        assert!(frontmatter_schema(src).is_err());
    }

    #[test]
    fn empty_string_schema_is_an_error() {
        let src = "---\n$schema: \"\"\n---\n# 題名\n";
        assert!(frontmatter_schema(src).is_err());
    }

    #[test]
    fn array_schema_is_an_error() {
        let src = "---\n$schema: [a, b]\n---\n# 題名\n";
        assert!(frontmatter_schema(src).is_err());
    }

    #[test]
    fn relative_ref_resolves_against_document_location() {
        let doc = std::path::Path::new("fixtures/adr/0001.md");
        let ref_ = SchemaRef::Relative("../.mds/schemas/adr.yaml".into());
        let resolved = resolve_schema(doc, &ref_);
        assert_eq!(
            resolved,
            ResolvedSchema::File(std::path::PathBuf::from("fixtures/.mds/schemas/adr.yaml"))
        );
    }

    #[test]
    fn url_ref_stays_a_url() {
        let doc = std::path::Path::new("fixtures/adr/0001.md");
        let ref_ = SchemaRef::Url("https://example.com/schema.yaml".into());
        assert_eq!(
            resolve_schema(doc, &ref_),
            ResolvedSchema::Url("https://example.com/schema.yaml".into())
        );
    }
}