//! Static semantic tests for v0.8 immutable-data intrinsics and conversions.

use nexa_hir::{lower, type_check, Analysis, Builtin, LoweringError, NameResolution, Type};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

#[test]
fn type_check_accepts_array_intrinsics_string_length_and_conversions(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function main(): Unit {
  const values = [20].append(22).concat([]);
  print(values.length);
  print("Nexa🙂".length);
  print(toString(parseInt("42")));
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
fn typed_program_records_stable_practical_core_builtin_facts(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit {
  const values = [20].append(22).concat([]);
  print("Nexa".length);
  print(toString(parseInt("42")));
}"#;
    let analysis = analyze(source)?;
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!("diagnostics: {:?}", analysis.diagnostics()))
    })?;
    let append = prefix_span(source, "append", "append".len())?;
    let concat = prefix_span(source, "concat", "concat".len())?;
    let length = prefix_span(source, "length", "length".len())?;
    let to_string = prefix_span(source, "toString", "toString".len())?;
    let parse_int = prefix_span(source, "parseInt", "parseInt".len())?;
    let string_length = prefix_span(source, "\"Nexa\".length", "\"Nexa\".length".len())?;

    assert_eq!(
        (
            typed.name_resolution(append),
            typed.name_resolution(concat),
            typed.name_resolution(length),
            typed.name_resolution(to_string),
            typed.name_resolution(parse_int),
            typed.expression_type(string_length)
        ),
        (
            Some(NameResolution::Builtin(Builtin::ArrayAppend)),
            Some(NameResolution::Builtin(Builtin::ArrayConcat)),
            Some(NameResolution::Builtin(Builtin::StringLength)),
            Some(NameResolution::Builtin(Builtin::ToString)),
            Some(NameResolution::Builtin(Builtin::ParseInt)),
            Some(&Type::Int)
        )
    );

    Ok(())
}

#[test]
fn type_check_rejects_extracting_array_intrinsics_at_member_spans(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit {
  const values = [1];
  const append = values.append;
  const concat = values.concat;
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E2005"),
        [
            prefix_span(source, "append;", "append".len())?,
            prefix_span(source, "concat;", "concat".len())?
        ]
    );

    Ok(())
}

#[test]
fn type_check_rejects_array_intrinsic_arity_and_type_mismatches(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit {
  const values = [1];
  values.append();
  values.concat([1], [2]);
  values.append(false);
  values.concat([false]);
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        (
            diagnostic_count(&analysis, "E2003"),
            diagnostic_count(&analysis, "E3001")
        ),
        (2, 2)
    );

    Ok(())
}

#[test]
fn type_check_rejects_conversion_arity_and_argument_types() -> Result<(), Box<dyn std::error::Error>>
{
    let analysis = analyze(
        r#"function main(): Unit {
  toString();
  parseInt("1", "2");
  toString("1");
  parseInt(1);
}"#,
    )?;

    assert_eq!(
        (
            diagnostic_count(&analysis, "E2003"),
            diagnostic_count(&analysis, "E3001")
        ),
        (2, 2)
    );

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
        FileId::new(8),
        TextRange::new(start, start + width),
    ))
}
