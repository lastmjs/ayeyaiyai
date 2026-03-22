use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use ayeyaiyai::{CompileOptions, compile_file};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Compile JavaScript directly to WASI Preview 2"
)]
struct Cli {
    input: PathBuf,

    #[arg(short, long)]
    output: PathBuf,

    #[arg(long, default_value = "wasm32-wasip2")]
    target: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let options = CompileOptions {
        output: cli.output,
        target: cli.target,
    };

    compile_file(&cli.input, &options)
}
