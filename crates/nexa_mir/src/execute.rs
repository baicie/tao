use std::fmt::{Display, Formatter};
use std::rc::Rc;

use nexa_hir::{BinaryOperator, UnaryOperator};
use nexa_span::SourceSpan;

use crate::{
    Callee, FunctionId, LocalId, MirExpression, MirLoweringError, MirProgram, MirStatement,
    MirTerminator,
};

/// A runtime value produced by the reference interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A signed 64-bit integer.
    Int(i64),
    /// A boolean value.
    Bool(bool),
    /// An immutable UTF-8 string.
    String(Rc<str>),
    /// An immutable, fixed-length array.
    Array(Rc<[Value]>),
    /// The absence of a value.
    Unit,
}

impl Display for Value {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(value) => write!(formatter, "{value}"),
            Self::Bool(value) => write!(formatter, "{value}"),
            Self::String(value) => formatter.write_str(value),
            Self::Array(_) => formatter.write_str("<array>"),
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

/// A failed execution together with output emitted before the failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFailure {
    error: RuntimeError,
    output: Vec<String>,
}

impl RuntimeFailure {
    /// Returns the runtime error that stopped execution.
    #[must_use]
    pub const fn error(&self) -> &RuntimeError {
        &self.error
    }

    /// Returns lines emitted by `print` before execution stopped.
    #[must_use]
    pub fn output(&self) -> &[String] {
        &self.output
    }

    /// Consumes the failure and returns its error and prior output.
    #[must_use]
    pub fn into_parts(self) -> (RuntimeError, Vec<String>) {
        (self.error, self.output)
    }
}

impl From<RuntimeError> for RuntimeFailure {
    fn from(error: RuntimeError) -> Self {
        Self {
            error,
            output: Vec::new(),
        }
    }
}

impl Display for RuntimeFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for RuntimeFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
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
/// Returns [`RuntimeFailure`] when `main` is missing, execution exceeds the
/// reference interpreter's call-depth or basic-block step limits, arithmetic
/// overflows, division by zero occurs, an array index is out of bounds, or an
/// internal MIR invariant fails.
pub fn run(program: &MirProgram) -> Result<Execution, RuntimeFailure> {
    run_with_args(program, &[])
}

/// Executes the `main` entry point with command-line string arguments.
///
/// # Errors
///
/// Returns [`RuntimeFailure`] under the same conditions as [`run`], and when
/// arguments are supplied to a parameterless `main` function.
pub fn run_with_args(
    program: &MirProgram,
    arguments: &[String],
) -> Result<Execution, RuntimeFailure> {
    let main = program
        .functions
        .iter()
        .position(|function| function.name == "main")
        .map(FunctionId)
        .ok_or_else(|| RuntimeError::new(program.span, "program has no `main` entry point"))
        .map_err(RuntimeFailure::from)?;
    let main_arguments = match program.functions[main.0].parameters.len() {
        0 if arguments.is_empty() => Vec::new(),
        0 => {
            return Err(RuntimeError::new(
                program.span,
                "parameterless `main` does not accept command-line arguments",
            )
            .into());
        }
        1 => vec![Value::Array(
            arguments
                .iter()
                .map(|argument| Value::String(Rc::<str>::from(argument.as_str())))
                .collect::<Vec<_>>()
                .into(),
        )],
        _ => {
            return Err(
                RuntimeError::new(program.span, "`main` has an invalid runtime signature").into(),
            );
        }
    };
    let mut interpreter = Interpreter {
        output: Vec::new(),
        call_depth: 0,
        steps: 0,
    };
    match interpreter.call(program, main, main_arguments, program.span) {
        Ok(value) => Ok(Execution {
            value,
            output: interpreter.output,
        }),
        Err(error) => Err(RuntimeFailure {
            error,
            output: interpreter.output,
        }),
    }
}

const MAX_CALL_DEPTH: usize = 64;
const MAX_EXECUTION_STEPS: usize = 100_000;

struct Interpreter {
    output: Vec<String>,
    call_depth: usize,
    steps: usize,
}

impl Interpreter {
    fn call(
        &mut self,
        program: &MirProgram,
        function_id: FunctionId,
        arguments: Vec<Value>,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(RuntimeError::new(
                span,
                format!("maximum call depth of {MAX_CALL_DEPTH} exceeded"),
            ));
        }

        self.call_depth += 1;
        let result = self.call_active(program, function_id, arguments, span);
        self.call_depth -= 1;
        result
    }

    fn call_active(
        &mut self,
        program: &MirProgram,
        function_id: FunctionId,
        arguments: Vec<Value>,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        let function = program
            .functions
            .get(function_id.0)
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

        let mut current = function.entry;
        loop {
            let block = function
                .blocks
                .get(current.0)
                .ok_or_else(|| RuntimeError::new(function.span, "basic block does not exist"))?;
            self.consume_step(block.span)?;

            for statement in &block.statements {
                self.execute_statement(program, statement, &mut frame)?;
            }

            current = match &block.terminator {
                MirTerminator::Goto { target, .. } => *target,
                MirTerminator::Branch {
                    condition,
                    then_target,
                    else_target,
                    span,
                } => match self.evaluate(program, condition, &frame)? {
                    Value::Bool(true) => *then_target,
                    Value::Bool(false) => *else_target,
                    value => {
                        return Err(RuntimeError::new(
                            *span,
                            format!(
                                "control-flow condition evaluated to `{}` instead of `Bool`",
                                value_type(&value)
                            ),
                        ));
                    }
                },
                MirTerminator::Return { value, .. } => {
                    return value
                        .as_ref()
                        .map(|value| self.evaluate(program, value, &frame))
                        .transpose()
                        .map(|value| value.unwrap_or(Value::Unit));
                }
            };
        }
    }

    fn consume_step(&mut self, span: SourceSpan) -> Result<(), RuntimeError> {
        if self.steps >= MAX_EXECUTION_STEPS {
            return Err(RuntimeError::new(
                span,
                format!("execution step limit of {MAX_EXECUTION_STEPS} exceeded"),
            ));
        }
        self.steps += 1;
        Ok(())
    }

    fn execute_statement(
        &mut self,
        program: &MirProgram,
        statement: &MirStatement,
        frame: &mut Frame,
    ) -> Result<(), RuntimeError> {
        match statement {
            MirStatement::Store { local, value, span } => {
                let value = self.evaluate(program, value, frame)?;
                frame.store(*local, value, *span)
            }
            MirStatement::Expression { expression, .. } => {
                let _ = self.evaluate(program, expression, frame)?;
                Ok(())
            }
        }
    }

    fn evaluate(
        &mut self,
        program: &MirProgram,
        expression: &MirExpression,
        frame: &Frame,
    ) -> Result<Value, RuntimeError> {
        match expression {
            MirExpression::Integer { value, .. } => Ok(Value::Int(*value)),
            MirExpression::Boolean { value, .. } => Ok(Value::Bool(*value)),
            MirExpression::String { value, .. } => {
                Ok(Value::String(Rc::<str>::from(value.as_str())))
            }
            MirExpression::Array { elements, .. } => elements
                .iter()
                .map(|element| self.evaluate(program, element, frame))
                .collect::<Result<Vec<_>, _>>()
                .map(|values| Value::Array(values.into())),
            MirExpression::Index {
                target,
                index,
                span,
            } => {
                let target = self.evaluate(program, target, frame)?;
                let index = self.evaluate(program, index, frame)?;
                evaluate_index(target, index, *span)
            }
            MirExpression::Length { target, span } => {
                let target = self.evaluate(program, target, frame)?;
                evaluate_length(target, *span)
            }
            MirExpression::Local { local, span } => frame.load(*local, *span),
            MirExpression::Unary {
                operator,
                expression,
                span,
            } => self.evaluate_unary(program, *operator, expression, *span, frame),
            MirExpression::Binary {
                operator,
                left,
                right,
                span,
            } => self.evaluate_binary(program, *operator, left, right, *span, frame),
            MirExpression::Call {
                callee,
                arguments,
                span,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.evaluate(program, argument, frame))
                    .collect::<Result<Vec<_>, _>>()?;
                self.evaluate_call(program, *callee, arguments, *span)
            }
        }
    }

    fn evaluate_unary(
        &mut self,
        program: &MirProgram,
        operator: UnaryOperator,
        expression: &MirExpression,
        span: SourceSpan,
        frame: &Frame,
    ) -> Result<Value, RuntimeError> {
        let value = self.evaluate(program, expression, frame)?;
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
        program: &MirProgram,
        operator: BinaryOperator,
        left: &MirExpression,
        right: &MirExpression,
        span: SourceSpan,
        frame: &Frame,
    ) -> Result<Value, RuntimeError> {
        match operator {
            BinaryOperator::LogicalAnd => {
                let left = boolean_operand(self.evaluate(program, left, frame)?, left.span())?;
                if !left {
                    return Ok(Value::Bool(false));
                }
                let right = boolean_operand(self.evaluate(program, right, frame)?, right.span())?;
                return Ok(Value::Bool(right));
            }
            BinaryOperator::LogicalOr => {
                let left = boolean_operand(self.evaluate(program, left, frame)?, left.span())?;
                if left {
                    return Ok(Value::Bool(true));
                }
                let right = boolean_operand(self.evaluate(program, right, frame)?, right.span())?;
                return Ok(Value::Bool(right));
            }
            _ => {}
        }

        let left = self.evaluate(program, left, frame)?;
        let right = self.evaluate(program, right, frame)?;

        match operator {
            BinaryOperator::Equal => equal_values(left, right, span),
            BinaryOperator::Add => add_values(left, right, span),
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
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => Err(RuntimeError::new(
                span,
                "logical operator reached eager MIR evaluation",
            )),
        }
    }

    fn evaluate_call(
        &mut self,
        program: &MirProgram,
        callee: Callee,
        arguments: Vec<Value>,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        match callee {
            Callee::Function(function) => self.call(program, function, arguments, span),
            Callee::Print => {
                let [value] = arguments.as_slice() else {
                    return Err(RuntimeError::new(
                        span,
                        "`print` requires one scalar argument",
                    ));
                };
                if matches!(value, Value::Array(_) | Value::Unit) {
                    return Err(RuntimeError::new(
                        span,
                        format!("`print` cannot print `{}`", value_type(value)),
                    ));
                }
                self.output.push(value.to_string());
                Ok(Value::Unit)
            }
        }
    }
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

fn boolean_operand(value: Value, span: SourceSpan) -> Result<bool, RuntimeError> {
    match value {
        Value::Bool(value) => Ok(value),
        value => Err(RuntimeError::new(
            span,
            format!(
                "logical operation received `{}` instead of `Bool`",
                value_type(&value)
            ),
        )),
    }
}

fn add_values(left: Value, right: Value, span: SourceSpan) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => left
            .checked_add(right)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::new(span, "integer addition overflow")),
        (Value::String(left), Value::String(right)) => {
            let mut result = String::with_capacity(left.len() + right.len());
            result.push_str(&left);
            result.push_str(&right);
            Ok(Value::String(result.into()))
        }
        (left, right) => Err(RuntimeError::new(
            span,
            format!(
                "addition received `{}` and `{}`",
                value_type(&left),
                value_type(&right)
            ),
        )),
    }
}

fn equal_values(left: Value, right: Value, span: SourceSpan) -> Result<Value, RuntimeError> {
    let equal = match (left, right) {
        (Value::Int(left), Value::Int(right)) => left == right,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::String(left), Value::String(right)) => left == right,
        (Value::Unit, Value::Unit) => true,
        (left, right) => {
            return Err(RuntimeError::new(
                span,
                format!(
                    "equality received `{}` and `{}`",
                    value_type(&left),
                    value_type(&right)
                ),
            ));
        }
    };

    Ok(Value::Bool(equal))
}

fn evaluate_index(target: Value, index: Value, span: SourceSpan) -> Result<Value, RuntimeError> {
    let values = match target {
        Value::Array(values) => values,
        target => {
            return Err(RuntimeError::new(
                span,
                format!("cannot index `{}`", value_type(&target)),
            ));
        }
    };
    let index = match index {
        Value::Int(index) => index,
        index => {
            return Err(RuntimeError::new(
                span,
                format!("array index must be `Int`, found `{}`", value_type(&index)),
            ));
        }
    };
    let array_index = usize::try_from(index).ok();
    array_index
        .and_then(|index| values.get(index))
        .cloned()
        .ok_or_else(|| {
            RuntimeError::new(
                span,
                format!(
                    "array index {index} out of bounds for length {}",
                    values.len()
                ),
            )
        })
}

fn evaluate_length(target: Value, span: SourceSpan) -> Result<Value, RuntimeError> {
    let values = match target {
        Value::Array(values) => values,
        target => {
            return Err(RuntimeError::new(
                span,
                format!("cannot read `length` from `{}`", value_type(&target)),
            ));
        }
    };
    i64::try_from(values.len())
        .map(Value::Int)
        .map_err(|_| RuntimeError::new(span, "array length exceeds `Int` range"))
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
        Value::String(_) => "String",
        Value::Array(_) => "Array",
        Value::Unit => "Unit",
    }
}
