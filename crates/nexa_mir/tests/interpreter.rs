//! MIR lowering and interpretation regression tests.

use nexa_hir::{lower as lower_hir, type_check};
use nexa_mir::{lower as lower_mir, run, MirProgram};
use nexa_parser::parse_source;
use nexa_span::FileId;

#[test]
fn interpreter_executes_function_calls_branches_and_print() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile(
        r#"function add(left: Int, right: Int): Int {
  return left + right;
}

function main(): Unit {
  const answer = add(40, 2);
  if (answer === 42) {
    print(answer);
  } else {
    print(0);
  }
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42"]);

    Ok(())
}

#[test]
fn interpreter_uses_lexically_scoped_local_slots() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function main(): Unit {
  const value = 1;
  if (true) {
    const value = 2;
    print(value);
  }
  print(value);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["2", "1"]);

    Ok(())
}

#[test]
fn interpreter_reports_division_by_zero_with_a_source_span(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit { print(1 / 0); }";
    let program = compile(source)?;
    let error = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a division error"))?;

    assert_eq!(error.message(), "division by zero");
    assert_eq!(error.span().range().start(), 30);

    Ok(())
}

fn compile(source: &str) -> Result<MirProgram, Box<dyn std::error::Error>> {
    let file = FileId::new(3);
    let parse = parse_source(file, source);
    assert!(
        parse.is_ok(),
        "parser diagnostics: {:?}",
        parse.diagnostics()
    );
    let hir = lower_hir(file, &parse.syntax())?;
    let analysis = type_check(&hir);
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!(
            "semantic diagnostics: {:?}",
            analysis.diagnostics()
        ))
    })?;

    Ok(lower_mir(typed)?)
}
