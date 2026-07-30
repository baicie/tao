//! Multi-module semantic regression tests.

use nexa_hir::{
    lower_module, type_check_modules, Analysis, ModuleId, NameResolution, ResolvedImport,
};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan};

#[test]
fn type_check_modules_resolves_cross_file_records_unions_functions_and_matches(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { Box, Choice, make, read } from "./model.nexa";
function main(): Unit {
  const choice: Choice = make(42);
  print(read(choice));
}"#;
    let model = r#"export type Box = { value: Int; };
export type Choice = | None() | Some(value: Box);
export function make(value: Int): Choice {
  return Choice.Some({ value: value });
}
export function read(choice: Choice): Int {
  return match (choice) {
    case Choice.None() => 0;
    case Choice.Some(box) => box.value;
  };
}
function main(value: Int): Bool { return true; }"#;

    let analysis = analyze(&[entry, model], &[(0, 0, 1)])?;

    assert!(
        analysis.is_ok(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics()
    );
    let typed = analysis
        .typed()
        .ok_or_else(|| std::io::Error::other("expected typed modules"))?;
    let imported_box = exact_span(0, entry, "Box")?;
    assert_eq!(
        typed.name_resolution(imported_box),
        Some(NameResolution::Record(nexa_hir::RecordId::in_module(
            ModuleId::new(1),
            0
        )))
    );

    Ok(())
}

#[test]
fn type_check_modules_distinguishes_private_and_missing_imports(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = "import { hidden, absent } from \"./library.nexa\";";
    let library = "function hidden(): Int { return 1; }";
    let analysis = analyze(&[entry, library], &[(0, 0, 1)])?;

    assert_eq!(diagnostic_count(&analysis, "E4004"), 1);
    assert_eq!(diagnostic_count(&analysis, "E4003"), 1);
    assert_eq!(
        label_spans(&analysis, "E4004"),
        [vec![
            exact_span(0, entry, "hidden")?,
            exact_span(1, library, "hidden")?
        ]]
    );
    assert_eq!(
        label_spans(&analysis, "E4003"),
        [vec![exact_span(0, entry, "absent")?]]
    );

    Ok(())
}

#[test]
fn type_check_modules_reports_only_the_cross_kind_export_collision(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"export type Item = { value: Int; };
export function Item(): Int { return 1; }"#;
    let analysis = analyze(&[source], &[])?;

    assert_eq!(diagnostic_count(&analysis, "E4005"), 1);
    assert_eq!(diagnostic_count(&analysis, "E2002"), 0);
    let occurrences = occurrence_spans(0, source, "Item");
    assert_eq!(
        label_spans(&analysis, "E4005"),
        [vec![occurrences[1], occurrences[0]]]
    );

    Ok(())
}

#[test]
fn type_check_modules_keeps_value_and_type_import_namespaces_separate(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"function Item(): Int { return 42; }
import { Item } from "./types.nexa";
function main(): Unit {
  const value: Item = { value: Item() };
  print(value.value);
}"#;
    let types = "export type Item = { value: Int; };";
    let analysis = analyze(&[entry, types], &[(0, 0, 1)])?;

    assert!(
        analysis.is_ok(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics()
    );

    Ok(())
}

#[test]
fn type_check_modules_reports_same_namespace_import_collisions_in_source_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"function value(): Int { return 1; }
import { value } from "./library.nexa";"#;
    let library = "export function value(): Int { return 2; }";
    let analysis = analyze(&[entry, library], &[(0, 0, 1)])?;
    let local = exact_span(0, entry, "value")?;
    let imported = occurrence_spans(0, entry, "value")[1];

    assert_eq!(label_spans(&analysis, "E2002"), [vec![imported, local]]);

    Ok(())
}

#[test]
fn type_check_modules_labels_each_late_value_collision_with_the_earliest_import(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { value } from "./library.nexa";
function value(): Int { return 1; }
function value(): Int { return 2; }"#;
    let library = "export function value(): Int { return 0; }";
    let analysis = analyze(&[entry, library], &[(0, 0, 1)])?;
    let occurrences = occurrence_spans(0, entry, "value");

    assert_eq!(
        label_spans(&analysis, "E2002"),
        [
            vec![occurrences[1], occurrences[0]],
            vec![occurrences[2], occurrences[0]],
        ]
    );

    Ok(())
}

#[test]
fn type_check_modules_labels_each_late_type_collision_with_the_earliest_import(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { Item } from "./library.nexa";
type Item = { value: Int; };
type Item = | Value();"#;
    let library = "export type Item = { value: Int; };";
    let analysis = analyze(&[entry, library], &[(0, 0, 1)])?;
    let occurrences = occurrence_spans(0, entry, "Item");

    assert_eq!(
        label_spans(&analysis, "E2002"),
        [
            vec![occurrences[1], occurrences[0]],
            vec![occurrences[2], occurrences[0]],
        ]
    );

    Ok(())
}

#[test]
fn type_check_modules_preserves_nominal_identity_across_files(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { Left, takeLeft } from "./left.nexa";
import { Right, makeRight } from "./right.nexa";
function main(): Unit { takeLeft(makeRight()); }"#;
    let left = r#"export type Left = { value: Int; };
export function takeLeft(value: Left): Unit {}"#;
    let right = r#"export type Right = { value: Int; };
export function makeRight(): Right { return { value: 1 }; }"#;
    let analysis = analyze(&[entry, left, right], &[(0, 0, 1), (0, 1, 2)])?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 1);
    assert_eq!(
        primary_spans(&analysis, "E3001"),
        [exact_span(0, entry, "makeRight()")?]
    );

    Ok(())
}

fn analyze(
    sources: &[&str],
    links: &[(usize, usize, usize)],
) -> Result<Analysis, Box<dyn std::error::Error>> {
    let programs = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let file = FileId::new(u32::try_from(index).map_err(std::io::Error::other)?);
            let parse = parse_source(file, source);
            if !parse.is_ok() {
                return Err(std::io::Error::other(format!(
                    "unexpected parser diagnostics: {:?}",
                    parse.diagnostics()
                ))
                .into());
            }
            lower_module(ModuleId::new(index), file, &parse.syntax()).map_err(Into::into)
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let resolved = links
        .iter()
        .map(|(importer, declaration, target)| {
            let program = programs.get(*importer).ok_or_else(|| {
                std::io::Error::other("resolved import references a missing importer")
            })?;
            let import = program.imports.get(*declaration).ok_or_else(|| {
                std::io::Error::other("resolved import references a missing declaration")
            })?;
            Ok(ResolvedImport::new(
                program.module,
                import.path_span,
                ModuleId::new(*target),
            ))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    Ok(type_check_modules(&programs, &resolved))
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

fn exact_span(file: u32, source: &str, text: &str) -> Result<SourceSpan, std::io::Error> {
    let start = source
        .find(text)
        .ok_or_else(|| std::io::Error::other(format!("expected `{text}` in source")))?;
    Ok(SourceSpan::new(
        FileId::new(file),
        nexa_span::TextRange::new(start, start + text.len()),
    ))
}

fn occurrence_spans(file: u32, source: &str, text: &str) -> Vec<SourceSpan> {
    source
        .match_indices(text)
        .map(|(start, value)| {
            SourceSpan::new(
                FileId::new(file),
                nexa_span::TextRange::new(start, start + value.len()),
            )
        })
        .collect()
}
