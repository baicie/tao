//! Parser-only differential comparison for the second self-hosted compiler slice.

use nexa_diagnostics::{LabelStyle, Severity};
use nexa_mir::{run_with_args_and_step_limit as run_mir_with_args, MirProgram};
use nexa_parser::parse_source;
use nexa_span::FileId;
use nexa_syntax::SyntaxKind;
use rowan::{NodeOrToken, WalkEvent};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{compile, CompileError, CompilerInput, CompilerOptions, CompilerSource};

/// Canonical schema used by parser-only differential snapshots.
pub const PARSER_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

const FUTAO_LEXER_SOURCE: &str = include_str!("../../../bootstrap/compiler/src/lexer.ft");
const FUTAO_PARSER_SOURCE: &str = include_str!("../../../bootstrap/compiler/src/parser.ft");
const FUTAO_BRIDGE_SOURCE: &str = include_str!("../../../bootstrap/compiler/src/parser_bridge.ft");
const FUTAO_PROFILE_ENTRY: &str = include_str!("../../../bootstrap/compiler/src/parser_profile.ft");
const FUTAO_DRIVER_ENTRY: &str = include_str!("../../../bootstrap/compiler/src/parser_driver.ft");
// This is a bounded tool budget; the Language 1.0 runtime default remains unchanged.
const FUTAO_PARSER_STEP_LIMIT: usize = 2_000_000;

/// Identifies one implementation participating in parser differential tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParserImplementation {
    /// The Rust Stage 0 parser.
    RustReference,
    /// The Futao Bootstrap Profile parser.
    Futao,
}

impl ParserImplementation {
    /// Returns the stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustReference => "rust-reference",
            Self::Futao => "futao-bootstrap-v1",
        }
    }
}

/// Canonical diagnostic severity at the parser boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParserSeverity {
    /// An error diagnostic.
    Error,
    /// A warning diagnostic.
    Warning,
}

impl ParserSeverity {
    /// Returns the canonical string value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }

    fn parse(value: &str) -> Result<Self, ParserAdapterError> {
        match value {
            "error" => Ok(Self::Error),
            "warning" => Ok(Self::Warning),
            _ => Err(ParserAdapterError::Protocol(format!(
                "unknown diagnostic severity `{value}`"
            ))),
        }
    }
}

/// Canonical source-label role at the parser boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParserLabelStyle {
    /// The primary source label.
    Primary,
    /// A supporting source label.
    Secondary,
}

impl ParserLabelStyle {
    /// Returns the canonical string value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }

    fn parse(value: &str) -> Result<Self, ParserAdapterError> {
        match value {
            "primary" => Ok(Self::Primary),
            "secondary" => Ok(Self::Secondary),
            _ => Err(ParserAdapterError::Protocol(format!(
                "unknown diagnostic label style `{value}`"
            ))),
        }
    }
}

/// One balanced event in the canonical lossless CST stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum ParserCstEvent {
    /// Opens one concrete syntax node at the current lossless cursor.
    StartNode {
        /// Frozen numeric `SyntaxKind` value for the node.
        kind_id: u16,
        /// UTF-8 byte cursor where the node opens.
        offset: u32,
    },
    /// Emits one complete lossless token.
    Token {
        /// Frozen numeric `SyntaxKind` value for the token.
        kind_id: u16,
        /// Inclusive UTF-8 byte start.
        start: u32,
        /// Exclusive UTF-8 byte end.
        end: u32,
    },
    /// Closes the innermost concrete syntax node.
    FinishNode {
        /// UTF-8 byte cursor where the node closes.
        offset: u32,
    },
}

/// One concrete parser recovery region represented by an `Error` CST node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserRecoverySnapshot {
    start: u32,
    end: u32,
}

impl ParserRecoverySnapshot {
    /// Returns the inclusive UTF-8 byte start.
    #[must_use]
    pub const fn start(&self) -> u32 {
        self.start
    }

    /// Returns the exclusive UTF-8 byte end.
    #[must_use]
    pub const fn end(&self) -> u32 {
        self.end
    }
}

/// One canonical parser-only diagnostic observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserDiagnosticSnapshot {
    code: String,
    severity: ParserSeverity,
    label_style: ParserLabelStyle,
    start: u32,
    end: u32,
}

impl ParserDiagnosticSnapshot {
    /// Returns the stable diagnostic code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the canonical severity.
    #[must_use]
    pub const fn severity(&self) -> ParserSeverity {
        self.severity
    }

    /// Returns the canonical source-label role.
    #[must_use]
    pub const fn label_style(&self) -> ParserLabelStyle {
        self.label_style
    }

    /// Returns the inclusive UTF-8 byte start.
    #[must_use]
    pub const fn start(&self) -> u32 {
        self.start
    }

    /// Returns the exclusive UTF-8 byte end.
    #[must_use]
    pub const fn end(&self) -> u32 {
        self.end
    }
}

/// Complete canonical output of one parser implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserSnapshot {
    schema_version: u32,
    cst_events: Vec<ParserCstEvent>,
    recovery_events: Vec<ParserRecoverySnapshot>,
    diagnostics: Vec<ParserDiagnosticSnapshot>,
}

impl ParserSnapshot {
    /// Returns the parser snapshot schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the complete balanced lossless CST event stream.
    #[must_use]
    pub fn cst_events(&self) -> &[ParserCstEvent] {
        &self.cst_events
    }

    /// Returns recovery regions in source and parser-observation order.
    #[must_use]
    pub fn recovery_events(&self) -> &[ParserRecoverySnapshot] {
        &self.recovery_events
    }

    /// Returns parser-only diagnostics in stable source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[ParserDiagnosticSnapshot] {
        &self.diagnostics
    }

    /// Serializes this snapshot as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the in-memory DTO cannot be serialized.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    fn validate(&self, source: &str) -> Result<(), ParserAdapterError> {
        if self.schema_version != PARSER_SNAPSHOT_SCHEMA_VERSION {
            return Err(ParserAdapterError::InvalidSnapshot(format!(
                "schemaVersion must be {PARSER_SNAPSHOT_SCHEMA_VERSION}"
            )));
        }

        let source_len =
            u32::try_from(source.len()).map_err(|_| ParserAdapterError::InputTooLarge)?;
        let mut cursor = 0_u32;
        let mut roots = 0_usize;
        let mut open_nodes = Vec::new();
        let mut derived_recovery = Vec::new();

        for (index, event) in self.cst_events.iter().enumerate() {
            match event {
                ParserCstEvent::StartNode { kind_id, offset } => {
                    if !is_node_kind(*kind_id) {
                        return Err(ParserAdapterError::InvalidSnapshot(format!(
                            "unknown CST node kind {kind_id}"
                        )));
                    }
                    validate_offset(source, *offset, source_len, "node start")?;
                    if *offset != cursor {
                        return Err(ParserAdapterError::InvalidSnapshot(format!(
                            "node kind {kind_id} opens at {offset}, cursor is {cursor}"
                        )));
                    }
                    if open_nodes.is_empty() {
                        roots += 1;
                        if roots != 1 || index != 0 || *kind_id != SyntaxKind::SourceFile as u16 {
                            return Err(ParserAdapterError::InvalidSnapshot(
                                "CST must begin with exactly one SourceFile root".to_owned(),
                            ));
                        }
                    } else if *kind_id == SyntaxKind::SourceFile as u16 {
                        return Err(ParserAdapterError::InvalidSnapshot(
                            "SourceFile cannot be nested".to_owned(),
                        ));
                    }
                    open_nodes.push((*kind_id, *offset));
                }
                ParserCstEvent::Token {
                    kind_id,
                    start,
                    end,
                } => {
                    if open_nodes.is_empty() {
                        return Err(ParserAdapterError::InvalidSnapshot(
                            "CST token appears outside the root node".to_owned(),
                        ));
                    }
                    if !is_token_kind(*kind_id) {
                        return Err(ParserAdapterError::InvalidSnapshot(format!(
                            "unknown CST token kind {kind_id}"
                        )));
                    }
                    validate_offset(source, *start, source_len, "token start")?;
                    validate_offset(source, *end, source_len, "token end")?;
                    if *start != cursor || *end <= *start {
                        return Err(ParserAdapterError::InvalidSnapshot(format!(
                            "token kind {kind_id} at {start}..{end} does not continue cursor {cursor}"
                        )));
                    }
                    cursor = *end;
                }
                ParserCstEvent::FinishNode { offset } => {
                    let Some((kind_id, start)) = open_nodes.pop() else {
                        return Err(ParserAdapterError::InvalidSnapshot(
                            "CST closes a node that was never opened".to_owned(),
                        ));
                    };
                    validate_offset(source, *offset, source_len, "node finish")?;
                    if *offset != cursor {
                        return Err(ParserAdapterError::InvalidSnapshot(format!(
                            "node kind {kind_id} closes at {offset}, cursor is {cursor}"
                        )));
                    }
                    if kind_id == SyntaxKind::Error as u16 {
                        if start == *offset {
                            return Err(ParserAdapterError::InvalidSnapshot(
                                "parser recovery region cannot be empty".to_owned(),
                            ));
                        }
                        derived_recovery.push(ParserRecoverySnapshot {
                            start,
                            end: *offset,
                        });
                    }
                    if open_nodes.is_empty() && index + 1 != self.cst_events.len() {
                        return Err(ParserAdapterError::InvalidSnapshot(
                            "CST contains events after the root closes".to_owned(),
                        ));
                    }
                }
            }
        }

        if roots != 1 || !open_nodes.is_empty() {
            return Err(ParserAdapterError::InvalidSnapshot(
                "CST node events are not balanced".to_owned(),
            ));
        }
        if cursor != source_len {
            return Err(ParserAdapterError::InvalidSnapshot(format!(
                "CST token stream ends at {cursor}, source ends at {source_len}"
            )));
        }
        if self.recovery_events != derived_recovery {
            return Err(ParserAdapterError::InvalidSnapshot(
                "recovery events do not correspond exactly to Error CST nodes".to_owned(),
            ));
        }

        let mut previous_range = None;
        for diagnostic in &self.diagnostics {
            if diagnostic.code != "E1001" {
                return Err(ParserAdapterError::InvalidSnapshot(format!(
                    "unknown parser diagnostic code `{}`",
                    diagnostic.code
                )));
            }
            if diagnostic.label_style != ParserLabelStyle::Primary {
                return Err(ParserAdapterError::InvalidSnapshot(
                    "parser diagnostic label must be primary".to_owned(),
                ));
            }
            validate_offset(source, diagnostic.start, source_len, "diagnostic start")?;
            validate_offset(source, diagnostic.end, source_len, "diagnostic end")?;
            if diagnostic.end < diagnostic.start {
                return Err(ParserAdapterError::InvalidSnapshot(format!(
                    "parser diagnostic range {}..{} is reversed",
                    diagnostic.start, diagnostic.end
                )));
            }
            let range = (diagnostic.start, diagnostic.end);
            if previous_range.is_some_and(|previous| previous > range) {
                return Err(ParserAdapterError::InvalidSnapshot(
                    "parser diagnostics are not in source order".to_owned(),
                ));
            }
            previous_range = Some(range);
        }
        Ok(())
    }
}

/// Failure while building or executing a parser adapter.
#[derive(Debug, Error)]
pub enum ParserAdapterError {
    /// The Rust compiler core rejected an explicit adapter source graph.
    #[error(transparent)]
    Compile(#[from] CompileError),
    /// Futao adapter source did not pass its required compilation profile.
    #[error("Futao parser source was rejected: {0}")]
    FutaoSource(String),
    /// Futao MIR execution failed.
    #[error("Futao parser execution failed: {0}")]
    Runtime(String),
    /// The private Host bridge emitted an invalid protocol.
    #[error("invalid Futao parser protocol: {0}")]
    Protocol(String),
    /// One adapter returned structurally invalid output.
    #[error("invalid parser snapshot: {0}")]
    InvalidSnapshot(String),
    /// The source cannot be represented by schema 1 offsets.
    #[error("source is too large for parser snapshot schema 1")]
    InputTooLarge,
    /// Canonical snapshot serialization failed.
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

/// Produces one canonical parser snapshot from explicit UTF-8 source.
pub trait ParserAdapter {
    /// Returns the implementation represented by this adapter.
    fn implementation(&self) -> ParserImplementation;

    /// Parses one source string without running later compiler phases.
    ///
    /// # Errors
    ///
    /// Returns an adapter, execution, protocol, or snapshot validation error.
    fn parse(&self, source: &str) -> Result<ParserSnapshot, ParserAdapterError>;
}

/// Adapter for the frozen Rust reference parser.
#[derive(Debug, Default, Clone, Copy)]
pub struct RustParserAdapter;

impl ParserAdapter for RustParserAdapter {
    fn implementation(&self) -> ParserImplementation {
        ParserImplementation::RustReference
    }

    fn parse(&self, source: &str) -> Result<ParserSnapshot, ParserAdapterError> {
        let _ = u32::try_from(source.len()).map_err(|_| ParserAdapterError::InputTooLarge)?;
        let parse = parse_source(FileId::new(0), source);
        let mut cst_events = Vec::new();
        let mut cursor = 0_u32;

        for event in parse.syntax().preorder_with_tokens() {
            match event {
                WalkEvent::Enter(NodeOrToken::Node(node)) => {
                    cst_events.push(ParserCstEvent::StartNode {
                        kind_id: node.kind() as u16,
                        offset: cursor,
                    });
                }
                WalkEvent::Enter(NodeOrToken::Token(token)) => {
                    let range = token.text_range();
                    let start = u32::from(range.start());
                    let end = u32::from(range.end());
                    cst_events.push(ParserCstEvent::Token {
                        kind_id: token.kind() as u16,
                        start,
                        end,
                    });
                    cursor = end;
                }
                WalkEvent::Leave(NodeOrToken::Node(_)) => {
                    cst_events.push(ParserCstEvent::FinishNode { offset: cursor });
                }
                WalkEvent::Leave(NodeOrToken::Token(_)) => {}
            }
        }

        let recovery_events = recovery_events_from_cst(&cst_events)?;
        let diagnostics = parse
            .parser_diagnostics()
            .iter()
            .map(|diagnostic| {
                if diagnostic.labels().len() != 1 {
                    return Err(ParserAdapterError::InvalidSnapshot(
                        "Rust parser diagnostic must have exactly one label".to_owned(),
                    ));
                }
                let label = diagnostic.labels().first().ok_or_else(|| {
                    ParserAdapterError::InvalidSnapshot(
                        "Rust parser diagnostic has no primary label".to_owned(),
                    )
                })?;
                if label.span().file() != FileId::new(0) {
                    return Err(ParserAdapterError::InvalidSnapshot(
                        "Rust parser diagnostic references the wrong file".to_owned(),
                    ));
                }
                Ok(ParserDiagnosticSnapshot {
                    code: diagnostic.code().as_str().to_owned(),
                    severity: match diagnostic.severity() {
                        Severity::Error => ParserSeverity::Error,
                        Severity::Warning => ParserSeverity::Warning,
                    },
                    label_style: match label.style() {
                        LabelStyle::Primary => ParserLabelStyle::Primary,
                        LabelStyle::Secondary => ParserLabelStyle::Secondary,
                    },
                    start: u32::try_from(label.span().range().start())
                        .map_err(|_| ParserAdapterError::InputTooLarge)?,
                    end: u32::try_from(label.span().range().end())
                        .map_err(|_| ParserAdapterError::InputTooLarge)?,
                })
            })
            .collect::<Result<Vec<_>, ParserAdapterError>>()?;

        let snapshot = ParserSnapshot {
            schema_version: PARSER_SNAPSHOT_SCHEMA_VERSION,
            cst_events,
            recovery_events,
            diagnostics,
        };
        snapshot.validate(source)?;
        Ok(snapshot)
    }
}

/// Adapter that executes the real Futao-written lexer and parser through verified MIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutaoParserAdapter {
    program: MirProgram,
}

impl FutaoParserAdapter {
    /// Compiles the pure parser under Bootstrap Profile v1 and its private Host driver.
    ///
    /// # Errors
    ///
    /// Returns an error when either source graph is rejected or produces no executable MIR.
    pub fn new() -> Result<Self, ParserAdapterError> {
        let profile_output = compile(&CompilerInput::with_options(
            "parser_profile.ft",
            futao_sources("parser_profile.ft", FUTAO_PROFILE_ENTRY),
            CompilerOptions::bootstrap_v1(),
        ))?;
        ensure_compiled("Bootstrap Profile entry", &profile_output)?;

        let driver_output = compile(&CompilerInput::new(
            "parser_driver.ft",
            futao_sources("parser_driver.ft", FUTAO_DRIVER_ENTRY),
        ))?;
        ensure_compiled("application driver", &driver_output)?;
        let program = driver_output.mir().cloned().ok_or_else(|| {
            ParserAdapterError::FutaoSource(
                "application driver produced no executable MIR".to_owned(),
            )
        })?;
        Ok(Self { program })
    }
}

impl ParserAdapter for FutaoParserAdapter {
    fn implementation(&self) -> ParserImplementation {
        ParserImplementation::Futao
    }

    fn parse(&self, source: &str) -> Result<ParserSnapshot, ParserAdapterError> {
        let arguments = encoded_source_arguments(source)?;
        let execution = run_mir_with_args(&self.program, &arguments, FUTAO_PARSER_STEP_LIMIT)
            .map_err(|failure| ParserAdapterError::Runtime(failure.to_string()))?;
        let snapshot = parse_futao_output(execution.output())?;
        snapshot.validate(source)?;
        Ok(snapshot)
    }
}

fn futao_sources(entry: &str, entry_source: &str) -> Vec<CompilerSource> {
    vec![
        CompilerSource::new(entry, entry_source),
        CompilerSource::new("lexer.ft", FUTAO_LEXER_SOURCE),
        CompilerSource::new("parser.ft", FUTAO_PARSER_SOURCE),
        CompilerSource::new("parser_bridge.ft", FUTAO_BRIDGE_SOURCE),
    ]
}

fn ensure_compiled(role: &str, output: &crate::CompilerOutput) -> Result<(), ParserAdapterError> {
    if output.is_ok() && output.diagnostics().is_empty() {
        return Ok(());
    }
    let summary = output
        .diagnostics()
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
        .collect::<Vec<_>>()
        .join("; ");
    Err(ParserAdapterError::FutaoSource(format!(
        "{role} failed: {summary}"
    )))
}

fn encoded_source_arguments(source: &str) -> Result<Vec<String>, ParserAdapterError> {
    let _ = u32::try_from(source.len()).map_err(|_| ParserAdapterError::InputTooLarge)?;
    let mut arguments = Vec::with_capacity(source.chars().count().saturating_mul(2) + 1);
    arguments.push(source.len().to_string());
    for (offset, character) in source.char_indices() {
        arguments.push(u32::from(character).to_string());
        arguments.push(offset.to_string());
    }
    Ok(arguments)
}

fn parse_futao_output(output: &[String]) -> Result<ParserSnapshot, ParserAdapterError> {
    let mut reader = ProtocolReader::new(output);
    reader.expect("header", "FUTAO-PARSER-1")?;
    let event_count = reader.count("CST event count")?;
    let mut cst_events = Vec::with_capacity(event_count);
    for _ in 0..event_count {
        let tag = reader.next("CST event tag")?;
        let event = match tag {
            "start" => ParserCstEvent::StartNode {
                kind_id: reader.u16("node kind")?,
                offset: reader.u32("node offset")?,
            },
            "token" => ParserCstEvent::Token {
                kind_id: reader.u16("token kind")?,
                start: reader.u32("token start")?,
                end: reader.u32("token end")?,
            },
            "finish" => ParserCstEvent::FinishNode {
                offset: reader.u32("node offset")?,
            },
            _ => {
                return Err(ParserAdapterError::Protocol(format!(
                    "unknown CST event tag `{tag}`"
                )))
            }
        };
        cst_events.push(event);
    }

    let recovery_count = reader.count("recovery event count")?;
    let mut recovery_events = Vec::with_capacity(recovery_count);
    for _ in 0..recovery_count {
        recovery_events.push(ParserRecoverySnapshot {
            start: reader.u32("recovery start")?,
            end: reader.u32("recovery end")?,
        });
    }

    let diagnostic_count = reader.count("diagnostic count")?;
    let mut diagnostics = Vec::with_capacity(diagnostic_count);
    for _ in 0..diagnostic_count {
        diagnostics.push(ParserDiagnosticSnapshot {
            code: reader.next("diagnostic code")?.to_owned(),
            severity: ParserSeverity::parse(reader.next("diagnostic severity")?)?,
            label_style: ParserLabelStyle::parse(reader.next("diagnostic label style")?)?,
            start: reader.u32("diagnostic start")?,
            end: reader.u32("diagnostic end")?,
        });
    }
    reader.finish()?;

    Ok(ParserSnapshot {
        schema_version: PARSER_SNAPSHOT_SCHEMA_VERSION,
        cst_events,
        recovery_events,
        diagnostics,
    })
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

    fn next(&mut self, field: &str) -> Result<&'output str, ParserAdapterError> {
        let value = self.output.get(self.position).ok_or_else(|| {
            ParserAdapterError::Protocol(format!("missing {field} at line {}", self.position + 1))
        })?;
        self.position += 1;
        Ok(value)
    }

    fn expect(&mut self, field: &str, expected: &str) -> Result<(), ParserAdapterError> {
        let value = self.next(field)?;
        if value == expected {
            Ok(())
        } else {
            Err(ParserAdapterError::Protocol(format!(
                "{field} must be `{expected}`, found `{value}`"
            )))
        }
    }

    fn usize(&mut self, field: &str) -> Result<usize, ParserAdapterError> {
        self.next(field)?.parse::<usize>().map_err(|_| {
            ParserAdapterError::Protocol(format!("{field} is not an unsigned integer"))
        })
    }

    fn count(&mut self, field: &str) -> Result<usize, ParserAdapterError> {
        let count = self.usize(field)?;
        let remaining_lines = self.output.len().saturating_sub(self.position);
        if count > remaining_lines {
            return Err(ParserAdapterError::Protocol(format!(
                "{field} {count} exceeds {remaining_lines} remaining protocol line(s)"
            )));
        }
        Ok(count)
    }

    fn u16(&mut self, field: &str) -> Result<u16, ParserAdapterError> {
        self.next(field)?
            .parse::<u16>()
            .map_err(|_| ParserAdapterError::Protocol(format!("{field} is not a u16 integer")))
    }

    fn u32(&mut self, field: &str) -> Result<u32, ParserAdapterError> {
        self.next(field)?
            .parse::<u32>()
            .map_err(|_| ParserAdapterError::Protocol(format!("{field} is not a u32 integer")))
    }

    fn finish(self) -> Result<(), ParserAdapterError> {
        if self.position == self.output.len() {
            Ok(())
        } else {
            Err(ParserAdapterError::Protocol(format!(
                "{} trailing line(s)",
                self.output.len() - self.position
            )))
        }
    }
}

/// Snapshot area that differs between two parser implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParserObservable {
    /// Lossless CST hierarchy, kind, range, or ordering differs.
    Cst,
    /// Concrete parser recovery regions differ.
    Recovery,
    /// Parser-only diagnostic structure or ordering differs.
    Diagnostics,
}

/// One unsuppressed parser differential mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserDifference {
    observable: ParserObservable,
    reference_digest: String,
    candidate_digest: String,
}

impl ParserDifference {
    /// Returns the differing observable category.
    #[must_use]
    pub const fn observable(&self) -> ParserObservable {
        self.observable
    }

    /// Returns the complete reference snapshot digest.
    #[must_use]
    pub fn reference_digest(&self) -> &str {
        &self.reference_digest
    }

    /// Returns the complete candidate snapshot digest.
    #[must_use]
    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }
}

/// Overall parser comparison result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserDifferentialOutcome {
    /// Both complete snapshots match.
    Match,
    /// At least one observable category differs.
    Differences,
}

/// Structured report for one stable parser corpus case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserDifferentialReport {
    case_id: String,
    reference: ParserImplementation,
    candidate: ParserImplementation,
    outcome: ParserDifferentialOutcome,
    differences: Vec<ParserDifference>,
    reference_snapshot: ParserSnapshot,
    candidate_snapshot: ParserSnapshot,
}

impl ParserDifferentialReport {
    /// Returns the stable corpus case identifier.
    #[must_use]
    pub fn case_id(&self) -> &str {
        &self.case_id
    }

    /// Returns the reference implementation.
    #[must_use]
    pub const fn reference(&self) -> ParserImplementation {
        self.reference
    }

    /// Returns the candidate implementation.
    #[must_use]
    pub const fn candidate(&self) -> ParserImplementation {
        self.candidate
    }

    /// Returns the comparison outcome.
    #[must_use]
    pub const fn outcome(&self) -> ParserDifferentialOutcome {
        self.outcome
    }

    /// Returns every unsuppressed observable mismatch.
    #[must_use]
    pub fn differences(&self) -> &[ParserDifference] {
        &self.differences
    }

    /// Returns true only when every observable matches.
    #[must_use]
    pub const fn is_match(&self) -> bool {
        matches!(self.outcome, ParserDifferentialOutcome::Match)
    }

    /// Returns true only for a complete match; no suppression list exists.
    #[must_use]
    pub const fn passes_gate(&self) -> bool {
        self.is_match()
    }

    /// Returns the validated reference snapshot for failure artifacts.
    #[must_use]
    pub const fn reference_snapshot(&self) -> &ParserSnapshot {
        &self.reference_snapshot
    }

    /// Returns the validated candidate snapshot for failure artifacts.
    #[must_use]
    pub const fn candidate_snapshot(&self) -> &ParserSnapshot {
        &self.candidate_snapshot
    }
}

/// Runs two real parser implementations over one shared source string.
pub struct ParserDifferentialHarness<'adapter> {
    reference: &'adapter dyn ParserAdapter,
    candidate: &'adapter dyn ParserAdapter,
}

impl<'adapter> ParserDifferentialHarness<'adapter> {
    /// Creates a parser-only differential harness.
    #[must_use]
    pub const fn new(
        reference: &'adapter dyn ParserAdapter,
        candidate: &'adapter dyn ParserAdapter,
    ) -> Self {
        Self {
            reference,
            candidate,
        }
    }

    /// Runs both adapters and compares every frozen parser observable.
    ///
    /// # Errors
    ///
    /// Returns an adapter, snapshot-validation, or serialization error.
    pub fn run_case(
        &self,
        case_id: impl Into<String>,
        source: &str,
    ) -> Result<ParserDifferentialReport, ParserAdapterError> {
        let reference_snapshot = self.reference.parse(source)?;
        let candidate_snapshot = self.candidate.parse(source)?;
        reference_snapshot.validate(source)?;
        candidate_snapshot.validate(source)?;

        let reference_digest = snapshot_digest(&reference_snapshot)?;
        let candidate_digest = snapshot_digest(&candidate_snapshot)?;
        let mut differences = Vec::new();
        if reference_snapshot.cst_events != candidate_snapshot.cst_events {
            differences.push(ParserDifference {
                observable: ParserObservable::Cst,
                reference_digest: reference_digest.clone(),
                candidate_digest: candidate_digest.clone(),
            });
        }
        if reference_snapshot.recovery_events != candidate_snapshot.recovery_events {
            differences.push(ParserDifference {
                observable: ParserObservable::Recovery,
                reference_digest: reference_digest.clone(),
                candidate_digest: candidate_digest.clone(),
            });
        }
        if reference_snapshot.diagnostics != candidate_snapshot.diagnostics {
            differences.push(ParserDifference {
                observable: ParserObservable::Diagnostics,
                reference_digest,
                candidate_digest,
            });
        }
        let outcome = if differences.is_empty() {
            ParserDifferentialOutcome::Match
        } else {
            ParserDifferentialOutcome::Differences
        };

        Ok(ParserDifferentialReport {
            case_id: case_id.into(),
            reference: self.reference.implementation(),
            candidate: self.candidate.implementation(),
            outcome,
            differences,
            reference_snapshot,
            candidate_snapshot,
        })
    }
}

fn snapshot_digest(snapshot: &ParserSnapshot) -> Result<String, ParserAdapterError> {
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(snapshot)?)
    ))
}

fn recovery_events_from_cst(
    events: &[ParserCstEvent],
) -> Result<Vec<ParserRecoverySnapshot>, ParserAdapterError> {
    let mut open_nodes = Vec::new();
    let mut recovery = Vec::new();
    for event in events {
        match event {
            ParserCstEvent::StartNode { kind_id, offset } => {
                open_nodes.push((*kind_id, *offset));
            }
            ParserCstEvent::Token { .. } => {}
            ParserCstEvent::FinishNode { offset } => {
                let Some((kind_id, start)) = open_nodes.pop() else {
                    return Err(ParserAdapterError::InvalidSnapshot(
                        "CST closes a node that was never opened".to_owned(),
                    ));
                };
                if kind_id == SyntaxKind::Error as u16 {
                    recovery.push(ParserRecoverySnapshot {
                        start,
                        end: *offset,
                    });
                }
            }
        }
    }
    if !open_nodes.is_empty() {
        return Err(ParserAdapterError::InvalidSnapshot(
            "CST node events are not balanced".to_owned(),
        ));
    }
    Ok(recovery)
}

fn validate_offset(
    source: &str,
    offset: u32,
    source_len: u32,
    role: &str,
) -> Result<(), ParserAdapterError> {
    if offset > source_len || !source.is_char_boundary(offset as usize) {
        return Err(ParserAdapterError::InvalidSnapshot(format!(
            "{role} {offset} is not an in-bounds UTF-8 boundary"
        )));
    }
    Ok(())
}

const fn is_token_kind(kind: u16) -> bool {
    matches!(
        kind,
        0..=38 | 64..=68 | 74 | 80..=84 | 92..=94 | 101
    )
}

const fn is_node_kind(kind: u16) -> bool {
    matches!(
        kind,
        39..=63 | 69..=73 | 75..=79 | 85..=91 | 95..=100 | 102..=106
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_futao_output, ParserAdapterError};

    #[test]
    fn futao_protocol_rejects_cst_count_larger_than_remaining_lines() {
        let output = vec!["FUTAO-PARSER-1".to_owned(), usize::MAX.to_string()];

        let error = parse_futao_output(&output).err();

        assert!(
            matches!(error, Some(ParserAdapterError::Protocol(message)) if message.contains("CST event count") && message.contains("remaining protocol line"))
        );
    }

    #[test]
    fn futao_protocol_rejects_recovery_count_larger_than_remaining_lines() {
        let output = vec![
            "FUTAO-PARSER-1".to_owned(),
            "0".to_owned(),
            usize::MAX.to_string(),
        ];

        let error = parse_futao_output(&output).err();

        assert!(
            matches!(error, Some(ParserAdapterError::Protocol(message)) if message.contains("recovery event count") && message.contains("remaining protocol line"))
        );
    }

    #[test]
    fn futao_protocol_rejects_diagnostic_count_larger_than_remaining_lines() {
        let output = vec![
            "FUTAO-PARSER-1".to_owned(),
            "0".to_owned(),
            "0".to_owned(),
            usize::MAX.to_string(),
        ];

        let error = parse_futao_output(&output).err();

        assert!(
            matches!(error, Some(ParserAdapterError::Protocol(message)) if message.contains("diagnostic count") && message.contains("remaining protocol line"))
        );
    }

    #[test]
    fn futao_protocol_rejects_missing_event_fields() {
        let output = ["FUTAO-PARSER-1", "1", "start", "39"].map(str::to_owned);

        let error = parse_futao_output(&output).err();

        assert!(
            matches!(error, Some(ParserAdapterError::Protocol(message)) if message.contains("missing node offset"))
        );
    }

    #[test]
    fn futao_protocol_rejects_unknown_event_tags() {
        let output = ["FUTAO-PARSER-1", "1", "mystery"].map(str::to_owned);

        let error = parse_futao_output(&output).err();

        assert!(
            matches!(error, Some(ParserAdapterError::Protocol(message)) if message.contains("unknown CST event tag"))
        );
    }

    #[test]
    fn futao_protocol_rejects_trailing_fields() {
        let output = [
            "FUTAO-PARSER-1",
            "2",
            "start",
            "39",
            "0",
            "finish",
            "0",
            "0",
            "0",
            "unexpected",
        ]
        .map(str::to_owned);

        let error = parse_futao_output(&output).err();

        assert!(
            matches!(error, Some(ParserAdapterError::Protocol(message)) if message == "1 trailing line(s)")
        );
    }
}
