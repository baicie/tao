//! MIR lowering and interpretation regression tests.

use nexa_hir::{
    lower as lower_hir, type_check, BinaryOperator, FieldId, PayloadId, RecordId, Type, UnionId,
    VariantId,
};
use nexa_mir::{
    lower as lower_mir, run, run_with_args, MirExpression, MirProgram, MirStatement, MirTerminator,
};
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
fn interpreter_erases_generic_function_instances_to_one_mir_body(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function identity<T>(value: T): T {
  return value;
}

function main(): Unit {
  print(identity(42));
  print(identity("nexa"));
}"#,
    )?;
    let execution = run(&program)?;
    let identity_count = program
        .functions()
        .iter()
        .filter(|function| function.name() == "identity")
        .count();

    assert_eq!(execution.output(), ["42", "nexa"]);
    assert_eq!(identity_count, 1);

    Ok(())
}

#[test]
fn interpreter_executes_generic_record_instances_with_one_definition_layout(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
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
    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42", "nexa"]);
    assert_eq!(program.records().len(), 1);

    Ok(())
}

#[test]
fn interpreter_executes_ordinary_generic_option_and_result_matches(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Option<T> = | Some(value: T) | None();
type Result<T, E> = | Ok(value: T) | Err(error: E);

function read(value: Result<Option<Int>, String>): Int {
  return match (value) {
    case Result.Ok(optional) => match (optional) {
      case Option.Some(number) => number;
      case Option.None() => 0;
    };
    case Result.Err(message) => 0;
  };
}

function main(): Unit {
  const some: Result<Option<Int>, String> = Result.Ok(Option.Some(42));
  const none: Result<Option<Int>, String> = Result.Ok(Option.None());
  const error: Result<Option<Int>, String> = Result.Err("failed");
  print(read(some));
  print(read(none));
  print(read(error));
}"#,
    )?;
    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42", "0", "0"]);
    assert_eq!(program.unions().len(), 2);

    Ok(())
}

#[test]
fn interpreter_executes_regular_recursive_generic_lists() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile(
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
    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42"]);
    assert_eq!(program.unions().len(), 1);

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
fn interpreter_executes_strings_and_immutable_arrays() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function sum(values: Int[]): Int {
  let index = 0;
  let total = 0;
  while (index < values.length) {
    total = total + values[index];
    index = index + 1;
  }
  return total;
}

function main(): Unit {
  const words: String[] = ["Nexa", "native"];
  print(words[0] + " " + words[1]);
  print(sum([20, 22]));
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["Nexa native", "42"]);

    Ok(())
}

#[test]
fn interpreter_passes_cli_arguments_to_main() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile("function main(args: String[]): Unit { print(args[1]); }")?;
    let arguments = ["first".to_owned(), "second".to_owned()];

    let execution = run_with_args(&program, &arguments)?;

    assert_eq!(execution.output(), ["second"]);

    Ok(())
}

#[test]
fn interpreter_evaluates_array_elements_left_to_right() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function first(): Int { print(1); return 20; }
function second(): Int { print(2); return 22; }
function main(): Unit {
  const values = [first(), second()];
  print(values[0] + values[1]);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1", "2", "42"]);

    Ok(())
}

#[test]
fn interpreter_reports_array_bounds_with_the_index_expression_span(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit {
  print("before");
  const values: Int[] = [1];
  print(values[1]);
}"#;
    let index_start = source
        .find("values[1]")
        .ok_or_else(|| std::io::Error::other("expected index expression"))?;
    let expected_span = SourceSpan::new(
        FileId::new(3),
        TextRange::new(index_start, index_start + "values[1]".len()),
    );
    let program = compile(source)?;

    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected an array bounds error"))?;

    assert_eq!(
        failure.error().message(),
        "array index 1 out of bounds for length 1"
    );
    assert_eq!(failure.error().span(), expected_span);
    assert_eq!(failure.output(), ["before"]);

    Ok(())
}

#[test]
fn interpreter_reports_a_negative_array_index_with_the_index_expression_span(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit {
  const values = [10];
  print(values[-1]);
}"#;
    let index_start = source
        .find("values[-1]")
        .ok_or_else(|| std::io::Error::other("expected negative index expression"))?;
    let expected_span = SourceSpan::new(
        FileId::new(3),
        TextRange::new(index_start, index_start + "values[-1]".len()),
    );
    let program = compile(source)?;

    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a negative array bounds error"))?;

    assert_eq!(
        failure.error().message(),
        "array index -1 out of bounds for length 1"
    );
    assert_eq!(failure.error().span(), expected_span);

    Ok(())
}

#[test]
fn interpreter_reads_the_first_and_last_valid_array_indices(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        "function main(): Unit { const values = [10, 20, 30]; print(values[0]); print(values[2]); }",
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["10", "30"]);

    Ok(())
}

#[test]
fn interpreter_evaluates_an_index_base_before_its_subscript(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function values(): Int[] { print(1); return [10, 20]; }
function subscript(): Int { print(2); return 1; }
function main(): Unit { print(values()[subscript()]); }"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1", "2", "20"]);

    Ok(())
}

#[test]
fn interpreter_evaluates_string_concatenation_operands_left_to_right(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function left(): String { print(1); return "Ne"; }
function right(): String { print(2); return "xa"; }
function main(): Unit { print(left() + right()); }"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1", "2", "Nexa"]);

    Ok(())
}

#[test]
fn interpreter_evaluates_string_equality_operands_left_to_right(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function left(): String { print(1); return "Nexa"; }
function right(): String { print(2); return "Nexa"; }
function main(): Unit { print(left() === right()); }"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1", "2", "true"]);

    Ok(())
}

#[test]
fn interpreter_compares_utf8_strings_without_unicode_normalization(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = format!(
        "function main(): Unit {{ print(\"{}\" === \"{}\"); }}",
        '\u{00e9}', "e\u{0301}"
    );
    let program = compile(&source)?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["false"]);

    Ok(())
}

#[test]
fn interpreter_decodes_all_supported_string_escapes() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function main(): Unit {
  print("slash:\\ quote:\" line\ncarriage\rtab\tend");
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(
        execution.output(),
        ["slash:\\ quote:\" line\ncarriage\rtab\tend"]
    );

    Ok(())
}

#[test]
fn interpreter_indexes_nested_immutable_arrays() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function main(): Unit {
  const matrix: Int[][] = [[10, 20], [30, 40]];
  print(matrix[1][0]);
  print(matrix.length);
  print(matrix[0].length);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["30", "2", "2"]);

    Ok(())
}

#[test]
fn interpreter_returns_immutable_arrays_from_functions() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"function pair(): Int[] { return [20, 22]; }
function main(): Unit {
  const values = pair();
  print(values[0] + values[1]);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42"]);

    Ok(())
}

#[test]
fn interpreter_evaluates_reordered_record_fields_once_in_literal_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Pair = { left: Int; right: Int; };

function left(): Int { print(1); return 20; }
function right(): Int { print(2); return 22; }

function main(): Unit {
  const pair: Pair = { right: right(), left: left() };
  print(pair.left);
  print(pair.right);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["2", "1", "20", "22"]);

    Ok(())
}

#[test]
fn interpreter_evaluates_a_record_field_base_exactly_once() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile(
        r#"type Box = { value: Int; };

function make(): Box {
  print(1);
  return { value: 42 };
}

function main(): Unit {
  print(make().value);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1", "42"]);

    Ok(())
}

#[test]
fn interpreter_passes_returns_nests_and_arrays_nominal_records(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Address = { city: String; };
type User = { name: String; address: Address; scores: Int[]; };

function makeUser(name: String): User {
  return { scores: [20, 22], address: { city: "London" }, name: name };
}

function city(user: User): String { return user.address.city; }

function main(): Unit {
  const users: User[] = [makeUser("Ada")];
  print(users[0].name);
  print(city(users[0]));
  print(users[0].scores[0] + users[0].scores[1]);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["Ada", "London", "42"]);

    Ok(())
}

#[test]
fn interpreter_preserves_output_before_a_record_initializer_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Result = { label: String; value: Int; };

function fail(): Int {
  print("before");
  const values = [1];
  return values[1];
}

function main(): Unit {
  const result: Result = { label: "Nexa", value: fail() };
}"#;
    let expected_start = source
        .find("values[1]")
        .ok_or_else(|| std::io::Error::other("expected failing field initializer"))?;
    let expected_span = SourceSpan::new(
        FileId::new(3),
        TextRange::new(expected_start, expected_start + "values[1]".len()),
    );
    let program = compile(source)?;

    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a record initializer failure"))?;

    assert_eq!(failure.error().span(), expected_span);
    assert_eq!(failure.output(), ["before"]);

    Ok(())
}

#[test]
fn interpreter_executes_recursive_unions_and_exhaustive_matches(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
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

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42"]);

    Ok(())
}

#[test]
fn interpreter_preserves_union_values_through_records_arrays_calls_and_returns(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Choice = | None() | Some(value: Int);
type Envelope = { values: Choice[]; };

function keep(value: Choice): Choice {
  return value;
}

function wrap(value: Choice): Envelope {
  return { values: [keep(value)] };
}

function read(envelope: Envelope): Int {
  const value = keep(envelope.values[0]);
  return match (value) {
    case Choice.None() => 0;
    case Choice.Some(inner) => inner;
  };
}

function main(): Unit {
  print(read(wrap(Choice.Some(42))));
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42"]);

    Ok(())
}

#[test]
fn interpreter_evaluates_variant_payloads_once_from_left_to_right(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Pair = | Values(left: Int, right: Int);

function first(): Int { print(1); return 20; }
function second(): Int { print(2); return 22; }

function main(): Unit {
  const pair = Pair.Values(first(), second());
  const total = match (pair) {
    case Pair.Values(left, right) => left + right;
  };
  print(total);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1", "2", "42"]);

    Ok(())
}

#[test]
fn interpreter_stops_variant_payload_evaluation_after_a_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Triple = | Values(first: Int, second: Int, third: Int);

function first(): Int { print(1); return 1; }
function fail(): Int {
  print(2);
  const values = [0];
  return values[1];
}
function third(): Int { print(3); return 3; }

function main(): Unit {
  const ignored = Triple.Values(first(), fail(), third());
}"#;
    let failure_start = source
        .find("values[1]")
        .ok_or_else(|| std::io::Error::other("expected failing constructor payload"))?;
    let expected_span = SourceSpan::new(
        FileId::new(3),
        TextRange::new(failure_start, failure_start + "values[1]".len()),
    );
    let program = compile(source)?;

    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a constructor payload failure"))?;

    assert_eq!(failure.error().span(), expected_span);
    assert_eq!(failure.output(), ["1", "2"]);

    Ok(())
}

#[test]
fn interpreter_evaluates_a_match_scrutinee_exactly_once() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile(
        r#"type Flag = | Off() | On();

function probe(): Flag {
  print(1);
  return Flag.On();
}

function main(): Unit {
  const result = match (probe()) {
    case Flag.Off() => 0;
    case Flag.On() => 42;
  };
  print(result);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["1", "42"]);

    Ok(())
}

#[test]
fn interpreter_evaluates_only_the_selected_match_arm() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Flag = | Off() | On();

function off(): Int { print(1); return 0; }
function on(): Int { print(2); return 42; }

function main(): Unit {
  const result = match (Flag.On()) {
    case Flag.Off() => off();
    case Flag.On() => on();
  };
  print(result);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["2", "42"]);

    Ok(())
}

#[test]
fn interpreter_selects_a_default_match_arm_for_an_uncovered_variant(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Choice = | Zero() | One() | Other();

function main(): Unit {
  const result = match (Choice.Other()) {
    case Choice.Zero() => 0;
    case Choice.One() => 1;
    default => 42;
  };
  print(result);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42"]);

    Ok(())
}

#[test]
fn interpreter_contextually_types_record_literals_in_nested_match_arms(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Box = { value: Int; };
type Inner = | Number(value: Int) | Missing();
type Outer = | Wrapped(value: Inner) | Empty();

function unpack(value: Outer): Box {
  return match (value) {
    case Outer.Wrapped(inner) => match (inner) {
      case Inner.Number(number) => { value: number };
      case Inner.Missing() => { value: 0 };
    };
    case Outer.Empty() => { value: 0 };
  };
}

function main(): Unit {
  print(unpack(Outer.Wrapped(Inner.Number(42))).value);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42"]);

    Ok(())
}

#[test]
fn mir_keeps_source_order_record_and_field_layout_ids() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type User = { name: String; age: Int; };
type Envelope = { user: User; };
function main(): Unit {}"#,
    )?;
    let user = program
        .records()
        .first()
        .ok_or_else(|| std::io::Error::other("expected User layout"))?;
    let envelope = program
        .records()
        .get(1)
        .ok_or_else(|| std::io::Error::other("expected Envelope layout"))?;

    assert_eq!(
        (
            user.id(),
            user.name(),
            user.fields()
                .iter()
                .map(|field| (field.id(), field.name(), field.ty().clone()))
                .collect::<Vec<_>>(),
            envelope.id(),
            envelope.fields().first().map(|field| field.ty())
        ),
        (
            RecordId::new(0),
            "User",
            vec![
                (FieldId::new(RecordId::new(0), 0), "name", Type::String),
                (FieldId::new(RecordId::new(0), 1), "age", Type::Int)
            ],
            RecordId::new(1),
            Some(&Type::Record {
                definition: RecordId::new(0),
                arguments: Box::new([]),
            })
        )
    );

    Ok(())
}

#[test]
fn mir_keeps_owner_scoped_union_variant_and_payload_layout_ids(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Left = | Same(value: Int) | Empty();
type Right = | Same(flag: Bool, left: Left);
function main(): Unit {}"#,
    )?;
    let left = program
        .unions()
        .first()
        .ok_or_else(|| std::io::Error::other("expected Left union layout"))?;
    let right = program
        .unions()
        .get(1)
        .ok_or_else(|| std::io::Error::other("expected Right union layout"))?;
    let left_id = UnionId::new(0);
    let right_id = UnionId::new(1);
    let left_same = VariantId::new(left_id, 0);
    let left_empty = VariantId::new(left_id, 1);
    let right_same = VariantId::new(right_id, 0);

    assert_eq!(
        (
            left.id(),
            left.name(),
            left.variants()
                .iter()
                .map(|variant| {
                    (
                        variant.id(),
                        variant.name(),
                        variant
                            .payloads()
                            .iter()
                            .map(|payload| (payload.id(), payload.name(), payload.ty().clone()))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            right.id(),
            right.variants().first().map(|variant| {
                (
                    variant.id(),
                    variant
                        .payloads()
                        .iter()
                        .map(|payload| (payload.id(), payload.ty().clone()))
                        .collect::<Vec<_>>(),
                )
            }),
        ),
        (
            left_id,
            "Left",
            vec![
                (
                    left_same,
                    "Same",
                    vec![(PayloadId::new(left_same, 0), "value", Type::Int)],
                ),
                (left_empty, "Empty", Vec::new()),
            ],
            right_id,
            Some((
                right_same,
                vec![
                    (PayloadId::new(right_same, 0), Type::Bool),
                    (
                        PayloadId::new(right_same, 1),
                        Type::Union {
                            definition: left_id,
                            arguments: Box::new([]),
                        },
                    ),
                ],
            )),
        )
    );

    Ok(())
}

#[test]
fn mir_switch_variant_uses_dense_resolved_targets_within_the_function(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Choice = | Zero() | One() | Two();

function choose(value: Choice): Int {
  return match (value) {
    case Choice.Zero() => 0;
    case Choice.One() => 1;
    case Choice.Two() => 2;
  };
}

function main(): Unit { print(choose(Choice.Two())); }"#,
    )?;
    let function = program
        .functions()
        .first()
        .ok_or_else(|| std::io::Error::other("expected choose function"))?;
    let switches = function
        .blocks()
        .iter()
        .filter_map(|block| match block.terminator() {
            MirTerminator::SwitchVariant { union, targets, .. } => {
                Some((*union, targets.as_slice()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let (union, targets) = switches
        .first()
        .ok_or_else(|| std::io::Error::other("expected resolved variant switch"))?;

    assert_eq!(switches.len(), 1);
    assert_eq!(*union, UnionId::new(0));
    assert_eq!(targets.len(), 3);
    assert!(
        targets
            .iter()
            .all(|target| target.index() < function.blocks().len()),
        "MIR blocks: {:?}",
        function.blocks()
    );

    Ok(())
}

#[test]
fn mir_projects_payloads_by_resolved_owner_and_position() -> Result<(), Box<dyn std::error::Error>>
{
    let program = compile(
        r#"type Pair = | Values(left: Int, right: Int);

function sum(value: Pair): Int {
  return match (value) {
    case Pair.Values(left, right) => left + right;
  };
}

function main(): Unit { print(sum(Pair.Values(20, 22))); }"#,
    )?;
    let function = program
        .functions()
        .first()
        .ok_or_else(|| std::io::Error::other("expected sum function"))?;
    let projections = function
        .blocks()
        .iter()
        .flat_map(|block| block.statements())
        .filter_map(|statement| match statement {
            MirStatement::Store {
                value:
                    MirExpression::VariantPayload {
                        union,
                        variant,
                        payload,
                        ..
                    },
                ..
            } => Some((*union, *variant, *payload)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let union = UnionId::new(0);
    let variant = VariantId::new(union, 0);

    assert_eq!(
        projections,
        [
            (union, variant, PayloadId::new(variant, 0)),
            (union, variant, PayloadId::new(variant, 1)),
        ]
    );

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
            MirTerminator::Goto { .. } | MirTerminator::SwitchVariant { .. } => false,
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
                MirTerminator::SwitchVariant { targets, .. } => {
                    targets.iter().all(|target| target.index() < block_count)
                }
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
fn interpreter_accepts_recursive_union_depth_1024() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Nest = | End() | Next(value: Nest);

function main(): Unit {
  let value: Nest = Nest.End();
  let depth = 1;
  while (depth < 1024) {
    value = Nest.Next(value);
    depth = depth + 1;
  }
  const result = match (value) {
    case Nest.End() => 0;
    case Nest.Next(inner) => 42;
  };
  print(result);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42"]);

    Ok(())
}

#[test]
fn interpreter_accepts_union_depth_1024_transmitted_through_record_payloads(
) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        r#"type Link = { value: Nest; };
type Nest = | End() | Next(link: Link);

function main(): Unit {
  let value: Nest = Nest.End();
  let depth = 1;
  while (depth < 1024) {
    value = Nest.Next({ value: value });
    depth = depth + 1;
  }
  const result = match (value) {
    case Nest.End() => 0;
    case Nest.Next(link) => 42;
  };
  print(result);
}"#,
    )?;

    let execution = run(&program)?;

    assert_eq!(execution.output(), ["42"]);

    Ok(())
}

#[test]
fn interpreter_rejects_union_depth_1025_transmitted_through_array_payloads(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Nest = | End() | Next(values: Nest[]);

function observe(values: Nest[]): Nest[] {
  print(7);
  return values;
}

function main(): Unit {
  let value: Nest = Nest.End();
  let depth = 1;
  while (depth < 1024) {
    value = Nest.Next([value]);
    depth = depth + 1;
  }
  value = Nest.Next(observe([value]));
  print(8);
}"#;
    let constructor = "Nest.Next(observe([value]))";
    let constructor_start = source
        .rfind(constructor)
        .ok_or_else(|| std::io::Error::other("expected depth-1025 array constructor"))?;
    let expected_span = SourceSpan::new(
        FileId::new(3),
        TextRange::new(constructor_start, constructor_start + constructor.len()),
    );
    let program = compile(source)?;

    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a recursive union depth error"))?;

    assert_eq!(
        failure.error().message(),
        "recursive union nesting limit of 1024 exceeded"
    );
    assert_eq!(failure.error().span(), expected_span);
    assert_eq!(failure.output(), ["7"]);

    Ok(())
}

#[test]
fn interpreter_rejects_recursive_union_depth_1025_at_the_constructor(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Nest = | End() | Next(value: Nest);

function observe(value: Nest): Nest {
  print(7);
  return value;
}

function main(): Unit {
  let value: Nest = Nest.End();
  let depth = 1;
  while (depth < 1024) {
    value = Nest.Next(value);
    depth = depth + 1;
  }
  value = Nest.Next(observe(value));
  print(8);
}"#;
    let constructor = "Nest.Next(observe(value))";
    let constructor_start = source
        .rfind(constructor)
        .ok_or_else(|| std::io::Error::other("expected depth-1025 constructor"))?;
    let expected_span = SourceSpan::new(
        FileId::new(3),
        TextRange::new(constructor_start, constructor_start + constructor.len()),
    );
    let program = compile(source)?;

    let failure = run(&program)
        .err()
        .ok_or_else(|| std::io::Error::other("expected a recursive union depth error"))?;

    assert_eq!(
        failure.error().message(),
        "recursive union nesting limit of 1024 exceeded"
    );
    assert_eq!(failure.error().span(), expected_span);
    assert_eq!(failure.output(), ["7"]);

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
        MirExpression::Array { elements, .. } => {
            elements.iter().any(expression_contains_logical_binary)
        }
        MirExpression::Record { fields, .. } => {
            fields.iter().any(expression_contains_logical_binary)
        }
        MirExpression::Variant { payloads, .. } => {
            payloads.iter().any(expression_contains_logical_binary)
        }
        MirExpression::Index { target, index, .. } => {
            expression_contains_logical_binary(target) || expression_contains_logical_binary(index)
        }
        MirExpression::Length { target, .. } => expression_contains_logical_binary(target),
        MirExpression::Field { target, .. } => expression_contains_logical_binary(target),
        MirExpression::Integer { .. }
        | MirExpression::Boolean { .. }
        | MirExpression::String { .. }
        | MirExpression::VariantPayload { .. }
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
