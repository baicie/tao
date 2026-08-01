//! Semantic regression tests for nominal tagged unions and exhaustive matching.

use nexa_hir::{lower, type_check, Analysis};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

const TEST_FILE: FileId = FileId::new(8);

#[test]
fn type_check_accepts_recursive_list_construction_and_exhaustive_match(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type List =
  | Empty()
  | Node(head: Int, tail: List);

function sum(values: List): Int {
  return match (values) {
    case List.Empty() => 0;
    case List.Node(head, tail) => head + sum(tail);
  };
}

function main(): Unit {
  const values = List.Node(20, List.Node(22, List.Empty()));
  print(sum(values));
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
fn type_check_accepts_forward_record_union_and_array_guarded_recursion(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Node = { next: Chain; };
type Forest = | Empty() | Trees(children: Forest[]);
type Chain = | End() | More(node: Node);

function main(): Unit {
  const node: Node = { next: Chain.End() };
  const chain = Chain.More(node);
  const forest = Forest.Trees([Forest.Empty()]);
  const chainValue = match (chain) {
    case Chain.End() => 0;
    case Chain.More(inner) => match (inner.next) {
      case Chain.End() => 1;
      case Chain.More(next) => 2;
    };
  };
  const forestValue = match (forest) {
    case Forest.Empty() => 0;
    case Forest.Trees(children) => children.length;
  };
  print(chainValue + forestValue);
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
fn type_check_accepts_mutually_recursive_unions() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Left = | Stop() | Right(value: Right);
type Right = | Stop() | Left(value: Left);
function main(): Unit {
  const value = Left.Right(Right.Left(Left.Stop()));
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
fn type_check_accepts_a_long_guarded_union_cycle() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type First = | Stop() | Next(value: Second);
type Second = | Next(value: Third);
type Third = | Next(value: First);
function main(): Unit { const value = First.Stop(); }"#,
    )?;

    assert!(
        analysis.is_ok(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics()
    );

    Ok(())
}

#[test]
fn type_check_accepts_unit_payloads() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Completion = | Done(value: Unit) | Pending();

function main(): Unit {
  const completion = Completion.Done(print(1));
  const code = match (completion) {
    case Completion.Done(value) => 1;
    case Completion.Pending() => 0;
  };
  print(code);
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
fn type_check_contextualizes_constructor_payloads_and_match_record_arms(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Item = { value: Int; };
type Choice = | None() | Some(item: Item);

function unwrap(choice: Choice): Item {
  return match (choice) {
    case Choice.None() => { value: 0 };
    case Choice.Some(item) => item;
  };
}

function main(): Unit {
  const choice = Choice.Some({ value: 42 });
  print(unwrap(choice).value);
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
fn type_check_accepts_a_reachable_default_for_missing_variants(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Flag = | Off() | On();
function read(flag: Flag): Int {
  return match (flag) {
    case Flag.On() => 1;
    default => 0;
  };
}
function main(): Unit { print(read(Flag.Off())); }"#,
    )?;

    assert!(
        analysis.is_ok(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics()
    );

    Ok(())
}

#[test]
fn type_check_allows_pattern_binders_to_shadow_outer_locals(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Choice = | None() | Some(value: Int);
function main(): Unit {
  const value = 7;
  const choice = Choice.Some(42);
  const result = match (choice) {
    case Choice.None() => value;
    case Choice.Some(value) => value;
  };
  print(result + value);
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
fn type_check_rejects_constructor_arity_mismatches_at_the_variant(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Pair = | Both(left: Int, right: Int);
function main(): Unit { const pair = Pair.Both(1); }"#;
    let analysis = analyze(source)?;

    assert_eq!(count_code(&analysis, "E2003"), 1);
    assert_eq!(
        primary_spans(&analysis, "E2003"),
        [prefix_span(source, "Both(1)", "Both".len())?]
    );

    Ok(())
}

#[test]
fn type_check_rejects_constructor_payload_type_mismatches_at_the_argument(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Value = | Integer(value: Int);
function main(): Unit { const value = Value.Integer(false); }"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [exact_span(source, "false")?]
    );

    Ok(())
}

#[test]
fn type_check_rejects_pattern_arity_mismatches_at_the_variant(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Pair = | Both(left: Int, right: Int);
function read(pair: Pair): Int {
  return match (pair) { case Pair.Both(left) => left; };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(count_code(&analysis, "E2003"), 1);
    assert_eq!(count_code(&analysis, "E3006"), 0);
    assert_eq!(
        primary_spans(&analysis, "E2003"),
        [prefix_span(source, "Both(left)", "Both".len())?]
    );

    Ok(())
}

#[test]
fn type_check_rejects_unknown_constructor_qualifiers() -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit { const value = Missing.Some(1); }";
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E2001"),
        [exact_span(source, "Missing")?]
    );

    Ok(())
}

#[test]
fn type_check_rejects_record_names_as_constructor_qualifiers(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Point = { value: Int; };
function main(): Unit { const value = Point.Some(1); }"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [prefix_span(source, "Point.Some", "Point".len())?]
    );

    Ok(())
}

#[test]
fn type_check_rejects_unknown_constructor_variants() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Choice = | None() | Some(value: Int);
function main(): Unit { const value = Choice.Missing(1); }"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E2005"),
        [exact_span(source, "Missing")?]
    );

    Ok(())
}

#[test]
fn type_check_rejects_unknown_pattern_variants() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Choice = | None() | Some(value: Int);
function read(choice: Choice): Int {
  return match (choice) {
    case Choice.Missing(value) => value;
    default => 0;
  };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E2005"),
        [exact_span(source, "Missing")?]
    );
    assert_eq!(count_code(&analysis, "E3006"), 0);

    Ok(())
}

#[test]
fn type_check_rejects_foreign_union_cases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Left = | Value(value: Int);
type Right = | Value(value: Int);
function read(value: Left): Int {
  return match (value) {
    case Right.Value(inner) => inner;
    default => 0;
  };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [prefix_span(source, "Right.Value(inner)", "Right".len())?]
    );
    assert_eq!(count_code(&analysis, "E3006"), 0);

    Ok(())
}

#[test]
fn type_check_rejects_duplicate_type_names_across_records_and_unions(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Shared = { value: Int; };
type Shared = | Empty();
function main(): Unit {}"#;
    let analysis = analyze(source)?;
    let shared = occurrence_spans(source, "Shared");

    assert_eq!(
        label_spans(&analysis, "E2002"),
        [vec![shared[1], shared[0]]]
    );

    Ok(())
}

#[test]
fn type_check_rejects_duplicate_variants_with_the_first_declaration_labeled(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Choice = | Same() | Same(value: Int); function main(): Unit {}";
    let analysis = analyze(source)?;
    let same = occurrence_spans(source, "Same");

    assert_eq!(label_spans(&analysis, "E2002"), [vec![same[1], same[0]]]);

    Ok(())
}

#[test]
fn type_check_rejects_duplicate_payload_names_with_the_first_payload_labeled(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Pair = | Both(value: Int, value: Bool); function main(): Unit {}";
    let analysis = analyze(source)?;
    let value = occurrence_spans(source, "value");

    assert_eq!(label_spans(&analysis, "E2002"), [vec![value[1], value[0]]]);

    Ok(())
}

#[test]
fn type_check_rejects_duplicate_pattern_binders_with_the_first_binder_labeled(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Pair = | Both(left: Int, right: Int);
function read(pair: Pair): Int {
  return match (pair) { case Pair.Both(value, value) => value; };
}"#;
    let analysis = analyze(source)?;
    let value = occurrence_spans(source, "value");

    assert_eq!(label_spans(&analysis, "E2002"), [vec![value[1], value[0]]]);

    Ok(())
}

#[test]
fn type_check_rejects_duplicate_cases_without_reachability_noise(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Flag = | Off() | On();
function read(flag: Flag): Int {
  return match (flag) {
    case Flag.Off() => 0;
    case Flag.Off() => 1;
    default => 2;
  };
}"#;
    let analysis = analyze(source)?;
    let off = occurrence_spans(source, "Off");

    assert_eq!(label_spans(&analysis, "E2002"), [vec![off[2], off[1]]]);
    assert_eq!(count_code(&analysis, "E3007"), 0);

    Ok(())
}

#[test]
fn type_check_reports_missing_variants_in_declaration_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Choice = | First() | Second(value: Int) | Third();
function read(choice: Choice): Int {
  return match (choice) {
    case Choice.First() => 1;
  };
}"#;
    let analysis = analyze(source)?;
    let match_text = "match (choice) {\n    case Choice.First() => 1;\n  }";

    assert_eq!(
        label_spans(&analysis, "E3006"),
        [vec![
            exact_span(source, match_text)?,
            exact_span(source, "Second(value: Int)")?,
            exact_span(source, "Third()")?,
        ]]
    );

    Ok(())
}

#[test]
fn type_check_reports_every_variant_for_an_empty_match() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Flag = | Off() | On();
function main(): Unit {
  const flag = Flag.Off();
  const result = match (flag) {};
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        label_spans(&analysis, "E3006"),
        [vec![
            exact_span(source, "match (flag) {}")?,
            exact_span(source, "Off()")?,
            exact_span(source, "On()")?,
        ]]
    );

    Ok(())
}

#[test]
fn type_check_rejects_default_after_complete_coverage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Flag = | Off() | On();
function read(flag: Flag): Int {
  return match (flag) {
    case Flag.Off() => 0;
    case Flag.On() => 1;
    default => 2;
  };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        label_spans(&analysis, "E3007"),
        [vec![
            exact_span(source, "default => 2;")?,
            exact_span(source, "case Flag.On() => 1;")?,
        ]]
    );

    Ok(())
}

#[test]
fn type_check_rejects_arms_after_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Flag = | Off() | On();
function read(flag: Flag): Int {
  return match (flag) {
    default => 0;
    case Flag.On() => 1;
  };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        label_spans(&analysis, "E3007"),
        [vec![
            exact_span(source, "case Flag.On() => 1;")?,
            exact_span(source, "default")?,
        ]]
    );

    Ok(())
}

#[test]
fn type_check_still_checks_unreachable_arm_expressions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Flag = | Off() | On();
function read(flag: Flag): Int {
  return match (flag) {
    default => 0;
    case Flag.On() => missing;
  };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        (
            primary_spans(&analysis, "E3007"),
            primary_spans(&analysis, "E2001")
        ),
        (
            vec![exact_span(source, "case Flag.On() => missing;")?],
            vec![exact_span(source, "missing")?],
        )
    );

    Ok(())
}

#[test]
fn type_check_keeps_pattern_binders_inside_their_arm_scope(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Choice = | None() | Some(value: Int);
function read(choice: Choice): Int {
  const result = match (choice) {
    case Choice.None() => 0;
    case Choice.Some(inner) => inner;
  };
  return inner + result;
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E2001"),
        [prefix_span(source, "inner + result", "inner".len())?]
    );

    Ok(())
}

#[test]
fn type_check_requires_a_common_match_arm_type_without_context(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Choice = | Number() | Truth();
function main(): Unit {
  const value = Choice.Number();
  const result = match (value) {
    case Choice.Number() => 1;
    case Choice.Truth() => false;
  };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [exact_span(source, "false")?]
    );

    Ok(())
}

#[test]
fn type_check_propagates_an_inferred_match_record_type_to_later_arms(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Box = { value: Int; };
type Choice = | Existing() | Fresh();
function choose(choice: Choice): Box {
  const existing: Box = { value: 42 };
  return match (choice) {
    case Choice.Existing() => existing;
    case Choice.Fresh() => { value: 0 };
  };
}
function main(): Unit { print(choose(Choice.Fresh()).value); }"#,
    )?;

    assert!(
        analysis.is_ok(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics()
    );

    Ok(())
}

#[test]
fn type_check_prefers_a_same_named_local_for_value_member_access(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Box = { value: Int; };
type Choice = | None();
function main(): Unit {
  const Choice: Box = { value: 42 };
  const none = Choice.None();
  print(Choice.value);
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
fn type_check_resolves_called_member_qualifiers_only_as_types(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit {
  const value = 1;
  const invalid = value.Some();
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E2001"),
        [prefix_span(source, "value.Some", "value".len())?]
    );
    assert_eq!(
        (
            count_code(&analysis, "E2005"),
            count_code(&analysis, "E3001")
        ),
        (0, 0)
    );

    Ok(())
}

#[test]
fn type_check_rejects_non_union_match_scrutinees() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit {
  const result = match (42) { default => 0; };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [exact_span(source, "42")?]
    );

    Ok(())
}

#[test]
fn type_check_rejects_union_equality_printing_and_value_members(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Flag = | Off() | On();
function main(): Unit {
  const leftValue = Flag.Off();
  const rightValue = Flag.Off();
  const equal = leftValue === rightValue;
  print(leftValue);
  const member = leftValue.value;
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [
            prefix_span(source, "rightValue;", "rightValue".len())?,
            prefix_span(source, "leftValue);", "leftValue".len())?,
            exact_span(source, "value")?,
        ]
    );

    Ok(())
}

#[test]
fn type_check_rejects_bare_qualified_constructors() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Flag = | Off() | On();
function main(): Unit { const constructor = Flag.On; }"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [prefix_span(source, "On;", "On".len())?]
    );

    Ok(())
}

#[test]
fn type_check_keeps_union_types_nominal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Left = | Value(value: Int);
type Right = | Value(value: Int);
function consume(value: Left): Unit {}
function main(): Unit { consume(Right.Value(42)); }"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [exact_span(source, "Right.Value(42)")?]
    );

    Ok(())
}

#[test]
fn type_check_preserves_record_cycle_rejection_without_a_union_boundary(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Node = { children: Node[]; };
function main(): Unit {}"#;
    let analysis = analyze(source)?;
    let node = occurrence_spans(source, "Node");

    assert_eq!(
        label_spans(&analysis, "E3005"),
        [vec![exact_span(source, "Node[]")?, node[0]]]
    );

    Ok(())
}

fn analyze(source: &str) -> Result<Analysis, Box<dyn std::error::Error>> {
    let parse = parse_source(TEST_FILE, source);
    if !parse.is_ok() {
        return Err(std::io::Error::other(format!(
            "unexpected parser diagnostics: {:?}",
            parse.diagnostics()
        ))
        .into());
    }
    let program = lower(TEST_FILE, &parse.syntax())?;
    Ok(type_check(&program))
}

fn count_code(analysis: &Analysis, code: &str) -> usize {
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

fn label_spans(analysis: &Analysis, code: &str) -> Vec<Vec<SourceSpan>> {
    analysis
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == code)
        .map(|diagnostic| {
            diagnostic
                .labels()
                .iter()
                .map(|label| label.span())
                .collect()
        })
        .collect()
}

fn exact_span(source: &str, text: &str) -> Result<SourceSpan, std::io::Error> {
    let start = source
        .find(text)
        .ok_or_else(|| std::io::Error::other(format!("expected `{text}` in source")))?;
    Ok(SourceSpan::new(
        TEST_FILE,
        TextRange::new(start, start + text.len()),
    ))
}

fn prefix_span(source: &str, occurrence: &str, width: usize) -> Result<SourceSpan, std::io::Error> {
    let start = source
        .find(occurrence)
        .ok_or_else(|| std::io::Error::other(format!("expected `{occurrence}` in test source")))?;
    Ok(SourceSpan::new(
        TEST_FILE,
        TextRange::new(start, start + width),
    ))
}

fn occurrence_spans(source: &str, text: &str) -> Vec<SourceSpan> {
    source
        .match_indices(text)
        .map(|(start, value)| {
            SourceSpan::new(TEST_FILE, TextRange::new(start, start + value.len()))
        })
        .collect()
}
