use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::builder::validate_identifier;
use crate::model::{
    BasicBlock, BlockId, Function, FunctionId, IntrinsicId, NirType, Operation, Terminator, TypeId,
    TypedValue, UnverifiedModule, ValueId, ValueOwnership,
};

/// Stable category for an independently detected NIR invariant violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationCode {
    /// An identity is duplicated or not in canonical dense order.
    InvalidIdentity,
    /// A referenced type does not exist.
    UnknownType,
    /// A referenced function does not exist.
    UnknownFunction,
    /// A referenced basic block does not exist.
    UnknownBlock,
    /// A referenced SSA value does not exist at the use site.
    UnknownValue,
    /// An operation or control-flow edge has incompatible types.
    TypeMismatch,
    /// An instruction has an invalid result shape.
    InvalidInstruction,
    /// A basic block has no terminator.
    MissingTerminator,
    /// An owned value was used after a consuming operation.
    UseAfterConsume,
    /// An owned value reaches a block exit without being moved or dropped.
    OwnedValueNotConsumed,
    /// A logical type has no canonical NIR meaning.
    InvalidType,
}

impl VerificationCode {
    /// Returns the stable ADR-003 diagnostic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidIdentity => "E5100",
            Self::UnknownType => "E5101",
            Self::UnknownFunction => "E5102",
            Self::UnknownBlock => "E5103",
            Self::UnknownValue => "E5104",
            Self::TypeMismatch => "E5105",
            Self::InvalidInstruction => "E5106",
            Self::MissingTerminator => "E5107",
            Self::UseAfterConsume => "E5108",
            Self::OwnedValueNotConsumed => "E5109",
            Self::InvalidType => "E5120",
        }
    }
}

/// One NIR verification failure with a stable category and deterministic context.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{}: {message}", code.as_str())]
pub struct VerificationError {
    code: VerificationCode,
    message: String,
}

impl VerificationError {
    /// Returns the stable failure category.
    #[must_use]
    pub const fn code(&self) -> VerificationCode {
        self.code
    }

    /// Returns deterministic human-readable context.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// A module proven to satisfy the NIR verifier contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedModule {
    module: UnverifiedModule,
}

impl VerifiedModule {
    /// Returns the logical module identity.
    #[must_use]
    pub fn module_id(&self) -> &str {
        &self.module.module_id
    }

    pub(crate) const fn module(&self) -> &UnverifiedModule {
        &self.module
    }
}

/// Independent validator for untrusted NIR values.
#[derive(Debug, Clone, Copy, Default)]
pub struct Verifier;

impl Verifier {
    /// Validates every type, CFG, SSA, instruction, and ownership invariant.
    ///
    /// # Errors
    ///
    /// Returns the first invariant violation in deterministic canonical order.
    pub fn verify(mut module: UnverifiedModule) -> Result<VerifiedModule, VerificationError> {
        verify_identifier(&module.module_id, "module")?;
        module.types.sort_by_key(|definition| definition.id);
        module.functions.sort_by_key(|function| function.id);
        verify_dense_ids(
            module.types.iter().map(|definition| definition.id.index()),
            "type",
        )?;
        verify_dense_ids(
            module.functions.iter().map(|function| function.id.index()),
            "function",
        )?;

        let types = module
            .types
            .iter()
            .map(|definition| (definition.id, &definition.ty))
            .collect::<BTreeMap<_, _>>();
        for definition in &module.types {
            if let NirType::Handle { handle_kind, .. } = &definition.ty {
                verify_identifier(handle_kind, "handle kind")?;
            }
            if matches!(
                &definition.ty,
                NirType::TaggedUnion { variants } if variants.is_empty()
            ) {
                return fail(
                    VerificationCode::InvalidType,
                    format!(
                        "tagged union type {} must declare at least one variant",
                        definition.id.index()
                    ),
                );
            }
            for referenced in definition.ty.referenced_types() {
                require_type(&types, referenced, "type definition")?;
            }
        }

        let mut function_names = BTreeSet::new();
        for function in &module.functions {
            verify_identifier(&function.name, "function")?;
            if !function_names.insert(function.name.as_str()) {
                return fail(
                    VerificationCode::InvalidIdentity,
                    format!("function name `{}` is duplicated", function.name),
                );
            }
        }
        let signatures = module
            .functions
            .iter()
            .map(|function| (function.id, function_signature(function)))
            .collect::<BTreeMap<_, _>>();
        for function in &mut module.functions {
            verify_function(function, &types, &signatures)?;
        }

        Ok(VerifiedModule { module })
    }
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    parameters: Vec<TypeId>,
    return_type: TypeId,
}

fn function_signature(function: &Function) -> FunctionSignature {
    FunctionSignature {
        parameters: function.parameters.iter().map(|value| value.ty()).collect(),
        return_type: function.return_type,
    }
}

fn verify_function(
    function: &mut Function,
    types: &BTreeMap<TypeId, &NirType>,
    signatures: &BTreeMap<FunctionId, FunctionSignature>,
) -> Result<(), VerificationError> {
    require_type(types, function.return_type, "function return")?;
    function.blocks.sort_by_key(|block| block.id);
    verify_dense_ids(
        function.blocks.iter().map(|block| block.id.index()),
        "block",
    )?;
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block.parameters.clone()))
        .collect::<BTreeMap<_, _>>();
    if !blocks.contains_key(&function.entry) {
        return fail(
            VerificationCode::UnknownBlock,
            format!(
                "function {} entry block {} does not exist",
                function.id.index(),
                function.entry.index()
            ),
        );
    }
    if blocks
        .get(&function.entry)
        .is_some_and(|parameters| !parameters.is_empty())
    {
        return fail(
            VerificationCode::InvalidIdentity,
            format!(
                "function {} entry block {} must not declare block parameters",
                function.id.index(),
                function.entry.index()
            ),
        );
    }
    for block in &function.blocks {
        if block.terminator.as_ref().is_some_and(|terminator| {
            terminator_targets_entry(&terminator.terminator, function.entry)
        }) {
            return fail(
                VerificationCode::InvalidIdentity,
                format!(
                    "block {} targets function {} entry block {}",
                    block.id.index(),
                    function.id.index(),
                    function.entry.index()
                ),
            );
        }
    }

    let mut all_values = BTreeMap::new();
    for parameter in &function.parameters {
        define_value(&mut all_values, *parameter, types, "function parameter")?;
    }
    for block in &mut function.blocks {
        block.instructions.sort_by_key(|instruction| instruction.id);
        verify_dense_ids(
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.index()),
            "instruction",
        )?;
        for parameter in &block.parameters {
            define_value(&mut all_values, *parameter, types, "block parameter")?;
        }
        for instruction in &block.instructions {
            if let Some(result) = instruction.result {
                define_value(&mut all_values, result, types, "instruction result")?;
            }
        }
    }
    verify_dense_ids(all_values.keys().map(|id| id.index()), "value")?;

    for block in &function.blocks {
        verify_block(function, block, types, signatures, &blocks, &all_values)?;
    }
    Ok(())
}

fn verify_block(
    function: &Function,
    block: &BasicBlock,
    types: &BTreeMap<TypeId, &NirType>,
    signatures: &BTreeMap<FunctionId, FunctionSignature>,
    blocks: &BTreeMap<BlockId, Vec<TypedValue>>,
    all_values: &BTreeMap<ValueId, TypeId>,
) -> Result<(), VerificationError> {
    let mut visible = BTreeSet::new();
    let mut active_owned = BTreeSet::new();
    if block.id == function.entry {
        for parameter in &function.parameters {
            visible.insert(parameter.id());
            activate_if_owned(&mut active_owned, *parameter, types)?;
        }
    }
    for parameter in &block.parameters {
        visible.insert(parameter.id());
        activate_if_owned(&mut active_owned, *parameter, types)?;
    }

    for instruction in &block.instructions {
        verify_span(
            instruction.span,
            &format!("instruction {}", instruction.id.index()),
        )?;
        verify_operation(
            instruction,
            types,
            signatures,
            all_values,
            &visible,
            &mut active_owned,
        )?;
        if let Some(result) = instruction.result {
            visible.insert(result.id());
            activate_if_owned(&mut active_owned, result, types)?;
        }
    }
    let terminator = block.terminator.as_ref().ok_or_else(|| VerificationError {
        code: VerificationCode::MissingTerminator,
        message: format!("block {} has no terminator", block.id.index()),
    })?;
    verify_span(
        terminator.span,
        &format!("block {} terminator", block.id.index()),
    )?;
    verify_terminator(
        &terminator.terminator,
        function.return_type,
        types,
        blocks,
        all_values,
        &visible,
        &active_owned,
    )
}

const fn terminator_targets_entry(terminator: &Terminator, entry: BlockId) -> bool {
    match terminator {
        Terminator::Goto { target, .. } => target.index() == entry.index(),
        Terminator::Branch {
            then_target,
            else_target,
            ..
        } => then_target.index() == entry.index() || else_target.index() == entry.index(),
        Terminator::Return { .. } | Terminator::Unreachable => false,
    }
}

fn verify_operation(
    instruction: &crate::model::Instruction,
    types: &BTreeMap<TypeId, &NirType>,
    signatures: &BTreeMap<FunctionId, FunctionSignature>,
    all_values: &BTreeMap<ValueId, TypeId>,
    visible: &BTreeSet<ValueId>,
    active_owned: &mut BTreeSet<ValueId>,
) -> Result<(), VerificationError> {
    match &instruction.operation {
        Operation::ConstI64 { .. } => {
            require_result_type(instruction.result, all_values, types, NirType::I64)
        }
        Operation::ConstI1 { .. } => {
            require_result_type(instruction.result, all_values, types, NirType::I1)
        }
        Operation::Copy { value } => {
            let source = use_type(*value, all_values, visible)?;
            if ownership(types, source)? != ValueOwnership::Copy {
                return type_mismatch("copy requires a copyable input");
            }
            require_result_exact(instruction.result, source)
        }
        Operation::Move { value } => {
            let source = use_type(*value, all_values, visible)?;
            if ownership(types, source)? != ValueOwnership::Owned {
                return type_mismatch("move requires an owned input");
            }
            consume(*value, active_owned)?;
            require_result_exact(instruction.result, source)
        }
        Operation::Drop { value } => {
            require_no_result(instruction.result, "drop")?;
            let source = use_type(*value, all_values, visible)?;
            if ownership(types, source)? != ValueOwnership::Owned {
                return type_mismatch("drop requires an owned input");
            }
            consume(*value, active_owned)
        }
        Operation::AddI64 { left, right } => {
            require_value_kind(*left, all_values, visible, types, NirType::I64)?;
            require_value_kind(*right, all_values, visible, types, NirType::I64)?;
            require_result_type(instruction.result, all_values, types, NirType::I64)
        }
        Operation::Call {
            function,
            arguments,
        } => {
            let signature = signatures.get(function).ok_or_else(|| VerificationError {
                code: VerificationCode::UnknownFunction,
                message: format!("call references unknown function {}", function.index()),
            })?;
            if signature.parameters.len() != arguments.len() {
                return type_mismatch("call argument count does not match the signature");
            }
            for (argument, expected) in arguments.iter().zip(&signature.parameters) {
                let actual = use_type(*argument, all_values, visible)?;
                if actual != *expected {
                    return type_mismatch("call argument type does not match the signature");
                }
                if ownership(types, actual)? == ValueOwnership::Owned {
                    consume(*argument, active_owned)?;
                }
            }
            if matches!(types.get(&signature.return_type), Some(NirType::Unit)) {
                require_no_result(instruction.result, "unit call")
            } else {
                require_result_exact(instruction.result, signature.return_type)
            }
        }
        Operation::CallIntrinsic {
            intrinsic,
            arguments,
        } => {
            require_no_result(instruction.result, "unit intrinsic call")?;
            let expected = match intrinsic {
                IntrinsicId::PrintI64 => NirType::I64,
                IntrinsicId::PrintI1 => NirType::I1,
            };
            if arguments.len() != 1 {
                return type_mismatch("print intrinsic requires exactly one argument");
            }
            require_value_kind(arguments[0], all_values, visible, types, expected)
        }
    }
}

fn verify_terminator(
    terminator: &Terminator,
    return_type: TypeId,
    types: &BTreeMap<TypeId, &NirType>,
    blocks: &BTreeMap<BlockId, Vec<TypedValue>>,
    all_values: &BTreeMap<ValueId, TypeId>,
    visible: &BTreeSet<ValueId>,
    active_owned: &BTreeSet<ValueId>,
) -> Result<(), VerificationError> {
    let remaining = match terminator {
        Terminator::Return { value } => {
            let mut remaining = active_owned.clone();
            match value {
                Some(value) => {
                    let actual = use_type(*value, all_values, visible)?;
                    if actual != return_type {
                        return type_mismatch("returned value does not match the function result");
                    }
                    if ownership(types, actual)? == ValueOwnership::Owned {
                        consume(*value, &mut remaining)?;
                    }
                }
                None => {
                    if !matches!(types.get(&return_type), Some(NirType::Unit)) {
                        return type_mismatch("non-unit function must return a value");
                    }
                }
            }
            remaining
        }
        Terminator::Goto { target, arguments } => verify_edge(
            *target,
            arguments,
            types,
            blocks,
            all_values,
            visible,
            active_owned.clone(),
        )?,
        Terminator::Branch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            require_value_kind(*condition, all_values, visible, types, NirType::I1)?;
            let then_remaining = verify_edge(
                *then_target,
                then_arguments,
                types,
                blocks,
                all_values,
                visible,
                active_owned.clone(),
            )?;
            let else_remaining = verify_edge(
                *else_target,
                else_arguments,
                types,
                blocks,
                all_values,
                visible,
                active_owned.clone(),
            )?;
            if then_remaining != else_remaining {
                return fail(
                    VerificationCode::OwnedValueNotConsumed,
                    "branch successors consume different owned values".to_owned(),
                );
            }
            then_remaining
        }
        Terminator::Unreachable => BTreeSet::new(),
    };
    if let Some(value) = remaining.iter().next() {
        return fail(
            VerificationCode::OwnedValueNotConsumed,
            format!("owned value {} reaches a block exit", value.index()),
        );
    }
    Ok(())
}

fn verify_edge(
    target: BlockId,
    arguments: &[ValueId],
    types: &BTreeMap<TypeId, &NirType>,
    blocks: &BTreeMap<BlockId, Vec<TypedValue>>,
    all_values: &BTreeMap<ValueId, TypeId>,
    visible: &BTreeSet<ValueId>,
    mut active_owned: BTreeSet<ValueId>,
) -> Result<BTreeSet<ValueId>, VerificationError> {
    let parameters = blocks.get(&target).ok_or_else(|| VerificationError {
        code: VerificationCode::UnknownBlock,
        message: format!("control flow references unknown block {}", target.index()),
    })?;
    if parameters.len() != arguments.len() {
        return type_mismatch("block argument count does not match block parameters");
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let actual = use_type(*argument, all_values, visible)?;
        if actual != parameter.ty() {
            return type_mismatch("block argument type does not match block parameter");
        }
        if ownership(types, actual)? == ValueOwnership::Owned {
            consume(*argument, &mut active_owned)?;
        }
    }
    Ok(active_owned)
}

fn define_value(
    values: &mut BTreeMap<ValueId, TypeId>,
    value: TypedValue,
    types: &BTreeMap<TypeId, &NirType>,
    context: &str,
) -> Result<(), VerificationError> {
    require_type(types, value.ty(), context)?;
    if values.insert(value.id(), value.ty()).is_some() {
        return fail(
            VerificationCode::InvalidIdentity,
            format!("value {} is defined more than once", value.id().index()),
        );
    }
    Ok(())
}

fn activate_if_owned(
    active: &mut BTreeSet<ValueId>,
    value: TypedValue,
    types: &BTreeMap<TypeId, &NirType>,
) -> Result<(), VerificationError> {
    if ownership(types, value.ty())? == ValueOwnership::Owned {
        active.insert(value.id());
    }
    Ok(())
}

fn consume(value: ValueId, active_owned: &mut BTreeSet<ValueId>) -> Result<(), VerificationError> {
    if !active_owned.remove(&value) {
        return fail(
            VerificationCode::UseAfterConsume,
            format!("owned value {} was already consumed", value.index()),
        );
    }
    Ok(())
}

fn use_type(
    value: ValueId,
    all_values: &BTreeMap<ValueId, TypeId>,
    visible: &BTreeSet<ValueId>,
) -> Result<TypeId, VerificationError> {
    let ty = all_values
        .get(&value)
        .copied()
        .ok_or_else(|| VerificationError {
            code: VerificationCode::UnknownValue,
            message: format!("value {} is not defined", value.index()),
        })?;
    if !visible.contains(&value) {
        return fail(
            VerificationCode::UnknownValue,
            format!("value {} does not dominate this use", value.index()),
        );
    }
    Ok(ty)
}

fn require_value_kind(
    value: ValueId,
    all_values: &BTreeMap<ValueId, TypeId>,
    visible: &BTreeSet<ValueId>,
    types: &BTreeMap<TypeId, &NirType>,
    expected: NirType,
) -> Result<(), VerificationError> {
    let ty = use_type(value, all_values, visible)?;
    if types.get(&ty).copied() != Some(&expected) {
        return type_mismatch("value has the wrong logical type");
    }
    Ok(())
}

fn require_result_type(
    result: Option<TypedValue>,
    all_values: &BTreeMap<ValueId, TypeId>,
    types: &BTreeMap<TypeId, &NirType>,
    expected: NirType,
) -> Result<(), VerificationError> {
    let result = result.ok_or_else(|| VerificationError {
        code: VerificationCode::InvalidInstruction,
        message: "value-producing instruction has no result".to_owned(),
    })?;
    let ty = all_values
        .get(&result.id())
        .copied()
        .ok_or_else(|| VerificationError {
            code: VerificationCode::UnknownValue,
            message: format!("result value {} is not defined", result.id().index()),
        })?;
    if ty != result.ty() || types.get(&ty).copied() != Some(&expected) {
        return type_mismatch("instruction result has the wrong logical type");
    }
    Ok(())
}

fn require_result_exact(
    result: Option<TypedValue>,
    expected: TypeId,
) -> Result<(), VerificationError> {
    if result.map(TypedValue::ty) != Some(expected) {
        return type_mismatch("instruction result type does not match its input or signature");
    }
    Ok(())
}

fn require_no_result(result: Option<TypedValue>, operation: &str) -> Result<(), VerificationError> {
    if result.is_some() {
        return fail(
            VerificationCode::InvalidInstruction,
            format!("{operation} must not define a result"),
        );
    }
    Ok(())
}

fn ownership(
    types: &BTreeMap<TypeId, &NirType>,
    id: TypeId,
) -> Result<ValueOwnership, VerificationError> {
    ownership_inner(types, id, &mut BTreeSet::new())
}

fn ownership_inner(
    types: &BTreeMap<TypeId, &NirType>,
    id: TypeId,
    visiting: &mut BTreeSet<TypeId>,
) -> Result<ValueOwnership, VerificationError> {
    if !visiting.insert(id) {
        return Ok(ValueOwnership::Copy);
    }
    let ownership = match require_type(types, id, "value")? {
        NirType::OwnedPtr { .. } => ValueOwnership::Owned,
        NirType::BorrowPtr { .. } => ValueOwnership::Borrowed,
        NirType::MutBorrowPtr { .. } => ValueOwnership::MutBorrowed,
        NirType::Handle { ownership, .. } => *ownership,
        NirType::Struct { fields } | NirType::TaggedUnion { variants: fields } => {
            let mut aggregate = ValueOwnership::Copy;
            for field in fields {
                aggregate = combine_ownership(aggregate, ownership_inner(types, *field, visiting)?);
            }
            aggregate
        }
        NirType::FixedArray { element, length } if *length > 0 => {
            ownership_inner(types, *element, visiting)?
        }
        NirType::I1
        | NirType::I8
        | NirType::I16
        | NirType::I32
        | NirType::I64
        | NirType::U8
        | NirType::U16
        | NirType::U32
        | NirType::U64
        | NirType::F32
        | NirType::F64
        | NirType::Char32
        | NirType::Unit
        | NirType::Never
        | NirType::RawPtr { .. }
        | NirType::FixedArray { .. }
        | NirType::FunctionRef => ValueOwnership::Copy,
    };
    visiting.remove(&id);
    Ok(ownership)
}

const fn combine_ownership(left: ValueOwnership, right: ValueOwnership) -> ValueOwnership {
    match (left, right) {
        (ValueOwnership::Owned, _) | (_, ValueOwnership::Owned) => ValueOwnership::Owned,
        (ValueOwnership::MutBorrowed, _) | (_, ValueOwnership::MutBorrowed) => {
            ValueOwnership::MutBorrowed
        }
        (ValueOwnership::Borrowed, _) | (_, ValueOwnership::Borrowed) => ValueOwnership::Borrowed,
        (ValueOwnership::Copy, ValueOwnership::Copy) => ValueOwnership::Copy,
    }
}

fn require_type<'a>(
    types: &'a BTreeMap<TypeId, &NirType>,
    id: TypeId,
    context: &str,
) -> Result<&'a NirType, VerificationError> {
    types.get(&id).copied().ok_or_else(|| VerificationError {
        code: VerificationCode::UnknownType,
        message: format!("{context} references unknown type {}", id.index()),
    })
}

fn verify_identifier(identifier: &str, kind: &str) -> Result<(), VerificationError> {
    validate_identifier(identifier).map_err(|_| VerificationError {
        code: VerificationCode::InvalidIdentity,
        message: format!("{kind} identifier `{identifier}` is invalid"),
    })
}

fn verify_span(span: crate::NirSpan, context: &str) -> Result<(), VerificationError> {
    if span.start() > span.end() {
        return fail(
            VerificationCode::InvalidInstruction,
            format!(
                "{context} has reversed source span {}..{}",
                span.start(),
                span.end()
            ),
        );
    }
    Ok(())
}

fn verify_dense_ids(
    ids: impl IntoIterator<Item = u32>,
    kind: &str,
) -> Result<(), VerificationError> {
    for (expected, actual) in (0_u32..).zip(ids) {
        if expected != actual {
            return fail(
                VerificationCode::InvalidIdentity,
                format!("{kind} ids must be unique and dense; expected {expected}, found {actual}"),
            );
        }
    }
    Ok(())
}

fn type_mismatch<T>(message: &str) -> Result<T, VerificationError> {
    fail(VerificationCode::TypeMismatch, message.to_owned())
}

fn fail<T>(code: VerificationCode, message: String) -> Result<T, VerificationError> {
    Err(VerificationError { code, message })
}
