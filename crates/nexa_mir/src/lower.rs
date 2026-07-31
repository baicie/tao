use std::fmt::{Display, Formatter};

use nexa_hir::{
    ArrowBody, BinaryOperator, Block, Builtin, CallFacts, ClosureFacts, ClosureId, Expression,
    FieldId, ForOfStatement, Function, FunctionId as HirFunctionId, IfStatement, MatchArm,
    MatchArmFacts, MatchPattern, Name, NameResolution, Parameter, PayloadId, RecordFacts,
    RecordFieldInitializer, RecordId, ReturnStatement, Statement, Type, TypeParameterFacts,
    TypeParameterId, TypedProgram, UnionFacts, UnionId, VariantConstructionFacts, VariantId,
    WhileStatement,
};
use nexa_span::SourceSpan;

use crate::{
    ArrayIntrinsic, BasicBlockId, Callee, LocalId, MirBasicBlock, MirClosure, MirExpression,
    MirFunction, MirPayload, MirProgram, MirRecord, MirRecordField, MirStatement, MirTerminator,
    MirUnion, MirVariant,
};

/// An invariant violation while lowering validated HIR to MIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirLoweringError {
    message: String,
    span: SourceSpan,
}

impl MirLoweringError {
    /// Returns the source range associated with the invalid HIR shape.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    /// Returns the invariant violation description.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for MirLoweringError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MirLoweringError {}

/// Lowers validated typed HIR into a resolved control-flow graph.
///
/// # Errors
///
/// Returns [`MirLoweringError`] only when the supplied typed HIR violates the
/// semantic invariants enforced by [`nexa_hir::type_check`].
pub fn lower(typed: &TypedProgram) -> Result<MirProgram, MirLoweringError> {
    let entry_program = typed.program();
    let records = typed
        .records()
        .iter()
        .map(|record| lower_record_layout(record.id(), record))
        .collect::<Result<Vec<_>, _>>()?;
    let unions = typed
        .unions()
        .iter()
        .map(|union| lower_union_layout(union.id(), union))
        .collect::<Result<Vec<_>, _>>()?;
    let lowered_functions = typed
        .modules()
        .iter()
        .flat_map(|program| {
            program
                .functions
                .iter()
                .enumerate()
                .map(move |(index, function)| {
                    (HirFunctionId::in_module(program.module, index), function)
                })
        })
        .map(|(id, function)| lower_function(id, function, typed))
        .collect::<Result<Vec<_>, _>>()?;
    let mut functions = Vec::with_capacity(lowered_functions.len());
    let mut closures = Vec::new();
    for lowered in lowered_functions {
        functions.push(lowered.function);
        closures.extend(lowered.closures);
    }
    closures.sort_by_key(|closure| {
        (
            closure.id.owner().module().index(),
            closure.id.owner().index(),
            closure.id.source_index(),
        )
    });
    let entry = entry_program
        .functions
        .iter()
        .position(|function| function.name.text == "main")
        .map(|index| HirFunctionId::in_module(entry_program.module, index));

    Ok(MirProgram {
        modules: typed
            .modules()
            .iter()
            .map(|program| program.module)
            .collect(),
        entry_module: entry_program.module,
        entry,
        records,
        unions,
        functions,
        closures,
        span: entry_program.span,
    })
}

struct LoweredFunction {
    function: MirFunction,
    closures: Vec<MirClosure>,
}

fn lower_record_layout(
    expected_id: RecordId,
    record: &RecordFacts,
) -> Result<MirRecord, MirLoweringError> {
    if record.id() != expected_id {
        return Err(error(
            record.span(),
            format!(
                "record `{}` has inconsistent typed HIR source-order identity",
                record.name()
            ),
        ));
    }

    let fields = record
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let expected_field = FieldId::new(expected_id, index);
            if field.id() != expected_field {
                return Err(error(
                    field.span(),
                    format!(
                        "field `{}` has inconsistent typed HIR declaration-order identity",
                        field.name()
                    ),
                ));
            }

            Ok(MirRecordField {
                id: expected_field,
                name: field.name().to_owned(),
                ty: field.ty().clone(),
                span: field.span(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(MirRecord {
        id: expected_id,
        name: record.name().to_owned(),
        fields,
        span: record.span(),
    })
}

fn lower_union_layout(
    expected_id: UnionId,
    union: &UnionFacts,
) -> Result<MirUnion, MirLoweringError> {
    if union.id() != expected_id {
        return Err(error(
            union.span(),
            format!(
                "union `{}` has inconsistent typed HIR source-order identity",
                union.name()
            ),
        ));
    }

    let variants = union
        .variants()
        .iter()
        .enumerate()
        .map(|(variant_index, variant)| {
            let expected_variant = VariantId::new(expected_id, variant_index);
            if variant.id() != expected_variant {
                return Err(error(
                    variant.span(),
                    format!(
                        "variant `{}` has inconsistent typed HIR declaration-order identity",
                        variant.name()
                    ),
                ));
            }

            let payloads = variant
                .payloads()
                .iter()
                .enumerate()
                .map(|(payload_index, payload)| {
                    let expected_payload = PayloadId::new(expected_variant, payload_index);
                    if payload.id() != expected_payload {
                        return Err(error(
                            payload.span(),
                            format!(
                                "payload `{}` has inconsistent typed HIR positional identity",
                                payload.name()
                            ),
                        ));
                    }

                    Ok(MirPayload {
                        id: expected_payload,
                        name: payload.name().to_owned(),
                        ty: payload.ty().clone(),
                        span: payload.span(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(MirVariant {
                id: expected_variant,
                name: variant.name().to_owned(),
                payloads,
                span: variant.span(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(MirUnion {
        id: expected_id,
        name: union.name().to_owned(),
        variants,
        span: union.span(),
    })
}

fn lower_function(
    function_id: HirFunctionId,
    function: &Function,
    typed: &TypedProgram,
) -> Result<LoweredFunction, MirLoweringError> {
    if typed.name_resolution(function.name.span) != Some(NameResolution::Function(function_id)) {
        return Err(error(
            function.name.span,
            format!(
                "function `{}` has no matching typed HIR resolution",
                function.name.text
            ),
        ));
    }
    let facts = typed.function_facts(function_id).ok_or_else(|| {
        error(
            function.span,
            format!(
                "function `{}` has no typed HIR local facts",
                function.name.text
            ),
        )
    })?;
    if facts.parameter_ids().len() != function.parameters.len() {
        return Err(error(
            function.span,
            format!(
                "function `{}` has inconsistent typed HIR parameter facts",
                function.name.text
            ),
        ));
    }
    if facts.parameter_types().len() != function.parameters.len() {
        return Err(error(
            function.span,
            format!(
                "function `{}` has inconsistent typed HIR parameter type facts",
                function.name.text
            ),
        ));
    }
    for (parameter, local) in function.parameters.iter().zip(facts.parameter_ids()) {
        if typed.name_resolution(parameter.name.span) != Some(NameResolution::Local(*local)) {
            return Err(error(
                parameter.name.span,
                format!(
                    "parameter `{}` has no matching typed HIR local resolution",
                    parameter.name.text
                ),
            ));
        }
    }

    let mut lowerer = FunctionLowerer {
        typed,
        function_id,
        parameters: facts
            .parameter_ids()
            .iter()
            .map(|local| LocalId(local.index()))
            .collect(),
        type_parameters: facts
            .type_parameters()
            .iter()
            .map(TypeParameterFacts::id)
            .collect(),
        hir_local_count: facts.local_count(),
        next_local: facts.local_count(),
        blocks: Vec::new(),
        loop_targets: Vec::new(),
        closures: Vec::new(),
    };

    let entry = lowerer.new_block(function.body.span);
    let end = lowerer.lower_block(&function.body, Some(entry))?;
    if let Some(end) = end {
        if facts.return_type() != &Type::Unit {
            return Err(error(
                function.return_type.span,
                "non-Unit function reached the end during MIR lowering",
            ));
        }
        lowerer.terminate(
            end,
            MirTerminator::Return {
                value: None,
                span: function.body.span,
            },
        )?;
    }

    let FinishedLowering {
        parameters,
        local_count,
        blocks,
        closures,
    } = lowerer.finish()?;

    Ok(LoweredFunction {
        function: MirFunction {
            id: function_id,
            name: function.name.text.clone(),
            parameters,
            parameter_types: facts.parameter_types().to_vec(),
            local_count,
            return_type: facts.return_type().clone(),
            entry,
            blocks,
            span: function.span,
        },
        closures,
    })
}

#[derive(Debug, Clone, Copy)]
struct LoopTargets {
    break_target: BasicBlockId,
    continue_target: BasicBlockId,
}

struct PendingBlock {
    statements: Vec<MirStatement>,
    terminator: Option<MirTerminator>,
    span: SourceSpan,
}

struct FunctionLowerer<'typed> {
    typed: &'typed TypedProgram,
    function_id: HirFunctionId,
    parameters: Vec<LocalId>,
    type_parameters: Vec<TypeParameterId>,
    hir_local_count: usize,
    next_local: usize,
    blocks: Vec<PendingBlock>,
    loop_targets: Vec<LoopTargets>,
    closures: Vec<MirClosure>,
}

struct FinishedLowering {
    parameters: Vec<LocalId>,
    local_count: usize,
    blocks: Vec<MirBasicBlock>,
    closures: Vec<MirClosure>,
}

impl FunctionLowerer<'_> {
    fn lower_block(
        &mut self,
        block: &Block,
        start: Option<BasicBlockId>,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        self.lower_statements(&block.statements, start)
    }

    fn lower_statements(
        &mut self,
        statements: &[Statement],
        mut current: Option<BasicBlockId>,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        for statement in statements {
            let Some(block) = current else {
                break;
            };
            current = self.lower_statement(statement, block)?;
        }

        Ok(current)
    }

    fn lower_statement(
        &mut self,
        statement: &Statement,
        current: BasicBlockId,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        match statement {
            Statement::Const(declaration) => self.lower_binding(
                &declaration.name,
                &declaration.initializer,
                declaration.span,
                current,
            ),
            Statement::Let(declaration) => self.lower_binding(
                &declaration.name,
                &declaration.initializer,
                declaration.span,
                current,
            ),
            Statement::Assignment(statement) => {
                let local = self.resolve_local(&statement.target, "assignment target")?;
                let (current, value) = self.lower_expression(&statement.value, current)?;
                self.push_statement(
                    current,
                    MirStatement::Store {
                        local,
                        value,
                        span: statement.span,
                    },
                )?;
                Ok(Some(current))
            }
            Statement::While(statement) => self.lower_while(statement, current),
            Statement::ForOf(statement) => self.lower_for_of(statement, current),
            Statement::Break(statement) => {
                let targets = self.loop_targets.last().copied().ok_or_else(|| {
                    error(statement.span, "break outside a loop reached MIR lowering")
                })?;
                self.terminate(
                    current,
                    MirTerminator::Goto {
                        target: targets.break_target,
                        span: statement.span,
                    },
                )?;
                Ok(None)
            }
            Statement::Continue(statement) => {
                let targets = self.loop_targets.last().copied().ok_or_else(|| {
                    error(
                        statement.span,
                        "continue outside a loop reached MIR lowering",
                    )
                })?;
                self.terminate(
                    current,
                    MirTerminator::Goto {
                        target: targets.continue_target,
                        span: statement.span,
                    },
                )?;
                Ok(None)
            }
            Statement::If(statement) => self.lower_if(statement, current),
            Statement::Return(statement) => self.lower_return(statement, current),
            Statement::Expression(statement) => {
                let (current, expression) =
                    self.lower_expression(&statement.expression, current)?;
                self.push_statement(
                    current,
                    MirStatement::Expression {
                        expression,
                        span: statement.span,
                    },
                )?;
                Ok(Some(current))
            }
        }
    }

    fn lower_binding(
        &mut self,
        name: &Name,
        initializer: &Expression,
        span: SourceSpan,
        current: BasicBlockId,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        let (current, value) = self.lower_expression(initializer, current)?;
        let local = self.resolve_local(name, "binding")?;
        self.push_statement(current, MirStatement::Store { local, value, span })?;

        Ok(Some(current))
    }

    fn lower_if(
        &mut self,
        statement: &IfStatement,
        current: BasicBlockId,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        let (condition_end, condition) = self.lower_expression(&statement.condition, current)?;
        let then_start = self.new_block(statement.then_branch.span);
        let else_span = statement
            .else_branch
            .as_ref()
            .map_or(statement.span, |branch| branch.span);
        let else_start = self.new_block(else_span);
        self.terminate(
            condition_end,
            MirTerminator::Branch {
                condition,
                then_target: then_start,
                else_target: else_start,
                span: statement.span,
            },
        )?;

        let then_end = self.lower_block(&statement.then_branch, Some(then_start))?;
        let else_end = statement
            .else_branch
            .as_ref()
            .map_or(Ok(Some(else_start)), |branch| {
                self.lower_block(branch, Some(else_start))
            })?;

        self.merge_paths(then_end, else_end, statement.span)
    }

    fn lower_while(
        &mut self,
        statement: &WhileStatement,
        current: BasicBlockId,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        let condition_block = self.new_block(statement.condition.span());
        let body_block = self.new_block(statement.body.span);
        let exit_block = self.new_block(statement.span);

        self.terminate(
            current,
            MirTerminator::Goto {
                target: condition_block,
                span: statement.span,
            },
        )?;

        let (condition_end, condition) =
            self.lower_expression(&statement.condition, condition_block)?;
        self.terminate(
            condition_end,
            MirTerminator::Branch {
                condition,
                then_target: body_block,
                else_target: exit_block,
                span: statement.span,
            },
        )?;

        self.loop_targets.push(LoopTargets {
            break_target: exit_block,
            continue_target: condition_block,
        });
        let body_result = self.lower_block(&statement.body, Some(body_block));
        let _ = self.loop_targets.pop();
        if let Some(body_end) = body_result? {
            self.terminate(
                body_end,
                MirTerminator::Goto {
                    target: condition_block,
                    span: statement.body.span,
                },
            )?;
        }

        Ok(Some(exit_block))
    }

    fn lower_for_of(
        &mut self,
        statement: &ForOfStatement,
        current: BasicBlockId,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        let iterable_type = self.expression_type(&statement.iterable, "for...of iterable")?;
        if !matches!(iterable_type, Type::Array(_)) {
            return Err(error(
                statement.iterable.span(),
                "for...of iterable does not have an array type",
            ));
        }
        let binding = self.resolve_local(&statement.binding, "for...of binding")?;

        let (current, iterable) = self.lower_expression(&statement.iterable, current)?;
        let iterable_local = self.allocate_local();
        self.push_statement(
            current,
            MirStatement::Store {
                local: iterable_local,
                value: iterable,
                span: statement.iterable.span(),
            },
        )?;
        let length_local = self.allocate_local();
        self.push_statement(
            current,
            MirStatement::Store {
                local: length_local,
                value: MirExpression::Length {
                    target: Box::new(MirExpression::Local {
                        local: iterable_local,
                        span: statement.iterable.span(),
                    }),
                    span: statement.iterable.span(),
                },
                span: statement.iterable.span(),
            },
        )?;
        let index_local = self.allocate_local();
        self.push_statement(
            current,
            MirStatement::Store {
                local: index_local,
                value: MirExpression::Integer {
                    value: 0,
                    span: statement.span,
                },
                span: statement.span,
            },
        )?;

        let condition_block = self.new_block(statement.iterable.span());
        let body_block = self.new_block(statement.body.span);
        let increment_block = self.new_block(statement.span);
        let exit_block = self.new_block(statement.span);
        self.terminate(
            current,
            MirTerminator::Goto {
                target: condition_block,
                span: statement.span,
            },
        )?;
        self.terminate(
            condition_block,
            MirTerminator::Branch {
                condition: MirExpression::Binary {
                    operator: BinaryOperator::Less,
                    left: Box::new(MirExpression::Local {
                        local: index_local,
                        span: statement.span,
                    }),
                    right: Box::new(MirExpression::Local {
                        local: length_local,
                        span: statement.iterable.span(),
                    }),
                    span: statement.span,
                },
                then_target: body_block,
                else_target: exit_block,
                span: statement.span,
            },
        )?;
        self.push_statement(
            body_block,
            MirStatement::Store {
                local: binding,
                value: MirExpression::Index {
                    target: Box::new(MirExpression::Local {
                        local: iterable_local,
                        span: statement.iterable.span(),
                    }),
                    index: Box::new(MirExpression::Local {
                        local: index_local,
                        span: statement.span,
                    }),
                    span: statement.binding.span,
                },
                span: statement.binding.span,
            },
        )?;

        self.loop_targets.push(LoopTargets {
            break_target: exit_block,
            continue_target: increment_block,
        });
        let body_result = self.lower_block(&statement.body, Some(body_block));
        let _ = self.loop_targets.pop();
        if let Some(body_end) = body_result? {
            self.terminate(
                body_end,
                MirTerminator::Goto {
                    target: increment_block,
                    span: statement.body.span,
                },
            )?;
        }

        self.push_statement(
            increment_block,
            MirStatement::Store {
                local: index_local,
                value: MirExpression::Binary {
                    operator: BinaryOperator::Add,
                    left: Box::new(MirExpression::Local {
                        local: index_local,
                        span: statement.span,
                    }),
                    right: Box::new(MirExpression::Integer {
                        value: 1,
                        span: statement.span,
                    }),
                    span: statement.span,
                },
                span: statement.span,
            },
        )?;
        self.terminate(
            increment_block,
            MirTerminator::Goto {
                target: condition_block,
                span: statement.span,
            },
        )?;

        Ok(Some(exit_block))
    }

    fn lower_return(
        &mut self,
        statement: &ReturnStatement,
        current: BasicBlockId,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        let (current, value) = if let Some(value) = &statement.value {
            let (current, value) = self.lower_expression(value, current)?;
            (current, Some(value))
        } else {
            (current, None)
        };
        self.terminate(
            current,
            MirTerminator::Return {
                value,
                span: statement.span,
            },
        )?;

        Ok(None)
    }

    fn merge_paths(
        &mut self,
        left: Option<BasicBlockId>,
        right: Option<BasicBlockId>,
        span: SourceSpan,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        if left.is_none() && right.is_none() {
            return Ok(None);
        }

        let merge = self.new_block(span);
        for block in [left, right].into_iter().flatten() {
            self.terminate(
                block,
                MirTerminator::Goto {
                    target: merge,
                    span,
                },
            )?;
        }

        Ok(Some(merge))
    }

    fn lower_expression(
        &mut self,
        expression: &Expression,
        current: BasicBlockId,
    ) -> Result<(BasicBlockId, MirExpression), MirLoweringError> {
        self.require_expression_type(expression)?;

        match expression {
            Expression::Integer { value, span } => Ok((
                current,
                MirExpression::Integer {
                    value: *value,
                    span: *span,
                },
            )),
            Expression::Boolean { value, span } => Ok((
                current,
                MirExpression::Boolean {
                    value: *value,
                    span: *span,
                },
            )),
            Expression::String { value, span } => Ok((
                current,
                MirExpression::String {
                    value: value.clone(),
                    span: *span,
                },
            )),
            Expression::Array { elements, span } => {
                let mut current = current;
                let mut lowered_elements = Vec::with_capacity(elements.len());
                for element in elements {
                    let (next, element) = self.lower_expression(element, current)?;
                    current = next;
                    lowered_elements.push(element);
                }
                self.materialize(
                    current,
                    MirExpression::Array {
                        elements: lowered_elements,
                        span: *span,
                    },
                )
            }
            Expression::Record { fields, span } => self.lower_record(fields, *span, current),
            Expression::Match {
                scrutinee,
                arms,
                span,
            } => self.lower_match(scrutinee, arms, *span, current),
            Expression::Index {
                collection,
                index,
                span,
            } => {
                let (current, target) = self.lower_expression(collection, current)?;
                let (current, index) = self.lower_expression(index, current)?;
                self.materialize(
                    current,
                    MirExpression::Index {
                        target: Box::new(target),
                        index: Box::new(index),
                        span: *span,
                    },
                )
            }
            Expression::Member {
                object,
                member,
                span,
            } => match self.typed.name_resolution(member.span) {
                Some(NameResolution::Builtin(Builtin::ArrayLength | Builtin::StringLength)) => {
                    let (current, target) = self.lower_expression(object, current)?;
                    self.materialize(
                        current,
                        MirExpression::Length {
                            target: Box::new(target),
                            span: *span,
                        },
                    )
                }
                Some(NameResolution::Field(field)) => {
                    let record = match self.typed.expression_type(object.span()) {
                        Some(Type::Record { definition, .. }) => *definition,
                        Some(ty) => {
                            return Err(error(
                                object.span(),
                                format!(
                                    "record field base has typed HIR type `{ty}` instead of a record"
                                ),
                            ));
                        }
                        None => {
                            return Err(error(
                                object.span(),
                                "record field base has no typed HIR type fact",
                            ));
                        }
                    };
                    if field.record() != record {
                        return Err(error(
                            member.span,
                            "record field resolution does not match the base record type",
                        ));
                    }
                    let (current, target) = self.lower_expression(object, current)?;
                    self.materialize(
                        current,
                        MirExpression::Field {
                            target: Box::new(target),
                            record,
                            field,
                            span: *span,
                        },
                    )
                }
                Some(resolution) => Err(error(
                    member.span,
                    format!(
                        "member `{}` resolved to `{resolution:?}` instead of a field operation",
                        member.text
                    ),
                )),
                None => Err(error(
                    member.span,
                    format!(
                        "member `{}` has no matching typed HIR resolution",
                        member.text
                    ),
                )),
            },
            Expression::Name(name) => match self.typed.name_resolution(name.span) {
                Some(NameResolution::Local(_)) => {
                    let local = self.resolve_local(name, "value")?;
                    Ok((
                        current,
                        MirExpression::Local {
                            local,
                            span: name.span,
                        },
                    ))
                }
                Some(NameResolution::Function(function)) => {
                    let facts = self.typed.function_facts(function).ok_or_else(|| {
                        error(name.span, "function value has no typed HIR function facts")
                    })?;
                    if !facts.type_parameters().is_empty() {
                        return Err(error(
                            name.span,
                            "generic function reached MIR as a first-class value",
                        ));
                    }
                    Ok((
                        current,
                        MirExpression::Function {
                            function,
                            span: name.span,
                        },
                    ))
                }
                Some(resolution) => Err(error(
                    name.span,
                    format!(
                        "value `{}` resolved to `{resolution:?}` instead of a local or function",
                        name.text
                    ),
                )),
                None => Err(error(
                    name.span,
                    format!("value `{}` has no typed HIR resolution", name.text),
                )),
            },
            Expression::Arrow {
                closure,
                parameters,
                return_type: _,
                body,
                span,
            } => self.lower_closure(*closure, parameters, body, *span, current),
            Expression::Unary {
                operator,
                expression,
                span,
            } => {
                let (current, expression) = self.lower_expression(expression, current)?;
                self.materialize(
                    current,
                    MirExpression::Unary {
                        operator: *operator,
                        expression: Box::new(expression),
                        span: *span,
                    },
                )
            }
            Expression::Binary {
                operator,
                left,
                right,
                span,
            } => {
                if matches!(
                    operator,
                    BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
                ) {
                    self.lower_logical(*operator, left, right, *span, current)
                } else {
                    let (current, left) = self.lower_expression(left, current)?;
                    let (current, right) = self.lower_expression(right, current)?;
                    self.materialize(
                        current,
                        MirExpression::Binary {
                            operator: *operator,
                            left: Box::new(left),
                            right: Box::new(right),
                            span: *span,
                        },
                    )
                }
            }
            Expression::Call {
                callee,
                arguments,
                span,
            } => self.lower_call(callee, arguments, *span, current),
            Expression::Parenthesized { expression, .. } => {
                self.lower_expression(expression, current)
            }
        }
    }

    fn lower_record(
        &mut self,
        fields: &[RecordFieldInitializer],
        span: SourceSpan,
        mut current: BasicBlockId,
    ) -> Result<(BasicBlockId, MirExpression), MirLoweringError> {
        let record = match self.typed.expression_type(span) {
            Some(Type::Record { definition, .. }) => *definition,
            Some(ty) => {
                return Err(error(
                    span,
                    format!("record literal has typed HIR type `{ty}` instead of a record"),
                ));
            }
            None => {
                return Err(error(span, "record literal has no typed HIR type fact"));
            }
        };
        let field_count = self
            .typed
            .record_facts(record)
            .ok_or_else(|| error(span, "record literal references a missing typed HIR layout"))?
            .fields()
            .len();
        let mut values = vec![None; field_count];

        for initializer in fields {
            let field = match self.typed.name_resolution(initializer.name.span) {
                Some(NameResolution::Field(field)) => field,
                Some(resolution) => {
                    return Err(error(
                        initializer.name.span,
                        format!(
                            "record initializer `{}` resolved to `{resolution:?}` instead of a field",
                            initializer.name.text
                        ),
                    ));
                }
                None => {
                    return Err(error(
                        initializer.name.span,
                        format!(
                            "record initializer `{}` has no typed HIR field resolution",
                            initializer.name.text
                        ),
                    ));
                }
            };
            if field.record() != record {
                return Err(error(
                    initializer.name.span,
                    "record initializer field belongs to a different nominal record",
                ));
            }
            let Some(slot) = values.get_mut(field.index()) else {
                return Err(error(
                    initializer.name.span,
                    "record initializer field index is outside its typed HIR layout",
                ));
            };
            if slot.is_some() {
                return Err(error(
                    initializer.name.span,
                    "duplicate record initializer reached MIR lowering",
                ));
            }

            let (next, value) = self.lower_expression(&initializer.value, current)?;
            let (next, value) = self.materialize(next, value)?;
            current = next;
            *slot = Some(value);
        }

        let fields = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                value.ok_or_else(|| {
                    error(
                        span,
                        format!("record literal is missing resolved field index {index}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.materialize(
            current,
            MirExpression::Record {
                record,
                fields,
                span,
            },
        )
    }

    fn lower_match(
        &mut self,
        scrutinee: &Expression,
        arms: &[MatchArm],
        span: SourceSpan,
        current: BasicBlockId,
    ) -> Result<(BasicBlockId, MirExpression), MirLoweringError> {
        let facts = self
            .typed
            .match_facts(span)
            .cloned()
            .ok_or_else(|| error(span, "match expression has no typed HIR dispatch facts"))?;
        let union = facts.union();
        let union_facts = self
            .typed
            .union_facts(union)
            .ok_or_else(|| error(span, "match expression references a missing union layout"))?;
        self.validate_type_arguments(
            union_facts.type_parameters(),
            facts.type_arguments(),
            span,
            "match expression",
        )?;
        let expected_scrutinee = Type::Union {
            definition: union,
            arguments: facts.type_arguments().to_vec().into_boxed_slice(),
        };
        let actual_scrutinee = self.expression_type(scrutinee, "match scrutinee")?;
        if actual_scrutinee != &expected_scrutinee {
            return Err(error(
                scrutinee.span(),
                "match scrutinee type does not agree with its typed HIR dispatch facts",
            ));
        }
        if arms.len() != facts.arms().len() {
            return Err(error(
                span,
                "match expression has inconsistent typed HIR arm facts",
            ));
        }

        let variant_payloads = union_facts
            .variants()
            .iter()
            .map(|variant| {
                (
                    variant.id(),
                    variant
                        .payloads()
                        .iter()
                        .map(|payload| payload.id())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        if variant_payloads.is_empty() {
            return Err(error(span, "match union has no variants"));
        }

        let (scrutinee_end, scrutinee_value) = self.lower_expression(scrutinee, current)?;
        let scrutinee_local = self.allocate_local();
        self.push_statement(
            scrutinee_end,
            MirStatement::Store {
                local: scrutinee_local,
                value: scrutinee_value,
                span: scrutinee.span(),
            },
        )?;

        let result = self.allocate_local();
        let join = self.new_block(span);
        let mut targets = vec![None; variant_payloads.len()];
        let mut default_target = None;

        for (arm, arm_facts) in arms.iter().zip(facts.arms()) {
            let arm_start = self.new_block(arm.span);
            match (&arm.pattern, arm_facts) {
                (
                    MatchPattern::Variant {
                        union: qualifier,
                        variant: variant_name,
                        bindings,
                        ..
                    },
                    MatchArmFacts::Variant {
                        variant,
                        bindings: binding_facts,
                    },
                ) => {
                    if variant.union() != union {
                        return Err(error(
                            variant_name.span,
                            "match variant belongs to a different union",
                        ));
                    }
                    if self.typed.name_resolution(qualifier.span)
                        != Some(NameResolution::Union(union))
                        || self.typed.name_resolution(variant_name.span)
                            != Some(NameResolution::Variant(*variant))
                    {
                        return Err(error(
                            arm.pattern.span(),
                            "match pattern does not agree with its typed HIR name resolutions",
                        ));
                    }
                    let Some((expected_variant, payloads)) = variant_payloads.get(variant.index())
                    else {
                        return Err(error(
                            variant_name.span,
                            "match variant index is outside its union layout",
                        ));
                    };
                    if expected_variant != variant {
                        return Err(error(
                            variant_name.span,
                            "match variant identity does not agree with its union layout",
                        ));
                    }
                    let Some(target) = targets.get_mut(variant.index()) else {
                        return Err(error(
                            variant_name.span,
                            "match target index is outside its dispatch table",
                        ));
                    };
                    if target.replace(arm_start).is_some() {
                        return Err(error(
                            variant_name.span,
                            "duplicate variant case reached MIR lowering",
                        ));
                    }
                    if bindings.len() != binding_facts.len() || bindings.len() != payloads.len() {
                        return Err(error(
                            arm.pattern.span(),
                            "match pattern has inconsistent typed HIR payload bindings",
                        ));
                    }

                    for ((binding, binding_facts), expected_payload) in
                        bindings.iter().zip(binding_facts).zip(payloads)
                    {
                        if binding_facts.payload() != *expected_payload
                            || self.typed.name_resolution(binding.span)
                                != Some(NameResolution::Local(binding_facts.local()))
                        {
                            return Err(error(
                                binding.span,
                                "match payload binding does not agree with its typed HIR facts",
                            ));
                        }
                        let local = LocalId(binding_facts.local().index());
                        if local.index() >= self.hir_local_count {
                            return Err(error(
                                binding.span,
                                "match payload binding local is outside the function frame",
                            ));
                        }
                        self.push_statement(
                            arm_start,
                            MirStatement::Store {
                                local,
                                value: MirExpression::VariantPayload {
                                    source: scrutinee_local,
                                    union,
                                    variant: *variant,
                                    payload: *expected_payload,
                                    span: binding.span,
                                },
                                span: binding.span,
                            },
                        )?;
                    }
                }
                (MatchPattern::Default { .. }, MatchArmFacts::Default) => {
                    if default_target.replace(arm_start).is_some() {
                        return Err(error(
                            arm.pattern.span(),
                            "duplicate default arm reached MIR lowering",
                        ));
                    }
                }
                _ => {
                    return Err(error(
                        arm.pattern.span(),
                        "match arm shape does not agree with its typed HIR facts",
                    ));
                }
            }

            let (arm_end, value) = self.lower_expression(&arm.value, arm_start)?;
            self.push_statement(
                arm_end,
                MirStatement::Store {
                    local: result,
                    value,
                    span: arm.value.span(),
                },
            )?;
            self.terminate(
                arm_end,
                MirTerminator::Goto {
                    target: join,
                    span: arm.span,
                },
            )?;
        }

        let targets = targets
            .into_iter()
            .enumerate()
            .map(|(index, target)| {
                target.or(default_target).ok_or_else(|| {
                    error(
                        span,
                        format!("match expression has no target for variant index {index}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.terminate(
            scrutinee_end,
            MirTerminator::SwitchVariant {
                scrutinee: scrutinee_local,
                union,
                targets,
                span,
            },
        )?;

        Ok((
            join,
            MirExpression::Local {
                local: result,
                span,
            },
        ))
    }

    fn lower_closure(
        &mut self,
        closure: ClosureId,
        parameters: &[Parameter],
        body: &ArrowBody,
        span: SourceSpan,
        current: BasicBlockId,
    ) -> Result<(BasicBlockId, MirExpression), MirLoweringError> {
        if closure.owner() != self.function_id {
            return Err(error(span, "closure identity has a foreign lexical owner"));
        }
        let facts = self
            .typed
            .closure_facts(closure)
            .cloned()
            .ok_or_else(|| error(span, "arrow expression has no typed HIR closure facts"))?;
        self.validate_closure_facts(&facts, parameters, span)?;

        let mut captures = Vec::with_capacity(facts.captures().len());
        for capture in facts.captures() {
            if capture.index() >= self.hir_local_count {
                return Err(error(
                    span,
                    "closure capture is outside its owner local frame",
                ));
            }
            captures.push(MirExpression::Local {
                local: LocalId(capture.index()),
                span,
            });
        }

        let mut lowerer = FunctionLowerer {
            typed: self.typed,
            function_id: self.function_id,
            parameters: facts
                .parameter_ids()
                .iter()
                .map(|local| LocalId(local.index()))
                .collect(),
            type_parameters: self.type_parameters.clone(),
            hir_local_count: self.hir_local_count,
            next_local: self.hir_local_count,
            blocks: Vec::new(),
            loop_targets: Vec::new(),
            closures: Vec::new(),
        };
        let entry = lowerer.new_block(body.span());
        match body {
            ArrowBody::Expression(expression) => {
                let (end, value) = lowerer.lower_expression(expression, entry)?;
                lowerer.terminate(
                    end,
                    MirTerminator::Return {
                        value: Some(value),
                        span: expression.span(),
                    },
                )?;
            }
            ArrowBody::Block(block) => {
                if let Some(end) = lowerer.lower_block(block, Some(entry))? {
                    if facts.return_type() != &Type::Unit {
                        return Err(error(
                            block.span,
                            "non-Unit closure reached the end during MIR lowering",
                        ));
                    }
                    lowerer.terminate(
                        end,
                        MirTerminator::Return {
                            value: None,
                            span: block.span,
                        },
                    )?;
                }
            }
        }
        let FinishedLowering {
            parameters: closure_parameters,
            local_count,
            blocks,
            closures: nested,
        } = lowerer.finish()?;
        self.closures.extend(nested);
        self.closures.push(MirClosure {
            id: closure,
            captures: facts
                .captures()
                .iter()
                .map(|local| LocalId(local.index()))
                .collect(),
            parameters: closure_parameters,
            parameter_types: facts.parameter_types().to_vec(),
            local_count,
            return_type: facts.return_type().clone(),
            entry,
            blocks,
            span,
        });

        self.materialize(
            current,
            MirExpression::Closure {
                closure,
                captures,
                span,
            },
        )
    }

    fn validate_closure_facts(
        &self,
        facts: &ClosureFacts,
        parameters: &[Parameter],
        span: SourceSpan,
    ) -> Result<(), MirLoweringError> {
        if facts.parameter_ids().len() != parameters.len()
            || facts.parameter_types().len() != parameters.len()
        {
            return Err(error(span, "closure has inconsistent typed HIR parameters"));
        }
        for (parameter, local) in parameters.iter().zip(facts.parameter_ids()) {
            if self.typed.name_resolution(parameter.name.span)
                != Some(NameResolution::Local(*local))
            {
                return Err(error(
                    parameter.name.span,
                    "closure parameter has no matching typed HIR local resolution",
                ));
            }
        }
        let expected = Type::Function {
            parameters: facts.parameter_types().to_vec().into_boxed_slice(),
            return_type: Box::new(facts.return_type().clone()),
        };
        if self.typed.expression_type(span) != Some(&expected) {
            return Err(error(
                span,
                "closure signature does not match its typed HIR expression type",
            ));
        }
        for ty in facts.parameter_types() {
            self.validate_type(ty, span, "closure parameter")?;
        }
        self.validate_type(facts.return_type(), span, "closure result")
    }

    fn lower_logical(
        &mut self,
        operator: BinaryOperator,
        left: &Expression,
        right: &Expression,
        span: SourceSpan,
        current: BasicBlockId,
    ) -> Result<(BasicBlockId, MirExpression), MirLoweringError> {
        let (left_end, left) = self.lower_expression(left, current)?;
        let right_start = self.new_block(right.span());
        let short_start = self.new_block(left.span());
        let join = self.new_block(span);
        let result = self.allocate_local();
        let (then_target, else_target, short_value) = match operator {
            BinaryOperator::LogicalAnd => (right_start, short_start, false),
            BinaryOperator::LogicalOr => (short_start, right_start, true),
            _ => {
                return Err(error(
                    span,
                    "non-logical operator reached logical MIR lowering",
                ));
            }
        };

        self.terminate(
            left_end,
            MirTerminator::Branch {
                condition: left,
                then_target,
                else_target,
                span,
            },
        )?;
        self.push_statement(
            short_start,
            MirStatement::Store {
                local: result,
                value: MirExpression::Boolean {
                    value: short_value,
                    span,
                },
                span,
            },
        )?;
        self.terminate(short_start, MirTerminator::Goto { target: join, span })?;

        let (right_end, right) = self.lower_expression(right, right_start)?;
        self.push_statement(
            right_end,
            MirStatement::Store {
                local: result,
                value: right,
                span,
            },
        )?;
        self.terminate(right_end, MirTerminator::Goto { target: join, span })?;

        Ok((
            join,
            MirExpression::Local {
                local: result,
                span,
            },
        ))
    }

    fn lower_call(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        span: SourceSpan,
        mut current: BasicBlockId,
    ) -> Result<(BasicBlockId, MirExpression), MirLoweringError> {
        if let Some(construction) = self.typed.variant_construction(span).cloned() {
            self.validate_variant_construction_facts(&construction, arguments, span)?;
            return self.lower_variant_construction(
                callee,
                arguments,
                construction.union(),
                construction.variant(),
                span,
                current,
            );
        }

        if let Expression::Member { object, member, .. } = callee {
            let operation = match self.typed.name_resolution(member.span) {
                Some(NameResolution::Builtin(Builtin::ArrayAppend)) => Some(ArrayIntrinsic::Append),
                Some(NameResolution::Builtin(Builtin::ArrayConcat)) => Some(ArrayIntrinsic::Concat),
                _ => None,
            };
            if let Some(operation) = operation {
                return self
                    .lower_array_intrinsic(object, member, arguments, operation, span, current);
            }
        }

        if let Expression::Name(name) = callee {
            let direct = match self.typed.name_resolution(name.span) {
                Some(NameResolution::Function(resolved_function)) => {
                    let call = self.typed.call_facts(span).ok_or_else(|| {
                        error(
                            span,
                            "source function call has no typed HIR instantiation facts",
                        )
                    })?;
                    if call.function() != resolved_function {
                        return Err(error(
                            span,
                            "function call does not agree with its typed HIR name resolution",
                        ));
                    }
                    self.validate_call_facts(call, arguments, span)?;
                    Some(Callee::Function(call.function()))
                }
                Some(NameResolution::Builtin(Builtin::Print)) => {
                    if self.typed.call_facts(span).is_some() {
                        return Err(error(
                            span,
                            "print call unexpectedly has source function instantiation facts",
                        ));
                    }
                    Some(Callee::Print)
                }
                Some(NameResolution::Builtin(Builtin::ToString)) => {
                    if self.typed.call_facts(span).is_some() {
                        return Err(error(
                            span,
                            "toString call unexpectedly has source function instantiation facts",
                        ));
                    }
                    Some(Callee::ToString)
                }
                Some(NameResolution::Builtin(Builtin::ParseInt)) => {
                    if self.typed.call_facts(span).is_some() {
                        return Err(error(
                            span,
                            "parseInt call unexpectedly has source function instantiation facts",
                        ));
                    }
                    Some(Callee::ParseInt)
                }
                _ => None,
            };

            if let Some(callee) = direct {
                let mut lowered_arguments = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    let (next, argument) = self.lower_expression(argument, current)?;
                    current = next;
                    lowered_arguments.push(argument);
                }

                return self.materialize(
                    current,
                    MirExpression::Call {
                        callee,
                        arguments: lowered_arguments,
                        span,
                    },
                );
            }
        }

        let callee_type = self
            .typed
            .expression_type(callee.span())
            .ok_or_else(|| error(callee.span(), "indirect callee has no typed HIR type fact"))?;
        let Type::Function {
            parameters,
            return_type: _,
        } = callee_type
        else {
            return Err(error(
                callee.span(),
                "indirect callee does not have a typed HIR function type",
            ));
        };
        if parameters.len() != arguments.len() {
            return Err(error(
                span,
                "indirect call has inconsistent typed HIR argument count",
            ));
        }
        for (index, (argument, expected)) in arguments.iter().zip(parameters).enumerate() {
            let actual = self.expression_type(argument, "indirect call argument")?;
            if actual != expected {
                return Err(error(
                    argument.span(),
                    format!(
                        "indirect call argument {index} does not agree with its typed HIR function type"
                    ),
                ));
            }
        }

        let (next, callee) = self.lower_expression(callee, current)?;
        current = next;
        let mut lowered_arguments = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let (next, argument) = self.lower_expression(argument, current)?;
            current = next;
            lowered_arguments.push(argument);
        }

        self.materialize(
            current,
            MirExpression::IndirectCall {
                callee: Box::new(callee),
                arguments: lowered_arguments,
                span,
            },
        )
    }

    fn lower_array_intrinsic(
        &mut self,
        object: &Expression,
        member: &Name,
        arguments: &[Expression],
        operation: ArrayIntrinsic,
        span: SourceSpan,
        current: BasicBlockId,
    ) -> Result<(BasicBlockId, MirExpression), MirLoweringError> {
        if arguments.len() != 1 {
            return Err(error(
                span,
                "array intrinsic has inconsistent typed HIR argument count",
            ));
        }
        if self.typed.call_facts(span).is_some() {
            return Err(error(
                span,
                "array intrinsic unexpectedly has source function instantiation facts",
            ));
        }
        let object_type = self.expression_type(object, "array intrinsic base")?;
        let Type::Array(element) = object_type else {
            return Err(error(
                object.span(),
                "array intrinsic base does not have an array type",
            ));
        };
        let element_type = element.as_ref().clone();
        let expected_argument = match operation {
            ArrayIntrinsic::Append => &element_type,
            ArrayIntrinsic::Concat => object_type,
        };
        let argument = arguments
            .first()
            .ok_or_else(|| error(span, "array intrinsic argument does not exist"))?;
        if self.expression_type(argument, "array intrinsic argument")? != expected_argument {
            return Err(error(
                argument.span(),
                "array intrinsic argument does not match its typed HIR base type",
            ));
        }
        if self.typed.expression_type(span) != Some(object_type) {
            return Err(error(
                span,
                "array intrinsic result does not match its typed HIR base type",
            ));
        }
        let expected_resolution = match operation {
            ArrayIntrinsic::Append => Builtin::ArrayAppend,
            ArrayIntrinsic::Concat => Builtin::ArrayConcat,
        };
        if self.typed.name_resolution(member.span)
            != Some(NameResolution::Builtin(expected_resolution))
        {
            return Err(error(
                member.span,
                "array intrinsic member has inconsistent typed HIR resolution",
            ));
        }

        let (current, target) = self.lower_expression(object, current)?;
        let (current, argument) = self.lower_expression(argument, current)?;
        self.materialize(
            current,
            MirExpression::ArrayIntrinsic {
                operation,
                element_type,
                target: Box::new(target),
                argument: Box::new(argument),
                span,
            },
        )
    }

    fn validate_call_facts(
        &self,
        call: &CallFacts,
        arguments: &[Expression],
        span: SourceSpan,
    ) -> Result<(), MirLoweringError> {
        let function = self.typed.function_facts(call.function()).ok_or_else(|| {
            error(
                span,
                "function call references missing typed HIR function facts",
            )
        })?;
        self.validate_type_arguments(
            function.type_parameters(),
            call.type_arguments(),
            span,
            "function call",
        )?;
        if arguments.len() != function.parameter_types().len() {
            return Err(error(
                span,
                "function call has inconsistent typed HIR argument count",
            ));
        }

        for (index, (argument, parameter)) in
            arguments.iter().zip(function.parameter_types()).enumerate()
        {
            let actual = self.expression_type(argument, "function call argument")?;
            let expected = instantiate_type(
                parameter,
                function.type_parameters(),
                call.type_arguments(),
                span,
                "function",
            )?;
            if actual != &expected {
                return Err(error(
                    span,
                    format!(
                        "function call argument {index} does not agree with its typed HIR instantiation facts"
                    ),
                ));
            }
        }

        let actual = self
            .typed
            .expression_type(span)
            .ok_or_else(|| error(span, "function call has no typed HIR result type"))?;
        let expected = instantiate_type(
            function.return_type(),
            function.type_parameters(),
            call.type_arguments(),
            span,
            "function",
        )?;
        if actual != &expected {
            return Err(error(
                span,
                "function call result does not agree with its typed HIR instantiation facts",
            ));
        }

        Ok(())
    }

    fn validate_variant_construction_facts(
        &self,
        construction: &VariantConstructionFacts,
        arguments: &[Expression],
        span: SourceSpan,
    ) -> Result<(), MirLoweringError> {
        let union = self
            .typed
            .union_facts(construction.union())
            .ok_or_else(|| {
                error(
                    span,
                    "variant construction references a missing union definition",
                )
            })?;
        self.validate_type_arguments(
            union.type_parameters(),
            construction.type_arguments(),
            span,
            "variant construction",
        )?;
        let expected_type = Type::Union {
            definition: construction.union(),
            arguments: construction.type_arguments().to_vec().into_boxed_slice(),
        };
        let actual_type = self
            .typed
            .expression_type(span)
            .ok_or_else(|| error(span, "variant construction has no typed HIR result type"))?;
        self.validate_type(actual_type, span, "variant construction")?;
        if actual_type != &expected_type {
            return Err(error(
                span,
                "variant construction type does not agree with its typed HIR instantiation facts",
            ));
        }

        let variant = union
            .variants()
            .get(construction.variant().index())
            .filter(|variant| variant.id() == construction.variant())
            .ok_or_else(|| {
                error(
                    span,
                    "variant construction references a missing variant definition",
                )
            })?;
        if arguments.len() != variant.payloads().len() {
            return Err(error(
                span,
                "variant construction has inconsistent typed HIR payload count",
            ));
        }
        for (index, (argument, payload)) in arguments.iter().zip(variant.payloads()).enumerate() {
            let actual = self.expression_type(argument, "variant construction payload")?;
            let expected = instantiate_type(
                payload.ty(),
                union.type_parameters(),
                construction.type_arguments(),
                span,
                "union",
            )?;
            if actual != &expected {
                return Err(error(
                    span,
                    format!(
                        "variant construction payload {index} does not agree with its typed HIR instantiation facts"
                    ),
                ));
            }
        }

        Ok(())
    }

    fn validate_type_arguments(
        &self,
        parameters: &[TypeParameterFacts],
        arguments: &[Type],
        span: SourceSpan,
        role: &str,
    ) -> Result<(), MirLoweringError> {
        if parameters.len() != arguments.len() {
            return Err(error(
                span,
                format!("{role} has inconsistent typed HIR type-argument count"),
            ));
        }
        for argument in arguments {
            self.validate_type(argument, span, role)?;
        }
        Ok(())
    }

    fn validate_type(
        &self,
        ty: &Type,
        span: SourceSpan,
        role: &str,
    ) -> Result<(), MirLoweringError> {
        match ty {
            Type::Parameter(parameter) => {
                if self.type_parameters.contains(parameter) {
                    Ok(())
                } else {
                    Err(error(
                        span,
                        format!("{role} contains a foreign typed HIR type parameter"),
                    ))
                }
            }
            Type::Array(element) => self.validate_type(element, span, role),
            Type::Function {
                parameters,
                return_type,
            } => {
                for parameter in parameters {
                    self.validate_type(parameter, span, role)?;
                }
                self.validate_type(return_type, span, role)
            }
            Type::Record {
                definition,
                arguments,
            } => {
                let record = self.typed.record_facts(*definition).ok_or_else(|| {
                    error(
                        span,
                        format!("{role} references a missing record definition"),
                    )
                })?;
                self.validate_type_arguments(record.type_parameters(), arguments, span, role)
            }
            Type::Union {
                definition,
                arguments,
            } => {
                let union = self.typed.union_facts(*definition).ok_or_else(|| {
                    error(
                        span,
                        format!("{role} references a missing union definition"),
                    )
                })?;
                self.validate_type_arguments(union.type_parameters(), arguments, span, role)
            }
            Type::Int | Type::Bool | Type::String | Type::Unit => Ok(()),
        }
    }

    fn expression_type(
        &self,
        expression: &Expression,
        role: &str,
    ) -> Result<&Type, MirLoweringError> {
        let ty = self
            .typed
            .expression_type(expression.span())
            .ok_or_else(|| error(expression.span(), format!("{role} has no typed HIR type")))?;
        self.validate_type(ty, expression.span(), role)?;
        Ok(ty)
    }

    fn lower_variant_construction(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        union: UnionId,
        variant: VariantId,
        span: SourceSpan,
        mut current: BasicBlockId,
    ) -> Result<(BasicBlockId, MirExpression), MirLoweringError> {
        let Expression::Member {
            object,
            member: variant_name,
            ..
        } = callee
        else {
            return Err(error(
                callee.span(),
                "variant construction has a non-member callee",
            ));
        };
        let Expression::Name(qualifier) = object.as_ref() else {
            return Err(error(
                object.span(),
                "variant construction has a non-name union qualifier",
            ));
        };
        if variant.union() != union
            || self.typed.name_resolution(qualifier.span) != Some(NameResolution::Union(union))
            || self.typed.name_resolution(variant_name.span)
                != Some(NameResolution::Variant(variant))
        {
            return Err(error(
                callee.span(),
                "variant construction does not agree with its typed HIR resolution",
            ));
        }

        let layout = self
            .typed
            .union_facts(union)
            .and_then(|layout| layout.variants().get(variant.index()))
            .ok_or_else(|| error(span, "variant construction references a missing layout"))?;
        if layout.id() != variant || arguments.len() != layout.payloads().len() {
            return Err(error(
                span,
                "variant construction has inconsistent typed HIR layout facts",
            ));
        }

        let mut payloads = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let (next, payload) = self.lower_expression(argument, current)?;
            current = next;
            payloads.push(payload);
        }

        self.materialize(
            current,
            MirExpression::Variant {
                union,
                variant,
                payloads,
                span,
            },
        )
    }

    fn materialize(
        &mut self,
        current: BasicBlockId,
        value: MirExpression,
    ) -> Result<(BasicBlockId, MirExpression), MirLoweringError> {
        let span = value.span();
        let local = self.allocate_local();
        self.push_statement(current, MirStatement::Store { local, value, span })?;

        Ok((current, MirExpression::Local { local, span }))
    }

    fn new_block(&mut self, span: SourceSpan) -> BasicBlockId {
        let block = BasicBlockId(self.blocks.len());
        self.blocks.push(PendingBlock {
            statements: Vec::new(),
            terminator: None,
            span,
        });
        block
    }

    fn push_statement(
        &mut self,
        block: BasicBlockId,
        statement: MirStatement,
    ) -> Result<(), MirLoweringError> {
        let Some(block) = self.blocks.get_mut(block.0) else {
            return Err(error(
                statement_span(&statement),
                "MIR block does not exist",
            ));
        };
        if block.terminator.is_some() {
            return Err(error(block.span, "cannot append to a terminated MIR block"));
        }
        block.statements.push(statement);
        Ok(())
    }

    fn terminate(
        &mut self,
        block: BasicBlockId,
        terminator: MirTerminator,
    ) -> Result<(), MirLoweringError> {
        let span = terminator_span(&terminator);
        let Some(block) = self.blocks.get_mut(block.0) else {
            return Err(error(span, "MIR block does not exist"));
        };
        if block.terminator.replace(terminator).is_some() {
            return Err(error(span, "MIR block already has a terminator"));
        }
        Ok(())
    }

    fn allocate_local(&mut self) -> LocalId {
        let local = LocalId(self.next_local);
        self.next_local += 1;
        local
    }

    fn resolve_local(&self, name: &Name, role: &str) -> Result<LocalId, MirLoweringError> {
        match self.typed.name_resolution(name.span) {
            Some(NameResolution::Local(local)) if local.index() < self.hir_local_count => {
                Ok(LocalId(local.index()))
            }
            Some(NameResolution::Local(_)) => Err(error(
                name.span,
                format!(
                    "{role} `{}` resolves outside the HIR local frame",
                    name.text
                ),
            )),
            Some(resolution) => Err(error(
                name.span,
                format!(
                    "{role} `{}` resolved to `{resolution:?}` instead of a local",
                    name.text
                ),
            )),
            None => Err(error(
                name.span,
                format!("{role} `{}` has no typed HIR resolution", name.text),
            )),
        }
    }

    fn require_expression_type(&self, expression: &Expression) -> Result<(), MirLoweringError> {
        self.typed
            .expression_type(expression.span())
            .map(|_| ())
            .ok_or_else(|| {
                error(
                    expression.span(),
                    "value expression has no typed HIR type fact",
                )
            })
    }

    fn finish(self) -> Result<FinishedLowering, MirLoweringError> {
        let Self {
            parameters,
            next_local,
            blocks,
            closures,
            ..
        } = self;
        let blocks = blocks
            .into_iter()
            .map(|block| {
                let terminator = block
                    .terminator
                    .ok_or_else(|| error(block.span, "MIR block has no terminator"))?;
                Ok(MirBasicBlock {
                    statements: block.statements,
                    terminator,
                    span: block.span,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(FinishedLowering {
            parameters,
            local_count: next_local,
            blocks,
            closures,
        })
    }
}

const fn statement_span(statement: &MirStatement) -> SourceSpan {
    match statement {
        MirStatement::Store { span, .. } | MirStatement::Expression { span, .. } => *span,
    }
}

const fn terminator_span(terminator: &MirTerminator) -> SourceSpan {
    match terminator {
        MirTerminator::Goto { span, .. }
        | MirTerminator::Branch { span, .. }
        | MirTerminator::SwitchVariant { span, .. }
        | MirTerminator::Return { span, .. } => *span,
    }
}

fn error(span: SourceSpan, message: impl Into<String>) -> MirLoweringError {
    MirLoweringError {
        message: message.into(),
        span,
    }
}

fn instantiate_type(
    ty: &Type,
    parameters: &[TypeParameterFacts],
    arguments: &[Type],
    span: SourceSpan,
    role: &str,
) -> Result<Type, MirLoweringError> {
    match ty {
        Type::Parameter(parameter) => {
            let index = parameters
                .iter()
                .position(|candidate| candidate.id() == *parameter)
                .ok_or_else(|| {
                    error(
                        span,
                        format!("{role} signature contains an unrelated typed HIR type parameter"),
                    )
                })?;
            arguments.get(index).cloned().ok_or_else(|| {
                error(
                    span,
                    format!("{role} signature substitution is missing a typed HIR type argument"),
                )
            })
        }
        Type::Array(element) => Ok(Type::Array(Box::new(instantiate_type(
            element, parameters, arguments, span, role,
        )?))),
        Type::Function {
            parameters: function_parameters,
            return_type,
        } => Ok(Type::Function {
            parameters: function_parameters
                .iter()
                .map(|parameter| instantiate_type(parameter, parameters, arguments, span, role))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            return_type: Box::new(instantiate_type(
                return_type,
                parameters,
                arguments,
                span,
                role,
            )?),
        }),
        Type::Record {
            definition,
            arguments: nested,
        } => Ok(Type::Record {
            definition: *definition,
            arguments: nested
                .iter()
                .map(|argument| instantiate_type(argument, parameters, arguments, span, role))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        }),
        Type::Union {
            definition,
            arguments: nested,
        } => Ok(Type::Union {
            definition: *definition,
            arguments: nested
                .iter()
                .map(|argument| instantiate_type(argument, parameters, arguments, span, role))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        }),
        Type::Int => Ok(Type::Int),
        Type::Bool => Ok(Type::Bool),
        Type::String => Ok(Type::String),
        Type::Unit => Ok(Type::Unit),
    }
}
