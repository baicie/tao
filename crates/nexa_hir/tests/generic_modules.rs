//! Cross-module bounded-generic semantic regression tests.

use nexa_hir::{
    lower_module, type_check_modules, Analysis, FunctionId, ModuleId, NameResolution, RecordId,
    ResolvedImport, Type, UnionId,
};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

#[test]
fn type_check_modules_imports_generic_functions_records_and_unions(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { Box, Option, box, some } from "./model.nexa";
function main(): Unit {
  const number: Box<Int> = box(42);
  const text: Box<String> = box("nexa");
  const optional: Option<Int> = some(number.value);
  const answer = match (optional) {
    case Option.Some(value) => value;
    case Option.None() => 0;
  };
  print(answer);
  print(text.value);
}"#;
    let model = r#"export type Box<T> = { value: T; };
export type Option<T> = | Some(value: T) | None();
export function box<T>(value: T): Box<T> {
  return { value: value };
}
export function some<T>(value: T): Option<T> {
  return Option.Some(value);
}"#;
    let analysis = analyze(&[entry, model], &[(0, 0, 1)])?;

    assert!(
        analysis.is_ok(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics()
    );
    let typed = analysis
        .typed()
        .ok_or_else(|| std::io::Error::other("expected typed generic modules"))?;
    let box_definition = RecordId::in_module(ModuleId::new(1), 0);

    assert_eq!(
        (
            typed.name_resolution(exact_span(0, entry, "Box")?),
            typed.name_resolution(exact_span(0, entry, "Option")?),
            typed.name_resolution(exact_span(0, entry, "box")?),
            typed.name_resolution(exact_span(0, entry, "some")?),
        ),
        (
            Some(NameResolution::Record(box_definition)),
            Some(NameResolution::Union(UnionId::in_module(
                ModuleId::new(1),
                0,
            ))),
            Some(NameResolution::Function(FunctionId::in_module(
                ModuleId::new(1),
                0,
            ))),
            Some(NameResolution::Function(FunctionId::in_module(
                ModuleId::new(1),
                1,
            ))),
        )
    );
    assert_eq!(
        (
            typed
                .expression_type(exact_span(0, entry, "box(42)")?)
                .cloned(),
            typed
                .expression_type(exact_span(0, entry, "box(\"nexa\")")?)
                .cloned(),
        ),
        (
            Some(Type::Record {
                definition: box_definition,
                arguments: vec![Type::Int].into_boxed_slice(),
            }),
            Some(Type::Record {
                definition: box_definition,
                arguments: vec![Type::String].into_boxed_slice(),
            }),
        )
    );

    Ok(())
}

#[test]
fn type_check_modules_distinguishes_arguments_of_one_imported_generic_record(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { Box, box } from "./model.nexa";
function takeNumber(value: Box<Int>): Unit {}
function main(): Unit {
  const text: Box<String> = box("nexa");
  takeNumber(text);
}"#;
    let model = r#"export type Box<T> = { value: T; };
export function box<T>(value: T): Box<T> {
  return { value: value };
}"#;
    let analysis = analyze(&[entry, model], &[(0, 0, 1)])?;
    let text_occurrences = occurrence_spans(0, entry, "text");
    let call_argument = text_occurrences
        .get(1)
        .copied()
        .ok_or_else(|| std::io::Error::other("expected call argument `text`"))?;

    assert_eq!(diagnostic_count(&analysis, "E3001"), 1);
    assert_eq!(primary_spans(&analysis, "E3001"), [call_argument]);

    Ok(())
}

#[test]
fn type_check_modules_keeps_private_and_unknown_generic_import_diagnostics(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = "import { Hidden, Missing } from \"./model.nexa\";";
    let model = "type Hidden<T> = { value: T; };";
    let analysis = analyze(&[entry, model], &[(0, 0, 1)])?;

    assert_eq!(diagnostic_count(&analysis, "E4004"), 1);
    assert_eq!(diagnostic_count(&analysis, "E4003"), 1);
    assert_eq!(
        label_spans(&analysis, "E4004"),
        [vec![
            exact_span(0, entry, "Hidden")?,
            exact_span(1, model, "Hidden")?,
        ]]
    );
    assert_eq!(
        label_spans(&analysis, "E4003"),
        [vec![exact_span(0, entry, "Missing")?]]
    );

    Ok(())
}

#[test]
fn type_check_modules_deduplicates_generic_identity_across_a_diamond(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { left } from "./left.nexa";
import { right } from "./right.nexa";
function main(): Unit {
  print(left(42).value);
  print(right("nexa").value);
}"#;
    let left = r#"import { Box, box } from "./core.nexa";
export function left(value: Int): Box<Int> {
  return box(value);
}"#;
    let core = r#"export type Box<T> = { value: T; };
export function box<T>(value: T): Box<T> {
  return { value: value };
}"#;
    let right = r#"import { Box, box } from "./core.nexa";
export function right(value: String): Box<String> {
  return box(value);
}"#;
    let analysis = analyze(
        &[entry, left, core, right],
        &[(0, 0, 1), (1, 0, 2), (0, 1, 3), (3, 0, 2)],
    )?;

    assert!(
        analysis.is_ok(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics()
    );
    let typed = analysis
        .typed()
        .ok_or_else(|| std::io::Error::other("expected typed diamond modules"))?;
    let shared_box = Some(NameResolution::Record(RecordId::in_module(
        ModuleId::new(2),
        0,
    )));
    let shared_box_function = Some(NameResolution::Function(FunctionId::in_module(
        ModuleId::new(2),
        0,
    )));

    assert_eq!(
        (
            typed.name_resolution(exact_span(1, left, "Box")?),
            typed.name_resolution(exact_span(3, right, "Box")?),
            typed.name_resolution(exact_span(1, left, "box")?),
            typed.name_resolution(exact_span(3, right, "box")?),
        ),
        (
            shared_box,
            shared_box,
            shared_box_function,
            shared_box_function,
        )
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
        TextRange::new(start, start + text.len()),
    ))
}

fn occurrence_spans(file: u32, source: &str, text: &str) -> Vec<SourceSpan> {
    source
        .match_indices(text)
        .map(|(start, value)| {
            SourceSpan::new(
                FileId::new(file),
                TextRange::new(start, start + value.len()),
            )
        })
        .collect()
}
