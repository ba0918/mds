//! 指摘のモデル。R18。

/// 指摘の種別。R18 に列挙された kind に対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    MissingTitle,
    MultipleTitles,
    TitlePatternMismatch,
    UndeclaredHeading,
    UndeclaredLine,
    MissingRequiredField,
    MissingRequiredSection,
    MissingStatement,
    MissingBullets,
    MissingTable,
    MissingCodeblock,
    FieldPatternMismatch,
    FieldEnumInvalid,
    StatementPatternMismatch,
    StatementEnumInvalid,
    BulletPatternMismatch,
    HeadingLevelMismatch,
    InvalidId,
    FieldOrderMismatch,
    TableHeaderMismatch,
    CodeblockLangMismatch,
    CodeblockLineMismatch,
    RepeatMinNotMet,
    RepeatMaxExceeded,
}

impl FindingKind {
    /// R18 の kind の文字列。snake_case。
    pub fn as_str(&self) -> &'static str {
        match self {
            FindingKind::MissingTitle => "missing_title",
            FindingKind::MultipleTitles => "multiple_titles",
            FindingKind::TitlePatternMismatch => "title_pattern_mismatch",
            FindingKind::UndeclaredHeading => "undeclared_heading",
            FindingKind::UndeclaredLine => "undeclared_line",
            FindingKind::MissingRequiredField => "missing_required_field",
            FindingKind::MissingRequiredSection => "missing_required_section",
            FindingKind::MissingStatement => "missing_statement",
            FindingKind::MissingBullets => "missing_bullets",
            FindingKind::MissingTable => "missing_table",
            FindingKind::MissingCodeblock => "missing_codeblock",
            FindingKind::FieldPatternMismatch => "field_pattern_mismatch",
            FindingKind::FieldEnumInvalid => "field_enum_invalid",
            FindingKind::StatementPatternMismatch => "statement_pattern_mismatch",
            FindingKind::StatementEnumInvalid => "statement_enum_invalid",
            FindingKind::BulletPatternMismatch => "bullet_pattern_mismatch",
            FindingKind::HeadingLevelMismatch => "heading_level_mismatch",
            FindingKind::InvalidId => "invalid_id",
            FindingKind::FieldOrderMismatch => "field_order_mismatch",
            FindingKind::TableHeaderMismatch => "table_header_mismatch",
            FindingKind::CodeblockLangMismatch => "codeblock_lang_mismatch",
            FindingKind::CodeblockLineMismatch => "codeblock_line_mismatch",
            FindingKind::RepeatMinNotMet => "repeat_min_not_met",
            FindingKind::RepeatMaxExceeded => "repeat_max_exceeded",
        }
    }
}

/// 指摘1件。`path` と `severity` は CLI 側が付ける。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub kind: FindingKind,
    /// 1始まりの行番号。行を持たない指摘は None。
    pub line: Option<usize>,
    /// 指摘の詳細（英語）
    pub detail: String,
}