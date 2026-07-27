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

    assert_eq!(error.error().message(), "division by zero");
    assert_eq!(error.error().span().range().start(), 30);

    Ok(())
}

#[test]
fn interpreter_preserves_output_before_a_runtime_failure() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile("function main(): Unit { print(1); print(1 / 0); }")?;
    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a division error"))?;

    assert_eq!(failure.output(), ["1"]);

    Ok(())
}

#[test]
fn interpreter_allows_64_active_calls() -> Result<(), Box<dyn std::error::Error>> {
    let source = call_chain_source(63);
    let program = compile(&source)?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1"]);

    Ok(())
}

#[test]
fn interpreter_reports_a_runtime_error_at_the_65th_active_call(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = call_chain_source(64);
    let expected_span_start = source
        .find("f64();")
        .ok_or_else(|| std::io::Error::other("expected final call in generated source"))?;
    let program = compile(&source)?;
    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a call depth error"))?;

    assert_eq!(
        failure.error().message(),
        "maximum call depth of 64 exceeded"
    );
    assert_eq!(failure.error().span().range().start(), expected_span_start);

    Ok(())
}

fn call_chain_source(function_count: usize) -> String {
    let mut source = String::new();

    for index in 1..=function_count {
        if index == function_count {
            source.push_str(&format!("function f{index}(): Unit {{ print(1); }}\n"));
        } else {
            source.push_str(&format!(
                "function f{index}(): Unit {{ f{}(); }}\n",
                index + 1
            ));
        }
    }

    source.push_str("function main(): Unit { f1(); }");
    source
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
