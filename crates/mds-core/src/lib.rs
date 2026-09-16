//! スキーマ言語・検証・抽出のコア。CLI のフレームワークには依存しない。

pub mod ast;
pub mod frontmatter;
pub mod document;
pub mod extract;
pub mod finding;
pub mod schema;
pub mod validate;