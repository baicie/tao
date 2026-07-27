//! Compiler-driver integration tests.

use nexa_compiler::{check, run, run_with_args};
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

#[test]
fn run_preserves_output_before_a_runtime_failure() {
    let result = run(
        FileId::new(0),
        "function main(): Unit { print(1); print(1 / 0); }",
    );

    assert_eq!(result.output(), ["1"]);
    assert_eq!(
        result.runtime_error().map(|error| error.message()),
        Some("division by zero")
    );
}

#[test]
fn run_with_args_executes_a_string_array_main_function() {
    let arguments = ["Nexa".to_owned()];
    let result = run_with_args(
        FileId::new(0),
        "function main(args: String[]): Unit { print(args[0]); }",
        &arguments,
    );

    assert!(result.is_ok(), "diagnostics: {:?}", result.diagnostics());
    assert_eq!(result.output(), ["Nexa"]);
}

#[test]
fn run_passes_an_empty_array_to_an_argument_taking_main() {
    let result = run(
        FileId::new(0),
        "function main(args: String[]): Unit { print(args.length); }",
    );

    assert!(result.is_ok(), "diagnostics: {:?}", result.diagnostics());
    assert_eq!(result.output(), ["0"]);
}
