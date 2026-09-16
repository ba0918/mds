use clap::{Parser, Subcommand};
use mds_core::document::Document;
use mds_core::finding::Finding;
use mds_core::frontmatter::{ResolvedSchema, SchemaRef};
use mds_core::schema::Schema;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::WalkDir;

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
        /// Output a typed tree built from the schema's extract rules
        #[arg(long)]
        schema: bool,
        /// Output format (json only)
        #[arg(long, value_parser = ["json"])]
        format: Option<String>,
    },
    /// Extract the values declared by the schema
    Values {
        /// Markdown file to extract from
        file: PathBuf,
        /// Output format
        #[arg(long, value_parser = ["json", "text"], default_value = "text")]
        format: String,
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
        Command::Ast {
            file,
            schema,
            format: _,
        } => {
            let src = read_document(&file)?;
            let json = if schema {
                let (schema, document) = load_schema_and_document(&file, &src)?;
                mds_core::extract::extract_typed(&schema, &document)
            } else {
                mds_core::ast::ast_json(&src).map_err(|e| Stop {
                    kind: "unreadable_file",
                    detail: format!("{}: {}", file.display(), e),
                })?
            };
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
            Ok(0)
        }
        Command::Values { file, format } => {
            let src = read_document(&file)?;
            let (schema, document) = load_schema_and_document(&file, &src)?;
            let values = mds_core::extract::extract_values(&schema, &document);
            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&values).unwrap()),
                "text" => print!("{}", mds_core::extract::render_values_text(&values)),
                _ => unreachable!("clap が --format の値を制限する"),
            }
            Ok(0)
        }
        Command::Check {
            path,
            format,
            open,
        } => {
            if path.is_dir() {
                let files = check_directory(&path, open)?;
                emit_check(&files, &format)?;
                Ok(if files.iter().any(|(_, f)| !f.is_empty()) {
                    1
                } else {
                    0
                })
            } else {
                let findings = check_document(&path, open)?;
                let files = vec![(path, findings)];
                emit_check(&files, &format)?;
                Ok(if files.iter().any(|(_, f)| !f.is_empty()) {
                    1
                } else {
                    0
                })
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

/// URL スキーマの応答の読み込み上限。壊れたサーバがメモリを食い潰すのを防ぐ。
const MAX_SCHEMA_BYTES: u64 = 4 * 1024 * 1024;

/// `$schema` の参照を解決してスキーマ YAML の文字列を読む。URL はキャッシュを優先する。
fn load_schema_yaml(doc_path: &Path, schema_ref: &SchemaRef) -> Result<String, Stop> {
    match mds_core::frontmatter::resolve_schema(doc_path, schema_ref) {
        ResolvedSchema::File(path) => std::fs::read_to_string(&path).map_err(|e| Stop {
            kind: "schema_not_found",
            detail: format!("cannot read schema {}: {}", path.display(), e),
        }),
        ResolvedSchema::Url(url) => {
            let cache = cache_path(&url);
            if let Ok(content) = std::fs::read_to_string(&cache) {
                return Ok(content);
            }
            let mut response = ureq::get(&url)
                .call()
                .map_err(|e| Stop {
                    kind: "schema_not_found",
                    detail: format!("cannot fetch schema {url}: {e}"),
                })?;
            let body = response
                .body_mut()
                .with_config()
                .limit(MAX_SCHEMA_BYTES)
                .lossy_utf8(true)
                .read_to_string()
                .map_err(|e| Stop {
                    kind: "schema_not_found",
                    detail: format!("cannot read schema {url}: {e}"),
                })?;
            // キャッシュへの保存はベストエフォート。書けなくても取得した内容で進める
            if let Some(parent) = cache.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&cache, &body);
            Ok(body)
        }
    }
}

/// 基準のディレクトリから上に向かって、最初に見つかった `.mds/` のあるディレクトリを返す。
/// 無ければカレントディレクトリ。
fn base_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut dir = cwd.as_path();
    loop {
        if dir.join(".mds").is_dir() {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return cwd,
        }
    }
}

/// URL スキーマのキャッシュファイルの置き場。基準のディレクトリの `.mds/cache/` に
/// URL の SHA-256 の16進で置く。
fn cache_path(url: &str) -> PathBuf {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    base_dir().join(".mds").join("cache").join(format!("{hash}.yaml"))
}

fn emit_check(files: &[(PathBuf, Vec<Finding>)], format: &str) -> Result<(), Stop> {
    let non_empty: Vec<&(PathBuf, Vec<Finding>)> =
        files.iter().filter(|(_, findings)| !findings.is_empty()).collect();
    if non_empty.is_empty() {
        return Ok(());
    }
    match format {
        "text" => {
            for (path, findings) in &non_empty {
                let path_str = path.to_string_lossy();
                for finding in findings {
                    match finding.line {
                        Some(line) => println!(
                            "{path_str}:{line}: {}: {}",
                            finding.kind.as_str(),
                            finding.detail
                        ),
                        None => {
                            println!("{path_str}: {}: {}", finding.kind.as_str(), finding.detail)
                        }
                    }
                }
            }
        }
        "json" => {
            let files_json: Vec<serde_json::Value> = non_empty
                .iter()
                .map(|(path, findings)| {
                    let path_str = path.to_string_lossy();
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
                    serde_json::json!({ "path": path_str, "findings": findings_json })
                })
                .collect();
            let output = serde_json::json!({ "files": files_json });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
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

/// ディレクトリ配下のスキーマ宣言文書を検査する。R20。
fn check_directory(root: &Path, open_flag: bool) -> Result<Vec<(PathBuf, Vec<Finding>)>, Stop> {
    let mut files = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            // 隠しディレクトリと .mds/ は辿らない。ルート自身は除く。
            if entry.depth() == 0 || !entry.file_type().is_dir() {
                return true;
            }
            !entry.file_name().to_string_lossy().starts_with('.')
        });

    for entry in walker {
        let entry = entry.map_err(|e| Stop {
            kind: "unreadable_file",
            detail: format!("cannot walk {}: {}", root.display(), e),
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let path = entry.path().to_path_buf();
        let src = read_document(&path)?;
        let schema_ref = mds_core::frontmatter::frontmatter_schema(&src).map_err(|e| Stop {
            kind: "frontmatter_invalid",
            detail: format!("{}: {}", path.display(), e.0),
        })?;
        // スキーマを持たない文書は対象外
        let Some(schema_ref) = schema_ref else {
            continue;
        };
        let schema_yaml = load_schema_yaml(&path, &schema_ref)?;
        let schema = mds_core::schema::parse_schema(&schema_yaml).map_err(|e| Stop {
            kind: "schema_invalid",
            detail: format!("{}: {}", path.display(), e.0),
        })?;
        let document = Document::parse(&src).map_err(|e| Stop {
            kind: "unreadable_file",
            detail: format!("{}: {}", path.display(), e),
        })?;
        let open = open_flag || schema.open;
        let findings = mds_core::validate::validate(&schema, &document, open);
        files.push((path, findings));
    }
    Ok(files)
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