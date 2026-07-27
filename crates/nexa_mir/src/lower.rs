use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use nexa_hir::{
    Block, ConstDeclaration, Expression, Function, IfStatement, Name, Program, ReturnStatement,
    Statement, TypedProgram,
};
use nexa_span::SourceSpan;

use crate::{
    Callee, FunctionId, LocalId, MirBlock, MirExpression, MirFunction, MirProgram, MirStatement,
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

/// Lowers validated typed HIR into resolved middle intermediate representation.
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
    };

    for parameter in &function.parameters {
        let local = lowerer.allocate_local();
        lowerer.bind(&parameter.name, local)?;
        lowerer.parameters.push(local);
    }

    let body = lowerer.lower_block(&function.body, false)?;
    Ok(MirFunction {
        name: function.name.text.clone(),
        parameters: lowerer.parameters,
        local_count: lowerer.next_local,
        return_type: function.return_type.kind,
        body,
        span: function.span,
    })
}

struct FunctionLowerer<'functions> {
    functions: &'functions HashMap<String, FunctionId>,
    scopes: Vec<HashMap<String, LocalId>>,
    parameters: Vec<LocalId>,
    next_local: usize,
}

impl FunctionLowerer<'_> {
    fn lower_block(
        &mut self,
        block: &Block,
        creates_scope: bool,
    ) -> Result<MirBlock, MirLoweringError> {
        if creates_scope {
            self.scopes.push(HashMap::new());
        }

        let statements = block
            .statements
            .iter()
            .map(|statement| self.lower_statement(statement))
            .collect::<Result<Vec<_>, _>>();

        if creates_scope {
            let _ = self.scopes.pop();
        }

        Ok(MirBlock {
            statements: statements?,
            span: block.span,
        })
    }

    fn lower_statement(&mut self, statement: &Statement) -> Result<MirStatement, MirLoweringError> {
        match statement {
            Statement::Const(declaration) => self.lower_const(declaration),
            Statement::If(statement) => self.lower_if(statement),
            Statement::Return(statement) => self.lower_return(statement),
            Statement::Expression(statement) => Ok(MirStatement::Expression {
                expression: self.lower_expression(&statement.expression)?,
                span: statement.span,
            }),
        }
    }

    fn lower_const(
        &mut self,
        declaration: &ConstDeclaration,
    ) -> Result<MirStatement, MirLoweringError> {
        let value = self.lower_expression(&declaration.initializer)?;
        let local = self.allocate_local();
        self.bind(&declaration.name, local)?;

        Ok(MirStatement::Initialize {
            local,
            value,
            span: declaration.span,
        })
    }

    fn lower_if(&mut self, statement: &IfStatement) -> Result<MirStatement, MirLoweringError> {
        Ok(MirStatement::If {
            condition: self.lower_expression(&statement.condition)?,
            then_branch: self.lower_block(&statement.then_branch, true)?,
            else_branch: statement
                .else_branch
                .as_ref()
                .map(|branch| self.lower_block(branch, true))
                .transpose()?,
            span: statement.span,
        })
    }

    fn lower_return(
        &mut self,
        statement: &ReturnStatement,
    ) -> Result<MirStatement, MirLoweringError> {
        Ok(MirStatement::Return {
            value: statement
                .value
                .as_ref()
                .map(|value| self.lower_expression(value))
                .transpose()?,
            span: statement.span,
        })
    }

    fn lower_expression(
        &mut self,
        expression: &Expression,
    ) -> Result<MirExpression, MirLoweringError> {
        match expression {
            Expression::Integer { value, span } => Ok(MirExpression::Integer {
                value: *value,
                span: *span,
            }),
            Expression::Boolean { value, span } => Ok(MirExpression::Boolean {
                value: *value,
                span: *span,
            }),
            Expression::Name(name) => {
                let local = self.lookup(name).ok_or_else(|| {
                    error(
                        name.span,
                        format!("unresolved local `{}` reached MIR lowering", name.text),
                    )
                })?;
                Ok(MirExpression::Local {
                    local,
                    span: name.span,
                })
            }
            Expression::Unary {
                operator,
                expression,
                span,
            } => Ok(MirExpression::Unary {
                operator: *operator,
                expression: Box::new(self.lower_expression(expression)?),
                span: *span,
            }),
            Expression::Binary {
                operator,
                left,
                right,
                span,
            } => Ok(MirExpression::Binary {
                operator: *operator,
                left: Box::new(self.lower_expression(left)?),
                right: Box::new(self.lower_expression(right)?),
                span: *span,
            }),
            Expression::Call {
                callee,
                arguments,
                span,
            } => self.lower_call(callee, arguments, *span),
            Expression::Parenthesized { expression, .. } => self.lower_expression(expression),
        }
    }

    fn lower_call(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        span: SourceSpan,
    ) -> Result<MirExpression, MirLoweringError> {
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
        let arguments = arguments
            .iter()
            .map(|argument| self.lower_expression(argument))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(MirExpression::Call {
            callee,
            arguments,
            span,
        })
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
}

fn error(span: SourceSpan, message: impl Into<String>) -> MirLoweringError {
    MirLoweringError {
        message: message.into(),
        span,
    }
}
