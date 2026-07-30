use nexa_hir::{
    BinaryOperator, FieldId, FunctionId, ModuleId, PayloadId, RecordId, Type, UnaryOperator,
    UnionId, VariantId,
};
use nexa_span::SourceSpan;

/// A complete Nexa program in resolved middle intermediate representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirProgram {
    pub(crate) modules: Vec<ModuleId>,
    pub(crate) entry_module: ModuleId,
    pub(crate) entry: Option<FunctionId>,
    pub(crate) records: Vec<MirRecord>,
    pub(crate) unions: Vec<MirUnion>,
    pub(crate) functions: Vec<MirFunction>,
    pub(crate) span: SourceSpan,
}

impl MirProgram {
    /// Returns modules in deterministic compiler-session discovery order.
    #[must_use]
    pub fn modules(&self) -> &[ModuleId] {
        &self.modules
    }

    /// Returns the compiler session's entry-module identity.
    #[must_use]
    pub const fn entry_module(&self) -> ModuleId {
        self.entry_module
    }

    /// Returns the already resolved entry function, when one was declared.
    #[must_use]
    pub const fn entry_function(&self) -> Option<FunctionId> {
        self.entry
    }

    /// Returns nominal record layouts in stable source order.
    #[must_use]
    pub fn records(&self) -> &[MirRecord] {
        &self.records
    }

    /// Returns nominal tagged union layouts in stable source order.
    #[must_use]
    pub fn unions(&self) -> &[MirUnion] {
        &self.unions
    }

    /// Returns all functions in their stable source-order identifiers.
    #[must_use]
    pub fn functions(&self) -> &[MirFunction] {
        &self.functions
    }

    /// Returns one function by its complete module-owned identity.
    #[must_use]
    pub fn function(&self, id: FunctionId) -> Option<&MirFunction> {
        self.functions.iter().find(|function| function.id == id)
    }

    /// Returns one record layout by its complete module-owned identity.
    #[must_use]
    pub fn record(&self, id: RecordId) -> Option<&MirRecord> {
        self.records.iter().find(|record| record.id == id)
    }

    /// Returns one union layout by its complete module-owned identity.
    #[must_use]
    pub fn union(&self, id: UnionId) -> Option<&MirUnion> {
        self.unions.iter().find(|union| union.id == id)
    }

    /// Returns the source range covered by the program.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// A nominal record layout resolved before MIR execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirRecord {
    pub(crate) id: RecordId,
    pub(crate) name: String,
    pub(crate) fields: Vec<MirRecordField>,
    pub(crate) span: SourceSpan,
}

impl MirRecord {
    /// Returns the record's stable source-order identifier.
    #[must_use]
    pub const fn id(&self) -> RecordId {
        self.id
    }

    /// Returns the declared record name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns fields in declaration and runtime-layout order.
    #[must_use]
    pub fn fields(&self) -> &[MirRecordField] {
        &self.fields
    }

    /// Returns the record declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// One typed field in a resolved nominal record layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirRecordField {
    pub(crate) id: FieldId,
    pub(crate) name: String,
    pub(crate) ty: Type,
    pub(crate) span: SourceSpan,
}

impl MirRecordField {
    /// Returns the field's stable declaration-order identifier.
    #[must_use]
    pub const fn id(&self) -> FieldId {
        self.id
    }

    /// Returns the declared field name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the resolved field type.
    #[must_use]
    pub const fn ty(&self) -> &Type {
        &self.ty
    }

    /// Returns the field declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// A nominal tagged union layout resolved before MIR execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirUnion {
    pub(crate) id: UnionId,
    pub(crate) name: String,
    pub(crate) variants: Vec<MirVariant>,
    pub(crate) span: SourceSpan,
}

impl MirUnion {
    /// Returns the union's stable source-order identifier.
    #[must_use]
    pub const fn id(&self) -> UnionId {
        self.id
    }

    /// Returns the declared union name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns variants in declaration and runtime-tag order.
    #[must_use]
    pub fn variants(&self) -> &[MirVariant] {
        &self.variants
    }

    /// Returns the union declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// One resolved tagged union variant layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirVariant {
    pub(crate) id: VariantId,
    pub(crate) name: String,
    pub(crate) payloads: Vec<MirPayload>,
    pub(crate) span: SourceSpan,
}

impl MirVariant {
    /// Returns this variant's owner-scoped identifier.
    #[must_use]
    pub const fn id(&self) -> VariantId {
        self.id
    }

    /// Returns the declared variant name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns positional payloads in declaration order.
    #[must_use]
    pub fn payloads(&self) -> &[MirPayload] {
        &self.payloads
    }

    /// Returns the variant declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// One resolved positional variant payload layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirPayload {
    pub(crate) id: PayloadId,
    pub(crate) name: String,
    pub(crate) ty: Type,
    pub(crate) span: SourceSpan,
}

impl MirPayload {
    /// Returns this payload's owner-scoped identifier.
    #[must_use]
    pub const fn id(&self) -> PayloadId {
        self.id
    }

    /// Returns the payload declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the resolved payload type.
    #[must_use]
    pub const fn ty(&self) -> &Type {
        &self.ty
    }

    /// Returns the payload declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// A function after local and function-name resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirFunction {
    pub(crate) id: FunctionId,
    pub(crate) name: String,
    pub(crate) parameters: Vec<LocalId>,
    pub(crate) parameter_types: Vec<Type>,
    pub(crate) local_count: usize,
    pub(crate) return_type: Type,
    pub(crate) entry: BasicBlockId,
    pub(crate) blocks: Vec<MirBasicBlock>,
    pub(crate) span: SourceSpan,
}

impl MirFunction {
    /// Returns the function's stable module-owned identity.
    #[must_use]
    pub const fn id(&self) -> FunctionId {
        self.id
    }

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

    /// Returns resolved parameter types in declaration order.
    #[must_use]
    pub fn parameter_types(&self) -> &[Type] {
        &self.parameter_types
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
    /// Dispatches a tagged union local through a dense variant target table.
    SwitchVariant {
        /// The local containing the scrutinee value.
        scrutinee: LocalId,
        /// The expected nominal union identity.
        union: UnionId,
        /// Targets indexed by variant declaration order.
        targets: Vec<BasicBlockId>,
        /// The full match expression range.
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
    /// Constructs an immutable nominal record in declaration field order.
    Record {
        /// The record's resolved nominal identity.
        record: RecordId,
        /// Field values in declaration and runtime-layout order.
        fields: Vec<MirExpression>,
        /// The full record literal range.
        span: SourceSpan,
    },
    /// Constructs one resolved tagged union variant.
    Variant {
        /// The union's resolved nominal identity.
        union: UnionId,
        /// The selected owner-scoped variant identity.
        variant: VariantId,
        /// Payload values in declaration order.
        payloads: Vec<MirExpression>,
        /// The full constructor call range.
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
    /// Reads a resolved field from an immutable nominal record.
    Field {
        /// The evaluated record expression.
        target: Box<MirExpression>,
        /// The expected nominal record identity.
        record: RecordId,
        /// The resolved field identity.
        field: FieldId,
        /// The full member expression range.
        span: SourceSpan,
    },
    /// Projects one resolved positional payload from a selected variant.
    VariantPayload {
        /// The local containing the matched union value.
        source: LocalId,
        /// The expected nominal union identity.
        union: UnionId,
        /// The selected owner-scoped variant identity.
        variant: VariantId,
        /// The projected owner-scoped payload identity.
        payload: PayloadId,
        /// The pattern binding's source range.
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
            | Self::Record { span, .. }
            | Self::Variant { span, .. }
            | Self::Index { span, .. }
            | Self::Length { span, .. }
            | Self::Field { span, .. }
            | Self::VariantPayload { span, .. }
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
