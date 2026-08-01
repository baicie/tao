use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_hir::{ArrowBody, Builtin, Expression, NameResolution, Statement, TypedProgram};

const MUTATION_NOT_ALLOWED: DiagnosticCode = DiagnosticCode::new("E6201");
const LOOP_FORM_NOT_ALLOWED: DiagnosticCode = DiagnosticCode::new("E6202");
const HOST_EFFECT_NOT_ALLOWED: DiagnosticCode = DiagnosticCode::new("E6203");

pub(crate) fn lint(typed: &TypedProgram) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for module in typed.modules() {
        for function in &module.functions {
            lint_block(typed, &function.body, &mut diagnostics);
        }
    }
    diagnostics
}

fn lint_block(typed: &TypedProgram, block: &nexa_hir::Block, diagnostics: &mut Vec<Diagnostic>) {
    for statement in &block.statements {
        lint_statement(typed, statement, diagnostics);
    }
}

fn lint_statement(typed: &TypedProgram, statement: &Statement, diagnostics: &mut Vec<Diagnostic>) {
    match statement {
        Statement::Const(declaration) => {
            lint_expression(typed, &declaration.initializer, diagnostics);
        }
        Statement::Let(declaration) => {
            diagnostics.push(
                Diagnostic::error(
                    MUTATION_NOT_ALLOWED,
                    "mutable bindings are outside Futao Bootstrap Profile v1",
                )
                .with_label(Label::primary(
                    declaration.span,
                    "use an immutable `const` binding and return a new value",
                )),
            );
            lint_expression(typed, &declaration.initializer, diagnostics);
        }
        Statement::Assignment(assignment) => {
            diagnostics.push(
                Diagnostic::error(
                    MUTATION_NOT_ALLOWED,
                    "assignment is outside Futao Bootstrap Profile v1",
                )
                .with_label(Label::primary(
                    assignment.span,
                    "construct and return a new value instead",
                )),
            );
            lint_expression(typed, &assignment.value, diagnostics);
        }
        Statement::If(statement) => {
            lint_expression(typed, &statement.condition, diagnostics);
            lint_block(typed, &statement.then_branch, diagnostics);
            if let Some(alternative) = &statement.else_branch {
                lint_block(typed, alternative, diagnostics);
            }
        }
        Statement::While(statement) => {
            diagnostics.push(loop_diagnostic(
                statement.span,
                "`while` is outside Futao Bootstrap Profile v1",
            ));
            lint_expression(typed, &statement.condition, diagnostics);
            lint_block(typed, &statement.body, diagnostics);
        }
        Statement::ForOf(statement) => {
            lint_expression(typed, &statement.iterable, diagnostics);
            lint_block(typed, &statement.body, diagnostics);
        }
        Statement::Break(statement) => diagnostics.push(loop_diagnostic(
            statement.span,
            "`break` is outside Futao Bootstrap Profile v1",
        )),
        Statement::Continue(statement) => diagnostics.push(loop_diagnostic(
            statement.span,
            "`continue` is outside Futao Bootstrap Profile v1",
        )),
        Statement::Return(statement) => {
            if let Some(value) = &statement.value {
                lint_expression(typed, value, diagnostics);
            }
        }
        Statement::Expression(statement) => {
            lint_expression(typed, &statement.expression, diagnostics);
        }
    }
}

fn loop_diagnostic(span: nexa_span::SourceSpan, message: &'static str) -> Diagnostic {
    Diagnostic::error(LOOP_FORM_NOT_ALLOWED, message).with_label(Label::primary(
        span,
        "use bounded `for` iteration or explicit recursion",
    ))
}

fn lint_expression(
    typed: &TypedProgram,
    expression: &Expression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expression::Integer { .. } | Expression::Boolean { .. } | Expression::String { .. } => {}
        Expression::Array { elements, .. } => {
            for element in elements {
                lint_expression(typed, element, diagnostics);
            }
        }
        Expression::Record { fields, .. } => {
            for field in fields {
                lint_expression(typed, &field.value, diagnostics);
            }
        }
        Expression::Match {
            scrutinee, arms, ..
        } => {
            lint_expression(typed, scrutinee, diagnostics);
            for arm in arms {
                lint_expression(typed, &arm.value, diagnostics);
            }
        }
        Expression::Index {
            collection, index, ..
        } => {
            lint_expression(typed, collection, diagnostics);
            lint_expression(typed, index, diagnostics);
        }
        Expression::Member { object, .. } => {
            lint_expression(typed, object, diagnostics);
        }
        Expression::Name(name) => {
            if typed.name_resolution(name.span) == Some(NameResolution::Builtin(Builtin::Print)) {
                diagnostics.push(
                    Diagnostic::error(
                        HOST_EFFECT_NOT_ALLOWED,
                        "ambient output is outside Futao Bootstrap Profile v1",
                    )
                    .with_label(Label::primary(
                        name.span,
                        "return structured data to the Host shell instead",
                    )),
                );
            }
        }
        Expression::Arrow { body, .. } => match body {
            ArrowBody::Expression(expression) => {
                lint_expression(typed, expression, diagnostics);
            }
            ArrowBody::Block(block) => lint_block(typed, block, diagnostics),
        },
        Expression::Unary { expression, .. } | Expression::Parenthesized { expression, .. } => {
            lint_expression(typed, expression, diagnostics);
        }
        Expression::Binary { left, right, .. } => {
            lint_expression(typed, left, diagnostics);
            lint_expression(typed, right, diagnostics);
        }
        Expression::Call {
            callee, arguments, ..
        } => {
            lint_expression(typed, callee, diagnostics);
            for argument in arguments {
                lint_expression(typed, argument, diagnostics);
            }
        }
    }
}
