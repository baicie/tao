#![forbid(unsafe_code)]
//! Compiler-driver entry points for the Nexa language front end.

mod canonical;
mod core;
mod differential;
mod session;

use std::collections::{HashMap, HashSet};

use nexa_diagnostics::{Diagnostic, Severity};
use nexa_hir::{
    lower, lower_module, type_check, type_check_modules, ModuleId, Program, ResolvedImport,
    TypedProgram,
};
use nexa_mir::{lower as lower_mir, run_with_args as run_mir_with_args, Execution};
use nexa_parser::parse_source;
use nexa_span::FileId;

pub use core::{
    compile, compile_session, CanonicalArtifact, CanonicalArtifactState, CanonicalArtifactStatus,
    CanonicalDiagnostic, CanonicalDumps, CanonicalLabel, CanonicalLabelStyle, CanonicalPhase,
    CanonicalSeverity, CompileError, CompilerInput, CompilerOptions, CompilerOutput,
    CompilerSource, LanguageVersion, CANONICAL_DUMP_SCHEMA_VERSION,
};
pub use differential::{
    CompilerAdapter, CompilerAdapterState, CompilerImplementation, DifferenceClassification,
    DifferentialHarness, DifferentialIssue, DifferentialIssueKind, DifferentialOutcome,
    DifferentialReport, RustReferenceCompiler,
};
pub use nexa_mir::{MirProgram, RuntimeError};
pub use session::{CompilerSession, ImportEdge, SessionBuildError, SessionModule};

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

/// Lowers and type-checks every module loaded by a compiler session.
///
/// Source-loading and parser diagnostics are preserved. Semantic analysis
/// continues for graph-complete modules that do not depend on a failed or
/// syntactically malformed source, without creating derivative name errors in
/// incomplete modules.
#[must_use]
pub fn check_session<P>(session: &CompilerSession<P>) -> CheckResult {
    let mut diagnostics = session.diagnostics().to_vec();
    let programs = session
        .modules()
        .iter()
        .filter_map(|module| {
            if !module.parse().is_ok() {
                return None;
            }

            match lower_module(module.id(), module.file(), &module.parse().syntax()) {
                Ok(program) => Some(program),
                Err(error) => {
                    diagnostics.push(error.diagnostic());
                    None
                }
            }
        })
        .collect::<Vec<_>>();
    let links = session
        .edges()
        .iter()
        .map(|edge| ResolvedImport::new(edge.importer(), edge.path_span(), edge.imported()))
        .collect::<Vec<_>>();
    let programs = graph_complete_programs(programs, &links);

    let typed = if programs.is_empty() {
        None
    } else {
        let analysis = type_check_modules(&programs, &links);
        diagnostics.extend_from_slice(analysis.diagnostics());
        analysis.typed().cloned()
    };

    diagnostics.sort_by_key(diagnostic_position);
    let typed = typed.filter(|_| {
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity() != Severity::Error)
    });
    CheckResult { typed, diagnostics }
}

fn graph_complete_programs(programs: Vec<Program>, links: &[ResolvedImport]) -> Vec<Program> {
    let targets = links
        .iter()
        .map(|link| ((link.importer(), link.path_span()), link.target()))
        .collect::<HashMap<_, _>>();
    let mut complete = programs
        .iter()
        .map(|program| program.module)
        .collect::<HashSet<_>>();

    loop {
        let incomplete = programs
            .iter()
            .filter(|program| complete.contains(&program.module))
            .filter(|program| {
                program.imports.iter().any(|import| {
                    targets
                        .get(&(program.module, import.path_span))
                        .map_or(true, |target| !complete.contains(target))
                })
            })
            .map(|program| program.module)
            .collect::<Vec<ModuleId>>();
        if incomplete.is_empty() {
            break;
        }
        for module in incomplete {
            let _ = complete.remove(&module);
        }
    }

    programs
        .into_iter()
        .filter(|program| complete.contains(&program.module))
        .collect()
}

/// Parses, checks, lowers, and executes one Nexa source file.
#[must_use]
pub fn run(file: FileId, source: &str) -> RunResult {
    run_with_args(file, source, &[])
}

/// Parses, checks, lowers, and executes one Nexa source file with arguments.
#[must_use]
pub fn run_with_args(file: FileId, source: &str, arguments: &[String]) -> RunResult {
    execute_checked(check(file, source), arguments)
}

/// Checks and executes a complete loaded module graph with command-line arguments.
#[must_use]
pub fn run_session_with_args<P>(session: &CompilerSession<P>, arguments: &[String]) -> RunResult {
    execute_checked(check_session(session), arguments)
}

/// Checks and executes a complete loaded module graph.
#[must_use]
pub fn run_session<P>(session: &CompilerSession<P>) -> RunResult {
    run_session_with_args(session, &[])
}

fn execute_checked(checked: CheckResult, arguments: &[String]) -> RunResult {
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

fn diagnostic_position(diagnostic: &Diagnostic) -> (u32, usize, usize, &'static str) {
    diagnostic.labels().first().map_or(
        (u32::MAX, usize::MAX, usize::MAX, diagnostic.code().as_str()),
        |label| {
            (
                label.span().file().raw(),
                label.span().range().start(),
                label.span().range().end(),
                diagnostic.code().as_str(),
            )
        },
    )
}
