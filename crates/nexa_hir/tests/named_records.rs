//! Semantic regression tests for nominal immutable records.

use nexa_hir::{lower, type_check, Analysis, FieldId, NameResolution, RecordId, Type};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

const TEST_FILE: FileId = FileId::new(7);

#[test]
fn type_check_accepts_nested_records_arrays_and_reordered_fields(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Address = { city: String; };
type User = {
  name: String;
  age: Int;
  address: Address;
  tags: String[];
};

function makeUser(): User {
  return {
    age: 42,
    tags: [],
    address: { city: "London" },
    name: "Ada"
  };
}

function main(): Unit {
  const users: User[] = [makeUser()];
  print(users[0].address.city);
  print(users[0].tags.length);
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
fn type_check_propagates_record_context_through_all_supported_positions(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Point = { x: Int; y: Int; };

function consume(point: Point): Point {
  return { y: point.y, x: point.x };
}

function main(): Unit {
  let point: Point = { x: 1, y: 2 };
  point = { y: 4, x: 3 };
  const points: Point[] = [{ x: 5, y: 6 }];
  const result = consume({ x: points[0].x, y: point.y });
  print(result.x);
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
fn type_check_accepts_forward_record_references_empty_records_and_value_name_overlap(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type User = { profile: Profile; };
type Profile = { label: String; };
type Empty = {};

function User(value: Int): Int { return value; }
function main(): Unit {
  const user: User = { profile: { label: "Ada" } };
  const empty: Empty = {};
  print(User(42));
  print(user.profile.label);
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
fn typed_program_records_stable_record_and_field_resolutions(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type User = { name: String; };
function main(): Unit {
  const user: User = { name: "Ada" };
  print(user.name);
}"#;
    let analysis = analyze(source)?;
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!("diagnostics: {:?}", analysis.diagnostics()))
    })?;
    let record = typed
        .record_facts(RecordId::new(0))
        .ok_or_else(|| std::io::Error::other("expected record facts"))?;
    let field = record
        .fields()
        .first()
        .ok_or_else(|| std::io::Error::other("expected field facts"))?;
    let record_spans = occurrence_spans(source, "User");
    let field_spans = occurrence_spans(source, "name");
    let literal_span = exact_span(source, "{ name: \"Ada\" }")?;
    let member_span = exact_span(source, "user.name")?;

    assert_eq!(
        (
            record.id(),
            record.name(),
            field.id(),
            field.name(),
            field.ty(),
            typed.name_resolution(record_spans[0]),
            typed.name_resolution(record_spans[1]),
            typed.name_resolution(field_spans[0]),
            typed.name_resolution(field_spans[1]),
            typed.name_resolution(field_spans[2]),
            typed.expression_type(literal_span),
            typed.expression_type(member_span),
        ),
        (
            RecordId::new(0),
            "User",
            FieldId::new(RecordId::new(0), 0),
            "name",
            &Type::String,
            Some(NameResolution::Record(RecordId::new(0))),
            Some(NameResolution::Record(RecordId::new(0))),
            Some(NameResolution::Field(FieldId::new(RecordId::new(0), 0))),
            Some(NameResolution::Field(FieldId::new(RecordId::new(0), 0))),
            Some(NameResolution::Field(FieldId::new(RecordId::new(0), 0))),
            Some(&Type::Record(RecordId::new(0))),
            Some(&Type::String),
        )
    );

    Ok(())
}

#[test]
fn typed_program_scopes_same_named_fields_to_their_record_owner(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Left = { value: Int; };
type Right = { value: Int; };
function main(): Unit {}"#,
    )?;
    let typed = analysis.typed().ok_or_else(|| {
        std::io::Error::other(format!("diagnostics: {:?}", analysis.diagnostics()))
    })?;
    let left = typed
        .record_facts(RecordId::new(0))
        .and_then(|record| record.fields().first())
        .ok_or_else(|| std::io::Error::other("expected Left.value facts"))?;
    let right = typed
        .record_facts(RecordId::new(1))
        .and_then(|record| record.fields().first())
        .ok_or_else(|| std::io::Error::other("expected Right.value facts"))?;

    assert_eq!(
        (left.id(), right.id()),
        (
            FieldId::new(RecordId::new(0), 0),
            FieldId::new(RecordId::new(1), 0)
        )
    );

    Ok(())
}

#[test]
fn type_check_rejects_unknown_and_duplicate_record_declarations(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type User = { missing: Unknown; name: String; name: String; };
type User = { value: Int; };
function main(): Unit {}"#,
    )?;

    assert_eq!(
        (
            count_code(&analysis, "E2001"),
            count_code(&analysis, "E2002")
        ),
        (1, 2)
    );

    Ok(())
}

#[test]
fn type_check_rejects_uncontextualized_record_literals() -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit { const value = { answer: 42 }; }";
    let analysis = analyze(source)?;

    assert_eq!(count_code(&analysis, "E3001"), 1);
    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [exact_span(source, "{ answer: 42 }")?]
    );

    Ok(())
}

#[test]
fn type_check_rejects_duplicate_unknown_and_missing_literal_fields(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type User = { name: String; age: Int; };
function main(): Unit {
  const user: User = { name: "Ada", name: "Grace", extra: true };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        (
            count_code(&analysis, "E2002"),
            count_code(&analysis, "E2005"),
            count_code(&analysis, "E2006")
        ),
        (1, 1, 1)
    );
    assert_eq!(
        primary_spans(&analysis, "E2006"),
        [exact_span(
            source,
            "{ name: \"Ada\", name: \"Grace\", extra: true }"
        )?]
    );
    assert_eq!(
        label_spans(&analysis, "E2005"),
        [vec![exact_span(source, "extra")?]]
    );
    assert_eq!(
        label_spans(&analysis, "E2006"),
        [vec![
            exact_span(source, "{ name: \"Ada\", name: \"Grace\", extra: true }")?,
            exact_span(source, "age: Int;")?,
        ]]
    );

    Ok(())
}

#[test]
fn type_check_rejects_field_type_mismatches_and_nominal_substitution(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Left = { value: Int; };
type Right = { value: Int; };
function consume(value: Left): Unit {}
function main(): Unit {
  const left: Left = { value: false };
  const right: Right = { value: 42 };
  consume(right);
}"#,
    )?;

    assert_eq!(count_code(&analysis, "E3001"), 2);

    Ok(())
}

#[test]
fn type_check_rejects_unit_fields_record_equality_and_print(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Invalid = { nothing: Unit; };
type User = { name: String; };
function main(): Unit {
  const left: User = { name: "Ada" };
  const right: User = { name: "Ada" };
  print(left === right);
  print(left);
}"#,
    )?;

    assert_eq!(count_code(&analysis, "E3001"), 3);

    Ok(())
}

#[test]
fn type_check_rejects_direct_indirect_and_array_recursive_records(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Direct = { next: Direct; };
type Left = { right: Right; };
type Right = { left: Left; };
type Tree = { children: Tree[]; };
function main(): Unit {}"#,
    )?;

    assert_eq!(count_code(&analysis, "E3005"), 4);

    Ok(())
}

#[test]
fn type_check_labels_duplicate_record_declared_and_literal_fields(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type User = { name: String; name: String; };
type User = { value: Int; };
function main(): Unit {
  const user: User = { name: "Ada", name: "Grace" };
}"#;
    let analysis = analyze(source)?;
    let user_spans = occurrence_spans(source, "User");
    let name_spans = occurrence_spans(source, "name");

    assert_eq!(
        label_spans(&analysis, "E2002"),
        [
            vec![name_spans[1], name_spans[0]],
            vec![user_spans[1], user_spans[0]],
            vec![name_spans[3], name_spans[2]],
        ]
    );

    Ok(())
}

#[test]
fn type_check_rejects_unit_arrays_and_record_literals_in_non_record_contexts(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Invalid = { direct: Unit[]; nested: Unit[][]; };
function main(): Unit {
  const value: Int = { answer: 42 };
}"#;
    let analysis = analyze(source)?;

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [
            exact_span(source, "Unit[]")?,
            exact_span(source, "Unit[][]")?,
            exact_span(source, "{ answer: 42 }")?,
        ]
    );

    Ok(())
}

#[test]
fn type_check_labels_record_field_mismatch_equality_and_print_rejections(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type User = { active: Bool; };
function main(): Unit {
  const left: User = { active: 1 };
  const right: User = { active: true };
  const equal = left === right;
  print(left);
}"#;
    let analysis = analyze(source)?;
    let left_spans = occurrence_spans(source, "left");
    let right_spans = occurrence_spans(source, "right");

    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [exact_span(source, "1")?, right_spans[1], left_spans[2],]
    );

    Ok(())
}

#[test]
fn type_check_rejects_and_labels_a_three_record_cycle() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type A = { b: B; };
type B = { c: C; };
type C = { a: A; };
function main(): Unit {}"#;
    let analysis = analyze(source)?;
    let a_spans = occurrence_spans(source, "A");
    let b_spans = occurrence_spans(source, "B");
    let c_spans = occurrence_spans(source, "C");

    assert_eq!(
        label_spans(&analysis, "E3005"),
        [
            vec![b_spans[0], a_spans[0]],
            vec![c_spans[0], b_spans[1]],
            vec![a_spans[1], c_spans[1]],
        ]
    );

    Ok(())
}

#[test]
fn type_check_reports_unknown_record_members_at_the_member_name(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type User = { name: String; };
function main(): Unit {
  const user: User = { name: "Ada" };
  print(user.age);
}"#;
    let analysis = analyze(source)?;
    let age_span = exact_span(source, "age")?;

    assert_eq!(primary_spans(&analysis, "E2005"), [age_span]);

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

fn occurrence_spans(source: &str, text: &str) -> Vec<SourceSpan> {
    source
        .match_indices(text)
        .map(|(start, value)| {
            SourceSpan::new(TEST_FILE, TextRange::new(start, start + value.len()))
        })
        .collect()
}
