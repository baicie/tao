//! Parser-only differential comparison for the second self-hosted compiler slice.

use nexa_diagnostics::{LabelStyle, Severity};
use nexa_parser::parse_source;
use nexa_span::FileId;
use nexa_syntax::SyntaxKind;
use rowan::{NodeOrToken, WalkEvent};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::CompileError;

/// Canonical schema used by parser-only differential snapshots.
pub const PARSER_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

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
