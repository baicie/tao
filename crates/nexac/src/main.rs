#![forbid(unsafe_code)]
//! Nexa compiler command-line entry point.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use nexa_parser::parse_source;
use nexa_span::FileId;

#[derive(Debug, Parser)]
#[command(name = "nexac", version, about = "Nexa bootstrap compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parse a source file and print its concrete syntax tree.
    Parse {
        /// Source file to parse.
        file: PathBuf,
    },
    /// Check a source file for front-end diagnostics.
    Check {
        /// Source file to check.
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Parse { file } => {
            let source = read_source(&file)?;
            let parse = parse_source(FileId::new(0), &source);

            println!("{}", parse.debug_tree());
            emit_diagnostics(&file, &parse);

            if !parse.is_ok() {
                bail!("parse produced {} diagnostic(s)", parse.diagnostics().len());
            }
        }
        Command::Check { file } => {
            let source = read_source(&file)?;
            let parse = parse_source(FileId::new(0), &source);

            emit_diagnostics(&file, &parse);

            if !parse.is_ok() {
                bail!(
                    "check failed with {} diagnostic(s)",
                    parse.diagnostics().len()
                );
            }

            println!("ok");
        }
    }

    Ok(())
}

fn read_source(file: &Path) -> Result<String> {
    std::fs::read_to_string(file).with_context(|| format!("failed to read {}", file.display()))
}

fn emit_diagnostics(file: &Path, parse: &nexa_parser::Parse) {
    for diagnostic in parse.diagnostics() {
        eprintln!("{:?}: {}", diagnostic.severity(), diagnostic.message());

        for label in diagnostic.labels() {
            let range = label.span().range();
            eprintln!(
                "  --> {}:{}..{}: {}",
                file.display(),
                range.start(),
                range.end(),
                label.message()
            );
        }
    }
}
