//! Semantic regression tests for Language Core v0.1.

use nexa_hir::{lower, type_check, Analysis, LoweringError};
use nexa_parser::parse_source;
use nexa_span::FileId;

#[test]
fn type_check_accepts_typed_functions_inferred_constants_and_branches(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function twice(value: Int): Int {
  return value * 2;
}

function main(): Unit {
  const inferred = twice(20 + 1);
  if (!(inferred < 42) === true) {
    print(inferred);
  } else {
    print(0);
  }
}"#,
    )?;

    assert!(
        analysis.is_ok(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics()
    );

    Ok(())
}

#[test]
fn type_check_reports_undefined_names() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { print(missing); }")?;

    assert!(has_diagnostic(&analysis, "E2001"));

    Ok(())
}

#[test]
fn type_check_reports_duplicate_bindings() -> Result<(), Box<dyn std::error::Error>> {
    let analysis =
        analyze("function main(): Unit { const answer = 1; const answer = 2; print(answer); }")?;

    assert!(has_diagnostic(&analysis, "E2002"));

    Ok(())
}

#[test]
fn type_check_reports_incorrect_call_arity() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { print(); }")?;

    assert!(has_diagnostic(&analysis, "E2003"));

    Ok(())
}

#[test]
fn type_check_reports_mismatched_types() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { const answer: Bool = 42; }")?;

    assert!(has_diagnostic(&analysis, "E3001"));

    Ok(())
}

#[test]
fn type_check_reports_non_boolean_conditions() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { if (42) { return; } }")?;

    assert!(has_diagnostic(&analysis, "E3002"));

    Ok(())
}

#[test]
fn type_check_reports_missing_non_unit_returns() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        "function value(flag: Bool): Int { if (flag) { return 1; } } function main(): Unit {}",
    )?;

    assert!(has_diagnostic(&analysis, "E3003"));

    Ok(())
}

fn analyze(source: &str) -> Result<Analysis, LoweringError> {
    let file = FileId::new(3);
    let parse = parse_source(file, source);
    assert!(
        parse.is_ok(),
        "parser diagnostics: {:?}",
        parse.diagnostics()
    );
    let program = lower(file, &parse.syntax())?;

    Ok(type_check(&program))
}

fn has_diagnostic(analysis: &Analysis, code: &str) -> bool {
    analysis
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code().as_str() == code)
}
