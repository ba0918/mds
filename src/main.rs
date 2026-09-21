use clap::{Parser, Subcommand};
use mds_core::document::Document;
use mds_core::finding::Finding;
use mds_core::frontmatter::{ResolvedSchema, SchemaRef};
use mds_core::schema::Schema;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;
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
        // 停止は1行（R19、R20）。detail が複数行でも後続の行は落とす。
        // スキーマのパーサは誤りの位置とともに参照先ファイルの中身を引用
        // するため、そのまま流すと読める任意のファイルの断片が標準エラー
        // へ出る。1行に切る場所をここに置くのは、停止の種類が増えても
        // 書き出し口が1つのままだから
        let first_line = self.detail.lines().next().unwrap_or_default();
        write!(f, "{}: {}", self.kind, first_line)
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
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => match e.kind() {
            // --help と --version は標準出力に出して終了コード0で終わる（R1）
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                let _ = e.print();
                return ExitCode::from(0);
            }
            // サブコマンドを省いたときの clap のメッセージは about の文で始まる。
            // それをそのまま停止の説明にすると誤りの説明として意味を成さない
            clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                eprintln!("mds: argument_error: no subcommand given (see `mds --help`)");
                return ExitCode::from(2);
            }
            _ => {
                // clap のメッセージは複数行に分かれるので、説明の先頭行だけを
                // 使って mds の1行の停止理由に合わせる（R19 の argument_error）
                let message = e.to_string();
                let first_line = message.lines().next().unwrap_or("invalid arguments");
                let detail = first_line
                    .strip_prefix("error: ")
                    .unwrap_or(first_line)
                    .to_string();
                eprintln!("mds: argument_error: {detail}");
                return ExitCode::from(2);
            }
        },
    };
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
                "text" => print!("{}", render_values_text(&values)),
                _ => unreachable!("clap が --format の値を制限する"),
            }
            Ok(0)
        }
        Command::Check { path, format, open } => {
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
        detail: format!(
            "{}: the document has no $schema in frontmatter",
            path.display()
        ),
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

/// URL スキーマの取得全体のタイムアウト。応答しないサーバに無期限で
/// ブロックしないための既定値。
const SCHEMA_FETCH_TIMEOUT: Duration = Duration::from_millis(10_000);

/// URL の authority にある認証情報（`user:pass@`）を伏せる。取得に失敗した URL は
/// 誤りの説明として標準エラーに出るので、そこへ認証情報を持ち込まない（R2）。
fn redact_userinfo(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let authority_start = scheme_end + 3;
    let rest = &url[authority_start..];
    let authority_end = rest
        .find(['/', '?', '#'])
        .map(|i| authority_start + i)
        .unwrap_or(url.len());
    let Some(at) = url[authority_start..authority_end].rfind('@') else {
        return url.to_string();
    };
    format!(
        "{}***{}",
        &url[..authority_start],
        &url[authority_start + at..]
    )
}

/// `$schema` の参照を解決してスキーマ YAML の文字列を読む。URL はキャッシュを優先する。
fn load_schema_yaml(doc_path: &Path, schema_ref: &SchemaRef) -> Result<String, Stop> {
    match mds_core::frontmatter::resolve_schema(doc_path, schema_ref) {
        ResolvedSchema::File(path) => std::fs::read_to_string(&path).map_err(|e| Stop {
            kind: "schema_not_found",
            detail: format!("cannot read schema {}: {}", path.display(), e),
        }),
        ResolvedSchema::Url(url) => {
            let cache = cache_path(&url);
            // キャッシュを読めたら内容を検証し、壊れた・空なら URL から再取得する
            if let Ok(content) = std::fs::read_to_string(&cache)
                && mds_core::schema::parse_schema(&content).is_ok()
            {
                return Ok(content);
            }
            let mut response = ureq::get(&url)
                .config()
                .timeout_global(Some(SCHEMA_FETCH_TIMEOUT))
                .build()
                .call()
                .map_err(|e| Stop {
                    kind: "schema_not_found",
                    detail: format!("cannot fetch schema {}: {e}", redact_userinfo(&url)),
                })?;
            let body = response
                .body_mut()
                .with_config()
                .limit(MAX_SCHEMA_BYTES)
                .lossy_utf8(true)
                .read_to_string()
                .map_err(|e| Stop {
                    kind: "schema_not_found",
                    detail: format!("cannot read schema {}: {e}", redact_userinfo(&url)),
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
    base_dir()
        .join(".mds")
        .join("cache")
        .join(format!("{hash}.yaml"))
}

fn emit_check(files: &[(PathBuf, Vec<Finding>)], format: &str) -> Result<(), Stop> {
    match format {
        "text" => {
            // text は指摘があるときだけ出力する（R18）
            for (path, findings) in files {
                if findings.is_empty() {
                    continue;
                }
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
            // json は指摘が無いときも files の空配列を出力する（R18）
            let files_json: Vec<serde_json::Value> = files
                .iter()
                .filter(|(_, findings)| !findings.is_empty())
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
        _ => unreachable!("clap が --format の値を制限する"),
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

/// `values` の text 出力。ネストは2文字のインデント、配列は番号付き。
/// 抽出（スキーマと文書から JSON を組み立てる）とは別の関心事であり、
/// 唯一の利用者はこの CLI なので、コアではなくここに置く。
fn render_values_text(value: &serde_json::Value) -> String {
    let mut out = String::new();
    render_text(value, &mut out, 0);
    out
}

fn render_text(value: &serde_json::Value, out: &mut String, indent: usize) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                match val {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        push_indent(out, indent);
                        out.push_str(&format!("{key}:\n"));
                        render_text(val, out, indent + 2);
                    }
                    serde_json::Value::String(s) => {
                        push_indent(out, indent);
                        out.push_str(key);
                        out.push_str(": ");
                        push_string(out, indent + 2, s);
                        out.push('\n');
                    }
                    other => {
                        push_indent(out, indent);
                        out.push_str(&format!("{key}: {other}\n"));
                    }
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                match item {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        push_indent(out, indent);
                        out.push_str(&format!("{}.\n", i + 1));
                        render_text(item, out, indent + 2);
                    }
                    serde_json::Value::String(s) => {
                        push_indent(out, indent);
                        out.push_str(&format!("{}. ", i + 1));
                        push_string(out, indent + 2, s);
                        out.push('\n');
                    }
                    other => {
                        push_indent(out, indent);
                        out.push_str(&format!("{}. {other}\n", i + 1));
                    }
                }
            }
        }
        _ => {}
    }
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push(' ');
    }
}

/// 値の文字列を出力する。改行を含む値は続きの行をインデントし、1件の値として
/// 読めるようにする。末尾の改行は表示の邪魔なので落とす（json の値は変えない）。
fn push_string(out: &mut String, indent: usize, s: &str) {
    for (i, line) in s.trim_end_matches('\n').split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            push_indent(out, indent);
        }
        out.push_str(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 認証情報の形（"利用者:合言葉@"）のリテラルはこのファイルに置かない。
    /// 走査の道具がそれを本物の認証情報として報告し、写した人が真似るため。
    fn url_with_userinfo(user: &str, password: &str) -> String {
        format!("https://{user}:{password}@example.com/ir.yaml")
    }

    // @kotowari[REQ-052]
    #[test]
    fn userinfo_in_a_url_is_hidden_before_it_reaches_a_message() {
        let url = url_with_userinfo("example-user", "example-password");
        assert_eq!(
            redact_userinfo(&url),
            "https://***@example.com/ir.yaml",
            "認証情報を伏せた形にする"
        );
        assert!(
            !redact_userinfo(&url).contains("example-password"),
            "合言葉が残っていない"
        );
    }

    // @kotowari[REQ-052]
    #[test]
    fn a_url_without_userinfo_is_left_alone() {
        assert_eq!(
            redact_userinfo("https://example.com/ir.yaml"),
            "https://example.com/ir.yaml"
        );
    }

    // @kotowari[REQ-052]
    #[test]
    fn an_at_sign_in_the_path_is_not_mistaken_for_userinfo() {
        assert_eq!(
            redact_userinfo("https://example.com/a@b/ir.yaml"),
            "https://example.com/a@b/ir.yaml"
        );
    }
}
