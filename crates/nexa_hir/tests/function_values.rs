//! Semantic tests for first-class function values and function types.

use nexa_hir::{lower, type_check, Analysis, LoweringError};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

#[test]
fn type_check_accepts_named_function_values_and_indirect_calls(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function increment(value: Int): Int {
  return value + 1;
}

function apply(transform: (input: Int) => Int, value: Int): Int {
  return transform(value);
}

function main(): Unit {
  const operation: (value: Int) => Int = increment;
  print(apply(operation, 41));
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
fn type_check_infers_generic_arguments_through_function_types(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function apply<T>(value: T, transform: (input: T) => T): T {
  return transform(value);
}

function increment(value: Int): Int {
  return value + 1;
}

function main(): Unit {
  print(apply(41, increment));
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
fn type_check_accepts_arrays_of_parenthesized_function_types(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function increment(value: Int): Int { return value + 1; }
function main(): Unit {
  const handlers: ((value: Int) => Int)[] = [increment];
  print(handlers[0](41));
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
fn type_check_rejects_generic_functions_used_as_values_with_a_declaration_label(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function identity<T>(value: T): T {
  return value;
}

function main(): Unit {
  const invalid: (value: Int) => Int = identity;
}"#,
    )?;
    let diagnostics = analysis
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == "E3012")
        .collect::<Vec<_>>();

    assert_eq!(
        diagnostics
            .first()
            .map(|diagnostic| diagnostic.labels().len()),
        Some(2)
    );

    Ok(())
}

#[test]
fn type_check_rejects_invalid_indirect_calls() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function increment(value: Int): Int { return value + 1; }
function main(): Unit {
  const operation = increment;
  operation();
  operation(false);
  const value = 1;
  value(2);
}"#,
    )?;

    assert_eq!(
        (
            diagnostic_count(&analysis, "E2003"),
            diagnostic_count(&analysis, "E3001")
        ),
        (1, 2)
    );

    Ok(())
}

#[test]
fn function_value_mismatch_labels_the_initializer_and_declared_type(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function increment(value: Int): Int { return value + 1; }
function main(): Unit {
  const operation: (value: Bool) => Bool = increment;
}"#;
    let analysis = analyze(source)?;
    let diagnostic = analysis
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code().as_str() == "E3001")
        .ok_or_else(|| std::io::Error::other("expected a function type mismatch"))?;

    assert_eq!(
        diagnostic
            .labels()
            .iter()
            .map(|label| label.span())
            .collect::<Vec<_>>(),
        [
            exact_span(source, "increment", 2)?,
            exact_span(source, "(value: Bool) => Bool", 1)?,
        ]
    );

    Ok(())
}

#[test]
fn type_check_rejects_function_equality_and_printing() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function increment(value: Int): Int { return value + 1; }
function main(): Unit {
  const operation = increment;
  const same = operation === operation;
  print(operation);
}"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 2);

    Ok(())
}

fn analyze(source: &str) -> Result<Analysis, LoweringError> {
    let file = FileId::new(8);
    let parse = parse_source(file, source);
    assert!(
        parse.is_ok(),
        "parser diagnostics: {:?}",
        parse.diagnostics()
    );
    let program = lower(file, &parse.syntax())?;
    Ok(type_check(&program))
}

fn diagnostic_count(analysis: &Analysis, code: &str) -> usize {
    analysis
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == code)
        .count()
}

fn exact_span(
    source: &str,
    occurrence: &str,
    ordinal: usize,
) -> Result<SourceSpan, std::io::Error> {
    let start = source
        .match_indices(occurrence)
        .nth(ordinal - 1)
        .map(|(start, _)| start)
        .ok_or_else(|| {
            std::io::Error::other(format!(
                "expected occurrence {ordinal} of `{occurrence}` in test source"
            ))
        })?;
    Ok(SourceSpan::new(
        FileId::new(8),
        TextRange::new(start, start + occurrence.len()),
    ))
}
