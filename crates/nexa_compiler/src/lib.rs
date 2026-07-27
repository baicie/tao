#![forbid(unsafe_code)]
//! Compiler-driver entry points for the Nexa language front end.

use nexa_diagnostics::Diagnostic;
use nexa_hir::{lower, type_check, TypedProgram};
use nexa_mir::{lower as lower_mir, run_with_args as run_mir_with_args, Execution};
use nexa_parser::parse_source;
use nexa_span::FileId;

pub use nexa_mir::RuntimeError;

/// The result of checking one Nexa source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    typed: Option<TypedProgram>,
    diagnostics: Vec<Diagnostic>,
}

/// The result of checking and executing one Nexa source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    diagnostics: Vec<Diagnostic>,
    execution: Option<Execution>,
    runtime_error: Option<RuntimeError>,
    output: Vec<String>,
}

impl RunResult {
    /// Returns parser and semantic diagnostics in source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns the successful execution result, when available.
    #[must_use]
    pub fn execution(&self) -> Option<&Execution> {
        self.execution.as_ref()
    }

    /// Returns a runtime error after successful semantic analysis, when present.
    #[must_use]
    pub fn runtime_error(&self) -> Option<&RuntimeError> {
        self.runtime_error.as_ref()
    }

    /// Returns lines emitted before execution completed or failed.
    #[must_use]
    pub fn output(&self) -> &[String] {
        match &self.execution {
            Some(execution) => execution.output(),
            None => &self.output,
        }
    }

    /// Returns true when checking succeeded and the interpreter completed.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.execution.is_some()
    }
}

impl CheckResult {
    /// Returns validated typed HIR when parsing and semantic analysis succeeded.
    #[must_use]
    pub fn typed(&self) -> Option<&TypedProgram> {
        self.typed.as_ref()
    }

    /// Returns parser, lowering, and semantic diagnostics in source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns true when the source file is ready for lowering to a backend.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.typed.is_some()
    }
}

/// Parses, lowers, resolves, and type-checks one Nexa source file.
#[must_use]
pub fn check(file: FileId, source: &str) -> CheckResult {
    let parse = parse_source(file, source);
    let mut diagnostics = parse.diagnostics().to_vec();
    let typed = if parse.is_ok() {
        match lower(file, &parse.syntax()) {
            Ok(program) => {
                let analysis = type_check(&program);
                diagnostics.extend_from_slice(analysis.diagnostics());
                analysis.typed().cloned()
            }
            Err(error) => {
                diagnostics.push(error.diagnostic());
                None
            }
        }
    } else {
        None
    };

    diagnostics.sort_by_key(diagnostic_position);
    CheckResult { typed, diagnostics }
}

/// Parses, checks, lowers, and executes one Nexa source file.
#[must_use]
pub fn run(file: FileId, source: &str) -> RunResult {
    run_with_args(file, source, &[])
}

/// Parses, checks, lowers, and executes one Nexa source file with arguments.
#[must_use]
pub fn run_with_args(file: FileId, source: &str, arguments: &[String]) -> RunResult {
    let checked = check(file, source);
    let diagnostics = checked.diagnostics.clone();
    let Some(typed) = checked.typed() else {
        return RunResult {
            diagnostics,
            execution: None,
            runtime_error: None,
            output: Vec::new(),
        };
    };

    let program = match lower_mir(typed) {
        Ok(program) => program,
        Err(error) => {
            return RunResult {
                diagnostics,
                execution: None,
                runtime_error: Some(error.into()),
                output: Vec::new(),
            };
        }
    };

    match run_mir_with_args(&program, arguments) {
        Ok(execution) => RunResult {
            diagnostics,
            execution: Some(execution),
            runtime_error: None,
            output: Vec::new(),
        },
        Err(failure) => {
            let (runtime_error, output) = failure.into_parts();
            RunResult {
                diagnostics,
                execution: None,
                runtime_error: Some(runtime_error),
                output,
            }
        }
    }
}

fn diagnostic_position(diagnostic: &Diagnostic) -> (usize, usize) {
    diagnostic
        .labels()
        .first()
        .map_or((usize::MAX, usize::MAX), |label| {
            (label.span().range().start(), label.span().range().end())
        })
}
