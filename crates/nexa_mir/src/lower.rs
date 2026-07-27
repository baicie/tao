use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use nexa_hir::{
    BinaryOperator, Block, Expression, Function, IfStatement, Name, Program, ReturnStatement,
    Statement, Type, TypedProgram, WhileStatement,
};
use nexa_span::SourceSpan;

use crate::{
    BasicBlockId, Callee, FunctionId, LocalId, MirBasicBlock, MirExpression, MirFunction,
    MirProgram, MirStatement, MirTerminator,
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
    let program = typed.program();
    let functions = function_ids(program)?;
    let functions = program
        .functions
        .iter()
        .map(|function| lower_function(function, &functions))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(MirProgram {
        functions,
        span: program.span,
    })
}

fn function_ids(program: &Program) -> Result<HashMap<String, FunctionId>, MirLoweringError> {
    let mut functions = HashMap::new();

    for (index, function) in program.functions.iter().enumerate() {
        if functions
            .insert(function.name.text.clone(), FunctionId(index))
            .is_some()
        {
            return Err(error(
                function.name.span,
                format!(
                    "duplicate function `{}` reached MIR lowering",
                    function.name.text
                ),
            ));
        }
    }

    Ok(functions)
}

fn lower_function(
    function: &Function,
    functions: &HashMap<String, FunctionId>,
) -> Result<MirFunction, MirLoweringError> {
    let mut lowerer = FunctionLowerer {
        functions,
        scopes: vec![HashMap::new()],
        parameters: Vec::new(),
        next_local: 0,
        blocks: Vec::new(),
        loop_targets: Vec::new(),
    };

    for parameter in &function.parameters {
        let local = lowerer.allocate_local();
        lowerer.bind(&parameter.name, local)?;
        lowerer.parameters.push(local);
    }

    let entry = lowerer.new_block(function.body.span);
    let end = lowerer.lower_block(&function.body, false, Some(entry))?;
    if let Some(end) = end {
        if function.return_type.kind != Type::Unit {
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

    let (parameters, local_count, blocks) = lowerer.finish()?;

    Ok(MirFunction {
        name: function.name.text.clone(),
        parameters,
        local_count,
        return_type: function.return_type.kind,
        entry,
        blocks,
        span: function.span,
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

struct FunctionLowerer<'functions> {
    functions: &'functions HashMap<String, FunctionId>,
    scopes: Vec<HashMap<String, LocalId>>,
    parameters: Vec<LocalId>,
    next_local: usize,
    blocks: Vec<PendingBlock>,
    loop_targets: Vec<LoopTargets>,
}

impl FunctionLowerer<'_> {
    fn lower_block(
        &mut self,
        block: &Block,
        creates_scope: bool,
        start: Option<BasicBlockId>,
    ) -> Result<Option<BasicBlockId>, MirLoweringError> {
        if creates_scope {
            self.scopes.push(HashMap::new());
        }

        let result = self.lower_statements(&block.statements, start);

        if creates_scope {
            let _ = self.scopes.pop();
        }

        result
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
                let local = self.lookup(&statement.target).ok_or_else(|| {
                    error(
                        statement.target.span,
                        format!(
                            "unresolved assignment target `{}` reached MIR lowering",
                            statement.target.text
                        ),
                    )
                })?;
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
        let local = self.allocate_local();
        self.bind(name, local)?;
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

        let then_end = self.lower_block(&statement.then_branch, true, Some(then_start))?;
        let else_end = statement
            .else_branch
            .as_ref()
            .map_or(Ok(Some(else_start)), |branch| {
                self.lower_block(branch, true, Some(else_start))
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
        let body_result = self.lower_block(&statement.body, true, Some(body_block));
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
            Expression::Name(name) => {
                let local = self.lookup(name).ok_or_else(|| {
                    error(
                        name.span,
                        format!("unresolved local `{}` reached MIR lowering", name.text),
                    )
                })?;
                Ok((
                    current,
                    MirExpression::Local {
                        local,
                        span: name.span,
                    },
                ))
            }
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
        let Expression::Name(name) = callee else {
            return Err(error(callee.span(), "non-name call reached MIR lowering"));
        };
        if self.lookup(name).is_some() {
            return Err(error(
                name.span,
                format!("local `{}` called as a function in MIR lowering", name.text),
            ));
        }

        let callee = if name.text == "print" {
            Callee::Print
        } else {
            self.functions
                .get(&name.text)
                .copied()
                .map(Callee::Function)
                .ok_or_else(|| {
                    error(
                        name.span,
                        format!("unresolved function `{}` reached MIR lowering", name.text),
                    )
                })?
        };
        let mut lowered_arguments = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let (next, argument) = self.lower_expression(argument, current)?;
            current = next;
            lowered_arguments.push(argument);
        }

        self.materialize(
            current,
            MirExpression::Call {
                callee,
                arguments: lowered_arguments,
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

    fn bind(&mut self, name: &Name, local: LocalId) -> Result<(), MirLoweringError> {
        let Some(scope) = self.scopes.last_mut() else {
            return Err(error(name.span, "MIR lowering lost its lexical scope"));
        };
        if scope.insert(name.text.clone(), local).is_some() {
            return Err(error(
                name.span,
                format!("duplicate local `{}` reached MIR lowering", name.text),
            ));
        }

        Ok(())
    }

    fn lookup(&self, name: &Name) -> Option<LocalId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&name.text).copied())
    }

    fn finish(self) -> Result<(Vec<LocalId>, usize, Vec<MirBasicBlock>), MirLoweringError> {
        let Self {
            parameters,
            next_local,
            blocks,
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

        Ok((parameters, next_local, blocks))
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
        | MirTerminator::Return { span, .. } => *span,
    }
}

fn error(span: SourceSpan, message: impl Into<String>) -> MirLoweringError {
    MirLoweringError {
        message: message.into(),
        span,
    }
}
