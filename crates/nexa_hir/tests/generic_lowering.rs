//! Generic HIR lowering and identity regression tests.

use nexa_hir::{
    lower_module, DefId, FunctionId, ModuleId, RecordId, TypeParameterId, TypeParameterOwner,
    TypeReference, TypeReferenceKind, UnionId,
};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan};

#[test]
fn lower_preserves_generic_declarations_and_nested_type_applications(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"export type Box<T> = { value: T; };
export type Result<T, E> = | Ok(value: T) | Err(error: E);
export function wrap<T, E>(value: Box<T[]>): Result<Box<T[]>, E> {
  return Result.Ok(value);
}"#;
    let program = lower_source(source)?;
    let record = program
        .records
        .first()
        .ok_or_else(|| std::io::Error::other("expected generic record"))?;
    let union = program
        .unions
        .first()
        .ok_or_else(|| std::io::Error::other("expected generic union"))?;
    let function = program
        .functions
        .first()
        .ok_or_else(|| std::io::Error::other("expected generic function"))?;

    assert_eq!(parameter_names(&record.type_parameters), ["T"]);
    assert_eq!(parameter_names(&union.type_parameters), ["T", "E"]);
    assert_eq!(parameter_names(&function.type_parameters), ["T", "E"]);
    assert_eq!(type_shape(&function.parameters[0].ty), "Box<T[]>");
    assert_eq!(type_shape(&function.return_type), "Result<Box<T[]>,E>");
    assert_eq!(
        function.return_type.span,
        exact_span(source, "Result<Box<T[]>, E>")?
    );

    Ok(())
}

#[test]
fn lower_keeps_non_generic_named_types_as_zero_argument_applications(
) -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "type Value = { value: Int; }; function read(value: Value): Value { return value; }";
    let program = lower_source(source)?;
    let function = program
        .functions
        .first()
        .ok_or_else(|| std::io::Error::other("expected function"))?;

    assert!(function.type_parameters.is_empty());
    assert_eq!(type_shape(&function.parameters[0].ty), "Value");
    assert_eq!(type_shape(&function.return_type), "Value");

    Ok(())
}

#[test]
fn type_parameter_ids_include_the_owner_kind_module_and_declaration() {
    let module = ModuleId::new(3);
    let function = FunctionId::in_module(module, 2);
    let record = RecordId::in_module(module, 2);
    let union = UnionId::in_module(module, 2);
    let function_parameter = TypeParameterId::new(TypeParameterOwner::Function(function), 1);
    let record_parameter = TypeParameterId::new(TypeParameterOwner::Record(record), 1);
    let union_parameter = TypeParameterId::new(TypeParameterOwner::Union(union), 1);

    assert_eq!(function_parameter.owner().module(), module);
    assert_eq!(
        function_parameter.owner().definition(),
        DefId::Function(function)
    );
    assert_eq!(function_parameter.index(), 1);
    assert_ne!(function_parameter, record_parameter);
    assert_ne!(function_parameter, union_parameter);
    assert_ne!(record_parameter, union_parameter);
}

fn lower_source(source: &str) -> Result<nexa_hir::Program, Box<dyn std::error::Error>> {
    let file = FileId::new(7);
    let parse = parse_source(file, source);
    if !parse.is_ok() {
        return Err(std::io::Error::other(format!(
            "unexpected parser diagnostics: {:?}",
            parse.diagnostics()
        ))
        .into());
    }
    lower_module(ModuleId::new(3), file, &parse.syntax()).map_err(Into::into)
}

fn parameter_names(parameters: &[nexa_hir::TypeParameter]) -> Vec<&str> {
    parameters
        .iter()
        .map(|parameter| parameter.name.text.as_str())
        .collect()
}

fn type_shape(reference: &TypeReference) -> String {
    match &reference.kind {
        TypeReferenceKind::Int => "Int".to_owned(),
        TypeReferenceKind::Bool => "Bool".to_owned(),
        TypeReferenceKind::String => "String".to_owned(),
        TypeReferenceKind::Unit => "Unit".to_owned(),
        TypeReferenceKind::Named { name, arguments } => {
            let arguments = arguments
                .iter()
                .map(type_shape)
                .collect::<Vec<_>>()
                .join(",");
            if arguments.is_empty() {
                name.text.clone()
            } else {
                format!("{}<{arguments}>", name.text)
            }
        }
        TypeReferenceKind::Array(element) => format!("{}[]", type_shape(element)),
        TypeReferenceKind::Function {
            parameters,
            return_type,
        } => {
            let parameters = parameters
                .iter()
                .map(|parameter| type_shape(&parameter.ty))
                .collect::<Vec<_>>()
                .join(",");
            format!("({parameters})=>{}", type_shape(return_type))
        }
    }
}

fn exact_span(source: &str, text: &str) -> Result<SourceSpan, std::io::Error> {
    let start = source
        .find(text)
        .ok_or_else(|| std::io::Error::other(format!("expected `{text}` in source")))?;
    Ok(SourceSpan::new(
        FileId::new(7),
        nexa_span::TextRange::new(start, start + text.len()),
    ))
}
