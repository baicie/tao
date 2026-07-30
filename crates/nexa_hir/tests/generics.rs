//! Bounded-generic semantic regression tests.

use nexa_diagnostics::LabelStyle;
use nexa_hir::{lower, type_check, Analysis};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

const TEST_FILE: FileId = FileId::new(7);

#[test]
fn type_check_infers_generic_function_arguments_locally() -> Result<(), Box<dyn std::error::Error>>
{
    let analysis = analyze(
        r#"function identity<T>(value: T): T {
  return value;
}

function main(): Unit {
  const number: Int = identity(42);
  const text: String = identity("nexa");
  print(number);
  print(text);
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
fn type_check_instantiates_generic_records_for_fields_and_calls(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Box<T> = { value: T; };

function unbox<T>(box: Box<T>): T {
  return box.value;
}

function main(): Unit {
  const number: Box<Int> = { value: 42 };
  const text: Box<String> = { value: "nexa" };
  print(unbox(number));
  print(unbox(text));
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
fn type_check_treats_option_and_result_as_ordinary_generic_unions(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Option<T> = | Some(value: T) | None();
type Result<T, E> = | Ok(value: T) | Err(error: E);

function present<T>(value: T): Option<T> {
  return Option.Some(value);
}

function main(): Unit {
  const some: Option<Int> = present(42);
  const none: Option<Int> = Option.None();
  const failed: Result<Int, String> = Result.Err("bad");
  const first = match (some) {
    case Option.Some(value) => value;
    case Option.None() => 0;
  };
  const second = match (none) {
    case Option.Some(value) => value;
    case Option.None() => 1;
  };
  const third = match (failed) {
    case Result.Ok(value) => value;
    case Result.Err(error) => 40;
  };
  print(first + second + third);
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
fn type_check_accepts_regular_recursive_generic_unions() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type List<T> = | Empty() | Node(head: T, tail: List<T>);

function firstOr<T>(values: List<T>, fallback: T): T {
  return match (values) {
    case List.Empty() => fallback;
    case List.Node(head, tail) => head;
  };
}

function main(): Unit {
  const values: List<Int> = List.Node(42, List.Empty());
  print(firstOr(values, 0));
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
fn type_check_rejects_duplicate_type_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function same<T, T>(value: T): T { return value; }")?;

    assert_eq!(diagnostic_count(&analysis, "E2002"), 1);

    Ok(())
}

#[test]
fn type_check_rejects_bare_generic_types_and_wrong_type_argument_counts(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Box<T> = { value: T; };
function bare(value: Box): Unit {}
function many(value: Box<Int, String>): Unit {}"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E2003"), 2);

    Ok(())
}

#[test]
fn type_check_rejects_unconstrained_function_and_data_parameters(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Phantom<T> = { value: Int; };
function create<T>(): T { return create(); }"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3008"), 2);

    Ok(())
}

#[test]
fn type_check_rejects_conflicting_local_inference() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function same<T>(left: T, right: T): T { return left; }
function main(): Unit { const value = same(1, true); }"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3009"), 1);

    Ok(())
}

#[test]
fn unresolved_function_inference_labels_every_type_parameter_declaration(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function choose<T, E>(left: T, right: E): Unit {}
function main(): Unit { choose(); }"#;
    let analysis = analyze(source)?;

    assert_eq!(
        labels(&analysis, "E3009"),
        [vec![
            (LabelStyle::Primary, exact_span(source, "choose()")?),
            (LabelStyle::Secondary, span_within(source, "<T, E>", 1, 1)?,),
            (LabelStyle::Secondary, span_within(source, "<T, E>", 4, 1)?,),
        ]]
    );

    Ok(())
}

#[test]
fn conflicting_function_inference_labels_parameter_and_first_constraint(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function same<T>(left: T, right: T): T { return left; }
function main(): Unit { const value = same(1, true); }"#;
    let analysis = analyze(source)?;

    assert_eq!(
        labels(&analysis, "E3009"),
        [vec![
            (LabelStyle::Primary, exact_span(source, "true")?),
            (LabelStyle::Secondary, span_within(source, "<T>", 1, 1)?,),
            (LabelStyle::Secondary, exact_span(source, "1")?),
        ]]
    );

    Ok(())
}

#[test]
fn type_check_rejects_a_payloadless_generic_constructor_without_context(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Option<T> = | Some(value: T) | None();
function main(): Unit { const value = Option.None(); }"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3009"), 1);

    Ok(())
}

#[test]
fn unresolved_constructor_inference_labels_every_type_parameter_declaration(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Pair<T, E> = | Both(left: T, right: E);
function main(): Unit { const value = Pair.Both(); }"#;
    let analysis = analyze(source)?;

    assert_eq!(
        labels(&analysis, "E3009"),
        [vec![
            (LabelStyle::Primary, exact_span(source, "Pair.Both()")?),
            (LabelStyle::Secondary, span_within(source, "<T, E>", 1, 1)?,),
            (LabelStyle::Secondary, span_within(source, "<T, E>", 4, 1)?,),
        ]]
    );

    Ok(())
}

#[test]
fn conflicting_constructor_inference_labels_parameter_and_first_constraint(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Pair<T> = | Both(left: T, right: T);
function main(): Unit { const value = Pair.Both(1, true); }"#;
    let analysis = analyze(source)?;

    assert_eq!(
        labels(&analysis, "E3009"),
        [vec![
            (LabelStyle::Primary, exact_span(source, "true")?),
            (LabelStyle::Secondary, span_within(source, "<T>", 1, 1)?,),
            (LabelStyle::Secondary, exact_span(source, "1")?),
        ]]
    );

    Ok(())
}

#[test]
fn type_check_keeps_generic_nominal_instantiations_distinct(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Box<T> = { value: T; };
function main(): Unit {
  const number: Box<Int> = { value: 42 };
  const text: Box<String> = number;
}"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 1);

    Ok(())
}

#[test]
fn type_check_rejects_expanding_recursive_generic_types_and_calls(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Grow<T> = | End() | Next(value: Grow<T[]>);
function grow<T>(value: T): Unit { grow([value]); }"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3010"), 2);

    Ok(())
}

#[test]
fn type_check_rejects_a_generic_main_entry() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main<T>(value: T): Unit {}")?;

    assert_eq!(diagnostic_count(&analysis, "E3003"), 1);

    Ok(())
}

#[test]
fn type_check_uses_argument_inference_before_result_context(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function identity<T>(value: T): T { return value; }
function main(): Unit { const text: String = identity(1); }"#,
    )?;

    assert_eq!(
        (
            diagnostic_count(&analysis, "E3001"),
            diagnostic_count(&analysis, "E3009")
        ),
        (1, 0)
    );

    Ok(())
}

#[test]
fn type_check_infers_function_arguments_independently_of_parameter_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Box<T> = { value: T; };
type Option<T> = | Some(value: T) | None();

function fromBox<T>(box: Box<T>, fallback: T): T { return fallback; }
function fromArray<T>(values: T[], fallback: T): T { return fallback; }
function fromOption<T>(value: Option<T>, fallback: T): T { return fallback; }

function main(): Unit {
  print(fromBox({ value: 1 }, 40));
  print(fromArray([], 1));
  print(fromOption(Option.None(), 1));
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
fn type_check_infers_constructor_arguments_independently_of_payload_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Box<T> = { value: T; };
type Option<T> = | Some(value: T) | None();
type Choice<T> =
  | Boxed(box: Box<T>, fallback: T)
  | Listed(values: T[], fallback: T)
  | Optional(value: Option<T>, fallback: T);

function main(): Unit {
  const boxed = Choice.Boxed({ value: 1 }, 40);
  const listed = Choice.Listed([], 1);
  const optional = Choice.Optional(Option.None(), 1);
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
fn type_check_rejects_equality_for_unbounded_type_parameters(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function same<T>(left: T, right: T): Bool { return left === right; }")?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 1);

    Ok(())
}

#[test]
fn type_parameter_shadowing_blocks_module_union_constructors(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type T = | Value(value: Int);
function preserve<T>(value: T): T {
  const hidden = T.Value(1);
  return value;
}"#,
    )?;

    assert!(has_diagnostic_message(
        &analysis,
        "type parameter `T` has no variant constructors"
    ));

    Ok(())
}

#[test]
fn type_parameter_shadowing_blocks_module_union_match_patterns(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type T = | Value(value: Int);
function inspect<T>(value: T): Int {
  return match (value) {
    case T.Value(inner) => inner;
    default => 0;
  };
}"#,
    )?;

    assert!(has_diagnostic_message(
        &analysis,
        "type parameter `T` cannot qualify a match pattern"
    ));

    Ok(())
}

#[test]
fn type_check_accepts_regular_mutually_recursive_generic_unions(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Left<T> = | Stop() | Next(value: Right<T>);
type Right<T> = | Stop() | Next(value: Left<T>);
function main(): Unit {
  const value: Left<Int> = Left.Next(Right.Stop());
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
fn type_check_rejects_swapped_arguments_in_a_recursive_generic_scc(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Left<T, E> = | Next(value: Right<E, T>);
type Right<T, E> = | Next(value: Left<T, E>);"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3010"), 1);

    Ok(())
}

#[test]
fn type_parameters_shadow_module_types_inside_their_declaration(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type T = { value: Int; };
function identity<T>(value: T): T { return value; }
function main(): Unit { print(identity(42)); }"#,
    )?;

    assert!(
        analysis.is_ok(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics()
    );

    Ok(())
}

#[test]
fn type_check_enforces_the_distinct_generic_instance_budget(
) -> Result<(), Box<dyn std::error::Error>> {
    let accepted = analyze(&generic_instance_program(256))?;
    let rejected = analyze(&generic_instance_program(257))?;

    assert_eq!(
        (
            diagnostic_count(&accepted, "E3010"),
            diagnostic_count(&rejected, "E3010")
        ),
        (0, 1)
    );

    Ok(())
}

#[test]
fn type_check_rejects_invalid_types_exposed_by_generic_instantiation(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Cell<T> = { value: T; };
type Items<T> = { values: T[]; };
function useCell(value: Cell<Unit>): Unit {}
function useItems(value: Items<Unit>): Unit {}"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 2);

    Ok(())
}

fn analyze(source: &str) -> Result<Analysis, Box<dyn std::error::Error>> {
    let parse = parse_source(TEST_FILE, source);
    if !parse.diagnostics().is_empty() {
        return Err(
            std::io::Error::other(format!("parse diagnostics: {:?}", parse.diagnostics())).into(),
        );
    }
    let program = lower(TEST_FILE, &parse.syntax())?;
    Ok(type_check(&program))
}

fn diagnostic_count(analysis: &Analysis, code: &str) -> usize {
    analysis
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == code)
        .count()
}

fn has_diagnostic_message(analysis: &Analysis, message: &str) -> bool {
    analysis
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message() == message)
}

fn labels(analysis: &Analysis, code: &str) -> Vec<Vec<(LabelStyle, SourceSpan)>> {
    analysis
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == code)
        .map(|diagnostic| {
            diagnostic
                .labels()
                .iter()
                .map(|label| (label.style(), label.span()))
                .collect()
        })
        .collect()
}

fn exact_span(source: &str, text: &str) -> Result<SourceSpan, std::io::Error> {
    span_within(source, text, 0, text.len())
}

fn span_within(
    source: &str,
    occurrence: &str,
    offset: usize,
    width: usize,
) -> Result<SourceSpan, std::io::Error> {
    let occurrence_start = source
        .find(occurrence)
        .ok_or_else(|| std::io::Error::other(format!("expected `{occurrence}` in test source")))?;
    let start = occurrence_start + offset;
    Ok(SourceSpan::new(
        TEST_FILE,
        TextRange::new(start, start + width),
    ))
}

fn generic_instance_program(instance_count: usize) -> String {
    let mut source = "type Box<T> = { value: T; };\n".to_owned();
    for index in 0..instance_count {
        source.push_str(&format!("type Item{index} = {{ value: Int; }};\n"));
    }
    source.push_str("function main(): Unit {\n");
    for index in 0..instance_count {
        source.push_str(&format!(
            "  const item{index}: Box<Item{index}> = {{ value: {{ value: {index} }} }};\n"
        ));
    }
    source.push_str("}\n");
    source
}
