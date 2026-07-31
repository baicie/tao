//! Function-value and `for...of` HIR lowering regression tests.

use nexa_hir::{
    lower_module, ArrowBody, ClosureId, Expression, FunctionId, ModuleId, Statement,
    TypeReferenceKind,
};
use nexa_parser::parse_source;
use nexa_span::{FileId, SourceSpan, TextRange};

const FILE: FileId = FileId::new(11);
const MODULE: ModuleId = ModuleId::new(4);

#[test]
fn lower_preserves_named_function_type_parameters_and_spans(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function use(callback: (input: Int, enabled: Bool) => String): Unit {}";
    let program = lower_source(source)?;
    let function = program
        .functions
        .first()
        .ok_or_else(|| std::io::Error::other("expected function"))?;
    let callback = function
        .parameters
        .first()
        .ok_or_else(|| std::io::Error::other("expected callback parameter"))?;
    let TypeReferenceKind::Function {
        parameters,
        return_type,
    } = &callback.ty.kind
    else {
        return Err(std::io::Error::other("expected function type").into());
    };

    assert_eq!(
        (
            parameters
                .iter()
                .map(|parameter| parameter.name.text.as_str())
                .collect::<Vec<_>>(),
            parameters
                .iter()
                .map(|parameter| &parameter.ty.kind)
                .collect::<Vec<_>>(),
            &return_type.kind,
            callback.ty.span,
        ),
        (
            vec!["input", "enabled"],
            vec![&TypeReferenceKind::Int, &TypeReferenceKind::Bool],
            &TypeReferenceKind::String,
            exact_span(source, "(input: Int, enabled: Bool) => String")?,
        )
    );

    Ok(())
}

#[test]
fn lower_assigns_nested_closure_ids_in_source_preorder() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit {
  const first = (value: Int): Int => value;
  const outer = (value: Int): Int => {
    const nested = (inner: Int): Int => inner;
    return value;
  };
}"#;
    let program = lower_source(source)?;
    let statements = &program
        .functions
        .first()
        .ok_or_else(|| std::io::Error::other("expected main function"))?
        .body
        .statements;
    let first = const_initializer(statements.first())?;
    let outer = const_initializer(statements.get(1))?;
    let (
        Expression::Arrow {
            closure: first_id,
            body: ArrowBody::Expression(first_body),
            span: first_span,
            ..
        },
        Expression::Arrow {
            closure: outer_id,
            body: ArrowBody::Block(outer_body),
            ..
        },
    ) = (first, outer)
    else {
        return Err(std::io::Error::other("expected expression and block arrows").into());
    };
    let nested = const_initializer(outer_body.statements.first())?;
    let Expression::Arrow {
        closure: nested_id, ..
    } = nested
    else {
        return Err(std::io::Error::other("expected nested arrow").into());
    };
    let owner = FunctionId::in_module(MODULE, 0);

    assert_eq!(
        (
            *first_id,
            *outer_id,
            *nested_id,
            *first_span,
            first_body.span(),
        ),
        (
            ClosureId::new(owner, 0),
            ClosureId::new(owner, 1),
            ClosureId::new(owner, 2),
            exact_span(source, "(value: Int): Int => value")?,
            exact_span(source, "value;")?.with_range_width("value".len()),
        )
    );

    Ok(())
}

#[test]
fn lower_preserves_for_of_binding_iterable_body_and_span() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"function visit(values: Int[]): Unit {
  for (const value of values) {
    print(value);
  }
}"#;
    let program = lower_source(source)?;
    let statement = program
        .functions
        .first()
        .and_then(|function| function.body.statements.first())
        .ok_or_else(|| std::io::Error::other("expected for statement"))?;
    let Statement::ForOf(statement) = statement else {
        return Err(std::io::Error::other("expected for-of HIR").into());
    };
    let Expression::Name(iterable) = &statement.iterable else {
        return Err(std::io::Error::other("expected named iterable").into());
    };

    assert_eq!(
        (
            statement.binding.text.as_str(),
            statement.binding.span,
            iterable.text.as_str(),
            iterable.span,
            statement.body.statements.len(),
            statement.span,
        ),
        (
            "value",
            nth_span(source, "value", 1)?,
            "values",
            nth_span(source, "values", 1)?,
            1,
            exact_span(
                source,
                "for (const value of values) {\n    print(value);\n  }",
            )?,
        )
    );

    Ok(())
}

fn lower_source(source: &str) -> Result<nexa_hir::Program, Box<dyn std::error::Error>> {
    let parse = parse_source(FILE, source);
    if !parse.is_ok() {
        return Err(std::io::Error::other(format!(
            "unexpected parser diagnostics: {:?}",
            parse.diagnostics()
        ))
        .into());
    }
    lower_module(MODULE, FILE, &parse.syntax()).map_err(Into::into)
}

fn const_initializer(
    statement: Option<&Statement>,
) -> Result<&Expression, Box<dyn std::error::Error>> {
    let Some(Statement::Const(declaration)) = statement else {
        return Err(std::io::Error::other("expected const declaration").into());
    };
    Ok(&declaration.initializer)
}

fn exact_span(source: &str, text: &str) -> Result<SourceSpan, std::io::Error> {
    nth_span(source, text, 0)
}

fn nth_span(source: &str, text: &str, occurrence: usize) -> Result<SourceSpan, std::io::Error> {
    let start = source
        .match_indices(text)
        .nth(occurrence)
        .map(|(start, _)| start)
        .ok_or_else(|| {
            std::io::Error::other(format!("expected occurrence {occurrence} of `{text}`"))
        })?;
    Ok(SourceSpan::new(
        FILE,
        TextRange::new(start, start + text.len()),
    ))
}

trait SourceSpanTestExt {
    fn with_range_width(self, width: usize) -> Self;
}

impl SourceSpanTestExt for SourceSpan {
    fn with_range_width(self, width: usize) -> Self {
        Self::new(
            self.file(),
            TextRange::new(self.range().start(), self.range().start() + width),
        )
    }
}
