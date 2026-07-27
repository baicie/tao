use nexa_span::SourceSpan;

/// A complete Nexa source file after syntax lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// All top-level function declarations in source order.
    pub functions: Vec<Function>,
    /// The source range covered by the source file node.
    pub span: SourceSpan,
}

/// A named, typed function declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The declared function name.
    pub name: Name,
    /// Function parameters in declaration order.
    pub parameters: Vec<Parameter>,
    /// The declared result type.
    pub return_type: TypeReference,
    /// The function statement body.
    pub body: Block,
    /// The declaration's full source range.
    pub span: SourceSpan,
}

/// A named and typed function parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// The declared parameter name.
    pub name: Name,
    /// The declared parameter type.
    pub ty: TypeReference,
    /// The parameter's full source range.
    pub span: SourceSpan,
}

/// A user-written name and its source range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    /// The source spelling of the name.
    pub text: String,
    /// The exact range of the identifier token.
    pub span: SourceSpan,
}

/// A source-level type reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReference {
    /// The resolved built-in type name.
    pub kind: Type,
    /// The exact range of the type token.
    pub span: SourceSpan,
}

/// The closed set of v0.1 value types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// A signed 64-bit integer.
    Int,
    /// A boolean value.
    Bool,
    /// The result type for expressions with no value.
    Unit,
}

impl std::fmt::Display for Type {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Int => "Int",
            Self::Bool => "Bool",
            Self::Unit => "Unit",
        })
    }
}

/// A braced sequence of statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Statements in source order.
    pub statements: Vec<Statement>,
    /// The source range including the braces.
    pub span: SourceSpan,
}

/// A statement in a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// An immutable local binding.
    Const(ConstDeclaration),
    /// A conditional branch.
    If(IfStatement),
    /// A function return.
    Return(ReturnStatement),
    /// An expression evaluated for its side effect.
    Expression(ExpressionStatement),
}

/// An immutable local binding declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDeclaration {
    /// The declared local name.
    pub name: Name,
    /// An optional explicit binding type.
    pub annotation: Option<TypeReference>,
    /// The initializer expression.
    pub initializer: Expression,
    /// The declaration's full source range.
    pub span: SourceSpan,
}

/// A conditional statement with an optional alternative branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStatement {
    /// The condition expression.
    pub condition: Expression,
    /// The branch evaluated when the condition is true.
    pub then_branch: Block,
    /// The optional branch evaluated when the condition is false.
    pub else_branch: Option<Block>,
    /// The statement's full source range.
    pub span: SourceSpan,
}

/// A return from the current function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnStatement {
    /// The optional returned expression.
    pub value: Option<Expression>,
    /// The statement's full source range.
    pub span: SourceSpan,
}

/// An expression used as a statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionStatement {
    /// The expression evaluated for its side effect.
    pub expression: Expression,
    /// The statement's full source range.
    pub span: SourceSpan,
}

/// A v0.1 expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    /// A decimal integer literal.
    Integer {
        /// The parsed signed 64-bit value.
        value: i64,
        /// The literal token range.
        span: SourceSpan,
    },
    /// A boolean literal.
    Boolean {
        /// The literal value.
        value: bool,
        /// The literal token range.
        span: SourceSpan,
    },
    /// A reference to a named local value.
    Name(Name),
    /// A prefix operator application.
    Unary {
        /// The prefix operator.
        operator: UnaryOperator,
        /// The operand expression.
        expression: Box<Expression>,
        /// The full expression range.
        span: SourceSpan,
    },
    /// An infix operator application.
    Binary {
        /// The infix operator.
        operator: BinaryOperator,
        /// The left operand.
        left: Box<Expression>,
        /// The right operand.
        right: Box<Expression>,
        /// The full expression range.
        span: SourceSpan,
    },
    /// A call to a named function or builtin.
    Call {
        /// The called expression.
        callee: Box<Expression>,
        /// Call arguments in source order.
        arguments: Vec<Expression>,
        /// The full expression range.
        span: SourceSpan,
    },
    /// An explicitly parenthesized expression.
    Parenthesized {
        /// The expression inside the parentheses.
        expression: Box<Expression>,
        /// The full expression range.
        span: SourceSpan,
    },
}

impl Expression {
    /// Returns the complete source range of this expression.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Integer { span, .. }
            | Self::Boolean { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Call { span, .. }
            | Self::Parenthesized { span, .. } => *span,
            Self::Name(name) => name.span,
        }
    }
}

/// A prefix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    /// Boolean negation.
    Not,
    /// Signed integer negation.
    Negate,
}

/// An infix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    /// Integer addition.
    Add,
    /// Integer subtraction.
    Subtract,
    /// Integer multiplication.
    Multiply,
    /// Integer division.
    Divide,
    /// Equality comparison.
    Equal,
    /// Less-than comparison.
    Less,
    /// Less-than-or-equal comparison.
    LessEqual,
    /// Greater-than comparison.
    Greater,
    /// Greater-than-or-equal comparison.
    GreaterEqual,
}
