//! Defensive validation for generic facts at the typed-HIR to MIR boundary.

use nexa_hir::{lower as lower_hir, type_check, Expression, Program, Statement, TypedProgram};
use nexa_mir::lower as lower_mir;
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan};

const TEST_FILE: FileId = FileId::new(13);

#[test]
fn lowering_accepts_current_function_parameters_and_keeps_one_definition_body(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = lower_source(
        r#"function consume<T>(value: T): Unit {}
function forward<U>(value: U): Unit { consume(value); }
function main(): Unit {
  forward(42);
  forward("nexa");
}"#,
    )?;
    let typed = type_checked(&program)?;

    let mir = lower_mir(&typed)?;
    let forward_count = mir
        .functions()
        .iter()
        .filter(|function| function.name() == "forward")
        .count();

    assert_eq!(forward_count, 1);

    Ok(())
}

#[test]
fn lowering_rejects_call_facts_that_do_not_instantiate_the_actual_argument(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut program = lower_source(
        r#"function identity<T>(value: T): T { return value; }
function main(): Unit {
  const number = identity(42);
  const text = identity("nexa");
}"#,
    )?;
    let shared_span = binding_initializer(&program, 1, 0)?.span();
    *call_span_mut(binding_initializer_mut(&mut program, 1, 1)?)? = shared_span;
    let typed = type_checked(&program)?;

    let failure = lower_mir(&typed)
        .err()
        .ok_or_else(|| std::io::Error::other("expected inconsistent call facts to be rejected"))?;

    assert_eq!(
        (failure.message(), failure.span()),
        (
            "function call argument 0 does not agree with its typed HIR instantiation facts",
            shared_span,
        )
    );

    Ok(())
}

#[test]
fn lowering_rejects_a_foreign_function_parameter_in_call_facts(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut program = lower_source(
        r#"function consume<T>(value: T): Unit {}
function first<U>(value: U): Unit { consume(value); }
function second<V>(value: V): Unit { consume(value); }
function main(): Unit { first(1); second(2); }"#,
    )?;
    let shared_span = expression_statement(&program, 1, 0)?.span();
    *call_span_mut(expression_statement_mut(&mut program, 2, 0)?)? = shared_span;
    let typed = type_checked(&program)?;

    let failure = lower_mir(&typed)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a foreign type parameter error"))?;

    assert_eq!(
        (failure.message(), failure.span()),
        (
            "function call contains a foreign typed HIR type parameter",
            shared_span,
        )
    );

    Ok(())
}

#[test]
fn lowering_rejects_constructor_facts_with_different_expression_type_arguments(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut program = lower_source(
        r#"type Option<T> = | Some(value: T) | None();
function main(): Unit {
  const number: Option<Int> = Option.Some(42);
  const text: Option<String> = Option.Some("nexa");
  const overwrite = text;
}"#,
    )?;
    let construction_span = binding_initializer(&program, 0, 0)?.span();
    *name_span_mut(binding_initializer_mut(&mut program, 0, 2)?)? = construction_span;
    let typed = type_checked(&program)?;

    let failure = lower_mir(&typed)
        .err()
        .ok_or_else(|| std::io::Error::other("expected inconsistent constructor facts"))?;

    assert_eq!(
        (failure.message(), failure.span()),
        (
            "variant construction type does not agree with its typed HIR instantiation facts",
            construction_span,
        )
    );

    Ok(())
}

#[test]
fn lowering_rejects_match_facts_with_different_scrutinee_type_arguments(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut program = lower_source(
        r#"type Option<T> = | Some(value: T) | None();
function main(): Unit {
  const number: Option<Int> = Option.Some(42);
  const text: Option<String> = Option.Some("nexa");
  const first = match (number) {
    case Option.Some(value) => value;
    case Option.None() => 0;
  };
  const second = match (text) {
    case Option.Some(value) => value;
    case Option.None() => "";
  };
}"#,
    )?;
    let first_match = binding_initializer(&program, 0, 2)?;
    let shared_span = first_match.span();
    let scrutinee_span = match_scrutinee(first_match)?.span();
    *match_span_mut(binding_initializer_mut(&mut program, 0, 3)?)? = shared_span;
    let typed = type_checked(&program)?;

    let failure = lower_mir(&typed)
        .err()
        .ok_or_else(|| std::io::Error::other("expected inconsistent match facts"))?;

    assert_eq!(
        (failure.message(), failure.span()),
        (
            "match scrutinee type does not agree with its typed HIR dispatch facts",
            scrutinee_span,
        )
    );

    Ok(())
}

fn lower_source(source: &str) -> Result<Program, Box<dyn std::error::Error>> {
    let parse = parse_source(TEST_FILE, source);
    if !parse.is_ok() {
        return Err(std::io::Error::other(format!(
            "unexpected parser diagnostics: {:?}",
            parse.diagnostics()
        ))
        .into());
    }

    lower_hir(TEST_FILE, &parse.syntax()).map_err(Into::into)
}

fn type_checked(program: &Program) -> Result<TypedProgram, Box<dyn std::error::Error>> {
    let analysis = type_check(program);
    analysis.typed().cloned().ok_or_else(|| {
        std::io::Error::other(format!(
            "unexpected semantic diagnostics: {:?}",
            analysis.diagnostics()
        ))
        .into()
    })
}

fn binding_initializer(
    program: &Program,
    function: usize,
    statement: usize,
) -> Result<&Expression, std::io::Error> {
    let statement = program
        .functions
        .get(function)
        .and_then(|function| function.body.statements.get(statement))
        .ok_or_else(|| std::io::Error::other("expected binding statement"))?;
    match statement {
        Statement::Const(declaration) => Ok(&declaration.initializer),
        Statement::Let(declaration) => Ok(&declaration.initializer),
        _ => Err(std::io::Error::other("expected binding initializer")),
    }
}

fn binding_initializer_mut(
    program: &mut Program,
    function: usize,
    statement: usize,
) -> Result<&mut Expression, std::io::Error> {
    let statement = program
        .functions
        .get_mut(function)
        .and_then(|function| function.body.statements.get_mut(statement))
        .ok_or_else(|| std::io::Error::other("expected binding statement"))?;
    match statement {
        Statement::Const(declaration) => Ok(&mut declaration.initializer),
        Statement::Let(declaration) => Ok(&mut declaration.initializer),
        _ => Err(std::io::Error::other("expected binding initializer")),
    }
}

fn expression_statement(
    program: &Program,
    function: usize,
    statement: usize,
) -> Result<&Expression, std::io::Error> {
    let statement = program
        .functions
        .get(function)
        .and_then(|function| function.body.statements.get(statement))
        .ok_or_else(|| std::io::Error::other("expected expression statement"))?;
    match statement {
        Statement::Expression(statement) => Ok(&statement.expression),
        _ => Err(std::io::Error::other("expected expression statement")),
    }
}

fn expression_statement_mut(
    program: &mut Program,
    function: usize,
    statement: usize,
) -> Result<&mut Expression, std::io::Error> {
    let statement = program
        .functions
        .get_mut(function)
        .and_then(|function| function.body.statements.get_mut(statement))
        .ok_or_else(|| std::io::Error::other("expected expression statement"))?;
    match statement {
        Statement::Expression(statement) => Ok(&mut statement.expression),
        _ => Err(std::io::Error::other("expected expression statement")),
    }
}

fn call_span_mut(expression: &mut Expression) -> Result<&mut SourceSpan, std::io::Error> {
    match expression {
        Expression::Call { span, .. } => Ok(span),
        _ => Err(std::io::Error::other("expected call expression")),
    }
}

fn name_span_mut(expression: &mut Expression) -> Result<&mut SourceSpan, std::io::Error> {
    match expression {
        Expression::Name(name) => Ok(&mut name.span),
        _ => Err(std::io::Error::other("expected name expression")),
    }
}

fn match_span_mut(expression: &mut Expression) -> Result<&mut SourceSpan, std::io::Error> {
    match expression {
        Expression::Match { span, .. } => Ok(span),
        _ => Err(std::io::Error::other("expected match expression")),
    }
}

fn match_scrutinee(expression: &Expression) -> Result<&Expression, std::io::Error> {
    match expression {
        Expression::Match { scrutinee, .. } => Ok(scrutinee),
        _ => Err(std::io::Error::other("expected match expression")),
    }
}
