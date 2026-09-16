use clap::{Parser, Subcommand};
use std::path::PathBuf;
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(stop) => {
            eprintln!("mds: {stop}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<(), Stop> {
    match cli.command {
        Command::Ast { file, .. } => {
            let src = read_document(&file)?;
            let json = mds_core::ast::ast_json(&src).map_err(|e| Stop {
                kind: "unreadable_file",
                detail: format!("{}: {}", file.display(), e),
            })?;
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
            Ok(())
        }
    }
}

/// 文書を読み、先頭の BOM を取り除いて返す。
fn read_document(path: &PathBuf) -> Result<String, Stop> {
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