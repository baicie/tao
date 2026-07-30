use std::fmt::{Display, Formatter};

use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::{FileId, SourceSpan, TextRange};
use nexa_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::{
    AssignmentStatement, BinaryOperator, Block, BreakStatement, ConstDeclaration,
    ContinueStatement, Expression, ExpressionStatement, Function, IfStatement, ImportDeclaration,
    LetDeclaration, MatchArm, MatchPattern, ModuleId, Name, Parameter, Program, RecordDeclaration,
    RecordFieldDeclaration, RecordFieldInitializer, ReturnStatement, Statement, TypeParameter,
    TypeReference, TypeReferenceKind, UnaryOperator, UnionDeclaration, UnionVariantDeclaration,
    VariantPayloadDeclaration, Visibility, WhileStatement,
};

const SYNTAX_ERROR: DiagnosticCode = DiagnosticCode::new("E1001");
const TYPE_ERROR: DiagnosticCode = DiagnosticCode::new("E3001");

/// A CST shape that cannot be lowered into valid Nexa HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweringError {
    kind: LoweringErrorKind,
    span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweringErrorKind {
    MalformedSyntax,
    IntegerOutOfRange,
}

impl LoweringError {
    /// Returns the source range that prevented lowering.
    #[must_use]
    pub const fn span(self) -> SourceSpan {
        self.span
    }

    /// Converts the lowering error into a user-facing compiler diagnostic.
    #[must_use]
    pub fn diagnostic(self) -> Diagnostic {
        let (code, message) = match self.kind {
            LoweringErrorKind::MalformedSyntax => {
                (SYNTAX_ERROR, "could not lower malformed syntax")
            }
            LoweringErrorKind::IntegerOutOfRange => {
                (TYPE_ERROR, "integer literal is out of range for `Int`")
            }
        };

        Diagnostic::error(code, message).with_label(Label::new(self.span, message))
    }
}

impl Display for LoweringError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.kind {
            LoweringErrorKind::MalformedSyntax => "could not lower malformed syntax",
            LoweringErrorKind::IntegerOutOfRange => "integer literal is out of range for `Int`",
        })
    }
}

impl std::error::Error for LoweringError {}

/// Lowers a syntax-valid source file into high-level intermediate representation.
///
/// # Errors
///
/// Returns [`LoweringError`] when the supplied CST is malformed or an integer
/// literal does not fit Nexa's signed 64-bit `Int` type.
pub fn lower(file: FileId, syntax: &SyntaxNode) -> Result<Program, LoweringError> {
    lower_module(ModuleId::ENTRY, file, syntax)
}

/// Lowers one syntax-valid source file under its compiler-session module identity.
///
/// # Errors
///
/// Returns [`LoweringError`] when the supplied CST is malformed or an integer
/// literal does not fit Nexa's signed 64-bit `Int` type.
pub fn lower_module(
    module: ModuleId,
    file: FileId,
    syntax: &SyntaxNode,
) -> Result<Program, LoweringError> {
    if syntax.kind() != SyntaxKind::SourceFile {
        return Err(malformed(node_span(file, syntax)));
    }

    let mut imports = Vec::new();
    let mut records = Vec::new();
    let mut unions = Vec::new();
    let mut functions = Vec::new();

    for item in syntax.children() {
        let (declaration, visibility) = match item.kind() {
            SyntaxKind::ImportDeclaration => {
                imports.push(lower_import(file, &item)?);
                continue;
            }
            SyntaxKind::FunctionDeclaration
            | SyntaxKind::RecordDeclaration
            | SyntaxKind::UnionDeclaration => (item, Visibility::Private),
            SyntaxKind::ExportedDeclaration => (
                exported_declaration_child(file, &item)?,
                Visibility::Exported,
            ),
            _ => return Err(malformed(node_span(file, &item))),
        };

        match declaration.kind() {
            SyntaxKind::FunctionDeclaration => {
                functions.push(lower_function(file, &declaration, visibility)?);
            }
            SyntaxKind::RecordDeclaration => {
                records.push(lower_record(file, &declaration, visibility)?);
            }
            SyntaxKind::UnionDeclaration => {
                unions.push(lower_union(file, &declaration, visibility)?);
            }
            _ => return Err(malformed(node_span(file, &declaration))),
        }
    }

    Ok(Program {
        module,
        imports,
        records,
        unions,
        functions,
        span: node_span(file, syntax),
    })
}

fn lower_import(file: FileId, node: &SyntaxNode) -> Result<ImportDeclaration, LoweringError> {
    const EXPECTED_TOKENS: [SyntaxKind; 6] = [
        SyntaxKind::ImportKw,
        SyntaxKind::LBrace,
        SyntaxKind::RBrace,
        SyntaxKind::FromKw,
        SyntaxKind::String,
        SyntaxKind::Semicolon,
    ];

    let direct_tokens = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect::<Vec<_>>();
    if !direct_tokens
        .iter()
        .map(SyntaxToken::kind)
        .eq(EXPECTED_TOKENS)
    {
        return Err(malformed(node_span(file, node)));
    }

    let mut children = node.children();
    let list = children
        .next()
        .filter(|child| child.kind() == SyntaxKind::ImportList)
        .ok_or_else(|| malformed(node_span(file, node)))?;
    if children.next().is_some() {
        return Err(malformed(node_span(file, node)));
    }

    let names = lower_import_names(file, &list)?;
    let path_token = required_direct_token(file, node, SyntaxKind::String)?;
    let path_span = token_span(file, &path_token);
    let path = decode_string_token(file, &path_token)?;

    Ok(ImportDeclaration {
        names,
        path,
        path_span,
        span: node_span(file, node),
    })
}

fn lower_import_names(file: FileId, node: &SyntaxNode) -> Result<Vec<Name>, LoweringError> {
    let mut names = Vec::new();
    let mut expects_name = true;

    for element in node.children_with_tokens() {
        let Some(token) = element.into_token() else {
            return Err(malformed(node_span(file, node)));
        };
        if token.kind().is_trivia() {
            continue;
        }

        if expects_name && token.kind() == SyntaxKind::Ident {
            names.push(Name {
                text: token.text().to_owned(),
                span: token_span(file, &token),
            });
            expects_name = false;
        } else if !expects_name && token.kind() == SyntaxKind::Comma {
            expects_name = true;
        } else {
            return Err(malformed(token_span(file, &token)));
        }
    }

    if names.is_empty() || expects_name {
        return Err(malformed(node_span(file, node)));
    }

    Ok(names)
}

fn exported_declaration_child(
    file: FileId,
    node: &SyntaxNode,
) -> Result<SyntaxNode, LoweringError> {
    let mut direct_tokens = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia());
    if !direct_tokens
        .next()
        .is_some_and(|token| token.kind() == SyntaxKind::ExportKw)
        || direct_tokens.next().is_some()
    {
        return Err(malformed(node_span(file, node)));
    }

    let mut children = node.children();
    let declaration = children
        .next()
        .filter(|child| {
            matches!(
                child.kind(),
                SyntaxKind::FunctionDeclaration
                    | SyntaxKind::RecordDeclaration
                    | SyntaxKind::UnionDeclaration
            )
        })
        .ok_or_else(|| malformed(node_span(file, node)))?;
    if children.next().is_some() {
        return Err(malformed(node_span(file, node)));
    }

    Ok(declaration)
}

fn lower_union(
    file: FileId,
    node: &SyntaxNode,
    visibility: Visibility,
) -> Result<UnionDeclaration, LoweringError> {
    let variants = node
        .children()
        .filter(|variant| variant.kind() == SyntaxKind::UnionVariant)
        .map(|variant| lower_union_variant(file, &variant))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(UnionDeclaration {
        name: lower_direct_name(file, node)?,
        type_parameters: lower_type_parameters(file, node)?,
        visibility,
        variants,
        span: node_span(file, node),
    })
}

fn lower_union_variant(
    file: FileId,
    node: &SyntaxNode,
) -> Result<UnionVariantDeclaration, LoweringError> {
    let payload = required_child(file, node, SyntaxKind::VariantPayload)?;
    let names = payload
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::Ident)
        .collect::<Vec<_>>();
    let types = payload
        .children()
        .filter(|child| matches!(child.kind(), SyntaxKind::Type | SyntaxKind::ArrayType))
        .collect::<Vec<_>>();
    if names.len() != types.len() {
        return Err(malformed(node_span(file, &payload)));
    }
    let payloads = names
        .into_iter()
        .zip(types)
        .map(|(name, ty)| {
            let name = Name {
                text: name.text().to_owned(),
                span: token_span(file, &name),
            };
            let ty = lower_type(file, &ty)?;
            let span = SourceSpan::new(
                file,
                TextRange::new(name.span.range().start(), ty.span.range().end()),
            );
            Ok(VariantPayloadDeclaration { name, ty, span })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;

    Ok(UnionVariantDeclaration {
        name: lower_direct_name(file, node)?,
        payloads,
        span: node_span(file, node),
    })
}

fn lower_record(
    file: FileId,
    node: &SyntaxNode,
    visibility: Visibility,
) -> Result<RecordDeclaration, LoweringError> {
    let body = required_child(file, node, SyntaxKind::RecordBody)?;
    let fields = body
        .children()
        .filter(|field| field.kind() == SyntaxKind::RecordFieldDeclaration)
        .map(|field| lower_record_field(file, &field))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(RecordDeclaration {
        name: lower_direct_name(file, node)?,
        type_parameters: lower_type_parameters(file, node)?,
        visibility,
        fields,
        span: node_span(file, node),
    })
}

fn lower_record_field(
    file: FileId,
    node: &SyntaxNode,
) -> Result<RecordFieldDeclaration, LoweringError> {
    Ok(RecordFieldDeclaration {
        name: lower_direct_name(file, node)?,
        ty: lower_type(file, &required_type_child(file, node)?)?,
        span: node_span(file, node),
    })
}

fn lower_function(
    file: FileId,
    node: &SyntaxNode,
    visibility: Visibility,
) -> Result<Function, LoweringError> {
    let name = lower_direct_name(file, node)?;
    let parameters = child(node, SyntaxKind::ParameterList)
        .map(|parameters| {
            parameters
                .children()
                .filter(|parameter| parameter.kind() == SyntaxKind::Parameter)
                .map(|parameter| lower_parameter(file, &parameter))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let return_type = lower_type(file, &required_type_child(file, node)?)?;
    let body = lower_block(file, &required_child(file, node, SyntaxKind::Block)?)?;

    Ok(Function {
        name,
        type_parameters: lower_type_parameters(file, node)?,
        visibility,
        parameters,
        return_type,
        body,
        span: node_span(file, node),
    })
}

fn lower_parameter(file: FileId, node: &SyntaxNode) -> Result<Parameter, LoweringError> {
    Ok(Parameter {
        name: lower_direct_name(file, node)?,
        ty: lower_type(file, &required_type_child(file, node)?)?,
        span: node_span(file, node),
    })
}

fn lower_type_parameters(
    file: FileId,
    declaration: &SyntaxNode,
) -> Result<Vec<TypeParameter>, LoweringError> {
    let Some(list) = child(declaration, SyntaxKind::TypeParameterList) else {
        return Ok(Vec::new());
    };
    let parameters = list
        .children()
        .map(|parameter| {
            if parameter.kind() != SyntaxKind::TypeParameter {
                return Err(malformed(node_span(file, &parameter)));
            }
            Ok(TypeParameter {
                name: lower_direct_name(file, &parameter)?,
                span: node_span(file, &parameter),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_angle_list(file, &list, parameters.len())?;
    Ok(parameters)
}

fn lower_type(file: FileId, node: &SyntaxNode) -> Result<TypeReference, LoweringError> {
    if node.kind() == SyntaxKind::ArrayType {
        let element = lower_type(file, &required_type_child(file, node)?)?;
        return Ok(TypeReference {
            kind: TypeReferenceKind::Array(Box::new(element)),
            span: node_span(file, node),
        });
    }
    if node.kind() != SyntaxKind::Type {
        return Err(malformed(node_span(file, node)));
    }

    let direct_tokens = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect::<Vec<_>>();
    let [token] = direct_tokens.as_slice() else {
        return Err(malformed(node_span(file, node)));
    };
    let mut children = node.children();
    let argument_list = children.next();
    if children.next().is_some()
        || argument_list
            .as_ref()
            .is_some_and(|child| child.kind() != SyntaxKind::TypeArgumentList)
    {
        return Err(malformed(node_span(file, node)));
    }
    let kind = match token.kind() {
        SyntaxKind::IntKw if argument_list.is_none() => TypeReferenceKind::Int,
        SyntaxKind::BoolKw if argument_list.is_none() => TypeReferenceKind::Bool,
        SyntaxKind::StringKw if argument_list.is_none() => TypeReferenceKind::String,
        SyntaxKind::UnitKw if argument_list.is_none() => TypeReferenceKind::Unit,
        SyntaxKind::Ident => TypeReferenceKind::Named {
            name: Name {
                text: token.text().to_owned(),
                span: token_span(file, token),
            },
            arguments: argument_list
                .as_ref()
                .map(|list| lower_type_arguments(file, list))
                .transpose()?
                .unwrap_or_default(),
        },
        _ => return Err(malformed(token_span(file, token))),
    };

    Ok(TypeReference {
        kind,
        span: node_span(file, node),
    })
}

fn lower_type_arguments(
    file: FileId,
    list: &SyntaxNode,
) -> Result<Vec<TypeReference>, LoweringError> {
    let arguments = list
        .children()
        .map(|argument| {
            if !matches!(argument.kind(), SyntaxKind::Type | SyntaxKind::ArrayType) {
                return Err(malformed(node_span(file, &argument)));
            }
            lower_type(file, &argument)
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_angle_list(file, list, arguments.len())?;
    Ok(arguments)
}

fn validate_angle_list(
    file: FileId,
    list: &SyntaxNode,
    item_count: usize,
) -> Result<(), LoweringError> {
    if item_count == 0 {
        return Err(malformed(node_span(file, list)));
    }

    let tokens = list
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect::<Vec<_>>();
    let mut expected = Vec::with_capacity(item_count + 1);
    expected.push(SyntaxKind::Lt);
    for index in 0..item_count {
        if index != 0 {
            expected.push(SyntaxKind::Comma);
        }
    }
    expected.push(SyntaxKind::Gt);

    if tokens != expected {
        return Err(malformed(node_span(file, list)));
    }
    Ok(())
}

fn lower_block(file: FileId, node: &SyntaxNode) -> Result<Block, LoweringError> {
    let statements = node
        .children()
        .filter(|statement| is_statement_kind(statement.kind()))
        .map(|statement| lower_statement(file, &statement))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Block {
        statements,
        span: node_span(file, node),
    })
}

fn lower_statement(file: FileId, node: &SyntaxNode) -> Result<Statement, LoweringError> {
    match node.kind() {
        SyntaxKind::ConstDeclaration => Ok(Statement::Const(lower_const(file, node)?)),
        SyntaxKind::LetDeclaration => Ok(Statement::Let(lower_let(file, node)?)),
        SyntaxKind::AssignmentStatement => Ok(Statement::Assignment(lower_assignment(file, node)?)),
        SyntaxKind::IfStatement => Ok(Statement::If(lower_if(file, node)?)),
        SyntaxKind::WhileStatement => Ok(Statement::While(lower_while(file, node)?)),
        SyntaxKind::BreakStatement => Ok(Statement::Break(BreakStatement {
            span: node_span(file, node),
        })),
        SyntaxKind::ContinueStatement => Ok(Statement::Continue(ContinueStatement {
            span: node_span(file, node),
        })),
        SyntaxKind::ReturnStatement => Ok(Statement::Return(lower_return(file, node)?)),
        SyntaxKind::ExpressionStatement => Ok(Statement::Expression(lower_expression_statement(
            file, node,
        )?)),
        _ => Err(malformed(node_span(file, node))),
    }
}

fn lower_let(file: FileId, node: &SyntaxNode) -> Result<LetDeclaration, LoweringError> {
    let annotation = type_child(node)
        .map(|ty| lower_type(file, &ty))
        .transpose()?;

    Ok(LetDeclaration {
        name: lower_direct_name(file, node)?,
        annotation,
        initializer: lower_expression(file, &required_expression_child(file, node)?)?,
        span: node_span(file, node),
    })
}

fn lower_assignment(file: FileId, node: &SyntaxNode) -> Result<AssignmentStatement, LoweringError> {
    Ok(AssignmentStatement {
        target: lower_direct_name(file, node)?,
        value: lower_expression(file, &required_expression_child(file, node)?)?,
        span: node_span(file, node),
    })
}

fn lower_const(file: FileId, node: &SyntaxNode) -> Result<ConstDeclaration, LoweringError> {
    let annotation = type_child(node)
        .map(|ty| lower_type(file, &ty))
        .transpose()?;

    Ok(ConstDeclaration {
        name: lower_direct_name(file, node)?,
        annotation,
        initializer: lower_expression(file, &required_expression_child(file, node)?)?,
        span: node_span(file, node),
    })
}

fn lower_if(file: FileId, node: &SyntaxNode) -> Result<IfStatement, LoweringError> {
    let condition = lower_expression(file, &required_expression_child(file, node)?)?;
    let then_branch = lower_block(file, &required_child(file, node, SyntaxKind::Block)?)?;
    let else_branch = child(node, SyntaxKind::ElseClause)
        .map(|else_clause| {
            lower_block(
                file,
                &required_child(file, &else_clause, SyntaxKind::Block)?,
            )
        })
        .transpose()?;

    Ok(IfStatement {
        condition,
        then_branch,
        else_branch,
        span: node_span(file, node),
    })
}

fn lower_while(file: FileId, node: &SyntaxNode) -> Result<WhileStatement, LoweringError> {
    Ok(WhileStatement {
        condition: lower_expression(file, &required_expression_child(file, node)?)?,
        body: lower_block(file, &required_child(file, node, SyntaxKind::Block)?)?,
        span: node_span(file, node),
    })
}

fn lower_return(file: FileId, node: &SyntaxNode) -> Result<ReturnStatement, LoweringError> {
    let value = expression_child(node)
        .map(|value| lower_expression(file, &value))
        .transpose()?;

    Ok(ReturnStatement {
        value,
        span: node_span(file, node),
    })
}

fn lower_expression_statement(
    file: FileId,
    node: &SyntaxNode,
) -> Result<ExpressionStatement, LoweringError> {
    Ok(ExpressionStatement {
        expression: lower_expression(file, &required_expression_child(file, node)?)?,
        span: node_span(file, node),
    })
}

fn lower_expression(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    match node.kind() {
        SyntaxKind::IntLiteral => lower_integer(file, node),
        SyntaxKind::BoolLiteral => lower_boolean(file, node),
        SyntaxKind::StringLiteral => lower_string(file, node),
        SyntaxKind::ArrayExpression => lower_array(file, node),
        SyntaxKind::RecordExpression => lower_record_expression(file, node),
        SyntaxKind::MatchExpression => lower_match(file, node),
        SyntaxKind::IndexExpression => lower_index(file, node),
        SyntaxKind::MemberExpression => lower_member(file, node),
        SyntaxKind::NameReference => Ok(Expression::Name(lower_direct_name(file, node)?)),
        SyntaxKind::UnaryExpression => lower_unary(file, node),
        SyntaxKind::BinaryExpression => lower_binary(file, node),
        SyntaxKind::CallExpression => lower_call(file, node),
        SyntaxKind::ParenthesizedExpression => lower_parenthesized(file, node),
        _ => Err(malformed(node_span(file, node))),
    }
}

fn lower_match(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let scrutinee = lower_expression(file, &required_expression_child(file, node)?)?;
    let arms = node
        .children()
        .filter(|arm| arm.kind() == SyntaxKind::MatchArm)
        .map(|arm| lower_match_arm(file, &arm))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Expression::Match {
        scrutinee: Box::new(scrutinee),
        arms,
        span: node_span(file, node),
    })
}

fn lower_match_arm(file: FileId, node: &SyntaxNode) -> Result<MatchArm, LoweringError> {
    let pattern = if let Some(pattern) = child(node, SyntaxKind::VariantPattern) {
        lower_variant_pattern(file, &pattern)?
    } else {
        let token = required_direct_token(file, node, SyntaxKind::DefaultKw)?;
        MatchPattern::Default {
            span: token_span(file, &token),
        }
    };

    Ok(MatchArm {
        pattern,
        value: lower_expression(file, &required_expression_child(file, node)?)?,
        span: node_span(file, node),
    })
}

fn lower_variant_pattern(file: FileId, node: &SyntaxNode) -> Result<MatchPattern, LoweringError> {
    let names = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::Ident)
        .collect::<Vec<_>>();
    let [qualifier, variant] = names.as_slice() else {
        return Err(malformed(node_span(file, node)));
    };
    let bindings = required_child(file, node, SyntaxKind::PatternBindingList)?
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| Name {
            text: token.text().to_owned(),
            span: token_span(file, &token),
        })
        .collect();

    Ok(MatchPattern::Variant {
        union: Name {
            text: qualifier.text().to_string(),
            span: token_span(file, qualifier),
        },
        variant: Name {
            text: variant.text().to_string(),
            span: token_span(file, variant),
        },
        bindings,
        span: node_span(file, node),
    })
}

fn lower_integer(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let token = required_direct_token(file, node, SyntaxKind::Int)?;
    let span = token_span(file, &token);
    let value = token.text().parse::<i64>().map_err(|_| LoweringError {
        kind: LoweringErrorKind::IntegerOutOfRange,
        span,
    })?;

    Ok(Expression::Integer { value, span })
}

fn lower_boolean(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let token = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| matches!(token.kind(), SyntaxKind::TrueKw | SyntaxKind::FalseKw))
        .ok_or_else(|| malformed(node_span(file, node)))?;

    Ok(Expression::Boolean {
        value: token.kind() == SyntaxKind::TrueKw,
        span: token_span(file, &token),
    })
}

fn lower_string(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let token = required_direct_token(file, node, SyntaxKind::String)?;
    let span = token_span(file, &token);
    let value = decode_string_token(file, &token)?;

    Ok(Expression::String { value, span })
}

fn decode_string_token(file: FileId, token: &SyntaxToken) -> Result<String, LoweringError> {
    let span = token_span(file, token);
    let text = token.text();
    let contents = text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .ok_or_else(|| malformed(span))?;
    let mut value = String::with_capacity(contents.len());
    let mut characters = contents.chars();

    while let Some(character) = characters.next() {
        if character != '\\' {
            value.push(character);
            continue;
        }

        let escaped = characters.next().ok_or_else(|| malformed(span))?;
        value.push(match escaped {
            '"' => '"',
            '\\' => '\\',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            _ => return Err(malformed(span)),
        });
    }

    Ok(value)
}

fn lower_array(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let elements = expression_children(node)
        .map(|element| lower_expression(file, &element))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Expression::Array {
        elements,
        span: node_span(file, node),
    })
}

fn lower_record_expression(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let fields = node
        .children()
        .filter(|field| field.kind() == SyntaxKind::RecordFieldInitializer)
        .map(|field| lower_record_initializer(file, &field))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Expression::Record {
        fields,
        span: node_span(file, node),
    })
}

fn lower_record_initializer(
    file: FileId,
    node: &SyntaxNode,
) -> Result<RecordFieldInitializer, LoweringError> {
    Ok(RecordFieldInitializer {
        name: lower_direct_name(file, node)?,
        value: lower_expression(file, &required_expression_child(file, node)?)?,
        span: node_span(file, node),
    })
}

fn lower_index(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let mut expressions = expression_children(node);
    let collection = expressions
        .next()
        .ok_or_else(|| malformed(node_span(file, node)))?;
    let index = expressions
        .next()
        .ok_or_else(|| malformed(node_span(file, node)))?;

    Ok(Expression::Index {
        collection: Box::new(lower_expression(file, &collection)?),
        index: Box::new(lower_expression(file, &index)?),
        span: node_span(file, node),
    })
}

fn lower_member(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    Ok(Expression::Member {
        object: Box::new(lower_expression(
            file,
            &required_expression_child(file, node)?,
        )?),
        member: lower_direct_name(file, node)?,
        span: node_span(file, node),
    })
}

fn lower_unary(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let token = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| matches!(token.kind(), SyntaxKind::Bang | SyntaxKind::Minus))
        .ok_or_else(|| malformed(node_span(file, node)))?;
    let operator = match token.kind() {
        SyntaxKind::Bang => UnaryOperator::Not,
        SyntaxKind::Minus => UnaryOperator::Negate,
        _ => return Err(malformed(token_span(file, &token))),
    };

    Ok(Expression::Unary {
        operator,
        expression: Box::new(lower_expression(
            file,
            &required_expression_child(file, node)?,
        )?),
        span: node_span(file, node),
    })
}

fn lower_binary(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let operator = binary_operator(node).ok_or_else(|| malformed(node_span(file, node)))?;
    let mut expressions = expression_children(node);
    let left = expressions
        .next()
        .ok_or_else(|| malformed(node_span(file, node)))?;
    let right = expressions
        .next()
        .ok_or_else(|| malformed(node_span(file, node)))?;

    Ok(Expression::Binary {
        operator,
        left: Box::new(lower_expression(file, &left)?),
        right: Box::new(lower_expression(file, &right)?),
        span: node_span(file, node),
    })
}

fn lower_call(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    let callee = required_expression_child(file, node)?;
    let arguments = child(node, SyntaxKind::ArgumentList)
        .map(|arguments| {
            expression_children(&arguments)
                .map(|argument| lower_expression(file, &argument))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(Expression::Call {
        callee: Box::new(lower_expression(file, &callee)?),
        arguments,
        span: node_span(file, node),
    })
}

fn lower_parenthesized(file: FileId, node: &SyntaxNode) -> Result<Expression, LoweringError> {
    Ok(Expression::Parenthesized {
        expression: Box::new(lower_expression(
            file,
            &required_expression_child(file, node)?,
        )?),
        span: node_span(file, node),
    })
}

fn lower_direct_name(file: FileId, node: &SyntaxNode) -> Result<Name, LoweringError> {
    let token = required_direct_token(file, node, SyntaxKind::Ident)?;

    Ok(Name {
        text: token.text().to_owned(),
        span: token_span(file, &token),
    })
}

fn binary_operator(node: &SyntaxNode) -> Option<BinaryOperator> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find_map(|token| match token.kind() {
            SyntaxKind::Plus => Some(BinaryOperator::Add),
            SyntaxKind::Minus => Some(BinaryOperator::Subtract),
            SyntaxKind::Star => Some(BinaryOperator::Multiply),
            SyntaxKind::Slash => Some(BinaryOperator::Divide),
            SyntaxKind::EqEqEq => Some(BinaryOperator::Equal),
            SyntaxKind::Lt => Some(BinaryOperator::Less),
            SyntaxKind::LtEq => Some(BinaryOperator::LessEqual),
            SyntaxKind::Gt => Some(BinaryOperator::Greater),
            SyntaxKind::GtEq => Some(BinaryOperator::GreaterEqual),
            SyntaxKind::AmpAmp => Some(BinaryOperator::LogicalAnd),
            SyntaxKind::PipePipe => Some(BinaryOperator::LogicalOr),
            _ => None,
        })
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|child| child.kind() == kind)
}

fn type_child(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children()
        .find(|child| matches!(child.kind(), SyntaxKind::Type | SyntaxKind::ArrayType))
}

fn required_type_child(file: FileId, node: &SyntaxNode) -> Result<SyntaxNode, LoweringError> {
    type_child(node).ok_or_else(|| malformed(node_span(file, node)))
}

fn required_child(
    file: FileId,
    node: &SyntaxNode,
    kind: SyntaxKind,
) -> Result<SyntaxNode, LoweringError> {
    child(node, kind).ok_or_else(|| malformed(node_span(file, node)))
}

fn expression_child(node: &SyntaxNode) -> Option<SyntaxNode> {
    expression_children(node).next()
}

fn required_expression_child(file: FileId, node: &SyntaxNode) -> Result<SyntaxNode, LoweringError> {
    expression_child(node).ok_or_else(|| malformed(node_span(file, node)))
}

fn expression_children(node: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> + '_ {
    node.children()
        .filter(|child| is_expression_kind(child.kind()))
}

const fn is_statement_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ConstDeclaration
            | SyntaxKind::LetDeclaration
            | SyntaxKind::AssignmentStatement
            | SyntaxKind::IfStatement
            | SyntaxKind::WhileStatement
            | SyntaxKind::BreakStatement
            | SyntaxKind::ContinueStatement
            | SyntaxKind::ReturnStatement
            | SyntaxKind::ExpressionStatement
    )
}

const fn is_expression_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::BinaryExpression
            | SyntaxKind::UnaryExpression
            | SyntaxKind::CallExpression
            | SyntaxKind::NameReference
            | SyntaxKind::IntLiteral
            | SyntaxKind::BoolLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::ArrayExpression
            | SyntaxKind::RecordExpression
            | SyntaxKind::MatchExpression
            | SyntaxKind::IndexExpression
            | SyntaxKind::MemberExpression
            | SyntaxKind::ParenthesizedExpression
    )
}

fn required_direct_token(
    file: FileId,
    node: &SyntaxNode,
    kind: SyntaxKind,
) -> Result<SyntaxToken, LoweringError> {
    direct_token(node, kind).ok_or_else(|| malformed(node_span(file, node)))
}

fn direct_token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == kind)
}

fn malformed(span: SourceSpan) -> LoweringError {
    LoweringError {
        kind: LoweringErrorKind::MalformedSyntax,
        span,
    }
}

fn node_span(file: FileId, node: &SyntaxNode) -> SourceSpan {
    let range = node.text_range();
    SourceSpan::new(
        file,
        TextRange::new(
            u32::from(range.start()) as usize,
            u32::from(range.end()) as usize,
        ),
    )
}

fn token_span(file: FileId, token: &SyntaxToken) -> SourceSpan {
    let range = token.text_range();
    SourceSpan::new(
        file,
        TextRange::new(
            u32::from(range.start()) as usize,
            u32::from(range.end()) as usize,
        ),
    )
}
