use std::fmt::{Display, Formatter};

use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::{FileId, SourceSpan, TextRange};
use nexa_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::{
    AssignmentStatement, BinaryOperator, Block, BreakStatement, ConstDeclaration,
    ContinueStatement, Expression, ExpressionStatement, Function, IfStatement, LetDeclaration,
    Name, Parameter, Program, ReturnStatement, Statement, Type, TypeReference, UnaryOperator,
    WhileStatement,
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
    if syntax.kind() != SyntaxKind::SourceFile {
        return Err(malformed(node_span(file, syntax)));
    }

    let functions = syntax
        .children()
        .filter(|node| node.kind() == SyntaxKind::FunctionDeclaration)
        .map(|node| lower_function(file, &node))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Program {
        functions,
        span: node_span(file, syntax),
    })
}

fn lower_function(file: FileId, node: &SyntaxNode) -> Result<Function, LoweringError> {
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
    let return_type = lower_type(file, &required_child(file, node, SyntaxKind::Type)?)?;
    let body = lower_block(file, &required_child(file, node, SyntaxKind::Block)?)?;

    Ok(Function {
        name,
        parameters,
        return_type,
        body,
        span: node_span(file, node),
    })
}

fn lower_parameter(file: FileId, node: &SyntaxNode) -> Result<Parameter, LoweringError> {
    Ok(Parameter {
        name: lower_direct_name(file, node)?,
        ty: lower_type(file, &required_child(file, node, SyntaxKind::Type)?)?,
        span: node_span(file, node),
    })
}

fn lower_type(file: FileId, node: &SyntaxNode) -> Result<TypeReference, LoweringError> {
    let token = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            matches!(
                token.kind(),
                SyntaxKind::IntKw | SyntaxKind::BoolKw | SyntaxKind::UnitKw
            )
        })
        .ok_or_else(|| malformed(node_span(file, node)))?;
    let kind = match token.kind() {
        SyntaxKind::IntKw => Type::Int,
        SyntaxKind::BoolKw => Type::Bool,
        SyntaxKind::UnitKw => Type::Unit,
        _ => return Err(malformed(token_span(file, &token))),
    };

    Ok(TypeReference {
        kind,
        span: token_span(file, &token),
    })
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
    let annotation = child(node, SyntaxKind::Type)
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
    let annotation = child(node, SyntaxKind::Type)
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
        SyntaxKind::NameReference => Ok(Expression::Name(lower_direct_name(file, node)?)),
        SyntaxKind::UnaryExpression => lower_unary(file, node),
        SyntaxKind::BinaryExpression => lower_binary(file, node),
        SyntaxKind::CallExpression => lower_call(file, node),
        SyntaxKind::ParenthesizedExpression => lower_parenthesized(file, node),
        _ => Err(malformed(node_span(file, node))),
    }
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
