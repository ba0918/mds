use clap::Parser;

#[derive(Parser)]
#[command(
    name = "mds",
    version,
    about = "Validate Markdown documents against a YAML schema declared in their frontmatter and extract structured values"
)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}