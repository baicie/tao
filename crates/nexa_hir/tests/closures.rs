//! Semantic tests for typed arrow functions and lexical captures.

use nexa_hir::{lower, type_check, Analysis, ClosureId, FunctionId, LocalId, LoweringError};
use nexa_parser::parse_source;
use nexa_span::FileId;

#[test]
fn type_check_records_arrow_parameters_and_const_captures_in_first_use_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function factory(base: Int): (value: Int) => Int {
  const offset = 2;
  return (value: Int): Int => value + base + offset;
}

function main(): Unit {}"#,
    )?;
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!("diagnostics: {:?}", analysis.diagnostics()))
    })?;
    let facts = typed
        .closure_facts(ClosureId::new(FunctionId::new(0), 0))
        .ok_or_else(|| std::io::Error::other("expected closure facts"))?;

    assert_eq!(
        (facts.parameter_ids(), facts.captures()),
        (
            &[LocalId::new(2)][..],
            &[LocalId::new(0), LocalId::new(1)][..]
        )
    );

    Ok(())
}

#[test]
fn type_check_forwards_distant_captures_through_nested_arrows(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function main(): Unit {
  const value = 42;
  const factory: () => () => Int = (): () => Int => (): Int => value;
  print(factory()());
}"#,
    )?;
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!("diagnostics: {:?}", analysis.diagnostics()))
    })?;
    let owner = FunctionId::new(0);

    assert_eq!(
        (
            typed
                .closure_facts(ClosureId::new(owner, 0))
                .map(|facts| facts.captures()),
            typed
                .closure_facts(ClosureId::new(owner, 1))
                .map(|facts| facts.captures())
        ),
        (Some(&[LocalId::new(0)][..]), Some(&[LocalId::new(0)][..]))
    );

    Ok(())
}

#[test]
fn type_check_rejects_mutable_captures_once_per_arrow_and_binding(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function main(): Unit {
  let value = 1;
  const read: () => Int = (): Int => value + value;
}"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3011"), 1);

    Ok(())
}

#[test]
fn type_check_rejects_forwarded_mutable_captures_for_each_required_environment(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function main(): Unit {
  let value = 1;
  const factory: () => () => Int = (): () => Int => (): Int => value;
}"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3011"), 2);

    Ok(())
}

#[test]
fn type_check_rejects_local_arrow_self_reference() -> Result<(), Box<dyn std::error::Error>> {
    let analysis =
        analyze("function main(): Unit { const recurse: () => Int = (): Int => recurse(); }")?;

    assert_eq!(diagnostic_count(&analysis, "E2001"), 1);

    Ok(())
}

fn analyze(source: &str) -> Result<Analysis, LoweringError> {
    let file = FileId::new(9);
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
