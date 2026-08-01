use thiserror::Error;

use crate::model::{
    BasicBlock, BlockId, Function, FunctionId, Instruction, InstructionId, NirSpan, NirType,
    Operation, SpannedTerminator, Terminator, TypeDefinition, TypeId, TypedValue, UnverifiedModule,
};

/// Maximum canonical byte length of a portable NIR identifier.
pub const MAX_NIR_IDENTIFIER_BYTES: usize = 255;

/// A structural NIR construction failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BuilderError {
    /// A module, function, or handle kind was empty or non-portable.
    #[error("E5000: invalid NIR identifier `{identifier}`")]
    InvalidIdentifier {
        /// Rejected identifier.
        identifier: String,
    },
    /// An identity was defined more than once in the same scope.
    #[error("E5001: duplicate {kind} id {id}")]
    DuplicateId {
        /// Identity category.
        kind: &'static str,
        /// Numeric identity.
        id: u32,
    },
    /// A requested owner was not defined.
    #[error("E5002: unknown {kind} id {id}")]
    UnknownOwner {
        /// Identity category.
        kind: &'static str,
        /// Numeric identity.
        id: u32,
    },
    /// A block already had a terminator.
    #[error("E5003: block {block} already has a terminator")]
    DuplicateTerminator {
        /// Block ordinal.
        block: u32,
    },
}

impl BuilderError {
    /// Returns the stable NIR builder diagnostic code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier { .. } => "E5000",
            Self::DuplicateId { .. } => "E5001",
            Self::UnknownOwner { .. } => "E5002",
            Self::DuplicateTerminator { .. } => "E5003",
        }
    }
}

/// Contract-first builder for an unverified target-neutral NIR module.
#[derive(Debug, Clone)]
pub struct ModuleBuilder {
    module: UnverifiedModule,
}

impl ModuleBuilder {
    /// Creates a builder with a portable logical module identity.
    ///
    /// # Errors
    ///
    /// Returns [`BuilderError::InvalidIdentifier`] for an empty identity or an
    /// identity containing Host path separators or control characters.
    pub fn new(module_id: impl Into<String>) -> Result<Self, BuilderError> {
        let module_id = module_id.into();
        validate_identifier(&module_id)?;
        Ok(Self {
            module: UnverifiedModule {
                module_id,
                types: Vec::new(),
                functions: Vec::new(),
            },
        })
    }

    /// Defines one logical type with a caller-stable identity.
    ///
    /// # Errors
    ///
    /// Returns [`BuilderError::DuplicateId`] when the type identity already exists.
    pub fn define_type(&mut self, id: TypeId, ty: NirType) -> Result<(), BuilderError> {
        if self
            .module
            .types
            .iter()
            .any(|definition| definition.id == id)
        {
            return Err(duplicate("type", id.index()));
        }
        if let NirType::Handle { handle_kind, .. } = &ty {
            validate_identifier(handle_kind)?;
        }
        self.module.types.push(TypeDefinition { id, ty });
        Ok(())
    }

    /// Defines one function signature and its entry block identity.
    ///
    /// # Errors
    ///
    /// Returns an error for a duplicate function identity or invalid name.
    pub fn define_function(
        &mut self,
        id: FunctionId,
        name: impl Into<String>,
        parameters: impl IntoIterator<Item = TypedValue>,
        return_type: TypeId,
        entry: BlockId,
    ) -> Result<(), BuilderError> {
        if self
            .module
            .functions
            .iter()
            .any(|function| function.id == id)
        {
            return Err(duplicate("function", id.index()));
        }
        let name = name.into();
        validate_identifier(&name)?;
        self.module.functions.push(Function {
            id,
            name,
            parameters: parameters.into_iter().collect(),
            return_type,
            entry,
            blocks: Vec::new(),
        });
        Ok(())
    }

    /// Defines a basic block and its SSA parameters.
    ///
    /// # Errors
    ///
    /// Returns an error when the function is unknown or the block identity is duplicated.
    pub fn define_block(
        &mut self,
        function: FunctionId,
        block: BlockId,
        parameters: impl IntoIterator<Item = TypedValue>,
    ) -> Result<(), BuilderError> {
        let function = self.function_mut(function)?;
        if function
            .blocks
            .iter()
            .any(|candidate| candidate.id == block)
        {
            return Err(duplicate("block", block.index()));
        }
        function.blocks.push(BasicBlock {
            id: block,
            parameters: parameters.into_iter().collect(),
            instructions: Vec::new(),
            terminator: None,
        });
        Ok(())
    }

    /// Appends one typed or effect-only instruction to a basic block.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown owners or a duplicate instruction identity.
    pub fn append_instruction(
        &mut self,
        function: FunctionId,
        block: BlockId,
        id: InstructionId,
        result: Option<TypedValue>,
        operation: Operation,
        span: NirSpan,
    ) -> Result<(), BuilderError> {
        let block = self.block_mut(function, block)?;
        if block
            .instructions
            .iter()
            .any(|instruction| instruction.id == id)
        {
            return Err(duplicate("instruction", id.index()));
        }
        block.instructions.push(Instruction {
            id,
            result,
            operation,
            span,
        });
        Ok(())
    }

    /// Installs the single terminator required by a basic block.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown owners or an already terminated block.
    pub fn set_terminator(
        &mut self,
        function: FunctionId,
        block: BlockId,
        terminator: Terminator,
        span: NirSpan,
    ) -> Result<(), BuilderError> {
        let block = self.block_mut(function, block)?;
        if block.terminator.is_some() {
            return Err(BuilderError::DuplicateTerminator {
                block: block.id.index(),
            });
        }
        block.terminator = Some(SpannedTerminator { terminator, span });
        Ok(())
    }

    /// Finishes construction without claiming semantic validity.
    #[must_use]
    pub fn finish(self) -> UnverifiedModule {
        self.module
    }

    fn function_mut(&mut self, id: FunctionId) -> Result<&mut Function, BuilderError> {
        self.module
            .functions
            .iter_mut()
            .find(|function| function.id == id)
            .ok_or(BuilderError::UnknownOwner {
                kind: "function",
                id: id.index(),
            })
    }

    fn block_mut(
        &mut self,
        function: FunctionId,
        id: BlockId,
    ) -> Result<&mut BasicBlock, BuilderError> {
        self.function_mut(function)?
            .blocks
            .iter_mut()
            .find(|block| block.id == id)
            .ok_or(BuilderError::UnknownOwner {
                kind: "block",
                id: id.index(),
            })
    }
}

fn duplicate(kind: &'static str, id: u32) -> BuilderError {
    BuilderError::DuplicateId { kind, id }
}

pub(crate) fn validate_identifier(identifier: &str) -> Result<(), BuilderError> {
    let valid = !identifier.is_empty()
        && identifier.len() <= MAX_NIR_IDENTIFIER_BYTES
        && identifier.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
        })
        && !identifier.starts_with('/')
        && !identifier
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if !valid {
        return Err(BuilderError::InvalidIdentifier {
            identifier: identifier.to_owned(),
        });
    }
    Ok(())
}
