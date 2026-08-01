//! End-to-end compiler-session tests for multi-file modules.

use nexa_compiler::{check_session, run_session, CompilerSession, RuntimeError};
use nexa_source::MemorySourceProvider;
use nexa_span::FileId;

#[test]
fn compiler_session_checks_and_executes_cross_file_nominal_values(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = MemorySourceProvider::default();
    let entry = provider.insert(
        "app/main.nexa",
        br#"import { Box, Choice, make, read } from "./model.nexa";
function main(): Unit { print(read(make(42))); }"#,
    );
    provider.insert(
        "app/model.nexa",
        br#"export type Box = { value: Int; };
export type Choice = | None() | Some(value: Box);
export function make(value: Int): Choice {
  return Choice.Some({ value: value });
}
export function read(choice: Choice): Int {
  return match (choice) {
    case Choice.None() => 0;
    case Choice.Some(box) => box.value;
  };
}"#,
    );
    let session = CompilerSession::build(provider, entry.as_path())?;

    let checked = check_session(&session);
    assert!(
        checked.is_ok(),
        "unexpected diagnostics: {:?}",
        checked.diagnostics()
    );
    assert_eq!(checked.typed().map(|typed| typed.modules().len()), Some(2));

    let run = run_session(&session);
    assert_eq!(run.output(), ["42"]);
    assert!(run.is_ok(), "runtime error: {:?}", run.runtime_error());

    Ok(())
}

#[test]
fn compiler_session_preserves_one_nominal_identity_through_a_runtime_diamond(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = MemorySourceProvider::default();
    let entry = provider.insert(
        "diamond/main.nexa",
        br#"import { makeLeft } from "./left.nexa";
import { readRight } from "./right.nexa";
function main(): Unit { print(readRight(makeLeft())); }"#,
    );
    provider.insert(
        "diamond/left.nexa",
        br#"import { Shared, make } from "./shared.nexa";
export function makeLeft(): Shared { return make(42); }"#,
    );
    provider.insert(
        "diamond/right.nexa",
        br#"import { Shared } from "./shared.nexa";
export function readRight(value: Shared): Int { return value.value; }"#,
    );
    provider.insert(
        "diamond/shared.nexa",
        br#"export type Shared = { value: Int; };
export function make(value: Int): Shared { return { value: value }; }"#,
    );
    let session = CompilerSession::build(provider, entry.as_path())?;

    let checked = check_session(&session);
    assert!(
        checked.is_ok(),
        "unexpected diagnostics: {:?}",
        checked.diagnostics()
    );
    assert_eq!(session.modules().len(), 4);

    let run = run_session(&session);
    assert_eq!(run.output(), ["42"]);
    assert!(run.is_ok(), "runtime error: {:?}", run.runtime_error());

    Ok(())
}

#[test]
fn compiler_session_preserves_dependency_runtime_spans_and_prior_output(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = MemorySourceProvider::default();
    let entry = provider.insert(
        "runtime/main.nexa",
        br#"import { fail } from "./library.nexa";
function main(): Unit { print(7); fail(); }"#,
    );
    provider.insert(
        "runtime/library.nexa",
        br#"export function fail(): Unit {
  print(8);
  const invalid = 1 / 0;
}"#,
    );
    let session = CompilerSession::build(provider, entry.as_path())?;

    let run = run_session(&session);
    let error = run
        .runtime_error()
        .ok_or_else(|| std::io::Error::other("expected dependency runtime error"))?;

    assert_eq!(run.output(), ["7", "8"]);
    assert_eq!(
        (error.message(), error.span().file()),
        ("division by zero", FileId::new(1))
    );

    Ok(())
}

#[test]
fn compiler_session_never_selects_an_imported_dependency_main_as_entry(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = MemorySourceProvider::default();
    let entry = provider.insert(
        "entry/main.nexa",
        br#"import { main } from "./dependency.nexa";
function start(): Bool { return main(1); }"#,
    );
    provider.insert(
        "entry/dependency.nexa",
        b"export function main(value: Int): Bool { return true; }",
    );
    let session = CompilerSession::build(provider, entry.as_path())?;

    let checked = check_session(&session);
    assert!(
        checked.is_ok(),
        "unexpected diagnostics: {:?}",
        checked.diagnostics()
    );
    let run = run_session(&session);

    assert_eq!(run.output(), [] as [&str; 0]);
    assert_eq!(
        run.runtime_error().map(RuntimeError::message),
        Some("program has no `main` entry point")
    );

    Ok(())
}

#[test]
fn compiler_session_reports_independent_semantic_errors_in_a_cycle(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = MemorySourceProvider::default();
    let entry = provider.insert(
        "cycle/a.nexa",
        br#"import { right } from "./b.nexa";
export function left(): Int { return missing; }
function main(): Unit {}"#,
    );
    provider.insert(
        "cycle/b.nexa",
        br#"import { left } from "./a.nexa";
export function right(): Int { return left(); }"#,
    );
    let session = CompilerSession::build(provider, entry.as_path())?;

    let checked = check_session(&session);

    assert_eq!(diagnostic_count(&checked, "E4002"), 1);
    assert_eq!(diagnostic_count(&checked, "E2001"), 1);
    assert!(!checked.is_ok());

    Ok(())
}

#[test]
fn compiler_session_checks_complete_siblings_when_an_import_cannot_load(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = MemorySourceProvider::default();
    let entry = provider.insert(
        "partial/main.nexa",
        br#"import { Missing } from "./missing.nexa";
import { value } from "./library.nexa";
function main(): Unit {}"#,
    );
    provider.insert(
        "partial/library.nexa",
        b"export function value(): Int { return unknown; }",
    );
    let session = CompilerSession::build(provider, entry.as_path())?;

    let checked = check_session(&session);
    let codes = checked
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code().as_str())
        .collect::<Vec<_>>();

    assert_eq!(codes, ["E4001", "E2001"]);
    assert!(!checked.is_ok());

    Ok(())
}

#[test]
fn compiler_session_checks_complete_siblings_when_an_import_does_not_parse(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = MemorySourceProvider::default();
    let entry = provider.insert(
        "partial/main.nexa",
        br#"import { Bad } from "./bad.nexa";
import { value } from "./library.nexa";
function main(): Unit {}"#,
    );
    provider.insert("partial/bad.nexa", b"@");
    provider.insert(
        "partial/library.nexa",
        b"export function value(): Int { return unknown; }",
    );
    let session = CompilerSession::build(provider, entry.as_path())?;

    let checked = check_session(&session);
    let codes = checked
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code().as_str())
        .collect::<Vec<_>>();

    assert_eq!(codes, ["E1001", "E2001"]);
    assert!(!checked.is_ok());

    Ok(())
}

fn diagnostic_count(checked: &nexa_compiler::CheckResult, code: &str) -> usize {
    checked
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == code)
        .count()
}
