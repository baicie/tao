use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

use nexa_hir::{
    BinaryOperator, ClosureId, FieldId, ModuleId, PayloadId, RecordId, UnaryOperator, UnionId,
    VariantId,
};
use nexa_span::SourceSpan;

use crate::{
    ArrayIntrinsic, BasicBlockId, Callee, FunctionId, LocalId, MirBasicBlock, MirExpression,
    MirLoweringError, MirProgram, MirStatement, MirTerminator, MirUnion, MirVariant,
};

/// A stable callable value understood by the reference interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallableValue {
    /// A non-generic source function resolved by its module-owned identity.
    Function(FunctionId),
    /// One closure instance with its immutable capture snapshot.
    Closure {
        /// The stable arrow-site identity.
        closure: ClosureId,
        /// Captured values in the closure layout's declared order.
        captures: Rc<[Value]>,
    },
}

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
    /// A statically typed function or closure value.
    Callable(CallableValue),
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
            Self::Callable(CallableValue::Function(function)) => write!(
                formatter,
                "<function#{}:{}>",
                function.module().index(),
                function.index()
            ),
            Self::Callable(CallableValue::Closure { closure, .. }) => write!(
                formatter,
                "<closure#{}:{}:{}>",
                closure.owner().module().index(),
                closure.owner().index(),
                closure.source_index()
            ),
            Self::Record { record, .. } => write!(
                formatter,
                "<record#{}:{}>",
                record.module().index(),
                record.index()
            ),
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
    validate_program(program).map_err(RuntimeFailure::from)?;
    let main = program
        .entry
        .ok_or_else(|| RuntimeError::new(program.span, "program has no `main` entry point"))
        .map_err(RuntimeFailure::from)?;
    let main_function = program
        .function(main)
        .ok_or_else(|| RuntimeError::new(program.span, "entry function does not exist"))
        .map_err(RuntimeFailure::from)?;
    let main_arguments = match main_function.parameters.len() {
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

fn validate_program(program: &MirProgram) -> Result<(), RuntimeError> {
    let modules = program.modules.iter().copied().collect::<HashSet<_>>();
    if modules.len() != program.modules.len() {
        return Err(RuntimeError::new(
            program.span,
            "program contains a duplicate module identity",
        ));
    }
    if !modules.contains(&program.entry_module) {
        return Err(RuntimeError::new(
            program.span,
            "program entry module does not exist",
        ));
    }
    if program.entry_module != ModuleId::ENTRY {
        return Err(RuntimeError::new(
            program.span,
            "program entry module is not module 0",
        ));
    }

    let mut functions = HashSet::new();
    for function in &program.functions {
        if !modules.contains(&function.id.module()) {
            return Err(RuntimeError::new(
                function.span,
                "function owner module does not exist",
            ));
        }
        if !functions.insert(function.id) {
            return Err(RuntimeError::new(
                function.span,
                "program contains a duplicate function identity",
            ));
        }
        if function.parameter_types.len() != function.parameters.len() {
            return Err(RuntimeError::new(
                function.span,
                "function parameter signature metadata is inconsistent",
            ));
        }
        let mut parameter_slots = HashSet::new();
        for parameter in &function.parameters {
            if parameter.0 >= function.local_count {
                return Err(RuntimeError::new(
                    function.span,
                    "function parameter slot is outside its local frame",
                ));
            }
            if !parameter_slots.insert(*parameter) {
                return Err(RuntimeError::new(
                    function.span,
                    "function contains a duplicate parameter slot",
                ));
            }
        }
        validate_cfg(&function.blocks, function.entry, function.span)?;
    }

    let mut closures = HashSet::new();
    for closure in &program.closures {
        if !functions.contains(&closure.id.owner()) {
            return Err(RuntimeError::new(
                closure.span,
                "closure owner function does not exist",
            ));
        }
        if !closures.insert(closure.id) {
            return Err(RuntimeError::new(
                closure.span,
                "program contains a duplicate closure identity",
            ));
        }
        if closure.parameter_types.len() != closure.parameters.len() {
            return Err(RuntimeError::new(
                closure.span,
                "closure parameter signature metadata is inconsistent",
            ));
        }

        let mut occupied = HashSet::new();
        for capture in &closure.captures {
            if capture.0 >= closure.local_count {
                return Err(RuntimeError::new(
                    closure.span,
                    "closure capture slot is outside its local frame",
                ));
            }
            if !occupied.insert(*capture) {
                return Err(RuntimeError::new(
                    closure.span,
                    "closure contains a duplicate capture slot",
                ));
            }
        }
        for parameter in &closure.parameters {
            if parameter.0 >= closure.local_count {
                return Err(RuntimeError::new(
                    closure.span,
                    "closure parameter slot is outside its local frame",
                ));
            }
            if !occupied.insert(*parameter) {
                return Err(RuntimeError::new(
                    closure.span,
                    "closure capture and parameter slots overlap",
                ));
            }
        }
        validate_cfg(&closure.blocks, closure.entry, closure.span)?;
    }

    let mut records = HashSet::new();
    for record in &program.records {
        if !modules.contains(&record.id.module()) {
            return Err(RuntimeError::new(
                record.span,
                "record owner module does not exist",
            ));
        }
        if !records.insert(record.id) {
            return Err(RuntimeError::new(
                record.span,
                "program contains a duplicate record identity",
            ));
        }
        for (index, field) in record.fields.iter().enumerate() {
            if field.id != FieldId::new(record.id, index) {
                return Err(RuntimeError::new(
                    field.span,
                    "record field layout identity is inconsistent",
                ));
            }
        }
    }

    let mut unions = HashSet::new();
    for union in &program.unions {
        if !modules.contains(&union.id.module()) {
            return Err(RuntimeError::new(
                union.span,
                "union owner module does not exist",
            ));
        }
        if !unions.insert(union.id) {
            return Err(RuntimeError::new(
                union.span,
                "program contains a duplicate union identity",
            ));
        }
        let _ = union_layout(program, union.id, union.span)?;
    }

    if let Some(entry) = program.entry {
        if entry.module() != program.entry_module {
            return Err(RuntimeError::new(
                program.span,
                "entry function is not owned by the entry module",
            ));
        }
        if !functions.contains(&entry) {
            return Err(RuntimeError::new(
                program.span,
                "entry function does not exist",
            ));
        }
    }

    validate_callable_references(program)?;

    Ok(())
}

fn validate_callable_references(program: &MirProgram) -> Result<(), RuntimeError> {
    for function in &program.functions {
        validate_block_references(program, &function.blocks)?;
    }
    for closure in &program.closures {
        validate_block_references(program, &closure.blocks)?;
    }
    Ok(())
}

fn validate_block_references(
    program: &MirProgram,
    blocks: &[MirBasicBlock],
) -> Result<(), RuntimeError> {
    for block in blocks {
        for statement in &block.statements {
            let expression = match statement {
                MirStatement::Store { value, .. } => value,
                MirStatement::Expression { expression, .. } => expression,
            };
            validate_expression_references(program, expression)?;
        }

        match &block.terminator {
            MirTerminator::Branch { condition, .. } => {
                validate_expression_references(program, condition)?;
            }
            MirTerminator::Return {
                value: Some(value), ..
            } => validate_expression_references(program, value)?,
            MirTerminator::Goto { .. }
            | MirTerminator::SwitchVariant { .. }
            | MirTerminator::Return { value: None, .. } => {}
        }
    }
    Ok(())
}

fn validate_expression_references(
    program: &MirProgram,
    expression: &MirExpression,
) -> Result<(), RuntimeError> {
    match expression {
        MirExpression::Function { function, span } => {
            if program.function(*function).is_none() {
                return Err(RuntimeError::new(
                    *span,
                    "function value target does not exist",
                ));
            }
        }
        MirExpression::Closure {
            closure,
            captures,
            span,
        } => {
            let layout = program
                .closure(*closure)
                .ok_or_else(|| RuntimeError::new(*span, "closure layout does not exist"))?;
            if layout.captures.len() != captures.len() {
                return Err(RuntimeError::new(
                    *span,
                    format!(
                        "closure construction expected {} capture(s), found {}",
                        layout.captures.len(),
                        captures.len()
                    ),
                ));
            }
            for capture in captures {
                validate_expression_references(program, capture)?;
            }
        }
        MirExpression::Array { elements, .. } => {
            for element in elements {
                validate_expression_references(program, element)?;
            }
        }
        MirExpression::Record { fields, .. } => {
            for field in fields {
                validate_expression_references(program, field)?;
            }
        }
        MirExpression::Variant { payloads, .. } => {
            for payload in payloads {
                validate_expression_references(program, payload)?;
            }
        }
        MirExpression::Index { target, index, .. } => {
            validate_expression_references(program, target)?;
            validate_expression_references(program, index)?;
        }
        MirExpression::Length { target, .. }
        | MirExpression::Field { target, .. }
        | MirExpression::Unary {
            expression: target, ..
        } => validate_expression_references(program, target)?,
        MirExpression::ArrayIntrinsic {
            target, argument, ..
        } => {
            validate_expression_references(program, target)?;
            validate_expression_references(program, argument)?;
        }
        MirExpression::Binary { left, right, .. } => {
            validate_expression_references(program, left)?;
            validate_expression_references(program, right)?;
        }
        MirExpression::Call {
            callee,
            arguments,
            span,
        } => {
            if let Callee::Function(function) = callee {
                if program.function(*function).is_none() {
                    return Err(RuntimeError::new(*span, "call target does not exist"));
                }
            }
            for argument in arguments {
                validate_expression_references(program, argument)?;
            }
        }
        MirExpression::IndirectCall {
            callee, arguments, ..
        } => {
            validate_expression_references(program, callee)?;
            for argument in arguments {
                validate_expression_references(program, argument)?;
            }
        }
        MirExpression::Integer { .. }
        | MirExpression::Boolean { .. }
        | MirExpression::String { .. }
        | MirExpression::Local { .. }
        | MirExpression::VariantPayload { .. } => {}
    }
    Ok(())
}

fn validate_cfg(
    blocks: &[MirBasicBlock],
    entry: BasicBlockId,
    span: SourceSpan,
) -> Result<(), RuntimeError> {
    if entry.0 >= blocks.len() {
        return Err(RuntimeError::new(
            span,
            "callable entry block does not exist",
        ));
    }
    for block in blocks {
        let valid_targets = match &block.terminator {
            MirTerminator::Goto { target, .. } => target.0 < blocks.len(),
            MirTerminator::Branch {
                then_target,
                else_target,
                ..
            } => then_target.0 < blocks.len() && else_target.0 < blocks.len(),
            MirTerminator::SwitchVariant { targets, .. } => {
                targets.iter().all(|target| target.0 < blocks.len())
            }
            MirTerminator::Return { .. } => true,
        };
        if !valid_targets {
            return Err(RuntimeError::new(
                block.span,
                "callable CFG target does not exist",
            ));
        }
    }
    Ok(())
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
        self.call_callable(
            program,
            CallableValue::Function(function_id),
            arguments,
            span,
        )
    }

    fn call_callable(
        &mut self,
        program: &MirProgram,
        callable: CallableValue,
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
        let result = match callable {
            CallableValue::Function(function) => {
                self.call_function_active(program, function, arguments, span)
            }
            CallableValue::Closure { closure, captures } => {
                self.call_closure_active(program, closure, &captures, arguments, span)
            }
        };
        self.call_depth -= 1;
        result
    }

    fn call_function_active(
        &mut self,
        program: &MirProgram,
        function_id: FunctionId,
        arguments: Vec<Value>,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        let function = program
            .function(function_id)
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
            locals: vec![None; function.local_count],
        };
        for (local, argument) in function.parameters.iter().copied().zip(arguments) {
            frame.store(local, argument, function.span)?;
        }

        self.execute_body(
            program,
            &function.blocks,
            function.entry,
            frame,
            function.span,
        )
    }

    fn call_closure_active(
        &mut self,
        program: &MirProgram,
        closure_id: ClosureId,
        captures: &[Value],
        arguments: Vec<Value>,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        let closure = program
            .closure(closure_id)
            .ok_or_else(|| RuntimeError::new(span, "closure call target does not exist"))?;
        if closure.captures.len() != captures.len() {
            return Err(RuntimeError::new(
                span,
                format!(
                    "closure expected {} capture(s), found {}",
                    closure.captures.len(),
                    captures.len()
                ),
            ));
        }
        if closure.parameters.len() != arguments.len() {
            return Err(RuntimeError::new(
                span,
                format!(
                    "closure expected {} argument(s), found {}",
                    closure.parameters.len(),
                    arguments.len()
                ),
            ));
        }

        let mut frame = Frame {
            locals: vec![None; closure.local_count],
        };
        for (local, value) in closure
            .captures
            .iter()
            .copied()
            .zip(captures.iter().cloned())
        {
            frame.store(local, value, closure.span)?;
        }
        for (local, argument) in closure.parameters.iter().copied().zip(arguments) {
            frame.store(local, argument, closure.span)?;
        }

        self.execute_body(program, &closure.blocks, closure.entry, frame, closure.span)
    }

    fn execute_body(
        &mut self,
        program: &MirProgram,
        blocks: &[MirBasicBlock],
        entry: BasicBlockId,
        mut frame: Frame,
        body_span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        let mut current = entry;

        loop {
            let block = blocks
                .get(current.0)
                .ok_or_else(|| RuntimeError::new(body_span, "basic block does not exist"))?;
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
            MirExpression::Function { function, .. } => {
                Ok(Value::Callable(CallableValue::Function(*function)))
            }
            MirExpression::Closure {
                closure,
                captures,
                span,
            } => {
                let layout = program
                    .closure(*closure)
                    .ok_or_else(|| RuntimeError::new(*span, "closure layout does not exist"))?;
                if layout.captures.len() != captures.len() {
                    return Err(RuntimeError::new(
                        *span,
                        format!(
                            "closure construction expected {} capture(s), found {}",
                            layout.captures.len(),
                            captures.len()
                        ),
                    ));
                }
                let captures = captures
                    .iter()
                    .map(|capture| self.evaluate(program, capture, frame))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Callable(CallableValue::Closure {
                    closure: *closure,
                    captures: captures.into(),
                }))
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
            MirExpression::ArrayIntrinsic {
                operation,
                element_type,
                target,
                argument,
                span,
            } => {
                let target = self.evaluate(program, target, frame)?;
                let argument = self.evaluate(program, argument, frame)?;
                evaluate_array_intrinsic(program, *operation, element_type, target, argument, *span)
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
            MirExpression::IndirectCall {
                callee,
                arguments,
                span,
            } => {
                let callee = self.evaluate(program, callee, frame)?;
                let arguments = arguments
                    .iter()
                    .map(|argument| self.evaluate(program, argument, frame))
                    .collect::<Result<Vec<_>, _>>()?;
                match callee {
                    Value::Callable(callable) => {
                        self.call_callable(program, callable, arguments, *span)
                    }
                    value => Err(RuntimeError::new(
                        *span,
                        format!(
                            "indirect call received `{}` instead of a function",
                            value_type(&value)
                        ),
                    )),
                }
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
            .record(record)
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
                    Value::Array(_)
                        | Value::Callable(_)
                        | Value::Record { .. }
                        | Value::Union { .. }
                        | Value::Unit
                ) {
                    return Err(RuntimeError::new(
                        span,
                        format!("`print` cannot print `{}`", value_type(value)),
                    ));
                }
                self.output.push(value.to_string());
                Ok(Value::Unit)
            }
            Callee::ToString => {
                let [value] = arguments.as_slice() else {
                    return Err(RuntimeError::new(span, "`toString` requires one argument"));
                };
                let Value::Int(value) = value else {
                    return Err(RuntimeError::new(
                        span,
                        format!("`toString` expected `Int`, found `{}`", value_type(value)),
                    ));
                };
                Ok(Value::String(value.to_string().into()))
            }
            Callee::ParseInt => {
                let [value] = arguments.as_slice() else {
                    return Err(RuntimeError::new(span, "`parseInt` requires one argument"));
                };
                let Value::String(value) = value else {
                    return Err(RuntimeError::new(
                        span,
                        format!(
                            "`parseInt` expected `String`, found `{}`",
                            value_type(value)
                        ),
                    ));
                };
                parse_ascii_int(value, span).map(Value::Int)
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
        .union(union)
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
        Value::Int(_) | Value::Bool(_) | Value::String(_) | Value::Callable(_) | Value::Unit => 0,
    }
}

struct Frame {
    locals: Vec<Option<Value>>,
}

impl Frame {
    fn load(&self, local: LocalId, span: SourceSpan) -> Result<Value, RuntimeError> {
        self.locals
            .get(local.0)
            .ok_or_else(|| RuntimeError::new(span, "local slot does not exist"))?
            .clone()
            .ok_or_else(|| RuntimeError::new(span, "local slot is uninitialized"))
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
        *slot = Some(value);
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
    let length = match target {
        Value::Array(values) => values.len(),
        Value::String(value) => value.chars().count(),
        target => {
            return Err(RuntimeError::new(
                span,
                format!("cannot read `length` from `{}`", value_type(&target)),
            ));
        }
    };
    i64::try_from(length)
        .map(Value::Int)
        .map_err(|_| RuntimeError::new(span, "length exceeds `Int` range"))
}

fn evaluate_array_intrinsic(
    program: &MirProgram,
    operation: ArrayIntrinsic,
    element_type: &nexa_hir::Type,
    target: Value,
    argument: Value,
    span: SourceSpan,
) -> Result<Value, RuntimeError> {
    let Value::Array(values) = target else {
        return Err(RuntimeError::new(
            span,
            format!(
                "array intrinsic expected `Array`, found `{}`",
                value_type(&target)
            ),
        ));
    };
    if !values
        .iter()
        .all(|value| value_matches_type(program, value, element_type))
    {
        return Err(RuntimeError::new(
            span,
            "array intrinsic base contains a value incompatible with its element type",
        ));
    }
    let mut result = Vec::new();
    match operation {
        ArrayIntrinsic::Append => {
            if !value_matches_type(program, &argument, element_type) {
                return Err(RuntimeError::new(
                    span,
                    "array append argument is incompatible with its element type",
                ));
            }
            result
                .try_reserve_exact(values.len().saturating_add(1))
                .map_err(|_| RuntimeError::new(span, "array append result is too large"))?;
            result.extend(values.iter().cloned());
            result.push(argument);
        }
        ArrayIntrinsic::Concat => {
            let Value::Array(other) = argument else {
                return Err(RuntimeError::new(
                    span,
                    format!(
                        "array concat expected `Array`, found `{}`",
                        value_type(&argument)
                    ),
                ));
            };
            if !other
                .iter()
                .all(|value| value_matches_type(program, value, element_type))
            {
                return Err(RuntimeError::new(
                    span,
                    "array concat argument contains a value incompatible with its element type",
                ));
            }
            let length = values
                .len()
                .checked_add(other.len())
                .ok_or_else(|| RuntimeError::new(span, "array concat result is too large"))?;
            result
                .try_reserve_exact(length)
                .map_err(|_| RuntimeError::new(span, "array concat result is too large"))?;
            result.extend(values.iter().cloned());
            result.extend(other.iter().cloned());
        }
    }
    Ok(Value::Array(result.into()))
}

fn value_matches_type(program: &MirProgram, value: &Value, expected: &nexa_hir::Type) -> bool {
    match (value, expected) {
        (Value::Int(_), nexa_hir::Type::Int)
        | (Value::Bool(_), nexa_hir::Type::Bool)
        | (Value::String(_), nexa_hir::Type::String)
        | (Value::Unit, nexa_hir::Type::Unit)
        | (_, nexa_hir::Type::Parameter(_)) => true,
        (Value::Array(values), nexa_hir::Type::Array(element)) => values
            .iter()
            .all(|value| value_matches_type(program, value, element)),
        (
            Value::Callable(CallableValue::Function(function)),
            nexa_hir::Type::Function {
                parameters,
                return_type,
            },
        ) => program.function(*function).is_some_and(|function| {
            function.parameter_types.len() == parameters.len()
                && function
                    .parameter_types
                    .iter()
                    .zip(parameters)
                    .all(|(actual, expected)| runtime_type_matches(actual, expected))
                && runtime_type_matches(&function.return_type, return_type)
        }),
        (
            Value::Callable(CallableValue::Closure { closure, captures }),
            nexa_hir::Type::Function {
                parameters,
                return_type,
            },
        ) => program.closure(*closure).is_some_and(|closure| {
            captures.len() == closure.captures.len()
                && closure.parameter_types.len() == parameters.len()
                && closure
                    .parameter_types
                    .iter()
                    .zip(parameters)
                    .all(|(actual, expected)| runtime_type_matches(actual, expected))
                && runtime_type_matches(&closure.return_type, return_type)
        }),
        (Value::Record { record, .. }, nexa_hir::Type::Record { definition, .. }) => {
            record == definition
        }
        (Value::Union { union, .. }, nexa_hir::Type::Union { definition, .. }) => {
            union == definition
        }
        _ => false,
    }
}

fn runtime_type_matches(actual: &nexa_hir::Type, expected: &nexa_hir::Type) -> bool {
    match (actual, expected) {
        (nexa_hir::Type::Parameter(_), _) | (_, nexa_hir::Type::Parameter(_)) => true,
        (nexa_hir::Type::Int, nexa_hir::Type::Int)
        | (nexa_hir::Type::Bool, nexa_hir::Type::Bool)
        | (nexa_hir::Type::String, nexa_hir::Type::String)
        | (nexa_hir::Type::Unit, nexa_hir::Type::Unit) => true,
        (nexa_hir::Type::Array(actual), nexa_hir::Type::Array(expected)) => {
            runtime_type_matches(actual, expected)
        }
        (
            nexa_hir::Type::Function {
                parameters: actual_parameters,
                return_type: actual_return,
            },
            nexa_hir::Type::Function {
                parameters: expected_parameters,
                return_type: expected_return,
            },
        ) => {
            actual_parameters.len() == expected_parameters.len()
                && actual_parameters
                    .iter()
                    .zip(expected_parameters)
                    .all(|(actual, expected)| runtime_type_matches(actual, expected))
                && runtime_type_matches(actual_return, expected_return)
        }
        (
            nexa_hir::Type::Record {
                definition: actual,
                arguments: actual_arguments,
            },
            nexa_hir::Type::Record {
                definition: expected,
                arguments: expected_arguments,
            },
        ) => {
            actual == expected
                && actual_arguments.len() == expected_arguments.len()
                && actual_arguments
                    .iter()
                    .zip(expected_arguments)
                    .all(|(actual, expected)| runtime_type_matches(actual, expected))
        }
        (
            nexa_hir::Type::Union {
                definition: actual,
                arguments: actual_arguments,
            },
            nexa_hir::Type::Union {
                definition: expected,
                arguments: expected_arguments,
            },
        ) => {
            actual == expected
                && actual_arguments.len() == expected_arguments.len()
                && actual_arguments
                    .iter()
                    .zip(expected_arguments)
                    .all(|(actual, expected)| runtime_type_matches(actual, expected))
        }
        _ => false,
    }
}

fn parse_ascii_int(value: &str, span: SourceSpan) -> Result<i64, RuntimeError> {
    let bytes = value.as_bytes();
    let digits = if bytes.first() == Some(&b'-') {
        &bytes[1..]
    } else {
        bytes
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(RuntimeError::new(
            span,
            "parseInt expected a complete ASCII decimal integer",
        ));
    }

    value
        .parse::<i64>()
        .map_err(|_| RuntimeError::new(span, "parseInt result is outside the Int range"))
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
        .record(record)
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
        Value::Callable(_) => "Function",
        Value::Record { .. } => "Record",
        Value::Union { .. } => "Union",
        Value::Unit => "Unit",
    }
}

#[cfg(test)]
mod tests {
    use nexa_hir::{
        lower as lower_hir, type_check, ClosureId, FunctionId, ModuleId, PayloadId, Type,
        TypeParameterId, TypeParameterOwner, UnionId, VariantId,
    };
    use nexa_parser::parse_source;
    use nexa_span::{FileId, TextRange};

    use super::{
        evaluate_field, evaluate_variant_payload, runtime_type_matches, select_variant_target,
        validate_field_layout, validate_payload_layout, variant_layout, FieldId, RecordId,
        SourceSpan, Value,
    };
    use crate::{
        ArrayIntrinsic, BasicBlockId, LocalId, MirBasicBlock, MirClosure, MirExpression,
        MirFunction, MirPayload, MirProgram, MirRecord, MirRecordField, MirStatement,
        MirTerminator, MirUnion, MirVariant,
    };

    #[test]
    fn interpreter_invokes_the_resolved_entry_without_searching_its_name(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let parse = parse_source(FileId::new(0), "function main(): Unit { print(42); }");
        let hir = lower_hir(FileId::new(0), &parse.syntax())?;
        let analysis = type_check(&hir);
        let typed = analysis
            .typed()
            .ok_or_else(|| std::io::Error::other("expected valid typed HIR"))?;
        let mut program = crate::lower(typed)?;
        let entry = program
            .entry
            .ok_or_else(|| std::io::Error::other("expected resolved entry"))?;
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.id == entry)
            .ok_or_else(|| std::io::Error::other("expected entry function layout"))?;
        function.name = "renamed".to_owned();

        let execution = super::run(&program)?;

        assert_eq!(execution.output(), ["42"]);
        Ok(())
    }

    #[test]
    fn interpreter_rejects_an_entry_owned_by_a_dependency_module(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let parse = parse_source(FileId::new(0), "function main(): Unit {}");
        let hir = lower_hir(FileId::new(0), &parse.syntax())?;
        let analysis = type_check(&hir);
        let typed = analysis
            .typed()
            .ok_or_else(|| std::io::Error::other("expected valid typed HIR"))?;
        let mut program = crate::lower(typed)?;
        program.modules.push(ModuleId::new(1));
        program.entry = Some(FunctionId::in_module(ModuleId::new(1), 0));

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected invalid entry failure"))?;

        assert_eq!(
            failure.error().message(),
            "entry function is not owned by the entry module"
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_duplicate_module_identities() -> Result<(), Box<dyn std::error::Error>> {
        let mut program = lower_program("function main(): Unit {}")?;
        program.modules.push(ModuleId::ENTRY);

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected duplicate module failure"))?;

        assert_eq!(
            failure.error().message(),
            "program contains a duplicate module identity"
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_a_missing_entry_module() -> Result<(), Box<dyn std::error::Error>> {
        let mut program = lower_program("function main(): Unit {}")?;
        program.entry_module = ModuleId::new(9);

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected missing entry module failure"))?;

        assert_eq!(
            failure.error().message(),
            "program entry module does not exist"
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_an_existing_dependency_selected_as_the_entry_module(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut program = lower_program("function main(): Unit {}")?;
        let dependency = ModuleId::new(1);
        let mut dependency_entry = program
            .functions
            .first()
            .cloned()
            .ok_or_else(|| std::io::Error::other("expected main function"))?;
        dependency_entry.id = FunctionId::in_module(dependency, 0);
        program.modules.push(dependency);
        program.functions.push(dependency_entry);
        program.entry_module = dependency;
        program.entry = Some(FunctionId::in_module(dependency, 0));

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected forged entry module failure"))?;

        assert_eq!(
            failure.error().message(),
            "program entry module is not module 0"
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_functions_owned_by_unknown_modules(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut program = lower_program("function main(): Unit {}")?;
        let function = program
            .functions
            .first_mut()
            .ok_or_else(|| std::io::Error::other("expected main function"))?;
        function.id = FunctionId::in_module(ModuleId::new(9), 0);

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected missing function owner failure"))?;

        assert_eq!(
            failure.error().message(),
            "function owner module does not exist"
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_duplicate_function_identities() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut program = lower_program("function helper(): Unit {} function main(): Unit {}")?;
        let duplicate = program
            .functions
            .first()
            .cloned()
            .ok_or_else(|| std::io::Error::other("expected helper function"))?;
        program.functions.push(duplicate);

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected duplicate function failure"))?;

        assert_eq!(
            failure.error().message(),
            "program contains a duplicate function identity"
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_a_foreign_function_value_identity(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(140, 150));
        let program = expression_program(
            MirExpression::Function {
                function: FunctionId::in_module(ModuleId::new(9), 0),
                span,
            },
            span,
        );

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a foreign function value failure"))?;

        assert_eq!(
            (failure.error().message(), failure.error().span()),
            ("function value target does not exist", span)
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_a_foreign_closure_value_identity(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(150, 160));
        let program = expression_program(
            MirExpression::Closure {
                closure: ClosureId::new(FunctionId::new(0), 9),
                captures: Vec::new(),
                span,
            },
            span,
        );

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a foreign closure value failure"))?;

        assert_eq!(
            (failure.error().message(), failure.error().span()),
            ("closure layout does not exist", span)
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_a_closure_construction_with_the_wrong_capture_count(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(160, 170));
        let closure = ClosureId::new(FunctionId::new(0), 0);
        let mut program = expression_program(
            MirExpression::Closure {
                closure,
                captures: Vec::new(),
                span,
            },
            span,
        );
        program.closures.push(unit_closure(
            closure,
            vec![LocalId(0)],
            Vec::new(),
            Vec::new(),
            1,
            span,
        ));

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a closure capture-count failure"))?;

        assert_eq!(
            (failure.error().message(), failure.error().span()),
            ("closure construction expected 1 capture(s), found 0", span)
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_a_closure_capture_slot_outside_its_frame(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(170, 180));
        let closure = ClosureId::new(FunctionId::new(0), 0);
        let mut program = expression_program(MirExpression::Integer { value: 0, span }, span);
        program.closures.push(unit_closure(
            closure,
            vec![LocalId(1)],
            Vec::new(),
            Vec::new(),
            1,
            span,
        ));

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected an invalid capture-slot failure"))?;

        assert_eq!(
            (failure.error().message(), failure.error().span()),
            ("closure capture slot is outside its local frame", span)
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_inconsistent_function_signature_metadata(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(180, 190));
        let mut program = expression_program(MirExpression::Integer { value: 0, span }, span);
        program.functions[0].parameters.push(LocalId(0));
        program.functions[0].local_count = 1;

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected invalid function metadata"))?;

        assert_eq!(
            (failure.error().message(), failure.error().span()),
            (
                "function parameter signature metadata is inconsistent",
                span
            )
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_inconsistent_closure_signature_metadata(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(190, 200));
        let closure = ClosureId::new(FunctionId::new(0), 0);
        let mut program = expression_program(MirExpression::Integer { value: 0, span }, span);
        program.closures.push(unit_closure(
            closure,
            Vec::new(),
            vec![LocalId(0)],
            Vec::new(),
            1,
            span,
        ));

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected invalid closure metadata"))?;

        assert_eq!(
            (failure.error().message(), failure.error().span()),
            ("closure parameter signature metadata is inconsistent", span)
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_a_non_callable_indirect_target() -> Result<(), Box<dyn std::error::Error>>
    {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(200, 210));
        let program = expression_program(
            MirExpression::IndirectCall {
                callee: Box::new(MirExpression::Integer { value: 42, span }),
                arguments: Vec::new(),
                span,
            },
            span,
        );

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected a non-callable indirect failure"))?;

        assert_eq!(
            (failure.error().message(), failure.error().span()),
            ("indirect call received `Int` instead of a function", span)
        );
        Ok(())
    }

    #[test]
    fn interpreter_rejects_an_array_intrinsic_with_a_non_array_target(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span = SourceSpan::new(FileId::new(9), TextRange::new(210, 220));
        let program = expression_program(
            MirExpression::ArrayIntrinsic {
                operation: ArrayIntrinsic::Append,
                element_type: Type::Int,
                target: Box::new(MirExpression::Integer { value: 1, span }),
                argument: Box::new(MirExpression::Integer { value: 2, span }),
                span,
            },
            span,
        );

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected an invalid intrinsic failure"))?;

        assert_eq!(
            (failure.error().message(), failure.error().span()),
            ("array intrinsic expected `Array`, found `Int`", span)
        );
        Ok(())
    }

    #[test]
    fn runtime_type_matching_erases_parameters_nested_in_nominal_function_types() {
        let parameter = Type::Parameter(TypeParameterId::new(
            TypeParameterOwner::Function(FunctionId::new(0)),
            0,
        ));
        let record = RecordId::new(0);
        let union = UnionId::new(0);
        let actual = Type::Function {
            parameters: vec![Type::Array(Box::new(Type::Record {
                definition: record,
                arguments: vec![Type::Int].into_boxed_slice(),
            }))]
            .into_boxed_slice(),
            return_type: Box::new(Type::Union {
                definition: union,
                arguments: vec![Type::String].into_boxed_slice(),
            }),
        };
        let expected = Type::Function {
            parameters: vec![Type::Array(Box::new(Type::Record {
                definition: record,
                arguments: vec![parameter.clone()].into_boxed_slice(),
            }))]
            .into_boxed_slice(),
            return_type: Box::new(Type::Union {
                definition: union,
                arguments: vec![parameter].into_boxed_slice(),
            }),
        };

        assert!(runtime_type_matches(&actual, &expected));
    }

    #[test]
    fn runtime_type_matching_rejects_different_concrete_nominal_arguments() {
        let record = RecordId::new(0);
        let actual = Type::Record {
            definition: record,
            arguments: vec![Type::Int].into_boxed_slice(),
        };
        let expected = Type::Record {
            definition: record,
            arguments: vec![Type::String].into_boxed_slice(),
        };

        assert!(!runtime_type_matches(&actual, &expected));
    }

    #[test]
    fn interpreter_rejects_a_missing_entry_function() -> Result<(), Box<dyn std::error::Error>> {
        let mut program = lower_program("function main(): Unit {}")?;
        program.entry = Some(FunctionId::in_module(ModuleId::ENTRY, 9));

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected missing entry function failure"))?;

        assert_eq!(failure.error().message(), "entry function does not exist");
        Ok(())
    }

    #[test]
    fn interpreter_rejects_record_layouts_owned_by_unknown_modules(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut program = lower_program("type Box = { value: Int; }; function main(): Unit {}")?;
        let record = program
            .records
            .first_mut()
            .ok_or_else(|| std::io::Error::other("expected record layout"))?;
        record.id = RecordId::in_module(ModuleId::new(9), 0);

        let failure = super::run(&program)
            .err()
            .ok_or_else(|| std::io::Error::other("expected missing record owner failure"))?;

        assert_eq!(
            failure.error().message(),
            "record owner module does not exist"
        );
        Ok(())
    }

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
            modules: vec![nexa_hir::ModuleId::ENTRY],
            entry_module: nexa_hir::ModuleId::ENTRY,
            entry: None,
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
            closures: Vec::new(),
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
            modules: vec![nexa_hir::ModuleId::ENTRY],
            entry_module: nexa_hir::ModuleId::ENTRY,
            entry: None,
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
            closures: Vec::new(),
            span,
        }
    }

    fn expression_program(expression: MirExpression, span: SourceSpan) -> MirProgram {
        let main = FunctionId::new(0);
        MirProgram {
            modules: vec![ModuleId::ENTRY],
            entry_module: ModuleId::ENTRY,
            entry: Some(main),
            records: Vec::new(),
            unions: Vec::new(),
            functions: vec![MirFunction {
                id: main,
                name: "main".to_owned(),
                parameters: Vec::new(),
                parameter_types: Vec::new(),
                local_count: 0,
                return_type: Type::Unit,
                entry: BasicBlockId(0),
                blocks: vec![MirBasicBlock {
                    statements: vec![MirStatement::Expression { expression, span }],
                    terminator: MirTerminator::Return { value: None, span },
                    span,
                }],
                span,
            }],
            closures: Vec::new(),
            span,
        }
    }

    fn unit_closure(
        id: ClosureId,
        captures: Vec<LocalId>,
        parameters: Vec<LocalId>,
        parameter_types: Vec<Type>,
        local_count: usize,
        span: SourceSpan,
    ) -> MirClosure {
        MirClosure {
            id,
            captures,
            parameters,
            parameter_types,
            local_count,
            return_type: Type::Unit,
            entry: BasicBlockId(0),
            blocks: vec![MirBasicBlock {
                statements: Vec::new(),
                terminator: MirTerminator::Return { value: None, span },
                span,
            }],
            span,
        }
    }

    fn lower_program(source: &str) -> Result<MirProgram, Box<dyn std::error::Error>> {
        let parse = parse_source(FileId::new(0), source);
        if !parse.is_ok() {
            return Err(std::io::Error::other(format!(
                "unexpected parser diagnostics: {:?}",
                parse.diagnostics()
            ))
            .into());
        }
        let hir = lower_hir(FileId::new(0), &parse.syntax())?;
        let analysis = type_check(&hir);
        let typed = analysis
            .typed()
            .ok_or_else(|| std::io::Error::other("expected valid typed HIR"))?;
        Ok(crate::lower(typed)?)
    }
}
