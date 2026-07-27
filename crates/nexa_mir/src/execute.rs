use std::fmt::{Display, Formatter};

use nexa_hir::{BinaryOperator, UnaryOperator};
use nexa_span::SourceSpan;

use crate::{
    Callee, FunctionId, LocalId, MirBlock, MirExpression, MirLoweringError, MirProgram,
    MirStatement,
};

/// A runtime value produced by the reference interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A signed 64-bit integer.
    Int(i64),
    /// A boolean value.
    Bool(bool),
    /// The absence of a value.
    Unit,
}

impl Display for Value {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(value) => write!(formatter, "{value}"),
            Self::Bool(value) => write!(formatter, "{value}"),
            Self::Unit => formatter.write_str("Unit"),
        }
    }
}

/// Successful execution of a Nexa `main` function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution {
    value: Value,
    output: Vec<String>,
}

impl Execution {
    /// Returns the value returned from `main`.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Returns lines emitted by the `print` builtin in order.
    #[must_use]
    pub fn output(&self) -> &[String] {
        &self.output
    }
}

/// A source-spanned failure raised while interpreting MIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    message: String,
    span: SourceSpan,
}

impl RuntimeError {
    /// Returns the runtime failure description.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the source range that triggered the failure.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    fn new(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl Display for RuntimeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeError {}

impl From<MirLoweringError> for RuntimeError {
    fn from(error: MirLoweringError) -> Self {
        Self::new(error.span(), error.message().to_owned())
    }
}

/// Executes the `main` entry point of a resolved MIR program.
///
/// # Errors
///
/// Returns [`RuntimeError`] when `main` is missing, arithmetic overflows, a
/// division by zero occurs, or an internal MIR invariant is violated.
pub fn run(program: &MirProgram) -> Result<Execution, RuntimeError> {
    let main = program
        .functions
        .iter()
        .position(|function| function.name == "main")
        .map(FunctionId)
        .ok_or_else(|| RuntimeError::new(program.span, "program has no `main` entry point"))?;
    let mut interpreter = Interpreter {
        program,
        output: Vec::new(),
    };
    let value = interpreter.call(main, Vec::new(), program.span)?;

    Ok(Execution {
        value,
        output: interpreter.output,
    })
}

struct Interpreter<'program> {
    program: &'program MirProgram,
    output: Vec<String>,
}

impl Interpreter<'_> {
    fn call(
        &mut self,
        function_id: FunctionId,
        arguments: Vec<Value>,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        let function = self
            .program
            .functions
            .get(function_id.0)
            .cloned()
            .ok_or_else(|| RuntimeError::new(span, "call target does not exist"))?;
        if function.parameters.len() != arguments.len() {
            return Err(RuntimeError::new(
                span,
                format!(
                    "function `{}` expected {} argument(s), found {}",
                    function.name,
                    function.parameters.len(),
                    arguments.len()
                ),
            ));
        }

        let mut frame = Frame {
            locals: vec![Value::Unit; function.local_count],
        };
        for (local, argument) in function.parameters.iter().copied().zip(arguments) {
            frame.store(local, argument, function.span)?;
        }

        match self.execute_block(&function.body, &mut frame)? {
            Control::Continue => Ok(Value::Unit),
            Control::Return(value) => Ok(value),
        }
    }

    fn execute_block(
        &mut self,
        block: &MirBlock,
        frame: &mut Frame,
    ) -> Result<Control, RuntimeError> {
        for statement in &block.statements {
            let control = self.execute_statement(statement, frame)?;
            if let Control::Return(_) = control {
                return Ok(control);
            }
        }

        Ok(Control::Continue)
    }

    fn execute_statement(
        &mut self,
        statement: &MirStatement,
        frame: &mut Frame,
    ) -> Result<Control, RuntimeError> {
        match statement {
            MirStatement::Initialize { local, value, span } => {
                let value = self.evaluate(value, frame)?;
                frame.store(*local, value, *span)?;
                Ok(Control::Continue)
            }
            MirStatement::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => match self.evaluate(condition, frame)? {
                Value::Bool(true) => self.execute_block(then_branch, frame),
                Value::Bool(false) => else_branch
                    .as_ref()
                    .map_or(Ok(Control::Continue), |branch| {
                        self.execute_block(branch, frame)
                    }),
                value => Err(RuntimeError::new(
                    *span,
                    format!(
                        "if condition evaluated to `{}` instead of `Bool`",
                        value_type(&value)
                    ),
                )),
            },
            MirStatement::Return { value, .. } => Ok(Control::Return(
                value
                    .as_ref()
                    .map(|value| self.evaluate(value, frame))
                    .transpose()?
                    .unwrap_or(Value::Unit),
            )),
            MirStatement::Expression { expression, .. } => {
                let _ = self.evaluate(expression, frame)?;
                Ok(Control::Continue)
            }
        }
    }

    fn evaluate(
        &mut self,
        expression: &MirExpression,
        frame: &Frame,
    ) -> Result<Value, RuntimeError> {
        match expression {
            MirExpression::Integer { value, .. } => Ok(Value::Int(*value)),
            MirExpression::Boolean { value, .. } => Ok(Value::Bool(*value)),
            MirExpression::Local { local, span } => frame.load(*local, *span),
            MirExpression::Unary {
                operator,
                expression,
                span,
            } => self.evaluate_unary(*operator, expression, *span, frame),
            MirExpression::Binary {
                operator,
                left,
                right,
                span,
            } => self.evaluate_binary(*operator, left, right, *span, frame),
            MirExpression::Call {
                callee,
                arguments,
                span,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.evaluate(argument, frame))
                    .collect::<Result<Vec<_>, _>>()?;
                self.evaluate_call(*callee, arguments, *span)
            }
        }
    }

    fn evaluate_unary(
        &mut self,
        operator: UnaryOperator,
        expression: &MirExpression,
        span: SourceSpan,
        frame: &Frame,
    ) -> Result<Value, RuntimeError> {
        let value = self.evaluate(expression, frame)?;
        match (operator, value) {
            (UnaryOperator::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
            (UnaryOperator::Negate, Value::Int(value)) => value
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| RuntimeError::new(span, "integer negation overflow")),
            (UnaryOperator::Not, value) => Err(RuntimeError::new(
                span,
                format!("cannot negate `{}` with `!`", value_type(&value)),
            )),
            (UnaryOperator::Negate, value) => Err(RuntimeError::new(
                span,
                format!("cannot negate `{}` with `-`", value_type(&value)),
            )),
        }
    }

    fn evaluate_binary(
        &mut self,
        operator: BinaryOperator,
        left: &MirExpression,
        right: &MirExpression,
        span: SourceSpan,
        frame: &Frame,
    ) -> Result<Value, RuntimeError> {
        let left = self.evaluate(left, frame)?;
        let right = self.evaluate(right, frame)?;

        match operator {
            BinaryOperator::Equal => Ok(Value::Bool(left == right)),
            BinaryOperator::Add => {
                checked_integer_operation(left, right, span, "addition", i64::checked_add)
            }
            BinaryOperator::Subtract => {
                checked_integer_operation(left, right, span, "subtraction", i64::checked_sub)
            }
            BinaryOperator::Multiply => {
                checked_integer_operation(left, right, span, "multiplication", i64::checked_mul)
            }
            BinaryOperator::Divide => checked_division(left, right, span),
            BinaryOperator::Less => compare_integers(left, right, span, |left, right| left < right),
            BinaryOperator::LessEqual => {
                compare_integers(left, right, span, |left, right| left <= right)
            }
            BinaryOperator::Greater => {
                compare_integers(left, right, span, |left, right| left > right)
            }
            BinaryOperator::GreaterEqual => {
                compare_integers(left, right, span, |left, right| left >= right)
            }
        }
    }

    fn evaluate_call(
        &mut self,
        callee: Callee,
        arguments: Vec<Value>,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        match callee {
            Callee::Function(function) => self.call(function, arguments, span),
            Callee::Print => {
                let [Value::Int(value)] = arguments.as_slice() else {
                    return Err(RuntimeError::new(
                        span,
                        "`print` requires one `Int` argument",
                    ));
                };
                self.output.push(value.to_string());
                Ok(Value::Unit)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Control {
    Continue,
    Return(Value),
}

struct Frame {
    locals: Vec<Value>,
}

impl Frame {
    fn load(&self, local: LocalId, span: SourceSpan) -> Result<Value, RuntimeError> {
        self.locals
            .get(local.0)
            .cloned()
            .ok_or_else(|| RuntimeError::new(span, "local slot does not exist"))
    }

    fn store(
        &mut self,
        local: LocalId,
        value: Value,
        span: SourceSpan,
    ) -> Result<(), RuntimeError> {
        let Some(slot) = self.locals.get_mut(local.0) else {
            return Err(RuntimeError::new(span, "local slot does not exist"));
        };
        *slot = value;
        Ok(())
    }
}

fn checked_integer_operation(
    left: Value,
    right: Value,
    span: SourceSpan,
    operation: &str,
    apply: impl FnOnce(i64, i64) -> Option<i64>,
) -> Result<Value, RuntimeError> {
    let (left, right) = integer_operands(left, right, span)?;
    apply(left, right)
        .map(Value::Int)
        .ok_or_else(|| RuntimeError::new(span, format!("integer {operation} overflow")))
}

fn checked_division(left: Value, right: Value, span: SourceSpan) -> Result<Value, RuntimeError> {
    let (left, right) = integer_operands(left, right, span)?;
    if right == 0 {
        return Err(RuntimeError::new(span, "division by zero"));
    }

    left.checked_div(right)
        .map(Value::Int)
        .ok_or_else(|| RuntimeError::new(span, "integer division overflow"))
}

fn compare_integers(
    left: Value,
    right: Value,
    span: SourceSpan,
    compare: impl FnOnce(i64, i64) -> bool,
) -> Result<Value, RuntimeError> {
    let (left, right) = integer_operands(left, right, span)?;

    Ok(Value::Bool(compare(left, right)))
}

fn integer_operands(
    left: Value,
    right: Value,
    span: SourceSpan,
) -> Result<(i64, i64), RuntimeError> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => Ok((left, right)),
        (left, right) => Err(RuntimeError::new(
            span,
            format!(
                "integer operation received `{}` and `{}`",
                value_type(&left),
                value_type(&right)
            ),
        )),
    }
}

const fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Int(_) => "Int",
        Value::Bool(_) => "Bool",
        Value::Unit => "Unit",
    }
}
