use std::fmt::{Display, Formatter};
use std::rc::Rc;

use nexa_hir::{BinaryOperator, FieldId, PayloadId, RecordId, UnaryOperator, UnionId, VariantId};
use nexa_span::SourceSpan;

use crate::{
    BasicBlockId, Callee, FunctionId, LocalId, MirExpression, MirLoweringError, MirProgram,
    MirStatement, MirTerminator, MirUnion, MirVariant,
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
    /// An immutable nominal record with declaration-ordered fields.
    Record {
        /// The record's resolved nominal identity.
        record: RecordId,
        /// Field values in declaration order.
        fields: Rc<[Value]>,
    },
    /// An immutable nominal tagged union value.
    Union {
        /// The union's resolved nominal identity.
        union: UnionId,
        /// The selected owner-scoped variant identity.
        variant: VariantId,
        /// Positional payload values in declaration order.
        payloads: Rc<[Value]>,
        /// Cached recursive union depth, including this value.
        depth: usize,
    },
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
            Self::Record { record, .. } => write!(formatter, "<record#{}>", record.index()),
            Self::Union { union, variant, .. } => write!(
                formatter,
                "<union#{}::variant#{}>",
                union.index(),
                variant.index()
            ),
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
/// overflows, division by zero occurs, an array index is out of bounds,
/// recursive union nesting exceeds its limit, or an internal MIR invariant
/// fails.
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
const MAX_RECURSIVE_UNION_DEPTH: usize = 1024;

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
                MirTerminator::SwitchVariant {
                    scrutinee,
                    union,
                    targets,
                    span,
                } => {
                    let value = frame.load(*scrutinee, *span)?;
                    select_variant_target(program, value, *union, targets, *span)?
                }
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
            MirExpression::Record {
                record,
                fields,
                span,
            } => self.evaluate_record(program, *record, fields, *span, frame),
            MirExpression::Variant {
                union,
                variant,
                payloads,
                span,
            } => self.evaluate_variant(program, *union, *variant, payloads, *span, frame),
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
            MirExpression::Field {
                target,
                record,
                field,
                span,
            } => {
                let target = self.evaluate(program, target, frame)?;
                validate_field_layout(program, *record, *field, *span)?;
                evaluate_field(target, *record, *field, *span)
            }
            MirExpression::VariantPayload {
                source,
                union,
                variant,
                payload,
                span,
            } => evaluate_variant_payload(
                program,
                frame.load(*source, *span)?,
                *union,
                *variant,
                *payload,
                *span,
            ),
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

    fn evaluate_record(
        &mut self,
        program: &MirProgram,
        record: RecordId,
        fields: &[MirExpression],
        span: SourceSpan,
        frame: &Frame,
    ) -> Result<Value, RuntimeError> {
        let layout = program
            .records
            .get(record.index())
            .filter(|layout| layout.id == record)
            .ok_or_else(|| RuntimeError::new(span, "record layout does not exist"))?;
        if fields.len() != layout.fields.len() {
            return Err(RuntimeError::new(
                span,
                format!(
                    "record construction expected {} field(s), found {}",
                    layout.fields.len(),
                    fields.len()
                ),
            ));
        }

        fields
            .iter()
            .map(|field| self.evaluate(program, field, frame))
            .collect::<Result<Vec<_>, _>>()
            .map(|fields| Value::Record {
                record,
                fields: fields.into(),
            })
    }

    fn evaluate_variant(
        &mut self,
        program: &MirProgram,
        union: UnionId,
        variant: VariantId,
        payloads: &[MirExpression],
        span: SourceSpan,
        frame: &Frame,
    ) -> Result<Value, RuntimeError> {
        let layout = variant_layout(program, union, variant, span)?;
        if payloads.len() != layout.payloads.len() {
            return Err(RuntimeError::new(
                span,
                format!(
                    "variant construction expected {} payload(s), found {}",
                    layout.payloads.len(),
                    payloads.len()
                ),
            ));
        }

        let mut values = Vec::with_capacity(payloads.len());
        for payload in payloads {
            values.push(self.evaluate(program, payload, frame)?);
        }
        let nested_depth = values.iter().map(recursive_union_depth).max().unwrap_or(0);
        let depth = nested_depth.saturating_add(1);
        if depth > MAX_RECURSIVE_UNION_DEPTH {
            return Err(RuntimeError::new(
                span,
                format!("recursive union nesting limit of {MAX_RECURSIVE_UNION_DEPTH} exceeded"),
            ));
        }

        Ok(Value::Union {
            union,
            variant,
            payloads: values.into(),
            depth,
        })
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
                if matches!(
                    value,
                    Value::Array(_) | Value::Record { .. } | Value::Union { .. } | Value::Unit
                ) {
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

fn union_layout(
    program: &MirProgram,
    union: UnionId,
    span: SourceSpan,
) -> Result<&MirUnion, RuntimeError> {
    let layout = program
        .unions
        .get(union.index())
        .filter(|layout| layout.id == union)
        .ok_or_else(|| RuntimeError::new(span, "union layout does not exist"))?;
    for (variant_index, variant) in layout.variants.iter().enumerate() {
        let expected_variant = VariantId::new(union, variant_index);
        if variant.id != expected_variant {
            return Err(RuntimeError::new(
                span,
                "union variant layout identity is inconsistent",
            ));
        }
        for (payload_index, payload) in variant.payloads.iter().enumerate() {
            if payload.id != PayloadId::new(expected_variant, payload_index) {
                return Err(RuntimeError::new(
                    span,
                    "variant payload layout identity is inconsistent",
                ));
            }
        }
    }

    Ok(layout)
}

fn variant_layout(
    program: &MirProgram,
    union: UnionId,
    variant: VariantId,
    span: SourceSpan,
) -> Result<&MirVariant, RuntimeError> {
    if variant.union() != union {
        return Err(RuntimeError::new(
            span,
            "variant identity does not belong to the expected union",
        ));
    }

    union_layout(program, union, span)?
        .variants
        .get(variant.index())
        .filter(|layout| layout.id == variant)
        .ok_or_else(|| RuntimeError::new(span, "variant layout does not exist"))
}

fn validate_payload_layout(
    program: &MirProgram,
    union: UnionId,
    variant: VariantId,
    payload: PayloadId,
    span: SourceSpan,
) -> Result<usize, RuntimeError> {
    if payload.variant() != variant {
        return Err(RuntimeError::new(
            span,
            "payload identity does not belong to the expected variant",
        ));
    }
    let variant = variant_layout(program, union, variant, span)?;
    variant
        .payloads
        .get(payload.index())
        .filter(|layout| layout.id == payload)
        .map(|_| variant.payloads.len())
        .ok_or_else(|| RuntimeError::new(span, "payload layout does not exist"))
}

fn select_variant_target(
    program: &MirProgram,
    value: Value,
    expected_union: UnionId,
    targets: &[BasicBlockId],
    span: SourceSpan,
) -> Result<BasicBlockId, RuntimeError> {
    let union_layout = union_layout(program, expected_union, span)?;
    if targets.len() != union_layout.variants.len() {
        return Err(RuntimeError::new(
            span,
            format!(
                "variant switch expected {} target(s), found {}",
                union_layout.variants.len(),
                targets.len()
            ),
        ));
    }
    let Value::Union {
        union,
        variant,
        payloads,
        ..
    } = value
    else {
        return Err(RuntimeError::new(
            span,
            format!(
                "variant switch expected `Union`, found `{}`",
                value_type(&value)
            ),
        ));
    };
    if union != expected_union {
        return Err(RuntimeError::new(
            span,
            format!(
                "variant switch expected union#{}, found union#{}",
                expected_union.index(),
                union.index()
            ),
        ));
    }
    let layout = variant_layout(program, expected_union, variant, span)?;
    if payloads.len() != layout.payloads.len() {
        return Err(RuntimeError::new(
            span,
            format!(
                "runtime variant expected {} payload(s), found {}",
                layout.payloads.len(),
                payloads.len()
            ),
        ));
    }

    targets
        .get(variant.index())
        .copied()
        .ok_or_else(|| RuntimeError::new(span, "variant tag is outside the switch table"))
}

fn evaluate_variant_payload(
    program: &MirProgram,
    value: Value,
    expected_union: UnionId,
    expected_variant: VariantId,
    payload: PayloadId,
    span: SourceSpan,
) -> Result<Value, RuntimeError> {
    let expected_payload_count =
        validate_payload_layout(program, expected_union, expected_variant, payload, span)?;
    let Value::Union {
        union,
        variant,
        payloads,
        ..
    } = value
    else {
        return Err(RuntimeError::new(
            span,
            format!(
                "payload projection expected `Union`, found `{}`",
                value_type(&value)
            ),
        ));
    };
    if union != expected_union {
        return Err(RuntimeError::new(
            span,
            "payload projection received a different nominal union",
        ));
    }
    if variant != expected_variant {
        return Err(RuntimeError::new(
            span,
            "payload projection received a different variant tag",
        ));
    }
    if payloads.len() != expected_payload_count {
        return Err(RuntimeError::new(
            span,
            format!(
                "runtime variant expected {expected_payload_count} payload(s), found {}",
                payloads.len()
            ),
        ));
    }

    payloads
        .get(payload.index())
        .cloned()
        .ok_or_else(|| RuntimeError::new(span, "payload index is outside the runtime value"))
}

fn recursive_union_depth(value: &Value) -> usize {
    match value {
        Value::Union { depth, .. } => *depth,
        Value::Array(values) => values.iter().map(recursive_union_depth).max().unwrap_or(0),
        Value::Record { fields, .. } => fields.iter().map(recursive_union_depth).max().unwrap_or(0),
        Value::Int(_) | Value::Bool(_) | Value::String(_) | Value::Unit => 0,
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

fn validate_field_layout(
    program: &MirProgram,
    record: RecordId,
    field: FieldId,
    span: SourceSpan,
) -> Result<(), RuntimeError> {
    if field.record() != record {
        return Err(RuntimeError::new(
            span,
            "record field identity does not match the projected record",
        ));
    }
    let layout = program
        .records
        .get(record.index())
        .filter(|layout| layout.id == record)
        .ok_or_else(|| RuntimeError::new(span, "record layout does not exist"))?;
    let Some(layout_field) = layout.fields.get(field.index()) else {
        return Err(RuntimeError::new(
            span,
            "record field index is outside its layout",
        ));
    };
    if layout_field.id != field {
        return Err(RuntimeError::new(
            span,
            "record field layout identity is inconsistent",
        ));
    }

    Ok(())
}

fn evaluate_field(
    target: Value,
    record: RecordId,
    field: FieldId,
    span: SourceSpan,
) -> Result<Value, RuntimeError> {
    let (actual_record, fields) = match target {
        Value::Record { record, fields } => (record, fields),
        target => {
            return Err(RuntimeError::new(
                span,
                format!("cannot read a record field from `{}`", value_type(&target)),
            ));
        }
    };
    if actual_record != record {
        return Err(RuntimeError::new(
            span,
            format!(
                "record field expected record#{}, found record#{}",
                record.index(),
                actual_record.index()
            ),
        ));
    }

    fields.get(field.index()).cloned().ok_or_else(|| {
        RuntimeError::new(
            span,
            format!(
                "record field index {} is outside the runtime value",
                field.index()
            ),
        )
    })
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
        Value::Record { .. } => "Record",
        Value::Union { .. } => "Union",
        Value::Unit => "Unit",
    }
}

#[cfg(test)]
mod tests {
    use nexa_hir::{PayloadId, Type, UnionId, VariantId};
    use nexa_span::{FileId, TextRange};

    use super::{
        evaluate_field, evaluate_variant_payload, select_variant_target, validate_field_layout,
        validate_payload_layout, variant_layout, FieldId, RecordId, SourceSpan, Value,
    };
    use crate::{
        BasicBlockId, MirPayload, MirProgram, MirRecord, MirRecordField, MirUnion, MirVariant,
    };

    #[test]
    fn field_projection_reports_a_nominal_tag_mismatch_at_the_access_span(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expected = RecordId::new(0);
        let actual = RecordId::new(1);
        let span = SourceSpan::new(FileId::new(9), TextRange::new(10, 20));
        let value = Value::Record {
            record: actual,
            fields: vec![Value::Int(42)].into(),
        };

        let error = evaluate_field(value, expected, FieldId::new(expected, 0), span)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a record tag error"))?;

        assert_eq!(
            (error.message(), error.span()),
            ("record field expected record#0, found record#1", span)
        );

        Ok(())
    }

    #[test]
    fn field_projection_reports_a_runtime_field_index_error_at_the_access_span(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let record = RecordId::new(0);
        let span = SourceSpan::new(FileId::new(9), TextRange::new(30, 40));
        let value = Value::Record {
            record,
            fields: vec![Value::Int(42)].into(),
        };

        let error = evaluate_field(value, record, FieldId::new(record, 1), span)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a record field index error"))?;

        assert_eq!(
            (error.message(), error.span()),
            ("record field index 1 is outside the runtime value", span)
        );

        Ok(())
    }

    #[test]
    fn field_layout_validation_rejects_a_field_owned_by_another_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let record = RecordId::new(0);
        let other = RecordId::new(1);
        let span = SourceSpan::new(FileId::new(9), TextRange::new(50, 60));
        let program = MirProgram {
            records: vec![MirRecord {
                id: record,
                name: "Box".to_owned(),
                fields: vec![MirRecordField {
                    id: FieldId::new(record, 0),
                    name: "value".to_owned(),
                    ty: Type::Int,
                    span,
                }],
                span,
            }],
            unions: Vec::new(),
            functions: Vec::new(),
            span,
        };

        let error = validate_field_layout(&program, record, FieldId::new(other, 0), span)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a field owner error"))?;

        assert_eq!(
            (error.message(), error.span()),
            (
                "record field identity does not match the projected record",
                span
            )
        );

        Ok(())
    }

    #[test]
    fn variant_layout_validation_rejects_a_variant_owned_by_another_union(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(70, 80));
        let program = union_program(span);
        let expected = UnionId::new(0);
        let forged = VariantId::new(UnionId::new(1), 0);

        let error = variant_layout(&program, expected, forged, span)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a variant owner error"))?;

        assert_eq!(
            (error.message(), error.span()),
            (
                "variant identity does not belong to the expected union",
                span
            )
        );

        Ok(())
    }

    #[test]
    fn variant_switch_rejects_a_forged_runtime_tag_index() -> Result<(), Box<dyn std::error::Error>>
    {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(90, 100));
        let program = union_program(span);
        let union = UnionId::new(0);
        let value = Value::Union {
            union,
            variant: VariantId::new(union, 1),
            payloads: Vec::new().into(),
            depth: 1,
        };

        let error = select_variant_target(&program, value, union, &[BasicBlockId(0)], span)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a runtime variant tag error"))?;

        assert_eq!(
            (error.message(), error.span()),
            ("variant layout does not exist", span)
        );

        Ok(())
    }

    #[test]
    fn payload_layout_validation_rejects_a_payload_owned_by_another_variant(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(110, 120));
        let program = union_program(span);
        let union = UnionId::new(0);
        let variant = VariantId::new(union, 0);
        let forged = PayloadId::new(VariantId::new(union, 1), 0);

        let error = validate_payload_layout(&program, union, variant, forged, span)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a payload owner error"))?;

        assert_eq!(
            (error.message(), error.span()),
            (
                "payload identity does not belong to the expected variant",
                span
            )
        );

        Ok(())
    }

    #[test]
    fn union_layout_validation_rejects_a_forged_payload_position(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(125, 130));
        let mut program = union_program(span);
        let union = UnionId::new(0);
        let variant = VariantId::new(union, 0);
        program.unions[0].variants[0].payloads[0].id = PayloadId::new(variant, 1);

        let error = variant_layout(&program, union, variant, span)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a payload layout position error"))?;

        assert_eq!(
            (error.message(), error.span()),
            ("variant payload layout identity is inconsistent", span)
        );

        Ok(())
    }

    #[test]
    fn payload_projection_rejects_a_missing_runtime_payload_position(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(130, 140));
        let program = union_program(span);
        let union = UnionId::new(0);
        let variant = VariantId::new(union, 0);
        let payload = PayloadId::new(variant, 0);
        let value = Value::Union {
            union,
            variant,
            payloads: Vec::new().into(),
            depth: 1,
        };

        let error = evaluate_variant_payload(&program, value, union, variant, payload, span)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a runtime payload index error"))?;

        assert_eq!(
            (error.message(), error.span()),
            ("runtime variant expected 1 payload(s), found 0", span)
        );

        Ok(())
    }

    fn union_program(span: SourceSpan) -> MirProgram {
        let union = UnionId::new(0);
        let variant = VariantId::new(union, 0);
        MirProgram {
            records: Vec::new(),
            unions: vec![MirUnion {
                id: union,
                name: "Value".to_owned(),
                variants: vec![MirVariant {
                    id: variant,
                    name: "Integer".to_owned(),
                    payloads: vec![MirPayload {
                        id: PayloadId::new(variant, 0),
                        name: "value".to_owned(),
                        ty: Type::Int,
                        span,
                    }],
                    span,
                }],
                span,
            }],
            functions: Vec::new(),
            span,
        }
    }
}
