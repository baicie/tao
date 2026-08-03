//! Resolver-stage contracts independent of type checking.

use nexa_hir::{
    lower_module, resolve_modules, type_check_modules, DefId, FieldId, FunctionId, ModuleId,
    NameResolution, PayloadId, RecordId, ResolvedImport, ResolverScopeKind, TypeParameterId,
    TypeParameterOwner, UnionId, VariantId,
};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

#[test]
fn resolver_exposes_symbols_imports_and_lexical_scopes_before_type_checking(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { Item, make } from "./model.ft";
function main(value: Int): Unit {
  const first = value;
  if (true) {
    const shadow = first;
    print(shadow);
  }
  print(first);
}"#;
    let model = r#"export type Item = { value: Int; };
export function make(value: Int): Item { return { value: value }; }"#;
    let (programs, links) = lower_graph(&[entry, model], &[(0, 0, 1)])?;

    let resolution = resolve_modules(&programs, &links);

    assert!(
        resolution.diagnostics().is_empty(),
        "unexpected resolver diagnostics: {:?}",
        resolution.diagnostics()
    );
    assert_eq!(
        resolution
            .symbols()
            .iter()
            .map(|symbol| symbol.definition())
            .collect::<Vec<_>>(),
        [
            DefId::Function(FunctionId::in_module(ModuleId::ENTRY, 0)),
            DefId::Record(RecordId::in_module(ModuleId::new(1), 0)),
            DefId::Function(FunctionId::in_module(ModuleId::new(1), 0)),
        ]
    );

    let imported_item = occurrence_span(0, entry, "Item", 0)?;
    let imported_make = occurrence_span(0, entry, "make", 0)?;
    assert_eq!(
        resolution.name_resolution(imported_item),
        Some(NameResolution::Record(RecordId::in_module(
            ModuleId::new(1),
            0
        )))
    );
    assert_eq!(
        resolution.name_resolution(imported_make),
        Some(NameResolution::Function(FunctionId::in_module(
            ModuleId::new(1),
            0
        )))
    );

    let main = FunctionId::in_module(ModuleId::ENTRY, 0);
    let scopes = resolution
        .scopes()
        .iter()
        .filter(|scope| scope.owner() == main)
        .collect::<Vec<_>>();
    assert_eq!(
        scopes.iter().map(|scope| scope.kind()).collect::<Vec<_>>(),
        [ResolverScopeKind::Function, ResolverScopeKind::Block]
    );
    assert_eq!(scopes[0].parent(), None);
    assert_eq!(scopes[1].parent(), Some(scopes[0].id()));

    let value_declaration = occurrence_span(0, entry, "value", 0)?;
    let value_reference = occurrence_span(0, entry, "value", 1)?;
    let first_declaration = occurrence_span(0, entry, "first", 0)?;
    let first_nested_reference = occurrence_span(0, entry, "first", 1)?;
    let first_final_reference = occurrence_span(0, entry, "first", 2)?;
    assert_eq!(
        resolution.name_resolution(value_declaration),
        resolution.name_resolution(value_reference)
    );
    assert_eq!(
        resolution.name_resolution(first_declaration),
        resolution.name_resolution(first_nested_reference)
    );
    assert_eq!(
        resolution.name_resolution(first_declaration),
        resolution.name_resolution(first_final_reference)
    );

    Ok(())
}

#[test]
fn resolver_reports_import_visibility_duplicate_bindings_and_undefined_names(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { Hidden, Missing } from "./model.ft";
function main(value: Int, value: Int): Unit {
  const duplicate = 1;
  const duplicate = unknown;
}"#;
    let model = "type Hidden = { value: Int; };";
    let (programs, links) = lower_graph(&[entry, model], &[(0, 0, 1)])?;

    let resolution = resolve_modules(&programs, &links);
    let codes = resolution
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code().as_str())
        .collect::<Vec<_>>();

    assert_eq!(codes, ["E4004", "E4003", "E2002", "E2002", "E2001"]);
    assert!(resolution
        .name_resolution(exact_span(0, entry, "unknown")?)
        .is_none());

    Ok(())
}

#[test]
fn resolver_resolves_generic_types_union_variants_and_nested_scope_targets(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Option<T> = | Some(value: T) | None();
function wrap<T>(value: T): Option<T> {
  const make = (item: T): Option<T> => Option.Some(item);
  return make(value);
}
function read(value: Option<Int>): Int {
  return match (value) {
    case Option.Some(item) => item;
    case Option.None() => 0;
  };
}"#;
    let (programs, links) = lower_graph(&[source], &[])?;

    let resolution = resolve_modules(&programs, &links);

    assert!(
        resolution.diagnostics().is_empty(),
        "unexpected resolver diagnostics: {:?}",
        resolution.diagnostics()
    );
    let option = UnionId::in_module(ModuleId::ENTRY, 0);
    let some = VariantId::new(option, 0);
    let wrap = FunctionId::in_module(ModuleId::ENTRY, 0);
    let wrap_parameter = TypeParameterId::new(TypeParameterOwner::Function(wrap), 0);
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "T", 3)?),
        Some(NameResolution::TypeParameter(wrap_parameter))
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "Option", 2)?),
        Some(NameResolution::Union(option))
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "Some", 1)?),
        Some(NameResolution::Variant(some))
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "Some", 2)?),
        Some(NameResolution::Variant(some))
    );
    assert_eq!(
        resolution
            .scopes()
            .iter()
            .map(|scope| scope.kind())
            .collect::<Vec<_>>(),
        [
            ResolverScopeKind::Function,
            ResolverScopeKind::Closure,
            ResolverScopeKind::Function,
            ResolverScopeKind::MatchArm,
            ResolverScopeKind::MatchArm,
        ]
    );

    Ok(())
}

#[test]
fn resolver_keeps_local_member_calls_out_of_union_constructor_resolution(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function merge(values: Int[]): Int[] { return values.concat(values); }";
    let (programs, links) = lower_graph(&[source], &[])?;

    let resolution = resolve_modules(&programs, &links);

    assert!(
        resolution.diagnostics().is_empty(),
        "unexpected resolver diagnostics: {:?}",
        resolution.diagnostics()
    );
    let declaration = occurrence_span(0, source, "values", 0)?;
    assert_eq!(
        resolution.name_resolution(declaration),
        resolution.name_resolution(occurrence_span(0, source, "values", 1)?)
    );
    assert_eq!(
        resolution.name_resolution(declaration),
        resolution.name_resolution(occurrence_span(0, source, "values", 2)?)
    );

    Ok(())
}

#[test]
fn resolver_defers_unknown_union_members_to_type_checking() -> Result<(), Box<dyn std::error::Error>>
{
    let source = "type Choice = | Good(); function make(): Choice { return Choice.Missing(); }";
    let (programs, links) = lower_graph(&[source], &[])?;

    let resolution = resolve_modules(&programs, &links);
    let choice = UnionId::in_module(ModuleId::ENTRY, 0);

    assert!(
        resolution.diagnostics().is_empty(),
        "unexpected resolver diagnostics: {:?}",
        resolution.diagnostics()
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "Choice", 2)?),
        Some(NameResolution::Union(choice))
    );
    assert!(resolution
        .name_resolution(exact_span(0, source, "Missing")?)
        .is_none());

    Ok(())
}

#[test]
fn resolver_honors_type_parameter_shadowing_for_qualified_calls(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "type T = | One(); function make<T>(): Unit { T.Missing(); }";
    let (programs, links) = lower_graph(&[source], &[])?;

    let resolution = resolve_modules(&programs, &links);
    let parameter = TypeParameterId::new(
        TypeParameterOwner::Function(FunctionId::in_module(ModuleId::ENTRY, 0)),
        0,
    );

    assert!(
        resolution.diagnostics().is_empty(),
        "unexpected resolver diagnostics: {:?}",
        resolution.diagnostics()
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "T", 2)?),
        Some(NameResolution::TypeParameter(parameter))
    );

    Ok(())
}

#[test]
fn resolver_reports_duplicate_top_level_names_within_each_namespace(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function run(): Unit {}
function run(): Unit {}
type Box = {};
type Box = {};
type Choice = | One();
type Choice = | Two();"#;
    let (programs, links) = lower_graph(&[source], &[])?;

    let resolution = resolve_modules(&programs, &links);

    assert_eq!(
        resolution
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect::<Vec<_>>(),
        ["E2002", "E2002", "E2002"]
    );

    Ok(())
}

#[test]
fn resolver_records_record_and_union_declaration_name_targets(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Box<T> = { value: T; }; type Choice<T> = | Some(value: T);";
    let (programs, links) = lower_graph(&[source], &[])?;

    let resolution = resolve_modules(&programs, &links);
    let record = RecordId::in_module(ModuleId::ENTRY, 0);
    let union = UnionId::in_module(ModuleId::ENTRY, 0);
    let variant = VariantId::new(union, 0);

    assert!(
        resolution.diagnostics().is_empty(),
        "unexpected resolver diagnostics: {:?}",
        resolution.diagnostics()
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "T", 0)?),
        Some(NameResolution::TypeParameter(TypeParameterId::new(
            TypeParameterOwner::Record(record),
            0,
        )))
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "T", 1)?),
        Some(NameResolution::TypeParameter(TypeParameterId::new(
            TypeParameterOwner::Record(record),
            0,
        )))
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "value", 0)?),
        Some(NameResolution::Field(FieldId::new(record, 0)))
    );
    assert_eq!(
        resolution.name_resolution(exact_span(0, source, "Some")?),
        Some(NameResolution::Variant(variant))
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "value", 1)?),
        Some(NameResolution::Payload(PayloadId::new(variant, 0)))
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "T", 2)?),
        Some(NameResolution::TypeParameter(TypeParameterId::new(
            TypeParameterOwner::Union(union),
            0,
        )))
    );
    assert_eq!(
        resolution.name_resolution(occurrence_span(0, source, "T", 3)?),
        Some(NameResolution::TypeParameter(TypeParameterId::new(
            TypeParameterOwner::Union(union),
            0,
        )))
    );

    Ok(())
}

#[test]
fn resolver_emits_the_deterministic_cycle_witness_from_import_edges(
) -> Result<(), Box<dyn std::error::Error>> {
    let left = r#"import { right } from "./right.ft";
export function left(): Int { return right(); }"#;
    let right = r#"import { left } from "./left.ft";
export function right(): Int { return left(); }"#;
    let (programs, links) = lower_graph(&[left, right], &[(0, 0, 1), (1, 0, 0)])?;

    let resolution = resolve_modules(&programs, &links);
    let cycles = resolution
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == "E4002")
        .collect::<Vec<_>>();

    assert_eq!(cycles.len(), 1);
    assert_eq!(
        cycles[0]
            .labels()
            .iter()
            .map(|label| label.span())
            .collect::<Vec<_>>(),
        [
            programs[1].imports[0].path_span,
            programs[0].imports[0].path_span
        ]
    );

    Ok(())
}

#[test]
fn type_checking_consumes_one_resolver_result_without_duplicate_diagnostics(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = r#"import { Hidden, Missing } from "./model.ft";
function main(value: Int, value: Int): Unit { return unknown; }"#;
    let model = "type Hidden = { value: Int; };";
    let (programs, links) = lower_graph(&[entry, model], &[(0, 0, 1)])?;

    let analysis = type_check_modules(&programs, &links);

    assert_eq!(analysis.resolution(), &resolve_modules(&programs, &links));
    assert_eq!(
        analysis
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .filter(|code| matches!(*code, "E2001" | "E2002" | "E4003" | "E4004" | "E4005"))
            .collect::<Vec<_>>(),
        ["E4004", "E4003", "E2002", "E2001"]
    );

    Ok(())
}

#[test]
fn type_checking_receives_cycle_diagnostics_from_the_resolver_stage(
) -> Result<(), Box<dyn std::error::Error>> {
    let left = r#"import { right } from "./right.ft";
export function left(): Int { return right(); }"#;
    let right = r#"import { left } from "./left.ft";
export function right(): Int { return left(); }"#;
    let (programs, links) = lower_graph(&[left, right], &[(0, 0, 1), (1, 0, 0)])?;

    let analysis = type_check_modules(&programs, &links);

    assert_eq!(
        analysis
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code().as_str() == "E4002")
            .count(),
        1
    );
    Ok(())
}

fn lower_graph(
    sources: &[&str],
    links: &[(usize, usize, usize)],
) -> Result<(Vec<nexa_hir::Program>, Vec<ResolvedImport>), Box<dyn std::error::Error>> {
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
    let links = links
        .iter()
        .map(|(importer, declaration, target)| {
            let program = &programs[*importer];
            let import = &program.imports[*declaration];
            ResolvedImport::new(program.module, import.path_span, ModuleId::new(*target))
        })
        .collect();
    Ok((programs, links))
}

fn exact_span(file: u32, source: &str, text: &str) -> Result<SourceSpan, std::io::Error> {
    occurrence_span(file, source, text, 0)
}

fn occurrence_span(
    file: u32,
    source: &str,
    text: &str,
    occurrence: usize,
) -> Result<SourceSpan, std::io::Error> {
    let start = source
        .match_indices(text)
        .nth(occurrence)
        .map(|(start, _)| start)
        .ok_or_else(|| {
            std::io::Error::other(format!("expected occurrence {occurrence} of `{text}`"))
        })?;
    Ok(SourceSpan::new(
        FileId::new(file),
        TextRange::new(start, start + text.len()),
    ))
}
