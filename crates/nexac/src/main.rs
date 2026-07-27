#![forbid(unsafe_code)]
//! Nexa compiler command-line entry point.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use nexa_compiler::{check, run, CheckResult, RunResult, RuntimeError};
use nexa_diagnostics::{Diagnostic, Severity};
use nexa_parser::{parse_source, Parse};
use nexa_source::SourceMap;
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
    /// Check and execute a source file's `main` function.
    Run {
        /// Source file to execute.
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
            emit_diagnostics(&sources, parse.diagnostics());

            if !parse.is_ok() {
                bail!(
                    "parse produced {} error(s)",
                    error_count(parse.diagnostics())
                );
            }
        }
        Command::Check { file } => {
            let mut sources = SourceMap::default();
            let result = check_file(&mut sources, &file)?;

            emit_diagnostics(&sources, result.diagnostics());

            if !result.is_ok() {
                bail!(
                    "check failed with {} error(s)",
                    error_count(result.diagnostics())
                );
            }

            println!("ok");
        }
        Command::Run { file } => {
            let mut sources = SourceMap::default();
            let result = run_file(&mut sources, &file)?;

            emit_diagnostics(&sources, result.diagnostics());

            for line in result.output() {
                println!("{line}");
            }

            if let Some(runtime_error) = result.runtime_error() {
                emit_runtime_error(&sources, runtime_error);
                bail!("run failed: {runtime_error}");
            }
            if !result.is_ok() {
                bail!(
                    "run failed with {} error(s)",
                    error_count(result.diagnostics())
                );
            }
        }
    }

    Ok(())
}

fn read_source(file: &Path) -> Result<String> {
    std::fs::read_to_string(file).with_context(|| format!("failed to read {}", file.display()))
}

fn parse_file(sources: &mut SourceMap, path: &Path) -> Result<Parse> {
    let file = register_source(sources, path)?;
    let source = source_text(sources, file)?;

    Ok(parse_source(file, source))
}

fn check_file(sources: &mut SourceMap, path: &Path) -> Result<CheckResult> {
    let file = register_source(sources, path)?;
    let source = source_text(sources, file)?;

    Ok(check(file, source))
}

fn run_file(sources: &mut SourceMap, path: &Path) -> Result<RunResult> {
    let file = register_source(sources, path)?;
    let source = source_text(sources, file)?;

    Ok(run(file, source))
}

fn register_source(sources: &mut SourceMap, path: &Path) -> Result<FileId> {
    let source = read_source(path)?;
    sources
        .add(path, source)
        .context("failed to register source file")
}

fn source_text(sources: &SourceMap, file: FileId) -> Result<&str> {
    sources
        .file(file)
        .map(|source| source.text())
        .ok_or_else(|| anyhow::anyhow!("registered source file was not found"))
}

fn emit_diagnostics(sources: &SourceMap, diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
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

fn error_count(diagnostics: &[Diagnostic]) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity() == Severity::Error)
        .count()
}

fn emit_runtime_error(sources: &SourceMap, error: &RuntimeError) {
    eprintln!("runtime error: {}", error.message());
    if let Some((file, location)) = sources.location(error.span()) {
        eprintln!(
            "  --> {}:{}:{}",
            file.path().display(),
            location.line(),
            location.column()
        );
    } else {
        let range = error.span().range();
        eprintln!("  --> <unknown>:{}..{}", range.start(), range.end());
    }
}
