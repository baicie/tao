//! Semantic regression tests for the Nexa Language Core.

use nexa_hir::{
    lower, type_check, Analysis, Builtin, Expression, FunctionId, LocalId, LoweringError,
    NameResolution, Statement, Type, TypeReferenceKind,
};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

#[test]
fn type_check_accepts_typed_functions_inferred_constants_and_branches(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function twice(value: Int): Int {
  return value * 2;
}

function main(): Unit {
  const inferred = twice(20 + 1);
  if (!(inferred < 42) === true) {
    print(inferred);
  } else {
    print(0);
  }
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
fn type_check_accepts_mutable_bindings_loops_and_logical_expressions(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function sum(limit: Int): Int {
  let total: Int = 0;
  let index = 0;
  while (index < limit && true) {
    index = index + 1;
    while (false) {
      continue;
    }
    if (index === 2 || false) {
      continue;
    }
    total = total + index;
    if (index >= limit) {
      break;
    }
  }
  return total;
}

function main(): Unit {
  print(sum(4));
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
fn type_check_accepts_and_decodes_string_expressions() -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit {\n  const message: String = \"Nexa\\n\" + \"\u{4e16}\u{754c}\";\n  if (message === \"Nexa\\n\u{4e16}\u{754c}\") {\n    print(message);\n  }\n}";
    let analysis = analyze(source)?;
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!("diagnostics: {:?}", analysis.diagnostics()))
    })?;
    let Statement::Const(declaration) = &typed.program().functions[0].body.statements[0] else {
        return Err(std::io::Error::other("expected const declaration").into());
    };
    let Expression::Binary { left, .. } = &declaration.initializer else {
        return Err(std::io::Error::other("expected string concatenation").into());
    };
    let Expression::String { value, .. } = left.as_ref() else {
        return Err(std::io::Error::other("expected decoded string literal").into());
    };

    assert_eq!(
        (
            value.as_str(),
            declaration.annotation.as_ref().map(|ty| &ty.kind)
        ),
        ("Nexa\n", Some(&TypeReferenceKind::String))
    );

    Ok(())
}

#[test]
fn type_check_accepts_nested_arrays_index_length_and_main_args(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function first(values: Int[]): Int {
  return values[0];
}

function main(args: String[]): Unit {
  const rows: Int[][] = [[], [20, 22]];
  print(args.length);
  print(first(rows[1]));
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
fn type_check_contextualizes_empty_arrays_in_every_supported_position(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function empty(): Int[] {
  return [];
}

function consume(values: Int[]): Unit {}

function main(): Unit {
  let values: Int[] = [];
  values = [];
  consume([]);
  consume(empty());
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
fn type_check_rejects_unconstrained_and_heterogeneous_arrays(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { const empty = []; const mixed = [1, true]; }")?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 2);

    Ok(())
}

#[test]
fn type_check_rejects_invalid_array_indexes_members_and_equality(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"function main(): Unit {
  const values = [1, 2];
  const wrongIndex = values[false];
  const wrongBase = 1[0];
  print(values.capacity);
  const same = values === values;
}"#,
    )?;

    assert_eq!(
        (
            diagnostic_count(&analysis, "E3001"),
            diagnostic_count(&analysis, "E2005")
        ),
        (3, 1)
    );

    Ok(())
}

#[test]
fn type_check_allows_mutable_shadowing_in_nested_scopes() -> Result<(), Box<dyn std::error::Error>>
{
    let analysis = analyze(
        r#"function main(): Unit {
  const value = 1;
  if (true) {
    let value = 2;
    value = 3;
  }
  print(value);
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
fn type_check_rejects_unit_array_elements_and_unprintable_values(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        "function main(): Unit { const empty: Unit[] = []; const values = [print(1)]; print([1]); }",
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 3);

    Ok(())
}

#[test]
fn type_check_rejects_string_coercion_and_string_members() -> Result<(), Box<dyn std::error::Error>>
{
    let analysis = analyze(
        r#"function main(): Unit {
  const mixed = "answer: " + 42;
  print("Nexa".length);
  const indexed = "Nexa"[0];
}"#,
    )?;

    assert_eq!(
        (
            diagnostic_count(&analysis, "E3001"),
            diagnostic_count(&analysis, "E2005")
        ),
        (2, 1)
    );

    Ok(())
}

#[test]
fn type_check_rejects_invalid_main_signatures() -> Result<(), Box<dyn std::error::Error>> {
    let wrong_argument = analyze("function main(args: Int[]): Unit {}")?;
    let wrong_arity = analyze("function main(left: String[], right: String[]): Unit {}")?;
    let wrong_return = analyze("function main(): Int { return 0; }")?;

    assert!([&wrong_argument, &wrong_arity, &wrong_return]
        .into_iter()
        .all(|analysis| has_diagnostic(analysis, "E3003")));

    Ok(())
}

#[test]
fn typed_program_records_stable_types_and_name_resolutions(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function identity(value: Int): Int {
  return value;
}

function main(args: String[]): Unit {
  let total = identity(1);
  if (true) {
    const total = 2;
    print(total);
  }
  total = total + args.length;
  print(total);
}"#;
    let analysis = analyze(source)?;
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!("diagnostics: {:?}", analysis.diagnostics()))
    })?;
    let identity_declaration = prefix_span(source, "identity(value", "identity".len())?;
    let identity_call = prefix_span(source, "identity(1)", "identity".len())?;
    let value_declaration = prefix_span(source, "value: Int", "value".len())?;
    let value_reference = prefix_span(source, "value;", "value".len())?;
    let outer_total = prefix_span(source, "total = identity", "total".len())?;
    let inner_total = prefix_span(source, "total = 2", "total".len())?;
    let assignment_total = prefix_span(source, "total = total +", "total".len())?;
    let length = prefix_span(source, "length", "length".len())?;
    let member = prefix_span(source, "args.length", "args.length".len())?;

    assert_eq!(
        (
            typed.name_resolution(identity_declaration),
            typed.name_resolution(identity_call),
            typed.name_resolution(value_declaration),
            typed.name_resolution(value_reference),
            typed.name_resolution(outer_total),
            typed.name_resolution(inner_total),
            typed.name_resolution(assignment_total),
            typed.name_resolution(length),
            typed.expression_type(member),
            typed
                .function_facts(FunctionId::new(0))
                .map(|facts| (facts.parameter_ids(), facts.local_count())),
            typed
                .function_facts(FunctionId::new(1))
                .map(|facts| (facts.parameter_ids(), facts.local_count()))
        ),
        (
            Some(NameResolution::Function(FunctionId::new(0))),
            Some(NameResolution::Function(FunctionId::new(0))),
            Some(NameResolution::Local(LocalId::new(0))),
            Some(NameResolution::Local(LocalId::new(0))),
            Some(NameResolution::Local(LocalId::new(1))),
            Some(NameResolution::Local(LocalId::new(2))),
            Some(NameResolution::Local(LocalId::new(1))),
            Some(NameResolution::Builtin(Builtin::ArrayLength)),
            Some(&Type::Int),
            Some((&[LocalId::new(0)][..], 1)),
            Some((&[LocalId::new(0)][..], 3))
        )
    );

    Ok(())
}

#[test]
fn typed_program_records_exact_immutable_data_expression_types(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit {
  const values = ["Nexa"];
  print(values[0]);
  print(values.length);
}"#;
    let analysis = analyze(source)?;
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!("diagnostics: {:?}", analysis.diagnostics()))
    })?;
    let string = prefix_span(source, r#""Nexa""#, r#""Nexa""#.len())?;
    let array = prefix_span(source, r#"["Nexa"]"#, r#"["Nexa"]"#.len())?;
    let index = prefix_span(source, "values[0]", "values[0]".len())?;
    let member = prefix_span(source, "values.length", "values.length".len())?;
    let string_array = Type::Array(Box::new(Type::String));

    assert_eq!(
        (
            typed.expression_type(string),
            typed.expression_type(array),
            typed.expression_type(index),
            typed.expression_type(member)
        ),
        (
            Some(&Type::String),
            Some(&string_array),
            Some(&Type::String),
            Some(&Type::Int)
        )
    );

    Ok(())
}

#[test]
fn immutable_data_diagnostics_keep_precise_primary_spans() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"function main(): Unit {
  const values = [1, 2];
  const invalid = values[false];
  print(values.capacity);
  const mixed = [1, true];
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        (
            primary_spans(&analysis, "E3001"),
            primary_spans(&analysis, "E2005")
        ),
        (
            vec![
                prefix_span(source, "false", "false".len())?,
                prefix_span(source, "true", "true".len())?
            ],
            vec![prefix_span(source, "capacity", "capacity".len())?]
        )
    );

    Ok(())
}

#[test]
fn typed_program_keeps_fact_spans_unique() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        "function main(args: String[]): Unit { const values = [1, 2]; print(values[0] + args.length); }",
    )?;
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!("diagnostics: {:?}", analysis.diagnostics()))
    })?;
    let expression_spans = typed
        .expression_types()
        .map(|(span, _)| span)
        .collect::<std::collections::HashSet<_>>();
    let resolution_spans = typed
        .name_resolutions()
        .map(|(span, _)| span)
        .collect::<std::collections::HashSet<_>>();

    assert_eq!((expression_spans.len(), resolution_spans.len()), (10, 7));

    Ok(())
}

#[test]
fn type_check_reports_undefined_names() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { print(missing); }")?;

    assert!(has_diagnostic(&analysis, "E2001"));

    Ok(())
}

#[test]
fn type_check_reports_undefined_assignment_targets() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { missing = 1; }")?;

    assert!(has_diagnostic(&analysis, "E2001"));

    Ok(())
}

#[test]
fn type_check_keeps_a_binding_in_scope_after_an_initializer_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis =
        analyze("function main(): Unit { let value = missing; value = 1; print(value); }")?;

    assert_eq!(diagnostic_count(&analysis, "E2001"), 1);

    Ok(())
}

#[test]
fn type_check_reports_a_duplicate_after_an_initializer_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { let value = missing; let value = 1; }")?;

    assert_eq!(
        (
            diagnostic_count(&analysis, "E2001"),
            diagnostic_count(&analysis, "E2002")
        ),
        (1, 1)
    );

    Ok(())
}

#[test]
fn parser_rejects_uninitialized_mutable_bindings() {
    let parse = parse_source(FileId::new(3), "function main(): Unit { let count: Int; }");

    assert!(parse
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code().as_str() == "E1001"));
}

#[test]
fn type_check_reports_duplicate_bindings() -> Result<(), Box<dyn std::error::Error>> {
    let analysis =
        analyze("function main(): Unit { const answer = 1; let answer = 2; print(answer); }")?;

    assert!(has_diagnostic(&analysis, "E2002"));

    Ok(())
}

#[test]
fn type_check_reports_incorrect_call_arity() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { print(); }")?;

    assert!(has_diagnostic(&analysis, "E2003"));

    Ok(())
}

#[test]
fn type_check_rejects_assignments_to_constants_and_parameters(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function update(value: Int): Int {
  value = value + 1;
  return value;
}

function main(): Unit {
  const answer = 42;
  answer = update(answer);
}"#;
    let analysis = analyze(source)?;
    let expected_spans = [
        prefix_span(source, "value = value", "value".len())?,
        prefix_span(source, "answer = update", "answer".len())?,
    ];

    assert_eq!(diagnostic_count(&analysis, "E2004"), 2);
    assert_eq!(primary_spans(&analysis, "E2004"), expected_spans);

    Ok(())
}

#[test]
fn type_check_reports_mismatched_types() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { const answer: Bool = 42; }")?;

    assert!(has_diagnostic(&analysis, "E3001"));

    Ok(())
}

#[test]
fn type_check_reports_assignment_type_mismatches() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { let answer: Int = 42; answer = false; }")?;

    assert!(has_diagnostic(&analysis, "E3001"));

    Ok(())
}

#[test]
fn type_check_requires_boolean_logical_operands() -> Result<(), Box<dyn std::error::Error>> {
    let analysis =
        analyze("function main(): Unit { const left = 1 && true; const right = false || 0; }")?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 2);

    Ok(())
}

#[test]
fn type_check_checks_known_logical_operand_after_name_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { const result = missing && 1; }")?;

    assert_eq!(diagnostic_count(&analysis, "E2001"), 1);
    assert_eq!(diagnostic_count(&analysis, "E3001"), 1);

    Ok(())
}

#[test]
fn type_check_reports_non_boolean_conditions() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { if (42) { return; } }")?;

    assert!(has_diagnostic(&analysis, "E3002"));

    Ok(())
}

#[test]
fn type_check_reports_non_boolean_while_conditions() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("function main(): Unit { while (42) { break; } }")?;

    assert!(has_diagnostic(&analysis, "E3002"));

    Ok(())
}

#[test]
fn type_check_rejects_loop_control_outside_loops() -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit { break; continue; }";
    let analysis = analyze(source)?;
    let expected_spans = [
        prefix_span(source, "break", "break;".len())?,
        prefix_span(source, "continue", "continue;".len())?,
    ];

    assert_eq!(diagnostic_count(&analysis, "E3004"), 2);
    assert_eq!(primary_spans(&analysis, "E3004"), expected_spans);

    Ok(())
}

#[test]
fn type_check_reports_missing_non_unit_returns() -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        "function value(flag: Bool): Int { if (flag) { return 1; } } function main(): Unit {}",
    )?;

    assert!(has_diagnostic(&analysis, "E3003"));

    Ok(())
}

#[test]
fn type_check_does_not_treat_while_as_a_guaranteed_return() -> Result<(), Box<dyn std::error::Error>>
{
    let analysis =
        analyze("function value(): Int { while (true) { return 1; } } function main(): Unit {}")?;

    assert!(has_diagnostic(&analysis, "E3003"));

    Ok(())
}

fn analyze(source: &str) -> Result<Analysis, LoweringError> {
    let file = FileId::new(3);
    let parse = parse_source(file, source);
    assert!(
        parse.is_ok(),
        "parser diagnostics: {:?}",
        parse.diagnostics()
    );
    let program = lower(file, &parse.syntax())?;

    Ok(type_check(&program))
}

fn has_diagnostic(analysis: &Analysis, code: &str) -> bool {
    analysis
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code().as_str() == code)
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
        FileId::new(3),
        TextRange::new(start, start + width),
    ))
}
