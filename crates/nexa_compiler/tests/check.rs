//! Compiler-driver integration tests.

use nexa_compiler::{check, run};
use nexa_span::FileId;

#[test]
fn check_returns_typed_hir_for_a_valid_program() {
    let result = check(
        FileId::new(0),
        "function main(): Unit { const answer = 42; print(answer); }",
    );

    assert!(result.is_ok(), "diagnostics: {:?}", result.diagnostics());
}

#[test]
fn check_returns_semantic_diagnostics_after_parsing() {
    let result = check(FileId::new(0), "function main(): Unit { if (42) {} }");

    assert!(!result.is_ok());
    assert!(result
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code().as_str() == "E3002"));
}

#[test]
fn check_does_not_lower_malformed_syntax() {
    let result = check(FileId::new(0), "function (): Unit {}");

    assert!(!result.is_ok());
    assert!(result
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code().as_str() == "E1001"));
}

#[test]
fn run_executes_a_checked_main_function() {
    let result = run(
        FileId::new(0),
        "function main(): Unit { const answer = 42; print(answer); }",
    );

    assert!(result.is_ok(), "diagnostics: {:?}", result.diagnostics());
    assert_eq!(
        result
            .execution()
            .and_then(|execution| execution.output().first())
            .map(String::as_str),
        Some("42")
    );
}
