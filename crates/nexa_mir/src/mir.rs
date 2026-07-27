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
    pub(crate) entry: BasicBlockId,
    pub(crate) blocks: Vec<MirBasicBlock>,
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
    pub const fn return_type(&self) -> &Type {
        &self.return_type
    }

    /// Returns the entry block for this function.
    #[must_use]
    pub const fn entry_block(&self) -> BasicBlockId {
        self.entry
    }

    /// Returns the function's control-flow graph in stable block order.
    #[must_use]
    pub fn blocks(&self) -> &[MirBasicBlock] {
        &self.blocks
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

/// A stable identifier for one basic block in a function control-flow graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BasicBlockId(pub(crate) usize);

impl BasicBlockId {
    /// Returns the block's zero-based index within its function.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A linear statement sequence ending in exactly one control-flow terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirBasicBlock {
    pub(crate) statements: Vec<MirStatement>,
    pub(crate) terminator: MirTerminator,
    pub(crate) span: SourceSpan,
}

impl MirBasicBlock {
    /// Returns statements in execution order.
    #[must_use]
    pub fn statements(&self) -> &[MirStatement] {
        &self.statements
    }

    /// Returns the control-flow operation that ends this block.
    #[must_use]
    pub const fn terminator(&self) -> &MirTerminator {
        &self.terminator
    }

    /// Returns the source range associated with this block.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// A statement in resolved MIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirStatement {
    /// Stores a value in a resolved local slot.
    Store {
        /// The destination local slot.
        local: LocalId,
        /// The expression evaluated before storing.
        value: MirExpression,
        /// The source range for the operation that produced the stored value.
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

/// The control-flow operation that ends a MIR basic block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirTerminator {
    /// Transfers control unconditionally.
    Goto {
        /// The destination block.
        target: BasicBlockId,
        /// The source construct responsible for the edge.
        span: SourceSpan,
    },
    /// Selects one of two blocks using a boolean expression.
    Branch {
        /// The validated boolean condition.
        condition: MirExpression,
        /// The destination selected by `true`.
        then_target: BasicBlockId,
        /// The destination selected by `false`.
        else_target: BasicBlockId,
        /// The source range of the control-flow statement.
        span: SourceSpan,
    },
    /// Returns from the current function.
    Return {
        /// The optional returned value.
        value: Option<MirExpression>,
        /// The return statement or function-body range.
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
    /// An immutable UTF-8 string value.
    String {
        /// The decoded string contents.
        value: String,
        /// The literal's source range.
        span: SourceSpan,
    },
    /// Constructs an immutable array in element evaluation order.
    Array {
        /// The element expressions in source order.
        elements: Vec<MirExpression>,
        /// The full array literal range.
        span: SourceSpan,
    },
    /// Reads an element from an immutable array.
    Index {
        /// The evaluated array expression.
        target: Box<MirExpression>,
        /// The evaluated integer index.
        index: Box<MirExpression>,
        /// The full indexing expression range.
        span: SourceSpan,
    },
    /// Reads the element count of an immutable array.
    Length {
        /// The evaluated array expression.
        target: Box<MirExpression>,
        /// The full member expression range.
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
            | Self::String { span, .. }
            | Self::Array { span, .. }
            | Self::Index { span, .. }
            | Self::Length { span, .. }
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
    /// The built-in scalar printing function.
    Print,
}
