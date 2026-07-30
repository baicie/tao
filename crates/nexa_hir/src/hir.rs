use nexa_span::SourceSpan;

/// A complete Nexa source file after syntax lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// All top-level nominal record declarations in source order.
    pub records: Vec<RecordDeclaration>,
    /// All top-level nominal tagged union declarations in source order.
    pub unions: Vec<UnionDeclaration>,
    /// All top-level function declarations in source order.
    pub functions: Vec<Function>,
    /// The source range covered by the source file node.
    pub span: SourceSpan,
}

/// A nominal immutable record declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordDeclaration {
    /// The declared record name.
    pub name: Name,
    /// Fields in declaration order.
    pub fields: Vec<RecordFieldDeclaration>,
    /// The declaration's full source range.
    pub span: SourceSpan,
}

/// One named field in a record declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFieldDeclaration {
    /// The declared field name.
    pub name: Name,
    /// The declared field type.
    pub ty: TypeReference,
    /// The field declaration's full source range.
    pub span: SourceSpan,
}

/// A nominal tagged union declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionDeclaration {
    /// The declared union name.
    pub name: Name,
    /// Variants in declaration order.
    pub variants: Vec<UnionVariantDeclaration>,
    /// The declaration's full source range.
    pub span: SourceSpan,
}

/// One variant in a nominal tagged union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionVariantDeclaration {
    /// The declared variant name.
    pub name: Name,
    /// Positional payload declarations in source order.
    pub payloads: Vec<VariantPayloadDeclaration>,
    /// The variant declaration's full source range.
    pub span: SourceSpan,
}

/// One named positional payload in a tagged union variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantPayloadDeclaration {
    /// The payload's declaration name.
    pub name: Name,
    /// The payload's declared type.
    pub ty: TypeReference,
    /// The payload declaration's full source range.
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
    /// The source type spelling before semantic name resolution.
    pub kind: TypeReferenceKind,
    /// The complete source range of the type syntax.
    pub span: SourceSpan,
}

/// A source-level type spelling before semantic name resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReferenceKind {
    /// The built-in signed integer type.
    Int,
    /// The built-in boolean type.
    Bool,
    /// The built-in UTF-8 string type.
    String,
    /// The built-in no-value result type.
    Unit,
    /// A named nominal type.
    Named(Name),
    /// An immutable homogeneous array type.
    Array(Box<TypeReference>),
}

/// A stable source-order identifier for a nominal record in one program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecordId(usize);

impl RecordId {
    /// Creates a record identifier from its zero-based source index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based source index within the program.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A stable field identifier scoped to one nominal record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldId {
    record: RecordId,
    index: usize,
}

impl FieldId {
    /// Creates a field identifier from its record and declaration-order index.
    #[must_use]
    pub const fn new(record: RecordId, index: usize) -> Self {
        Self { record, index }
    }

    /// Returns the record that owns this field.
    #[must_use]
    pub const fn record(self) -> RecordId {
        self.record
    }

    /// Returns the zero-based declaration-order index within the record.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }
}

/// A stable source-order identifier for a nominal tagged union in one program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnionId(usize);

impl UnionId {
    /// Creates a union identifier from its zero-based source index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based source index within the program.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A stable variant identifier scoped to one nominal tagged union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariantId {
    union: UnionId,
    index: usize,
}

impl VariantId {
    /// Creates a variant identifier from its union and declaration-order index.
    #[must_use]
    pub const fn new(union: UnionId, index: usize) -> Self {
        Self { union, index }
    }

    /// Returns the union that owns this variant.
    #[must_use]
    pub const fn union(self) -> UnionId {
        self.union
    }

    /// Returns the zero-based declaration-order index within the union.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }
}

/// A stable payload identifier scoped to one tagged union variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PayloadId {
    variant: VariantId,
    index: usize,
}

impl PayloadId {
    /// Creates a payload identifier from its variant and positional index.
    #[must_use]
    pub const fn new(variant: VariantId, index: usize) -> Self {
        Self { variant, index }
    }

    /// Returns the variant that owns this payload.
    #[must_use]
    pub const fn variant(self) -> VariantId {
        self.variant
    }

    /// Returns the zero-based positional index within the variant.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }
}

/// The closed set of Language Core value types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    /// A signed 64-bit integer.
    Int,
    /// A boolean value.
    Bool,
    /// A UTF-8 string value.
    String,
    /// An immutable homogeneous array value.
    Array(Box<Type>),
    /// A nominal immutable record value.
    Record(RecordId),
    /// A nominal tagged union value.
    Union(UnionId),
    /// The result type for expressions with no value.
    Unit,
}

impl std::fmt::Display for Type {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int => formatter.write_str("Int"),
            Self::Bool => formatter.write_str("Bool"),
            Self::String => formatter.write_str("String"),
            Self::Array(element) => write!(formatter, "{element}[]"),
            Self::Record(record) => write!(formatter, "record#{}", record.index()),
            Self::Union(union) => write!(formatter, "union#{}", union.index()),
            Self::Unit => formatter.write_str("Unit"),
        }
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
    /// A mutable local binding.
    Let(LetDeclaration),
    /// An assignment to a mutable local binding.
    Assignment(AssignmentStatement),
    /// A conditional branch.
    If(IfStatement),
    /// A conditional loop.
    While(WhileStatement),
    /// An exit from the nearest enclosing loop.
    Break(BreakStatement),
    /// A jump to the next iteration of the nearest enclosing loop.
    Continue(ContinueStatement),
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

/// A mutable local binding declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetDeclaration {
    /// The declared local name.
    pub name: Name,
    /// An optional explicit binding type.
    pub annotation: Option<TypeReference>,
    /// The initializer expression.
    pub initializer: Expression,
    /// The declaration's full source range.
    pub span: SourceSpan,
}

/// An assignment to a named local binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentStatement {
    /// The binding receiving the value.
    pub target: Name,
    /// The value assigned to the binding.
    pub value: Expression,
    /// The statement's full source range.
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

/// A loop that repeats while its condition is true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileStatement {
    /// The condition evaluated before every iteration.
    pub condition: Expression,
    /// The repeated statement body.
    pub body: Block,
    /// The statement's full source range.
    pub span: SourceSpan,
}

/// An exit from the nearest enclosing loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakStatement {
    /// The statement's full source range.
    pub span: SourceSpan,
}

/// A jump to the next iteration of the nearest enclosing loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinueStatement {
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

/// A source expression after syntax lowering.
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
    /// A decoded UTF-8 string literal.
    String {
        /// The literal value after escape decoding.
        value: String,
        /// The literal token range.
        span: SourceSpan,
    },
    /// An immutable array literal.
    Array {
        /// Elements in source order.
        elements: Vec<Expression>,
        /// The full expression range.
        span: SourceSpan,
    },
    /// An immutable record literal whose type is supplied by context.
    Record {
        /// Field initializers in source order.
        fields: Vec<RecordFieldInitializer>,
        /// The full expression range.
        span: SourceSpan,
    },
    /// An exhaustive tagged-union match expression.
    Match {
        /// The tagged union value inspected exactly once.
        scrutinee: Box<Expression>,
        /// Match arms in source order.
        arms: Vec<MatchArm>,
        /// The full match expression range.
        span: SourceSpan,
    },
    /// An indexed array access.
    Index {
        /// The array-valued expression.
        collection: Box<Expression>,
        /// The integer index expression.
        index: Box<Expression>,
        /// The full expression range.
        span: SourceSpan,
    },
    /// A named member access.
    Member {
        /// The expression whose member is read.
        object: Box<Expression>,
        /// The selected member name.
        member: Name,
        /// The full expression range.
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
            | Self::String { span, .. }
            | Self::Array { span, .. }
            | Self::Record { span, .. }
            | Self::Match { span, .. }
            | Self::Index { span, .. }
            | Self::Member { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Call { span, .. }
            | Self::Parenthesized { span, .. } => *span,
            Self::Name(name) => name.span,
        }
    }
}

/// One named initializer in an immutable record literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFieldInitializer {
    /// The source-written field name.
    pub name: Name,
    /// The initializer expression.
    pub value: Expression,
    /// The initializer's full source range.
    pub span: SourceSpan,
}

/// One expression arm in a tagged-union match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    /// The variant or default pattern selecting this arm.
    pub pattern: MatchPattern,
    /// The expression evaluated when the pattern is selected.
    pub value: Expression,
    /// The arm's full source range.
    pub span: SourceSpan,
}

/// A non-nested v0.5 match pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchPattern {
    /// A qualified variant pattern with positional immutable bindings.
    Variant {
        /// The source-written union qualifier.
        union: Name,
        /// The source-written variant name.
        variant: Name,
        /// Positional payload bindings in source order.
        bindings: Vec<Name>,
        /// The full pattern range.
        span: SourceSpan,
    },
    /// A catch-all pattern covering every remaining variant.
    Default {
        /// The `default` token range.
        span: SourceSpan,
    },
}

impl MatchPattern {
    /// Returns the complete source range of this pattern.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Variant { span, .. } | Self::Default { span } => *span,
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
    /// Short-circuiting boolean conjunction.
    LogicalAnd,
    /// Short-circuiting boolean disjunction.
    LogicalOr,
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
