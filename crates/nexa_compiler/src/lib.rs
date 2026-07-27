#![forbid(unsafe_code)]
//! Compiler-driver entry points for the Nexa language front end.

use nexa_diagnostics::Diagnostic;
use nexa_hir::{lower, type_check, TypedProgram};
use nexa_parser::parse_source;
use nexa_span::FileId;

/// The result of checking one Nexa source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    typed: Option<TypedProgram>,
    diagnostics: Vec<Diagnostic>,
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

fn diagnostic_position(diagnostic: &Diagnostic) -> (usize, usize) {
    diagnostic
        .labels()
        .first()
        .map_or((usize::MAX, usize::MAX), |label| {
            (label.span().range().start(), label.span().range().end())
        })
}
