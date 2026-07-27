//! MIR lowering and interpretation regression tests.

use nexa_hir::{lower as lower_hir, type_check, BinaryOperator};
use nexa_mir::{lower as lower_mir, run, MirExpression, MirProgram, MirStatement, MirTerminator};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

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
fn interpreter_uses_lexically_scoped_mutable_local_slots() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile(
        r#"function main(): Unit {
  let value = 1;
  if (true) {
    let value = 2;
    value = 3;
    print(value);
  }
  print(value);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["3", "1"]);

    Ok(())
}

#[test]
fn interpreter_executes_mutable_loops_break_and_continue() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile(
        r#"function main(): Unit {
  let sum = 0;
  let index = 0;
  while (index < 6) {
    index = index + 1;
    if (index === 3) {
      continue;
    }
    if (index === 6) {
      break;
    }
    sum = sum + index;
  }
  print(sum);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["12"]);

    Ok(())
}

#[test]
fn interpreter_targets_the_nearest_loop_for_break_and_continue(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function main(): Unit {
  let outer = 0;
  let score = 0;
  while (outer < 3) {
    outer = outer + 1;
    let inner = 0;
    while (inner < 3) {
      inner = inner + 1;
      if (inner === 2) {
        continue;
      }
      if (outer === 2) {
        break;
      }
      score = score + 1;
    }
  }
  print(score);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["4"]);

    Ok(())
}

#[test]
fn interpreter_skips_a_false_while_body() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile("function main(): Unit { while (false) { print(1); } print(2); }")?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["2"]);

    Ok(())
}

#[test]
fn interpreter_short_circuits_boolean_operators() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function sideEffect(): Bool {
  print(99);
  return true;
}

function main(): Unit {
  if (false && sideEffect()) {
    print(0);
  }
  if (true || sideEffect()) {
    print(1);
  }
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1"]);

    Ok(())
}

#[test]
fn mir_lowering_expands_logical_operators_into_cfg_branches(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        "function main(): Unit { const both = true && false; const either = false || true; }",
    )?;
    let function = program
        .functions()
        .first()
        .ok_or_else(|| std::io::Error::other("expected main function"))?;
    let branch_count = function
        .blocks()
        .iter()
        .filter(|block| matches!(block.terminator(), MirTerminator::Branch { .. }))
        .count();
    let contains_logical_binary = function.blocks().iter().any(|block| {
        block.statements().iter().any(|statement| {
            let expression = match statement {
                MirStatement::Store { value, .. } => value,
                MirStatement::Expression { expression, .. } => expression,
            };
            expression_contains_logical_binary(expression)
        }) || match block.terminator() {
            MirTerminator::Branch { condition, .. } => {
                expression_contains_logical_binary(condition)
            }
            MirTerminator::Return { value, .. } => value
                .as_ref()
                .is_some_and(expression_contains_logical_binary),
            MirTerminator::Goto { .. } => false,
        }
    });

    assert_eq!(branch_count, 2, "MIR blocks: {:?}", function.blocks());
    assert!(
        !contains_logical_binary,
        "MIR blocks: {:?}",
        function.blocks()
    );

    Ok(())
}

#[test]
fn interpreter_preserves_argument_order_before_logical_control_flow(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function first(): Bool { print(1); return true; }
function second(): Bool { print(2); return true; }
function third(): Bool { print(3); return true; }
function consume(left: Bool, right: Bool): Bool { return left && right; }
function main(): Unit {
  if (consume(first(), second() && third())) {}
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1", "2", "3"]);

    Ok(())
}

#[test]
fn interpreter_reports_a_left_operand_error_before_right_logical_effects(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function fail(): Bool { const invalid = 1 / 0; return true; }
function probe(): Bool { print(2); return true; }
function main(): Unit {
  if (fail() === (probe() && true)) {}
}"#,
    )?;
    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a division error"))?;

    assert_eq!(failure.error().message(), "division by zero");
    assert!(
        failure.output().is_empty(),
        "output: {:?}",
        failure.output()
    );

    Ok(())
}

#[test]
fn mir_lowering_builds_explicit_control_flow_terminators() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile(
        "function main(): Unit { let value = 0; while (value < 1) { value = value + 1; } }",
    )?;
    let function = program
        .functions()
        .first()
        .ok_or_else(|| std::io::Error::other("expected main function"))?;

    let has_control_flow = function
        .blocks()
        .iter()
        .any(|block| matches!(block.terminator(), MirTerminator::Branch { .. }))
        && function
            .blocks()
            .iter()
            .any(|block| matches!(block.terminator(), MirTerminator::Goto { .. }));
    let block_count = function.blocks().len();
    let all_targets_are_local = function.entry_block().index() < block_count
        && function
            .blocks()
            .iter()
            .all(|block| match block.terminator() {
                MirTerminator::Goto { target, .. } => target.index() < block_count,
                MirTerminator::Branch {
                    then_target,
                    else_target,
                    ..
                } => then_target.index() < block_count && else_target.index() < block_count,
                MirTerminator::Return { .. } => true,
            });
    let has_explicit_return = function
        .blocks()
        .iter()
        .any(|block| matches!(block.terminator(), MirTerminator::Return { .. }));

    assert!(has_control_flow, "MIR blocks: {:?}", function.blocks());
    assert!(all_targets_are_local, "MIR blocks: {:?}", function.blocks());
    assert!(has_explicit_return, "MIR blocks: {:?}", function.blocks());

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
fn interpreter_allows_100000_steps_and_rejects_step_100001(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit { print(1); while (true) {} }";
    let body_start = source
        .rfind(" {}")
        .ok_or_else(|| std::io::Error::other("expected loop body"))?;
    let expected_span = SourceSpan::new(
        FileId::new(3),
        TextRange::new(body_start, body_start + " {}".len()),
    );
    let program = compile(source)?;
    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected an execution step error"))?;

    assert_eq!(
        failure.error().message(),
        "execution step limit of 100000 exceeded"
    );
    assert_eq!(failure.error().span(), expected_span);
    assert_eq!(failure.output(), ["1"]);

    Ok(())
}

#[test]
fn interpreter_shares_the_step_budget_across_nested_calls() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile(
        r#"function work(): Unit {
  let index = 0;
  while (index < 25000) {
    index = index + 1;
  }
}

function main(): Unit {
  work();
  print(1);
  work();
}"#,
    )?;
    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a shared step-budget error"))?;

    assert_eq!(
        failure.error().message(),
        "execution step limit of 100000 exceeded"
    );
    assert_eq!(failure.output(), ["1"]);

    Ok(())
}

#[test]
fn interpreter_allows_64_active_calls() -> Result<(), Box<dyn std::error::Error>> {
    let source = call_chain_source(63);
    let program = compile(&source)?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["7", "1"]);

    Ok(())
}

#[test]
fn interpreter_reports_a_runtime_error_at_the_65th_active_call(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = call_chain_source(64);
    let expected_span_start = source
        .find("f64();")
        .ok_or_else(|| std::io::Error::other("expected final call in generated source"))?;
    let expected_span = SourceSpan::new(
        FileId::new(3),
        TextRange::new(expected_span_start, expected_span_start + "f64()".len()),
    );
    let program = compile(&source)?;
    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a call depth error"))?;

    assert_eq!(
        failure.error().message(),
        "maximum call depth of 64 exceeded"
    );
    assert_eq!(failure.error().span(), expected_span);
    assert_eq!(failure.output(), ["7"]);

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

    source.push_str("function main(): Unit { print(7); f1(); }");
    source
}

fn expression_contains_logical_binary(expression: &MirExpression) -> bool {
    match expression {
        MirExpression::Unary { expression, .. } => expression_contains_logical_binary(expression),
        MirExpression::Binary {
            operator,
            left,
            right,
            ..
        } => {
            matches!(
                operator,
                BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
            ) || expression_contains_logical_binary(left)
                || expression_contains_logical_binary(right)
        }
        MirExpression::Call { arguments, .. } => {
            arguments.iter().any(expression_contains_logical_binary)
        }
        MirExpression::Integer { .. }
        | MirExpression::Boolean { .. }
        | MirExpression::Local { .. } => false,
    }
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
