use nexa_hir::{BinaryOperator, Type, UnaryOperator};
use nexa_span::SourceSpan;

/// A complete Nexa program in resolved middle intermediate representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirProgram {
    pub(crate) functions: Vec<MirFunction>,
    pub(crate) span: SourceSpan,
}

impl MirProgram {
    /// Returns all functions in their stable source-order identifiers.
    #[must_use]
    pub fn functions(&self) -> &[MirFunction] {
        &self.functions
    }

    /// Returns the source range covered by the program.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// A function after local and function-name resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirFunction {
    pub(crate) name: String,
    pub(crate) parameters: Vec<LocalId>,
    pub(crate) local_count: usize,
    pub(crate) return_type: Type,
    pub(crate) body: MirBlock,
    pub(crate) span: SourceSpan,
}

impl MirFunction {
    /// Returns the source name of the function.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the number of parameters accepted by the function.
    #[must_use]
    pub fn parameter_count(&self) -> usize {
        self.parameters.len()
    }

    /// Returns the declared return type.
    #[must_use]
    pub const fn return_type(&self) -> Type {
        self.return_type
    }

    /// Returns the function's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// A resolved lexical local slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub(crate) usize);

impl LocalId {
    /// Returns the slot's zero-based index within its function frame.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A resolved function identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub(crate) usize);

impl FunctionId {
    /// Returns the function's zero-based index within its program.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A braced sequence of MIR statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirBlock {
    pub(crate) statements: Vec<MirStatement>,
    pub(crate) span: SourceSpan,
}

impl MirBlock {
    /// Returns statements in execution order.
    #[must_use]
    pub fn statements(&self) -> &[MirStatement] {
        &self.statements
    }

    /// Returns the source range of the original block.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// A statement in resolved MIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirStatement {
    /// Initializes an immutable local slot.
    Initialize {
        /// The destination local slot.
        local: LocalId,
        /// The initializer expression.
        value: MirExpression,
        /// The declaration's source range.
        span: SourceSpan,
    },
    /// Selects one of two blocks based on a condition.
    If {
        /// The boolean condition expression.
        condition: MirExpression,
        /// The branch for a true condition.
        then_branch: MirBlock,
        /// The optional branch for a false condition.
        else_branch: Option<MirBlock>,
        /// The statement's source range.
        span: SourceSpan,
    },
    /// Returns from the current function.
    Return {
        /// The optional returned value.
        value: Option<MirExpression>,
        /// The statement's source range.
        span: SourceSpan,
    },
    /// Evaluates an expression for its side effect.
    Expression {
        /// The evaluated expression.
        expression: MirExpression,
        /// The statement's source range.
        span: SourceSpan,
    },
}

/// A resolved MIR expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirExpression {
    /// A signed integer literal.
    Integer {
        /// The literal value.
        value: i64,
        /// The literal's source range.
        span: SourceSpan,
    },
    /// A boolean literal.
    Boolean {
        /// The literal value.
        value: bool,
        /// The literal's source range.
        span: SourceSpan,
    },
    /// Reads a local slot.
    Local {
        /// The resolved local slot.
        local: LocalId,
        /// The identifier's source range.
        span: SourceSpan,
    },
    /// Applies a prefix operation.
    Unary {
        /// The validated prefix operator.
        operator: UnaryOperator,
        /// The operand expression.
        expression: Box<MirExpression>,
        /// The full expression range.
        span: SourceSpan,
    },
    /// Applies an infix operation.
    Binary {
        /// The validated infix operator.
        operator: BinaryOperator,
        /// The left operand.
        left: Box<MirExpression>,
        /// The right operand.
        right: Box<MirExpression>,
        /// The full expression range.
        span: SourceSpan,
    },
    /// Calls a resolved function or builtin.
    Call {
        /// The resolved call target.
        callee: Callee,
        /// Call arguments in source order.
        arguments: Vec<MirExpression>,
        /// The full call range.
        span: SourceSpan,
    },
}

impl MirExpression {
    /// Returns the complete source range of this expression.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Integer { span, .. }
            | Self::Boolean { span, .. }
            | Self::Local { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Call { span, .. } => *span,
        }
    }
}

/// A resolved callable target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    /// A user-defined function.
    Function(FunctionId),
    /// The built-in integer printing function.
    Print,
}
