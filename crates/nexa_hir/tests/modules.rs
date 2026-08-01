//! HIR lowering tests for module imports and declaration visibility.

use nexa_hir::{lower, lower_module, ModuleId, Visibility};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

const TEST_FILE: FileId = FileId::new(12);

#[test]
fn lower_module_records_import_names_decoded_path_and_exact_spans(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"import { First, Second } from "./dir\\value.nexa";"#;
    let program = parse_and_lower_module(ModuleId::new(4), source)?;
    let import = program
        .imports
        .first()
        .ok_or_else(|| std::io::Error::other("expected one lowered import"))?;

    assert_eq!(
        (
            program.module,
            import
                .names
                .iter()
                .map(|name| (name.text.as_str(), name.span))
                .collect::<Vec<_>>(),
            import.path.as_str(),
            import.path_span,
            import.span,
        ),
        (
            ModuleId::new(4),
            vec![
                ("First", exact_span(source, "First")?),
                ("Second", exact_span(source, "Second")?),
            ],
            "./dir\\value.nexa",
            exact_span(source, r#""./dir\\value.nexa""#)?,
            exact_span(source, source)?,
        )
    );

    Ok(())
}

#[test]
fn lower_module_collects_imports_from_every_top_level_position_in_source_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"import { First } from "./first.nexa";
function between(): Unit {}
import { Second } from "./second.nexa";
type Value = { inner: Int; };
import { Third } from "./third.nexa";"#;
    let program = parse_and_lower_module(ModuleId::new(8), source)?;

    assert_eq!(
        program
            .imports
            .iter()
            .map(|import| import.path.as_str())
            .collect::<Vec<_>>(),
        ["./first.nexa", "./second.nexa", "./third.nexa"]
    );

    Ok(())
}

#[test]
fn lower_module_marks_exported_declarations_and_preserves_each_kind_source_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function privateFunction(): Unit {}
export function publicFunction(): Unit {}
type PrivateRecord = { value: Int; };
export type PublicRecord = { value: Int; };
type PrivateUnion = None();
export type PublicUnion = None();"#;
    let program = parse_and_lower_module(ModuleId::new(2), source)?;

    assert_eq!(
        (
            declarations(&program.functions),
            record_declarations(&program.records),
            union_declarations(&program.unions),
        ),
        (
            vec![
                ("privateFunction", Visibility::Private),
                ("publicFunction", Visibility::Exported),
            ],
            vec![
                ("PrivateRecord", Visibility::Private),
                ("PublicRecord", Visibility::Exported),
            ],
            vec![
                ("PrivateUnion", Visibility::Private),
                ("PublicUnion", Visibility::Exported),
            ],
        )
    );
    assert_eq!(
        program.functions[1].span,
        exact_span(source, "function publicFunction(): Unit {}")?
    );

    Ok(())
}

#[test]
fn lower_keeps_v05_sources_in_the_private_entry_module() -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Value = { inner: Int; }; function main(): Unit {}";
    let parse = parse_source(TEST_FILE, source);
    let program = lower(TEST_FILE, &parse.syntax())?;

    assert_eq!(
        (
            program.module,
            program.imports.len(),
            program.records[0].visibility,
            program.functions[0].visibility,
        ),
        (ModuleId::ENTRY, 0, Visibility::Private, Visibility::Private,)
    );

    Ok(())
}

#[test]
fn lower_module_rejects_a_malformed_import_list_even_if_parser_errors_are_ignored() {
    let source = "import { First Second } from \"./values.nexa\";";
    let parse = parse_source(TEST_FILE, source);

    assert!(
        lower_module(ModuleId::new(1), TEST_FILE, &parse.syntax()).is_err(),
        "parser diagnostics: {:?}",
        parse.diagnostics()
    );
}

#[test]
fn lower_module_rejects_an_export_wrapper_without_an_exportable_declaration() {
    let source = "export import { Value } from \"./values.nexa\";";
    let parse = parse_source(TEST_FILE, source);

    assert!(
        lower_module(ModuleId::new(1), TEST_FILE, &parse.syntax()).is_err(),
        "parser diagnostics: {:?}",
        parse.diagnostics()
    );
}

fn parse_and_lower_module(
    module: ModuleId,
    source: &str,
) -> Result<nexa_hir::Program, Box<dyn std::error::Error>> {
    let parse = parse_source(TEST_FILE, source);
    if !parse.is_ok() {
        return Err(std::io::Error::other(format!(
            "unexpected parser diagnostics: {:?}",
            parse.diagnostics()
        ))
        .into());
    }

    Ok(lower_module(module, TEST_FILE, &parse.syntax())?)
}

fn declarations(functions: &[nexa_hir::Function]) -> Vec<(&str, Visibility)> {
    functions
        .iter()
        .map(|declaration| (declaration.name.text.as_str(), declaration.visibility))
        .collect()
}

fn record_declarations(records: &[nexa_hir::RecordDeclaration]) -> Vec<(&str, Visibility)> {
    records
        .iter()
        .map(|declaration| (declaration.name.text.as_str(), declaration.visibility))
        .collect()
}

fn union_declarations(unions: &[nexa_hir::UnionDeclaration]) -> Vec<(&str, Visibility)> {
    unions
        .iter()
        .map(|declaration| (declaration.name.text.as_str(), declaration.visibility))
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
