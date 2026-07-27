//! Semantic regression tests for the Nexa Language Core.

use nexa_hir::{lower, type_check, Analysis, LoweringError};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

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
fn type_check_accepts_mutable_bindings_loops_and_logical_expressions(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function sum(limit: Int): Int {
  let total: Int = 0;
  let index = 0;
  while (index < limit && true) {
    index = index + 1;
    while (false) {
      continue;
    }
    if (index === 2 || false) {
      continue;
    }
    total = total + index;
    if (index >= limit) {
      break;
    }
  }
  return total;
}

function main(): Unit {
  print(sum(4));
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
fn type_check_allows_mutable_shadowing_in_nested_scopes() -> Result<(), Box<dyn std::error::Error>>
{
    let analysis = analyze(
        r#"function main(): Unit {
  const value = 1;
  if (true) {
    let value = 2;
    value = 3;
  }
  print(value);
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
fn type_check_reports_undefined_assignment_targets() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { missing = 1; }")?;

    assert!(has_diagnostic(&analysis, "E2001"));

    Ok(())
}

#[test]
fn type_check_keeps_a_binding_in_scope_after_an_initializer_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis =
        analyze("function main(): Unit { let value = missing; value = 1; print(value); }")?;

    assert_eq!(diagnostic_count(&analysis, "E2001"), 1);

    Ok(())
}

#[test]
fn type_check_reports_a_duplicate_after_an_initializer_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { let value = missing; let value = 1; }")?;

    assert_eq!(
        (
            diagnostic_count(&analysis, "E2001"),
            diagnostic_count(&analysis, "E2002")
        ),
        (1, 1)
    );

    Ok(())
}

#[test]
fn parser_rejects_uninitialized_mutable_bindings() {
    let parse = parse_source(FileId::new(3), "function main(): Unit { let count: Int; }");

    assert!(parse
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code().as_str() == "E1001"));
}

#[test]
fn type_check_reports_duplicate_bindings() -> Result<(), Box<dyn std::error::Error>> {
    let analysis =
        analyze("function main(): Unit { const answer = 1; let answer = 2; print(answer); }")?;

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
fn type_check_rejects_assignments_to_constants_and_parameters(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function update(value: Int): Int {
  value = value + 1;
  return value;
}

function main(): Unit {
  const answer = 42;
  answer = update(answer);
}"#;
    let analysis = analyze(source)?;
    let expected_spans = [
        prefix_span(source, "value = value", "value".len())?,
        prefix_span(source, "answer = update", "answer".len())?,
    ];

    assert_eq!(diagnostic_count(&analysis, "E2004"), 2);
    assert_eq!(primary_spans(&analysis, "E2004"), expected_spans);

    Ok(())
}

#[test]
fn type_check_reports_mismatched_types() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { const answer: Bool = 42; }")?;

    assert!(has_diagnostic(&analysis, "E3001"));

    Ok(())
}

#[test]
fn type_check_reports_assignment_type_mismatches() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { let answer: Int = 42; answer = false; }")?;

    assert!(has_diagnostic(&analysis, "E3001"));

    Ok(())
}

#[test]
fn type_check_requires_boolean_logical_operands() -> Result<(), Box<dyn std::error::Error>> {
    let analysis =
        analyze("function main(): Unit { const left = 1 && true; const right = false || 0; }")?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 2);

    Ok(())
}

#[test]
fn type_check_checks_known_logical_operand_after_name_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { const result = missing && 1; }")?;

    assert_eq!(diagnostic_count(&analysis, "E2001"), 1);
    assert_eq!(diagnostic_count(&analysis, "E3001"), 1);

    Ok(())
}

#[test]
fn type_check_reports_non_boolean_conditions() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { if (42) { return; } }")?;

    assert!(has_diagnostic(&analysis, "E3002"));

    Ok(())
}

#[test]
fn type_check_reports_non_boolean_while_conditions() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { while (42) { break; } }")?;

    assert!(has_diagnostic(&analysis, "E3002"));

    Ok(())
}

#[test]
fn type_check_rejects_loop_control_outside_loops() -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit { break; continue; }";
    let analysis = analyze(source)?;
    let expected_spans = [
        prefix_span(source, "break", "break;".len())?,
        prefix_span(source, "continue", "continue;".len())?,
    ];

    assert_eq!(diagnostic_count(&analysis, "E3004"), 2);
    assert_eq!(primary_spans(&analysis, "E3004"), expected_spans);

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

#[test]
fn type_check_does_not_treat_while_as_a_guaranteed_return() -> Result<(), Box<dyn std::error::Error>>
{
    let analysis =
        analyze("function value(): Int { while (true) { return 1; } } function main(): Unit {}")?;

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

fn diagnostic_count(analysis: &Analysis, code: &str) -> usize {
    analysis
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == code)
        .count()
}

fn primary_spans(analysis: &Analysis, code: &str) -> Vec<SourceSpan> {
    analysis
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == code)
        .filter_map(|diagnostic| diagnostic.labels().first())
        .map(|label| label.span())
        .collect()
}

fn prefix_span(source: &str, occurrence: &str, width: usize) -> Result<SourceSpan, std::io::Error> {
    let start = source
        .find(occurrence)
        .ok_or_else(|| std::io::Error::other(format!("expected `{occurrence}` in test source")))?;

    Ok(SourceSpan::new(
        FileId::new(3),
        TextRange::new(start, start + width),
    ))
}
