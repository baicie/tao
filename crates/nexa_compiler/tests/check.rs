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

#[test]
fn check_returns_typed_hir_for_nominal_records() {
    let result = check(
        FileId::new(0),
        r#"type User = { name: String; scores: Int[]; };
function main(): Unit {
  const user: User = { scores: [20, 22], name: "Ada" };
  print(user.name);
}"#,
    );

    assert!(result.is_ok(), "diagnostics: {:?}", result.diagnostics());
}

#[test]
fn check_reports_missing_and_recursive_record_diagnostics() {
    let result = check(
        FileId::new(0),
        r#"type Node = { children: Node[]; };
type User = { name: String; age: Int; };
function main(): Unit { const user: User = { name: "Ada" }; }"#,
    );
    let codes = result
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code().as_str())
        .collect::<Vec<_>>();

    assert!(!result.is_ok());
    assert!(codes.contains(&"E2006"), "codes: {codes:?}");
    assert!(codes.contains(&"E3005"), "codes: {codes:?}");
}

#[test]
fn run_executes_records_with_source_order_initializers_and_declared_layout() {
    let result = run(
        FileId::new(0),
        r#"type Pair = { first: Int; second: Int; };
function emit(value: Int): Int { print(value); return value; }
function main(): Unit {
  const pair: Pair = { second: emit(2), first: emit(1) };
  print(pair.first);
  print(pair.second);
}"#,
    );

    assert!(result.is_ok(), "diagnostics: {:?}", result.diagnostics());
    assert_eq!(result.output(), ["2", "1", "1", "2"]);
}

#[test]
fn run_preserves_output_before_a_record_initializer_failure() {
    let result = run(
        FileId::new(0),
        r#"type Pair = { first: Int; second: Int; };
function fail(): Int { print(7); return [1][2]; }
function main(): Unit {
  const pair: Pair = { first: fail(), second: 2 };
  print(pair.second);
}"#,
    );

    assert_eq!(result.output(), ["7"]);
    assert_eq!(
        result.runtime_error().map(|error| error.message()),
        Some("array index 2 out of bounds for length 1")
    );
}

#[test]
fn check_returns_typed_hir_for_recursive_tagged_unions_and_exhaustive_match() {
    let result = check(FileId::new(0), tagged_union_program());

    assert!(result.is_ok(), "diagnostics: {:?}", result.diagnostics());
}

#[test]
fn check_reports_tagged_union_match_diagnostics() {
    let result = check(
        FileId::new(0),
        r#"type Flag = | Off() | On();
function read(flag: Flag): Int {
  return match (flag) {
    case Flag.Off() => 0;
  };
}"#,
    );

    assert!(!result.is_ok());
    assert!(result
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code().as_str() == "E3006"));
}

#[test]
fn run_executes_recursive_tagged_unions_and_exhaustive_match() {
    let result = run(FileId::new(0), tagged_union_program());

    assert!(result.is_ok(), "diagnostics: {:?}", result.diagnostics());
    assert_eq!(result.output(), ["42"]);
}

fn tagged_union_program() -> &'static str {
    r#"type List =
  | Empty()
  | Node(head: Int, tail: List);

function sum(values: List): Int {
  return match (values) {
    case List.Empty() => 0;
    case List.Node(head, tail) => head + sum(tail);
  };
}

function main(): Unit {
  const values = List.Node(20, List.Node(22, List.Empty()));
  print(sum(values));
}"#
}
