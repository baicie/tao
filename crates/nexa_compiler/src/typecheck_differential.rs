//! Differential contract for the first Futao type-checker expression kernel.

use std::fmt::Display;

use nexa_mir::{run_with_args_and_step_limit, MirProgram, RuntimeFailure};
use nexa_source::SourceMap;
use serde::Serialize;
use thiserror::Error;

use crate::{compile, CompileError, CompilerInput, CompilerOptions, CompilerSource};

/// Canonical schema used by type-checker differential snapshots.
pub const TYPECHECK_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

const FUTAO_TYPECHECK_STEP_LIMIT: usize = 2_000_000;
const FUTAO_TYPECHECK_SOURCE: &str =
    include_str!("../../../bootstrap/compiler/typecheck/typecheck.ft");
const FUTAO_TYPECHECK_BRIDGE_SOURCE: &str =
    include_str!("../../../bootstrap/compiler/typecheck/typecheck_bridge.ft");
const FUTAO_TYPECHECK_PROFILE_ENTRY: &str =
    include_str!("../../../bootstrap/compiler/typecheck/typecheck_profile.ft");
const FUTAO_TYPECHECK_DRIVER_ENTRY: &str =
    include_str!("../../../bootstrap/compiler/typecheck/typecheck_driver.ft");

/// A source span attached to one semantic expression node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TypecheckSpan {
    /// Stable source ordinal.
    pub source: u32,
    /// Inclusive UTF-8 byte start.
    pub start: u32,
    /// Exclusive UTF-8 byte end.
    pub end: u32,
}

impl TypecheckSpan {
    /// Creates a span in one source file.
    #[must_use]
    pub const fn new(source: u32, start: u32, end: u32) -> Self {
        Self { source, start, end }
    }
}

/// One primitive type accepted by the expression kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypecheckType {
    /// A signed integer.
    Int,
    /// A boolean.
    Bool,
    /// A UTF-8 string.
    String,
    /// The unit value.
    Unit,
}

impl TypecheckType {
    fn tag(self) -> i32 {
        match self {
            Self::Int => 0,
            Self::Bool => 1,
            Self::String => 2,
            Self::Unit => 3,
        }
    }
}

/// Expression node kinds in canonical pre-order-independent node tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypecheckNodeKind {
    /// An integer literal.
    Int,
    /// A boolean literal.
    Bool,
    /// A string literal.
    String,
    /// A unit literal.
    Unit,
    /// Integer negation.
    Neg,
    /// Boolean negation.
    Not,
    /// Integer addition.
    Add,
    /// Scalar equality.
    Equal,
    /// Boolean conjunction.
    And,
    /// Boolean disjunction.
    Or,
    /// A conditional expression.
    If,
    /// A return value checked against an expected type.
    Return,
}

impl TypecheckNodeKind {
    fn tag(self) -> i32 {
        match self {
            Self::Int => 0,
            Self::Bool => 1,
            Self::String => 2,
            Self::Unit => 3,
            Self::Neg => 4,
            Self::Not => 5,
            Self::Add => 6,
            Self::Equal => 7,
            Self::And => 8,
            Self::Or => 9,
            Self::If => 10,
            Self::Return => 11,
        }
    }
}

/// One source-spanned expression in a [`TypecheckInput`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypecheckNode {
    kind: TypecheckNodeKind,
    left: Option<usize>,
    right: Option<usize>,
    extra: Option<usize>,
    expected: Option<TypecheckType>,
    span: TypecheckSpan,
}

impl TypecheckNode {
    /// Creates a literal node.
    #[must_use]
    pub const fn literal(kind: TypecheckNodeKind, start: u32, end: u32) -> Self {
        Self {
            kind,
            left: None,
            right: None,
            extra: None,
            expected: None,
            span: TypecheckSpan::new(0, start, end),
        }
    }

    /// Creates a unary node.
    #[must_use]
    pub const fn unary(kind: TypecheckNodeKind, operand: usize, start: u32, end: u32) -> Self {
        Self {
            kind,
            left: Some(operand),
            right: None,
            extra: None,
            expected: None,
            span: TypecheckSpan::new(0, start, end),
        }
    }

    /// Creates a binary node.
    #[must_use]
    pub const fn binary(
        kind: TypecheckNodeKind,
        left: usize,
        right: usize,
        start: u32,
        end: u32,
    ) -> Self {
        Self {
            kind,
            left: Some(left),
            right: Some(right),
            extra: None,
            expected: None,
            span: TypecheckSpan::new(0, start, end),
        }
    }

    /// Creates a conditional node.
    #[must_use]
    pub const fn conditional(
        condition: usize,
        then_branch: usize,
        else_branch: usize,
        start: u32,
        end: u32,
    ) -> Self {
        Self {
            kind: TypecheckNodeKind::If,
            left: Some(condition),
            right: Some(then_branch),
            extra: Some(else_branch),
            expected: None,
            span: TypecheckSpan::new(0, start, end),
        }
    }

    /// Creates a return-check node.
    #[must_use]
    pub const fn return_check(value: usize, expected: TypecheckType, start: u32, end: u32) -> Self {
        Self {
            kind: TypecheckNodeKind::Return,
            left: Some(value),
            right: None,
            extra: None,
            expected: Some(expected),
            span: TypecheckSpan::new(0, start, end),
        }
    }
}

/// A validated, source-spanned expression table for type checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypecheckInput {
    source_len: u32,
    nodes: Vec<TypecheckNode>,
}

impl TypecheckInput {
    /// Creates an input; structural validation happens at the adapter boundary.
    #[must_use]
    pub const fn new(source_len: u32, nodes: Vec<TypecheckNode>) -> Self {
        Self { source_len, nodes }
    }

    /// Returns the source byte length used to validate spans.
    #[must_use]
    pub const fn source_len(&self) -> u32 {
        self.source_len
    }

    /// Returns the canonical node table.
    #[must_use]
    pub fn nodes(&self) -> &[TypecheckNode] {
        &self.nodes
    }
}

/// One canonical type-checker diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypecheckDiagnosticSnapshot {
    code: String,
    span: TypecheckSpan,
}

impl TypecheckDiagnosticSnapshot {
    /// Returns the stable diagnostic code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the primary source span.
    #[must_use]
    pub const fn span(&self) -> TypecheckSpan {
        self.span
    }
}

/// Canonical output of one type-checker implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypecheckSnapshot {
    schema_version: u32,
    types: Vec<String>,
    diagnostics: Vec<TypecheckDiagnosticSnapshot>,
}

impl TypecheckSnapshot {
    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns one canonical inferred type per input node.
    #[must_use]
    pub fn types(&self) -> &[String] {
        &self.types
    }

    /// Returns source-ordered type diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[TypecheckDiagnosticSnapshot] {
        &self.diagnostics
    }

    /// Serializes the snapshot as compact canonical JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Adapter failures are fail-closed at the semantic boundary.
#[derive(Debug, Error)]
pub enum TypecheckAdapterError {
    /// The input violates the canonical expression-table contract.
    #[error("invalid type-checker input: {0}")]
    InvalidInput(String),
    /// The Futao source did not compile to executable MIR.
    #[error("Futao type-checker source failed: {0}")]
    FutaoSource(String),
    /// The Futao driver emitted malformed output.
    #[error("type-checker protocol error: {0}")]
    Protocol(String),
    /// The candidate snapshot violated schema bounds.
    #[error("invalid type-checker snapshot: {0}")]
    InvalidSnapshot(String),
    /// The Futao interpreter exceeded its bounded tool budget or failed at runtime.
    #[error("Futao type-checker runtime failed at {location}: {failure}")]
    Runtime {
        /// The structured interpreter failure.
        failure: RuntimeFailure,
        /// The source-aware location rendered by the adapter.
        location: String,
    },
    /// The input cannot be represented by the private protocol.
    #[error("type-checker input is too large")]
    InputTooLarge,
    /// Rust compilation of the checked-in Futao source failed.
    #[error(transparent)]
    Compile(#[from] CompileError),
}

/// A type-checker implementation participating in differential comparison.
pub trait TypecheckAdapter {
    /// Returns the implementation identity.
    fn implementation(&self) -> TypecheckImplementation;

    /// Checks one validated expression table.
    fn check(&self, input: &TypecheckInput) -> Result<TypecheckSnapshot, TypecheckAdapterError>;
}

/// Stable implementation identities for reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypecheckImplementation {
    /// Rust Stage 0 reference checker.
    RustReference,
    /// Futao Bootstrap Profile checker.
    Futao,
}

/// Observable categories compared by the differential harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypecheckObservable {
    /// Inferred type tags for every node.
    Types,
    /// Diagnostic code and source spans.
    Diagnostics,
}

/// A report produced after both adapters check one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypecheckDifferentialReport {
    outcome: TypecheckDifferentialOutcome,
    differences: Vec<TypecheckObservable>,
    reference_snapshot: TypecheckSnapshot,
    candidate_snapshot: TypecheckSnapshot,
}

impl TypecheckDifferentialReport {
    /// Returns match or difference status.
    #[must_use]
    pub const fn outcome(&self) -> TypecheckDifferentialOutcome {
        self.outcome
    }

    /// Returns all unsuppressed differing observables.
    #[must_use]
    pub fn differences(&self) -> &[TypecheckObservable] {
        &self.differences
    }

    /// Returns the Rust reference snapshot.
    #[must_use]
    pub const fn reference_snapshot(&self) -> &TypecheckSnapshot {
        &self.reference_snapshot
    }

    /// Returns the Futao candidate snapshot.
    #[must_use]
    pub const fn candidate_snapshot(&self) -> &TypecheckSnapshot {
        &self.candidate_snapshot
    }
}

/// Differential outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypecheckDifferentialOutcome {
    /// Both snapshots are identical.
    Match,
    /// At least one observable differs.
    Difference,
}

/// Compares a Rust reference adapter with a Futao candidate adapter.
#[derive(Debug, Clone, Copy)]
pub struct TypecheckDifferentialHarness<'a, R, C> {
    reference: &'a R,
    candidate: &'a C,
}

impl<'a, R, C> TypecheckDifferentialHarness<'a, R, C>
where
    R: TypecheckAdapter,
    C: TypecheckAdapter,
{
    /// Creates a differential harness.
    #[must_use]
    pub const fn new(reference: &'a R, candidate: &'a C) -> Self {
        Self {
            reference,
            candidate,
        }
    }

    /// Checks one input with both implementations.
    pub fn run_case(
        &self,
        _case_id: &str,
        input: &TypecheckInput,
    ) -> Result<TypecheckDifferentialReport, TypecheckAdapterError> {
        let reference_snapshot = self.reference.check(input)?;
        let candidate_snapshot = self.candidate.check(input)?;
        let mut differences = Vec::new();
        if reference_snapshot.types != candidate_snapshot.types {
            differences.push(TypecheckObservable::Types);
        }
        if reference_snapshot.diagnostics != candidate_snapshot.diagnostics {
            differences.push(TypecheckObservable::Diagnostics);
        }
        let outcome = if differences.is_empty() {
            TypecheckDifferentialOutcome::Match
        } else {
            TypecheckDifferentialOutcome::Difference
        };
        Ok(TypecheckDifferentialReport {
            outcome,
            differences,
            reference_snapshot,
            candidate_snapshot,
        })
    }
}

/// The Rust reference implementation of the expression kernel.
#[derive(Debug, Clone, Copy, Default)]
pub struct RustTypeCheckerAdapter;

impl TypecheckAdapter for RustTypeCheckerAdapter {
    fn implementation(&self) -> TypecheckImplementation {
        TypecheckImplementation::RustReference
    }

    fn check(&self, input: &TypecheckInput) -> Result<TypecheckSnapshot, TypecheckAdapterError> {
        validate_input(input)?;
        let mut types = Vec::with_capacity(input.nodes.len());
        let mut diagnostics = Vec::new();
        for node in &input.nodes {
            let ty = infer_node(node, &types, &mut diagnostics);
            types.push(ty);
        }
        Ok(snapshot(types, diagnostics))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeTag {
    Int,
    Bool,
    String,
    Unit,
    Unknown,
}

impl TypeTag {
    fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Bool => "bool",
            Self::String => "string",
            Self::Unit => "unit",
            Self::Unknown => "unknown",
        }
    }
}

fn infer_node(
    node: &TypecheckNode,
    types: &[TypeTag],
    diagnostics: &mut Vec<TypecheckDiagnosticSnapshot>,
) -> TypeTag {
    let left = node.left.map_or(TypeTag::Unknown, |index| types[index]);
    let right = node.right.map_or(TypeTag::Unknown, |index| types[index]);
    let extra = node.extra.map_or(TypeTag::Unknown, |index| types[index]);
    match node.kind {
        TypecheckNodeKind::Int => TypeTag::Int,
        TypecheckNodeKind::Bool => TypeTag::Bool,
        TypecheckNodeKind::String => TypeTag::String,
        TypecheckNodeKind::Unit => TypeTag::Unit,
        TypecheckNodeKind::Neg => expect_type(left, TypeTag::Int, &node.span, diagnostics),
        TypecheckNodeKind::Not => expect_type(left, TypeTag::Bool, &node.span, diagnostics),
        TypecheckNodeKind::Add => binary_type(left, right, TypeTag::Int, &node.span, diagnostics),
        TypecheckNodeKind::Equal => {
            if left == TypeTag::Unknown || right == TypeTag::Unknown {
                TypeTag::Unknown
            } else if left == right
                && matches!(left, TypeTag::Int | TypeTag::Bool | TypeTag::String)
            {
                TypeTag::Bool
            } else {
                diagnostics.push(diagnostic("E3001", node.span));
                TypeTag::Unknown
            }
        }
        TypecheckNodeKind::And | TypecheckNodeKind::Or => {
            binary_type(left, right, TypeTag::Bool, &node.span, diagnostics)
        }
        TypecheckNodeKind::If => {
            if left != TypeTag::Unknown && left != TypeTag::Bool {
                diagnostics.push(diagnostic("E3002", node.span));
            }
            if right == TypeTag::Unknown || extra == TypeTag::Unknown {
                TypeTag::Unknown
            } else if right == extra {
                right
            } else {
                diagnostics.push(diagnostic("E3001", node.span));
                TypeTag::Unknown
            }
        }
        TypecheckNodeKind::Return => {
            let expected = node.expected.map_or(TypeTag::Unknown, type_tag);
            if left != TypeTag::Unknown && left != expected {
                diagnostics.push(diagnostic("E3003", node.span));
            }
            TypeTag::Unit
        }
    }
}

fn expect_type(
    actual: TypeTag,
    expected: TypeTag,
    span: &TypecheckSpan,
    diagnostics: &mut Vec<TypecheckDiagnosticSnapshot>,
) -> TypeTag {
    if actual == TypeTag::Unknown {
        return TypeTag::Unknown;
    }
    if actual == expected {
        expected
    } else {
        diagnostics.push(diagnostic("E3001", *span));
        TypeTag::Unknown
    }
}

fn binary_type(
    left: TypeTag,
    right: TypeTag,
    expected: TypeTag,
    span: &TypecheckSpan,
    diagnostics: &mut Vec<TypecheckDiagnosticSnapshot>,
) -> TypeTag {
    if left == TypeTag::Unknown || right == TypeTag::Unknown {
        return TypeTag::Unknown;
    }
    if left == expected && right == expected {
        expected
    } else {
        diagnostics.push(diagnostic("E3001", *span));
        TypeTag::Unknown
    }
}

fn type_tag(expected: TypecheckType) -> TypeTag {
    match expected {
        TypecheckType::Int => TypeTag::Int,
        TypecheckType::Bool => TypeTag::Bool,
        TypecheckType::String => TypeTag::String,
        TypecheckType::Unit => TypeTag::Unit,
    }
}

fn diagnostic(code: &str, span: TypecheckSpan) -> TypecheckDiagnosticSnapshot {
    TypecheckDiagnosticSnapshot {
        code: code.to_owned(),
        span,
    }
}

fn snapshot(
    types: Vec<TypeTag>,
    diagnostics: Vec<TypecheckDiagnosticSnapshot>,
) -> TypecheckSnapshot {
    TypecheckSnapshot {
        schema_version: TYPECHECK_SNAPSHOT_SCHEMA_VERSION,
        types: types.into_iter().map(|ty| ty.name().to_owned()).collect(),
        diagnostics,
    }
}

fn validate_input(input: &TypecheckInput) -> Result<(), TypecheckAdapterError> {
    if input.nodes.len() > 1024 {
        return Err(TypecheckAdapterError::InputTooLarge);
    }
    for (index, node) in input.nodes.iter().enumerate() {
        if node.span.source != 0
            || node.span.start >= node.span.end
            || node.span.end > input.source_len
        {
            return Err(TypecheckAdapterError::InvalidInput(format!(
                "node {index} has an invalid source span"
            )));
        }
        for (field, child) in [
            ("left", node.left),
            ("right", node.right),
            ("extra", node.extra),
        ] {
            if let Some(child) = child {
                if child >= index {
                    return Err(TypecheckAdapterError::InvalidInput(format!(
                        "node {index} {field} child {child} is not a prior node"
                    )));
                }
            }
        }
        if matches!(node.kind, TypecheckNodeKind::Return) && node.expected.is_none() {
            return Err(TypecheckAdapterError::InvalidInput(format!(
                "node {index} return has no expected type"
            )));
        }
    }
    Ok(())
}

/// Adapter that executes the real Futao-written expression kernel through MIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutaoTypeCheckerAdapter {
    program: MirProgram,
    sources: SourceMap,
}

impl FutaoTypeCheckerAdapter {
    /// Compiles the Bootstrap Profile and application driver.
    pub fn new() -> Result<Self, TypecheckAdapterError> {
        let profile = compile(&CompilerInput::with_options(
            "typecheck_profile.ft",
            futao_sources("typecheck_profile.ft", FUTAO_TYPECHECK_PROFILE_ENTRY),
            CompilerOptions::bootstrap_v1(),
        ))?;
        ensure_compiled("Bootstrap Profile entry", &profile)?;

        let driver = compile(&CompilerInput::new(
            "typecheck_driver.ft",
            futao_sources("typecheck_driver.ft", FUTAO_TYPECHECK_DRIVER_ENTRY),
        ))?;
        ensure_compiled("application driver", &driver)?;
        let program = driver.mir().cloned().ok_or_else(|| {
            TypecheckAdapterError::FutaoSource("application driver produced no MIR".to_owned())
        })?;
        Ok(Self {
            program,
            sources: driver.sources().clone(),
        })
    }
}

impl TypecheckAdapter for FutaoTypeCheckerAdapter {
    fn implementation(&self) -> TypecheckImplementation {
        TypecheckImplementation::Futao
    }

    fn check(&self, input: &TypecheckInput) -> Result<TypecheckSnapshot, TypecheckAdapterError> {
        validate_input(input)?;
        let arguments = encoded_arguments(input)?;
        let execution =
            run_with_args_and_step_limit(&self.program, &arguments, FUTAO_TYPECHECK_STEP_LIMIT)
                .map_err(|failure| TypecheckAdapterError::Runtime {
                    location: runtime_location(&self.sources, failure.error().span()),
                    failure,
                })?;
        parse_output(execution.output(), input.source_len)
    }
}

fn futao_sources(entry: &str, source: &str) -> Vec<CompilerSource> {
    vec![
        CompilerSource::new(entry, source),
        CompilerSource::new("typecheck.ft", FUTAO_TYPECHECK_SOURCE),
        CompilerSource::new("typecheck_bridge.ft", FUTAO_TYPECHECK_BRIDGE_SOURCE),
    ]
}

fn ensure_compiled(
    role: &str,
    output: &crate::CompilerOutput,
) -> Result<(), TypecheckAdapterError> {
    if output.is_ok() && output.diagnostics().is_empty() {
        return Ok(());
    }
    let summary = output
        .diagnostics()
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
        .collect::<Vec<_>>()
        .join("; ");
    Err(TypecheckAdapterError::FutaoSource(format!(
        "{role} failed: {summary}"
    )))
}

fn encoded_arguments(input: &TypecheckInput) -> Result<Vec<String>, TypecheckAdapterError> {
    let count =
        i32::try_from(input.nodes.len()).map_err(|_| TypecheckAdapterError::InputTooLarge)?;
    let mut arguments = Vec::with_capacity(input.nodes.len().saturating_mul(7).saturating_add(2));
    arguments.push(count.to_string());
    for node in &input.nodes {
        arguments.push(node.kind.tag().to_string());
        arguments.push(node.left.map_or(-1, |value| value as i32).to_string());
        arguments.push(node.right.map_or(-1, |value| value as i32).to_string());
        arguments.push(node.extra.map_or(-1, |value| value as i32).to_string());
        arguments.push(node.expected.map_or(-1, TypecheckType::tag).to_string());
        arguments.push(node.span.start.to_string());
        arguments.push(node.span.end.to_string());
    }
    arguments.push(input.source_len.to_string());
    Ok(arguments)
}

fn parse_output(
    output: &[String],
    source_len: u32,
) -> Result<TypecheckSnapshot, TypecheckAdapterError> {
    let mut reader = ProtocolReader::new(output);
    reader.expect("header", "FUTAO-TYPECHECK-1")?;
    match reader.next("status")? {
        "ok" => {}
        "invalid-input" => {
            return Err(TypecheckAdapterError::InvalidInput(
                "Futao rejected input".to_owned(),
            ))
        }
        status => {
            return Err(TypecheckAdapterError::Protocol(format!(
                "unknown status `{status}`"
            )))
        }
    }
    let schema_version = reader.u32("schema version")?;
    let node_count = reader.count("node count")?;
    let mut types = Vec::with_capacity(node_count);
    for _ in 0..node_count {
        let value = reader.next("type tag")?;
        if !matches!(value, "int" | "bool" | "string" | "unit" | "unknown") {
            return Err(TypecheckAdapterError::Protocol(format!(
                "unknown type tag `{value}`"
            )));
        }
        types.push(value.to_owned());
    }
    let diagnostic_count = reader.count("diagnostic count")?;
    let mut diagnostics = Vec::with_capacity(diagnostic_count);
    for _ in 0..diagnostic_count {
        let code = reader.next("diagnostic code")?;
        if !matches!(code, "E3001" | "E3002" | "E3003") {
            return Err(TypecheckAdapterError::Protocol(format!(
                "unknown diagnostic code `{code}`"
            )));
        }
        let span = TypecheckSpan::new(
            reader.u32("diagnostic source")?,
            reader.u32("diagnostic start")?,
            reader.u32("diagnostic end")?,
        );
        diagnostics.push(TypecheckDiagnosticSnapshot {
            code: code.to_owned(),
            span,
        });
    }
    reader.finish()?;
    let snapshot = TypecheckSnapshot {
        schema_version,
        types,
        diagnostics,
    };
    validate_snapshot(&snapshot, node_count, source_len)?;
    Ok(snapshot)
}

fn validate_snapshot(
    snapshot: &TypecheckSnapshot,
    node_count: usize,
    source_len: u32,
) -> Result<(), TypecheckAdapterError> {
    if snapshot.schema_version != TYPECHECK_SNAPSHOT_SCHEMA_VERSION {
        return Err(TypecheckAdapterError::InvalidSnapshot(
            "unexpected schema version".to_owned(),
        ));
    }
    if snapshot.types.len() != node_count {
        return Err(TypecheckAdapterError::InvalidSnapshot(
            "type count does not match node count".to_owned(),
        ));
    }
    for diagnostic in &snapshot.diagnostics {
        let span = diagnostic.span;
        if span.source != 0 || span.start >= span.end || span.end > source_len {
            return Err(TypecheckAdapterError::InvalidSnapshot(
                "diagnostic span is out of bounds".to_owned(),
            ));
        }
    }
    Ok(())
}

fn runtime_location(sources: &SourceMap, span: nexa_span::SourceSpan) -> String {
    let range = span.range();
    sources.location(span).map_or_else(
        || format!("<unknown> (bytes {}..{})", range.start(), range.end()),
        |(file, location)| {
            format!(
                "{}:{}:{}",
                file.path().display(),
                location.line(),
                location.column()
            )
        },
    )
}

struct ProtocolReader<'output> {
    output: &'output [String],
    position: usize,
}

impl<'output> ProtocolReader<'output> {
    const fn new(output: &'output [String]) -> Self {
        Self {
            output,
            position: 0,
        }
    }

    fn next(&mut self, field: &str) -> Result<&'output str, TypecheckAdapterError> {
        let value = self.output.get(self.position).ok_or_else(|| {
            TypecheckAdapterError::Protocol(format!(
                "missing {field} at line {}",
                self.position + 1
            ))
        })?;
        self.position += 1;
        Ok(value)
    }

    fn expect(&mut self, field: &str, expected: &str) -> Result<(), TypecheckAdapterError> {
        let actual = self.next(field)?;
        if actual == expected {
            Ok(())
        } else {
            Err(TypecheckAdapterError::Protocol(format!(
                "{field} must be `{expected}`, got `{actual}`"
            )))
        }
    }

    fn count(&mut self, field: &str) -> Result<usize, TypecheckAdapterError> {
        let value = self.next(field)?;
        let count = value.parse::<usize>().map_err(|error| {
            TypecheckAdapterError::Protocol(format!("{field} is not a count: {error}"))
        })?;
        if count > 1024 {
            return Err(TypecheckAdapterError::Protocol(format!(
                "{field} exceeds the bound"
            )));
        }
        Ok(count)
    }

    fn u32(&mut self, field: &str) -> Result<u32, TypecheckAdapterError> {
        self.next(field)?.parse::<u32>().map_err(|error| {
            TypecheckAdapterError::Protocol(format!("{field} is not u32: {error}"))
        })
    }

    fn finish(self) -> Result<(), TypecheckAdapterError> {
        if self.position == self.output.len() {
            Ok(())
        } else {
            Err(TypecheckAdapterError::Protocol(
                "trailing output fields".to_owned(),
            ))
        }
    }
}

impl Display for TypecheckImplementation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::RustReference => "rust-reference",
            Self::Futao => "futao-bootstrap-v1",
        })
    }
}
