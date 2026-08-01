use nexa_hir::Type;
use nexa_mir::{
    Callee, LocalId as MirLocalId, MirExpression, MirFunction, MirProgram, MirStatement,
    MirTerminator,
};
use nexa_nir::{
    BlockId, BuilderError, FunctionId, InstructionId, IntrinsicId, ModuleBuilder, NirSpan, NirType,
    Operation, Terminator, TypeId, TypedValue, ValueId, VerificationError, VerifiedModule,
    Verifier,
};
use nexa_span::SourceSpan;
use thiserror::Error;

const I1: TypeId = TypeId::new(0);
const I64: TypeId = TypeId::new(1);
const UNIT: TypeId = TypeId::new(2);

pub(crate) enum NirLoweringOutcome {
    Produced(VerifiedModule),
    Deferred(NirDeferred),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NirDeferred {
    pub(crate) reason_code: &'static str,
    pub(crate) planned_phase: &'static str,
}

impl NirDeferred {
    const fn unsupported_type() -> Self {
        Self {
            reason_code: "unsupported-type",
            planned_phase: "ADR-003 N4",
        }
    }

    const fn local_storage() -> Self {
        Self {
            reason_code: "local-storage-lowering",
            planned_phase: "toolchain 0.0.11",
        }
    }

    const fn control_flow() -> Self {
        Self {
            reason_code: "multi-block-lowering",
            planned_phase: "toolchain 0.0.11",
        }
    }

    const fn unsupported_operation() -> Self {
        Self {
            reason_code: "unsupported-operation",
            planned_phase: "ADR-003 N2-N4",
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum NirLoweringError {
    #[error(transparent)]
    Builder(#[from] BuilderError),
    #[error(transparent)]
    Verification(#[from] VerificationError),
    #[error("NIR source offset exceeded the canonical u32 range")]
    SourceOffsetOverflow,
    #[error("NIR identity exceeded the canonical u32 range")]
    IdentityOverflow,
    #[error("supported MIR referenced an unknown function")]
    UnknownFunction,
    #[error("supported MIR referenced a non-parameter local")]
    UnknownLocal,
    #[error("supported MIR produced Unit where a value was required")]
    UnitValue,
}

pub(crate) fn lower(program: &MirProgram) -> Result<NirLoweringOutcome, NirLoweringError> {
    if let Err(deferred) = supports_n1(program) {
        return Ok(NirLoweringOutcome::Deferred(deferred));
    }

    let mut builder = ModuleBuilder::new(format!("module-{}", program.entry_module().index()))?;
    builder.define_type(I1, NirType::I1)?;
    builder.define_type(I64, NirType::I64)?;
    builder.define_type(UNIT, NirType::Unit)?;

    let function_ids = program
        .functions()
        .iter()
        .enumerate()
        .map(|(index, function)| {
            Ok((
                function.id(),
                FunctionId::new(
                    u32::try_from(index).map_err(|_| NirLoweringError::IdentityOverflow)?,
                ),
            ))
        })
        .collect::<Result<Vec<_>, NirLoweringError>>()?;

    for function in program.functions() {
        let id = mapped_function(&function_ids, function.id())?;
        let parameters = function
            .parameter_ids()
            .iter()
            .zip(function.parameter_types())
            .enumerate()
            .map(|(index, (_, ty))| {
                Ok(TypedValue::new(
                    ValueId::new(
                        u32::try_from(index).map_err(|_| NirLoweringError::IdentityOverflow)?,
                    ),
                    lower_type(ty).ok_or(NirLoweringError::UnitValue)?,
                ))
            })
            .collect::<Result<Vec<_>, NirLoweringError>>()?;
        builder.define_function(
            id,
            format!("m{}::{}", function.id().module().index(), function.name()),
            parameters,
            lower_type(function.return_type()).ok_or(NirLoweringError::UnitValue)?,
            BlockId::new(0),
        )?;
        builder.define_block(id, BlockId::new(0), [])?;
    }

    for function in program.functions() {
        lower_function(&mut builder, function, &function_ids, program)?;
    }

    Ok(NirLoweringOutcome::Produced(Verifier::verify(
        builder.finish(),
    )?))
}

fn supports_n1(program: &MirProgram) -> Result<(), NirDeferred> {
    if !program.records().is_empty()
        || !program.unions().is_empty()
        || !program.closures().is_empty()
    {
        return Err(NirDeferred::unsupported_type());
    }
    for function in program.functions() {
        if lower_type(function.return_type()).is_none()
            || function
                .parameter_types()
                .iter()
                .any(|ty| lower_type(ty).is_none())
        {
            return Err(NirDeferred::unsupported_type());
        }
        if function.blocks().len() != 1 {
            return Err(NirDeferred::control_flow());
        }
        let block = &function.blocks()[0];
        let mut assigned = function.parameter_ids().to_vec();
        for statement in block.statements() {
            match statement {
                MirStatement::Store { local, value, .. } => {
                    if assigned.contains(local) {
                        return Err(NirDeferred::local_storage());
                    }
                    assigned.push(*local);
                    supports_expression(value)?;
                }
                MirStatement::Expression { expression, .. } => supports_expression(expression)?,
            }
        }
        match block.terminator() {
            MirTerminator::Return { value, .. } => {
                if let Some(value) = value {
                    supports_expression(value)?;
                }
            }
            MirTerminator::Goto { .. }
            | MirTerminator::Branch { .. }
            | MirTerminator::SwitchVariant { .. } => return Err(NirDeferred::control_flow()),
        }
    }
    Ok(())
}

fn supports_expression(expression: &MirExpression) -> Result<(), NirDeferred> {
    match expression {
        MirExpression::Integer { .. }
        | MirExpression::Boolean { .. }
        | MirExpression::Local { .. } => Ok(()),
        MirExpression::Call {
            callee: Callee::Function(_) | Callee::Print,
            arguments,
            ..
        } => {
            for argument in arguments {
                supports_expression(argument)?;
            }
            Ok(())
        }
        MirExpression::String { .. }
        | MirExpression::Array { .. }
        | MirExpression::Record { .. }
        | MirExpression::Variant { .. }
        | MirExpression::Closure { .. }
        | MirExpression::Function { .. } => Err(NirDeferred::unsupported_type()),
        MirExpression::Index { .. }
        | MirExpression::Length { .. }
        | MirExpression::ArrayIntrinsic { .. }
        | MirExpression::Field { .. }
        | MirExpression::VariantPayload { .. }
        | MirExpression::Unary { .. }
        | MirExpression::Binary { .. }
        | MirExpression::IndirectCall { .. }
        | MirExpression::Call {
            callee: Callee::ToString | Callee::ParseInt,
            ..
        } => Err(NirDeferred::unsupported_operation()),
    }
}

fn lower_function(
    builder: &mut ModuleBuilder,
    function: &MirFunction,
    function_ids: &[(nexa_mir::FunctionId, FunctionId)],
    program: &MirProgram,
) -> Result<(), NirLoweringError> {
    let function_id = mapped_function(function_ids, function.id())?;
    let locals = function
        .parameter_ids()
        .iter()
        .zip(function.parameter_types())
        .enumerate()
        .map(|(index, (local, ty))| {
            Ok((
                *local,
                LoweredValue::Value {
                    id: ValueId::new(
                        u32::try_from(index).map_err(|_| NirLoweringError::IdentityOverflow)?,
                    ),
                    ty: lower_type(ty).ok_or(NirLoweringError::UnitValue)?,
                },
            ))
        })
        .collect::<Result<Vec<_>, NirLoweringError>>()?;
    let values = locals
        .iter()
        .filter_map(|(_, value)| value.parts())
        .collect();
    let mut lowerer = FunctionLowerer {
        builder,
        function: function_id,
        block: BlockId::new(0),
        next_instruction: 0,
        next_value: u32::try_from(function.parameter_count())
            .map_err(|_| NirLoweringError::IdentityOverflow)?,
        locals,
        values,
        function_ids,
        program,
    };
    let block = &function.blocks()[0];
    for statement in block.statements() {
        match statement {
            MirStatement::Store { local, value, .. } => {
                let value = lowerer.expression(value)?;
                if lowerer
                    .locals
                    .iter()
                    .any(|(candidate, _)| candidate == local)
                {
                    return Err(NirLoweringError::UnknownLocal);
                }
                lowerer.locals.push((*local, value));
            }
            MirStatement::Expression { expression, .. } => {
                let _ = lowerer.expression(expression)?;
            }
        }
    }
    let MirTerminator::Return { value, span } = block.terminator() else {
        return Err(NirLoweringError::UnknownLocal);
    };
    let value = value
        .as_ref()
        .map(|value| lowerer.expression(value).and_then(LoweredValue::required))
        .transpose()?;
    lowerer.builder.set_terminator(
        function_id,
        BlockId::new(0),
        Terminator::Return { value },
        lower_span(*span)?,
    )?;
    Ok(())
}

struct FunctionLowerer<'a> {
    builder: &'a mut ModuleBuilder,
    function: FunctionId,
    block: BlockId,
    next_instruction: u32,
    next_value: u32,
    locals: Vec<(MirLocalId, LoweredValue)>,
    values: Vec<(ValueId, TypeId)>,
    function_ids: &'a [(nexa_mir::FunctionId, FunctionId)],
    program: &'a MirProgram,
}

impl FunctionLowerer<'_> {
    fn expression(&mut self, expression: &MirExpression) -> Result<LoweredValue, NirLoweringError> {
        match expression {
            MirExpression::Integer { value, span } => {
                self.emit_value(I64, Operation::ConstI64 { value: *value }, *span)
            }
            MirExpression::Boolean { value, span } => {
                self.emit_value(I1, Operation::ConstI1 { value: *value }, *span)
            }
            MirExpression::Local { local, .. } => self
                .locals
                .iter()
                .find_map(|(candidate, value)| (*candidate == *local).then_some(*value))
                .ok_or(NirLoweringError::UnknownLocal),
            MirExpression::Call {
                callee,
                arguments,
                span,
            } => self.call(*callee, arguments, *span),
            _ => Err(NirLoweringError::UnitValue),
        }
    }

    fn call(
        &mut self,
        callee: Callee,
        arguments: &[MirExpression],
        span: SourceSpan,
    ) -> Result<LoweredValue, NirLoweringError> {
        let arguments = arguments
            .iter()
            .map(|argument| self.expression(argument).and_then(LoweredValue::required))
            .collect::<Result<Vec<_>, _>>()?;
        match callee {
            Callee::Function(source_id) => {
                let function = mapped_function(self.function_ids, source_id)?;
                let return_type = self
                    .program
                    .function(source_id)
                    .and_then(|function| lower_type(function.return_type()))
                    .ok_or(NirLoweringError::UnknownFunction)?;
                let operation = Operation::Call {
                    function,
                    arguments,
                };
                if return_type == UNIT {
                    self.emit_effect(operation, span)?;
                    Ok(LoweredValue::Unit)
                } else {
                    self.emit_value(return_type, operation, span)
                }
            }
            Callee::Print => {
                let [argument] = arguments.as_slice() else {
                    return Err(NirLoweringError::UnitValue);
                };
                let ty = self.value_type(*argument)?;
                let intrinsic = match ty {
                    I64 => IntrinsicId::PrintI64,
                    I1 => IntrinsicId::PrintI1,
                    _ => return Err(NirLoweringError::UnitValue),
                };
                self.emit_effect(
                    Operation::CallIntrinsic {
                        intrinsic,
                        arguments,
                    },
                    span,
                )?;
                Ok(LoweredValue::Unit)
            }
            Callee::ToString | Callee::ParseInt => Err(NirLoweringError::UnitValue),
        }
    }

    fn emit_value(
        &mut self,
        ty: TypeId,
        operation: Operation,
        span: SourceSpan,
    ) -> Result<LoweredValue, NirLoweringError> {
        let id = ValueId::new(self.next_value);
        self.next_value = self
            .next_value
            .checked_add(1)
            .ok_or(NirLoweringError::IdentityOverflow)?;
        self.emit(Some(TypedValue::new(id, ty)), operation, span)?;
        self.values.push((id, ty));
        Ok(LoweredValue::Value { id, ty })
    }

    fn emit_effect(
        &mut self,
        operation: Operation,
        span: SourceSpan,
    ) -> Result<(), NirLoweringError> {
        self.emit(None, operation, span)
    }

    fn emit(
        &mut self,
        result: Option<TypedValue>,
        operation: Operation,
        span: SourceSpan,
    ) -> Result<(), NirLoweringError> {
        let instruction = InstructionId::new(self.next_instruction);
        self.next_instruction = self
            .next_instruction
            .checked_add(1)
            .ok_or(NirLoweringError::IdentityOverflow)?;
        self.builder.append_instruction(
            self.function,
            self.block,
            instruction,
            result,
            operation,
            lower_span(span)?,
        )?;
        Ok(())
    }

    fn value_type(&self, id: ValueId) -> Result<TypeId, NirLoweringError> {
        self.values
            .iter()
            .find_map(|(candidate, ty)| (*candidate == id).then_some(*ty))
            .ok_or(NirLoweringError::UnknownLocal)
    }
}

#[derive(Debug, Clone, Copy)]
enum LoweredValue {
    Unit,
    Value { id: ValueId, ty: TypeId },
}

impl LoweredValue {
    fn required(self) -> Result<ValueId, NirLoweringError> {
        match self {
            Self::Unit => Err(NirLoweringError::UnitValue),
            Self::Value { id, .. } => Ok(id),
        }
    }

    const fn parts(self) -> Option<(ValueId, TypeId)> {
        match self {
            Self::Unit => None,
            Self::Value { id, ty } => Some((id, ty)),
        }
    }
}

fn lower_type(ty: &Type) -> Option<TypeId> {
    match ty {
        Type::Bool => Some(I1),
        Type::Int => Some(I64),
        Type::Unit => Some(UNIT),
        Type::String
        | Type::Array(_)
        | Type::Function { .. }
        | Type::Parameter(_)
        | Type::Record { .. }
        | Type::Union { .. } => None,
    }
}

fn mapped_function(
    function_ids: &[(nexa_mir::FunctionId, FunctionId)],
    source: nexa_mir::FunctionId,
) -> Result<FunctionId, NirLoweringError> {
    function_ids
        .iter()
        .find_map(|(candidate, lowered)| (*candidate == source).then_some(*lowered))
        .ok_or(NirLoweringError::UnknownFunction)
}

fn lower_span(span: SourceSpan) -> Result<NirSpan, NirLoweringError> {
    Ok(NirSpan::new(
        span.file().raw(),
        u32::try_from(span.range().start()).map_err(|_| NirLoweringError::SourceOffsetOverflow)?,
        u32::try_from(span.range().end()).map_err(|_| NirLoweringError::SourceOffsetOverflow)?,
    ))
}
