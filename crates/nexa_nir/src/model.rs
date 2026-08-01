use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u32);

        impl $name {
            #[doc = concat!("Creates a `", stringify!($name), "` from its canonical ordinal.")]
            #[must_use]
            pub const fn new(index: u32) -> Self {
                Self(index)
            }

            /// Returns the canonical zero-based ordinal.
            #[must_use]
            pub const fn index(self) -> u32 {
                self.0
            }
        }
    };
}

id_type!(TypeId, "A module-scoped NIR type identity.");
id_type!(FunctionId, "A module-scoped NIR function identity.");
id_type!(BlockId, "A function-scoped NIR basic-block identity.");
id_type!(InstructionId, "A block-scoped NIR instruction identity.");
id_type!(ValueId, "A function-scoped NIR SSA value identity.");

/// A stable source location attached to a user-observable NIR operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NirSpan {
    source: u32,
    start: u32,
    end: u32,
}

impl NirSpan {
    /// Creates a span using a source ordinal and half-open byte range.
    #[must_use]
    pub const fn new(source: u32, start: u32, end: u32) -> Self {
        Self { source, start, end }
    }

    /// Returns the stable source ordinal.
    #[must_use]
    pub const fn source(self) -> u32 {
        self.source
    }

    /// Returns the inclusive byte start.
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    /// Returns the exclusive byte end.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }
}

/// Ownership behavior carried by an opaque NIR handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValueOwnership {
    /// The handle can be copied without cleanup.
    Copy,
    /// The handle must be moved or dropped exactly once on every path.
    Owned,
    /// The handle is a shared non-owning borrow.
    Borrowed,
    /// The handle is a unique non-owning mutable borrow.
    MutBorrowed,
}

/// A target-neutral logical NIR type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NirType {
    /// One-bit logical integer used for conditions.
    I1,
    /// Signed 8-bit integer.
    I8,
    /// Signed 16-bit integer.
    I16,
    /// Signed 32-bit integer.
    I32,
    /// Signed 64-bit integer.
    I64,
    /// Unsigned 8-bit integer.
    U8,
    /// Unsigned 16-bit integer.
    U16,
    /// Unsigned 32-bit integer.
    U32,
    /// Unsigned 64-bit integer.
    U64,
    /// IEEE-754 binary32.
    F32,
    /// IEEE-754 binary64.
    F64,
    /// A Unicode scalar value encoded as 32 bits.
    Char32,
    /// The zero-sized unit type.
    Unit,
    /// A type with no values.
    Never,
    /// A non-owning unchecked pointer.
    RawPtr {
        /// The logical pointee type.
        pointee: TypeId,
    },
    /// A uniquely owned pointer that requires explicit consumption.
    OwnedPtr {
        /// The logical pointee type.
        pointee: TypeId,
    },
    /// A shared non-owning pointer.
    BorrowPtr {
        /// The logical pointee type.
        pointee: TypeId,
    },
    /// A unique mutable non-owning pointer.
    MutBorrowPtr {
        /// The logical pointee type.
        pointee: TypeId,
    },
    /// An opaque runtime or Host handle.
    Handle {
        /// Registry-defined handle kind.
        handle_kind: String,
        /// Explicit ownership behavior.
        ownership: ValueOwnership,
    },
    /// A target-neutral aggregate with fields in fixed logical order.
    Struct {
        /// Field types in logical order.
        fields: Vec<TypeId>,
    },
    /// A fixed-length homogeneous aggregate.
    FixedArray {
        /// Element type.
        element: TypeId,
        /// Number of elements.
        length: u64,
    },
    /// A resolved function reference.
    FunctionRef,
}

impl NirType {
    pub(crate) fn referenced_types(&self) -> impl Iterator<Item = TypeId> + '_ {
        let mut references = Vec::new();
        match self {
            Self::RawPtr { pointee }
            | Self::OwnedPtr { pointee }
            | Self::BorrowPtr { pointee }
            | Self::MutBorrowPtr { pointee } => references.push(*pointee),
            Self::Struct { fields } => references.extend(fields.iter().copied()),
            Self::FixedArray { element, .. } => references.push(*element),
            Self::I1
            | Self::I8
            | Self::I16
            | Self::I32
            | Self::I64
            | Self::U8
            | Self::U16
            | Self::U32
            | Self::U64
            | Self::F32
            | Self::F64
            | Self::Char32
            | Self::Unit
            | Self::Never
            | Self::Handle { .. }
            | Self::FunctionRef => {}
        }
        references.into_iter()
    }
}

/// One SSA value paired with its declared type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypedValue {
    id: ValueId,
    ty: TypeId,
}

impl TypedValue {
    /// Creates a typed SSA definition.
    #[must_use]
    pub const fn new(id: ValueId, ty: TypeId) -> Self {
        Self { id, ty }
    }

    /// Returns the SSA identity.
    #[must_use]
    pub const fn id(self) -> ValueId {
        self.id
    }

    /// Returns the declared NIR type identity.
    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// A typed NIR operation evaluated within one basic block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Operation {
    /// Produces a signed 64-bit integer constant.
    ConstI64 {
        /// Literal value.
        value: i64,
    },
    /// Produces a logical one-bit constant.
    ConstI1 {
        /// Literal value.
        value: bool,
    },
    /// Copies a copyable value.
    Copy {
        /// Source value.
        value: ValueId,
    },
    /// Transfers ownership into a fresh SSA value.
    Move {
        /// Source owned value.
        value: ValueId,
    },
    /// Drops one owned value.
    Drop {
        /// Source owned value.
        value: ValueId,
    },
    /// Adds two signed 64-bit integers.
    AddI64 {
        /// Left operand.
        left: ValueId,
        /// Right operand.
        right: ValueId,
    },
    /// Calls a statically resolved function.
    Call {
        /// Callee identity.
        function: FunctionId,
        /// Arguments in signature order.
        arguments: Vec<ValueId>,
    },
}

/// A control-flow operation that ends a basic block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Terminator {
    /// Returns from the current function.
    Return {
        /// Returned value, absent for `Unit`.
        value: Option<ValueId>,
    },
    /// Transfers control and block arguments unconditionally.
    Goto {
        /// Destination block.
        target: BlockId,
        /// Values passed to destination block parameters.
        arguments: Vec<ValueId>,
    },
    /// Selects one of two successors using an `I1` value.
    Branch {
        /// Condition value.
        condition: ValueId,
        /// True successor.
        then_target: BlockId,
        /// Values passed to the true successor.
        then_arguments: Vec<ValueId>,
        /// False successor.
        else_target: BlockId,
        /// Values passed to the false successor.
        else_arguments: Vec<ValueId>,
    },
    /// Marks a statically unreachable block exit.
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TypeDefinition {
    pub(crate) id: TypeId,
    pub(crate) ty: NirType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Instruction {
    pub(crate) id: InstructionId,
    pub(crate) result: Option<TypedValue>,
    pub(crate) operation: Operation,
    pub(crate) span: NirSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SpannedTerminator {
    pub(crate) terminator: Terminator,
    pub(crate) span: NirSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BasicBlock {
    pub(crate) id: BlockId,
    pub(crate) parameters: Vec<TypedValue>,
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) terminator: Option<SpannedTerminator>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Function {
    pub(crate) id: FunctionId,
    pub(crate) name: String,
    pub(crate) parameters: Vec<TypedValue>,
    pub(crate) return_type: TypeId,
    pub(crate) entry: BlockId,
    pub(crate) blocks: Vec<BasicBlock>,
}

/// A typed module that has not yet passed independent verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnverifiedModule {
    pub(crate) module_id: String,
    pub(crate) types: Vec<TypeDefinition>,
    pub(crate) functions: Vec<Function>,
}
