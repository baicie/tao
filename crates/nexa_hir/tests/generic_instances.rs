//! Closed generic-instance and recursive-layout regression tests.

use nexa_diagnostics::LabelStyle;
use nexa_hir::{lower, type_check, Analysis};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

const TEST_FILE: FileId = FileId::new(11);

#[test]
fn type_check_rejects_a_recursive_record_revealed_by_owner_substitution(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Box<T> = { value: T; };
type Node = { boxed: Box<Node>; };"#;
    let analysis = analyze(source)?;
    let node_names = occurrence_spans(source, "Node");

    assert_eq!(diagnostic_count(&analysis, "E3005"), 1);
    assert_eq!(
        label_spans(&analysis, "E3005"),
        [vec![exact_span(source, "Box<Node>")?, node_names[0]]]
    );

    Ok(())
}

#[test]
fn type_check_suppresses_non_regular_noise_for_an_unguarded_record_edge(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze("type Bad<T> = { next: Bad<T[]>; };")?;

    assert_eq!(
        (
            diagnostic_count(&analysis, "E3005"),
            diagnostic_count(&analysis, "E3010")
        ),
        (1, 0)
    );

    Ok(())
}

#[test]
fn unconstrained_parameter_diagnostic_labels_its_owner() -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Phantom<Unused> = { value: Int; };";
    let analysis = analyze(source)?;

    assert_eq!(
        labels(&analysis, "E3008"),
        [vec![
            (LabelStyle::Primary, exact_span(source, "Unused")?),
            (LabelStyle::Secondary, exact_span(source, "Phantom")?),
        ]]
    );

    Ok(())
}

#[test]
fn non_regular_recursion_diagnostic_labels_the_generic_definition(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Grow<T> = | Next(value: Grow<T[]>);";
    let analysis = analyze(source)?;

    assert_eq!(
        labels(&analysis, "E3010"),
        [vec![
            (LabelStyle::Primary, exact_span(source, "Grow<T[]>")?),
            (LabelStyle::Secondary, exact_span(source, "Grow")?),
        ]]
    );

    Ok(())
}

#[test]
fn non_regular_call_diagnostic_labels_the_generic_function(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function grow<T>(value: T): Unit { grow([value]); }";
    let analysis = analyze(source)?;

    assert_eq!(
        labels(&analysis, "E3010"),
        [vec![
            (LabelStyle::Primary, exact_span(source, "grow([value])")?,),
            (LabelStyle::Secondary, exact_span(source, "grow")?),
        ]]
    );

    Ok(())
}

#[test]
fn type_check_expands_record_fields_before_validating_closed_instances(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Cell<T> = { value: T; };
type Envelope<T> = { cell: Cell<T>; };
function consume(value: Envelope<Unit>): Unit {}"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 1);

    Ok(())
}

#[test]
fn type_check_expands_union_payloads_before_validating_closed_instances(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Items<T> = { values: T[]; };
type Choice<T> = | Some(value: Items<T>);
function consume(value: Choice<Unit>): Unit {}"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 1);

    Ok(())
}

#[test]
fn type_check_expands_function_signatures_before_validating_closed_instances(
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(
        r#"type Items<T> = { values: T[]; };
function expose<T>(value: T): Items<T> {
  return { values: [value] };
}
function main(): Unit {
  expose(print(1));
}"#,
    )?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 1);

    Ok(())
}

#[test]
fn type_check_deduplicates_expanded_instances_and_rejects_instance_257(
) -> Result<(), Box<dyn std::error::Error>> {
    let accepted_source = layered_instance_program(255);
    let accepted = analyze(&accepted_source)?;
    let rejected_source = layered_instance_program(256);
    let rejected = analyze(&rejected_source)?;
    let layer_zero_applications = occurrence_spans(&rejected_source, "Layer0<T>");

    assert_eq!(diagnostic_count(&accepted, "E3010"), 0);
    assert_eq!(
        labels(&rejected, "E3010"),
        [vec![
            (LabelStyle::Primary, layer_zero_applications[1],),
            (
                LabelStyle::Secondary,
                exact_span(&rejected_source, "Layer0")?,
            ),
        ]]
    );

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

fn layered_instance_program(layer_count: usize) -> String {
    let mut source = "type Layer0<T> = { value: T; };\n".to_owned();
    for index in 1..layer_count {
        source.push_str(&format!(
            "type Layer{index}<T> = {{ value: Layer{}<T>; }};\n",
            index - 1
        ));
    }
    let outer = layer_count - 1;
    source.push_str(&format!(
        "type Root<T> = {{ first: Layer{outer}<T>; second: Layer{outer}<T>; }};\n"
    ));
    source.push_str("function consume(value: Root<Int>): Unit {}\n");
    source
}
