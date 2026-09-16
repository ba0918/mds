use clap::{Parser, Subcommand};
use mds_core::document::Document;
use mds_core::finding::Finding;
use mds_core::frontmatter::{ResolvedSchema, SchemaRef};
use mds_core::schema::Schema;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "mds",
    version,
    about = "Validate Markdown documents against a YAML schema declared in their frontmatter and extract structured values"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate a document or every schema-declaring document under a directory
    Check {
        /// File or directory to check
        path: PathBuf,
        /// Output format
        #[arg(long, value_parser = ["json", "text"], default_value = "text")]
        format: String,
        /// Allow undeclared headings and lines (overrides the schema's `open`)
        #[arg(long)]
        open: bool,
    },
    /// Output the raw syntax tree (mdast) of a document as JSON
    Ast {
        /// Markdown file to parse
        file: PathBuf,
        /// Output format (json only)
        #[arg(long, value_parser = ["json"])]
        format: Option<String>,
    },
}

/// 検査を行えないときの停止。終了コード 2 で終わり、理由を標準エラーに 1 行出す。
struct Stop {
    kind: &'static str,
    detail: String,
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.detail)
    }
}

#[derive(Serialize)]
struct FindingJson<'a> {
    kind: &'a str,
    severity: &'a str,
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    detail: &'a str,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(stop) => {
            eprintln!("mds: {stop}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<u8, Stop> {
    match cli.command {
        Command::Ast { file, .. } => {
            let src = read_document(&file)?;
            let json = mds_core::ast::ast_json(&src).map_err(|e| Stop {
                kind: "unreadable_file",
                detail: format!("{}: {}", file.display(), e),
            })?;
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
            Ok(0)
        }
        Command::Check {
            path,
            format,
            open,
        } => {
            if path.is_dir() {
                check_directory(&path, &format, open)
            } else {
                let findings = check_document(&path, open)?;
                emit_check(&path, &findings, &format)?;
                Ok(if findings.is_empty() { 0 } else { 1 })
            }
        }
    }
}

/// 1文書を読み、スキーマを解決して検証し、指摘を返す。
fn check_document(path: &Path, open_flag: bool) -> Result<Vec<Finding>, Stop> {
    let src = read_document(path)?;
    let (schema, document) = load_schema_and_document(path, &src)?;
    let open = open_flag || schema.open;
    Ok(mds_core::validate::validate(&schema, &document, open))
}

/// スキーマを解決して文書と共に返す。check・values・ast --schema が共有する。
fn load_schema_and_document(path: &Path, src: &str) -> Result<(Schema, Document), Stop> {
    let schema_ref = mds_core::frontmatter::frontmatter_schema(src).map_err(|e| Stop {
        kind: "frontmatter_invalid",
        detail: format!("{}: {}", path.display(), e.0),
    })?;
    let schema_ref = schema_ref.ok_or_else(|| Stop {
        kind: "schema_not_found",
        detail: format!("{}: the document has no $schema in frontmatter", path.display()),
    })?;
    let schema_yaml = load_schema_yaml(path, &schema_ref)?;
    let schema = mds_core::schema::parse_schema(&schema_yaml).map_err(|e| Stop {
        kind: "schema_invalid",
        detail: format!("{}: {}", path.display(), e.0),
    })?;
    let document = Document::parse(src).map_err(|e| Stop {
        kind: "unreadable_file",
        detail: format!("{}: {}", path.display(), e),
    })?;
    Ok((schema, document))
}

/// `$schema` の参照を解決してスキーマ YAML の文字列を読む。
fn load_schema_yaml(doc_path: &Path, schema_ref: &SchemaRef) -> Result<String, Stop> {
    match mds_core::frontmatter::resolve_schema(doc_path, schema_ref) {
        ResolvedSchema::File(path) => std::fs::read_to_string(&path).map_err(|e| Stop {
            kind: "schema_not_found",
            detail: format!("cannot read schema {}: {}", path.display(), e),
        }),
        ResolvedSchema::Url(_url) => {
            // Step 12 で URL 取得とキャッシュを実装する
            Err(Stop {
                kind: "schema_not_found",
                detail: "cannot fetch a URL schema".into(),
            })
        }
    }
}

fn emit_check(path: &Path, findings: &[Finding], format: &str) -> Result<(), Stop> {
    if findings.is_empty() {
        return Ok(());
    }
    let path_str = path.to_string_lossy();
    match format {
        "text" => {
            for finding in findings {
                match finding.line {
                    Some(line) => println!(
                        "{path_str}:{line}: {}: {}",
                        finding.kind.as_str(),
                        finding.detail
                    ),
                    None => println!("{path_str}: {}: {}", finding.kind.as_str(), finding.detail),
                }
            }
        }
        "json" => {
            let findings_json: Vec<FindingJson> = findings
                .iter()
                .map(|f| FindingJson {
                    kind: f.kind.as_str(),
                    severity: "error",
                    path: &path_str,
                    line: f.line,
                    detail: &f.detail,
                })
                .collect();
            let files = serde_json::json!({ "files": [{ "path": path_str, "findings": findings_json }] });
            println!("{}", serde_json::to_string_pretty(&files).unwrap());
        }
        _ => {
            return Err(Stop {
                kind: "schema_invalid",
                detail: format!("unknown format {format}"),
            })
        }
    }
    Ok(())
}

/// 文書を読み、先頭の BOM を取り除いて返す。
fn read_document(path: &Path) -> Result<String, Stop> {
    let bytes = std::fs::read(path).map_err(|e| Stop {
        kind: "unreadable_file",
        detail: format!("{}: {}", path.display(), e),
    })?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec()).map_err(|e| Stop {
        kind: "unreadable_file",
        detail: format!("{}: {}", path.display(), e),
    })
}

fn check_directory(_path: &Path, _format: &str, _open: bool) -> Result<u8, Stop> {
    // Step 11 でディレクトリ検査を実装する
    Err(Stop {
        kind: "unreadable_file",
        detail: "directory checking is not implemented yet".into(),
    })
}