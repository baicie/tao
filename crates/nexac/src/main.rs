#![forbid(unsafe_code)]
//! Nexa compiler command-line entry point.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use nexa_parser::{parse_source, Parse};
use nexa_source::SourceMap;

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
            let mut sources = SourceMap::default();
            let parse = parse_file(&mut sources, &file)?;

            println!("{}", parse.debug_tree());
            emit_diagnostics(&sources, &parse);

            if !parse.is_ok() {
                bail!("parse produced {} diagnostic(s)", parse.diagnostics().len());
            }
        }
        Command::Check { file } => {
            let mut sources = SourceMap::default();
            let parse = parse_file(&mut sources, &file)?;

            emit_diagnostics(&sources, &parse);

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

fn parse_file(sources: &mut SourceMap, path: &Path) -> Result<Parse> {
    let source = read_source(path)?;
    let file = sources
        .add(path, source)
        .context("failed to register source file")?;
    let source = sources
        .file(file)
        .ok_or_else(|| anyhow::anyhow!("registered source file was not found"))?;

    Ok(parse_source(file, source.text()))
}

fn emit_diagnostics(sources: &SourceMap, parse: &Parse) {
    for diagnostic in parse.diagnostics() {
        eprintln!(
            "{} {:?}: {}",
            diagnostic.code(),
            diagnostic.severity(),
            diagnostic.message()
        );

        for label in diagnostic.labels() {
            let range = label.span().range();
            if let Some((file, location)) = sources.location(label.span()) {
                eprintln!(
                    "  --> {}:{}:{}: {}",
                    file.path().display(),
                    location.line(),
                    location.column(),
                    label.message()
                );
            } else {
                eprintln!(
                    "  --> <unknown>:{}..{}: {}",
                    range.start(),
                    range.end(),
                    label.message()
                );
            }
        }
    }
}
