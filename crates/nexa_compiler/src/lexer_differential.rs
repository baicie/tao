//! Lexer-only differential comparison for the first self-hosted compiler slice.

use nexa_diagnostics::{LabelStyle, Severity};
use nexa_mir::{run_with_args as run_mir_with_args, MirProgram};
use nexa_parser::lex_source;
use nexa_span::FileId;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{compile, CompileError, CompilerInput, CompilerOptions, CompilerSource};

/// Canonical schema used by lexer-only differential snapshots.
pub const LEXER_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

const FUTAO_LEXER_SOURCE: &str = include_str!("../../../bootstrap/compiler/src/lexer.ft");
const FUTAO_BRIDGE_SOURCE: &str = include_str!("../../../bootstrap/compiler/src/lexer_bridge.ft");
const FUTAO_PROFILE_ENTRY: &str = include_str!("../../../bootstrap/compiler/src/lexer_profile.ft");
const FUTAO_DRIVER_ENTRY: &str = include_str!("../../../bootstrap/compiler/src/lexer_driver.ft");

/// Identifies one implementation participating in lexer differential tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexerImplementation {
    /// The Rust Stage 0 lexer.
    RustReference,
    /// The Futao Bootstrap Profile lexer.
    Futao,
}

impl LexerImplementation {
    /// Returns the stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustReference => "rust-reference",
            Self::Futao => "futao-bootstrap-v1",
        }
    }
}

/// Canonical diagnostic severity at the lexer boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LexerSeverity {
    /// An error diagnostic.
    Error,
    /// A warning diagnostic.
    Warning,
}

impl LexerSeverity {
    /// Returns the canonical string value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }

    fn parse(value: &str) -> Result<Self, LexerAdapterError> {
        match value {
            "error" => Ok(Self::Error),
            "warning" => Ok(Self::Warning),
            _ => Err(LexerAdapterError::Protocol(format!(
                "unknown diagnostic severity `{value}`"
            ))),
        }
    }
}

/// Canonical source-label role at the lexer boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LexerLabelStyle {
    /// The primary source label.
    Primary,
    /// A supporting source label.
    Secondary,
}

impl LexerLabelStyle {
    /// Returns the canonical string value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }

    fn parse(value: &str) -> Result<Self, LexerAdapterError> {
        match value {
            "primary" => Ok(Self::Primary),
            "secondary" => Ok(Self::Secondary),
            _ => Err(LexerAdapterError::Protocol(format!(
                "unknown diagnostic label style `{value}`"
            ))),
        }
    }
}

/// One canonical lossless token observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LexerTokenSnapshot {
    kind_id: u16,
    start: u32,
    end: u32,
    trivia: bool,
}

impl LexerTokenSnapshot {
    /// Returns the frozen numeric `SyntaxKind` value.
    #[must_use]
    pub const fn kind_id(&self) -> u16 {
        self.kind_id
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

    /// Returns whether this token is whitespace or a line comment.
    #[must_use]
    pub const fn is_trivia(&self) -> bool {
        self.trivia
    }
}

/// One canonical lexical diagnostic observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LexerDiagnosticSnapshot {
    code: String,
    severity: LexerSeverity,
    label_style: LexerLabelStyle,
    start: u32,
    end: u32,
}

impl LexerDiagnosticSnapshot {
    /// Returns the stable diagnostic code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the canonical severity.
    #[must_use]
    pub const fn severity(&self) -> LexerSeverity {
        self.severity
    }

    /// Returns the canonical source-label role.
    #[must_use]
    pub const fn label_style(&self) -> LexerLabelStyle {
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

/// Complete canonical output of one lexer implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LexerSnapshot {
    schema_version: u32,
    tokens: Vec<LexerTokenSnapshot>,
    diagnostics: Vec<LexerDiagnosticSnapshot>,
}

impl LexerSnapshot {
    /// Returns the lexer snapshot schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns tokens in exact source order, including trivia.
    #[must_use]
    pub fn tokens(&self) -> &[LexerTokenSnapshot] {
        &self.tokens
    }

    /// Returns lexical diagnostics in exact source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[LexerDiagnosticSnapshot] {
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

    fn validate(&self, source: &str) -> Result<(), LexerAdapterError> {
        if self.schema_version != LEXER_SNAPSHOT_SCHEMA_VERSION {
            return Err(LexerAdapterError::InvalidSnapshot(format!(
                "schemaVersion must be {LEXER_SNAPSHOT_SCHEMA_VERSION}"
            )));
        }

        let mut cursor = 0_u32;
        let source_len =
            u32::try_from(source.len()).map_err(|_| LexerAdapterError::InputTooLarge)?;
        for token in &self.tokens {
            if !is_token_kind(token.kind_id) {
                return Err(LexerAdapterError::InvalidSnapshot(format!(
                    "unknown token kind {}",
                    token.kind_id
                )));
            }
            if token.start != cursor || token.end <= token.start || token.end > source_len {
                return Err(LexerAdapterError::InvalidSnapshot(format!(
                    "token {}..{} does not continue source offset {cursor}",
                    token.start, token.end
                )));
            }
            if !source.is_char_boundary(token.start as usize)
                || !source.is_char_boundary(token.end as usize)
            {
                return Err(LexerAdapterError::InvalidSnapshot(format!(
                    "token {}..{} is not on UTF-8 boundaries",
                    token.start, token.end
                )));
            }
            if token.trivia != matches!(token.kind_id, 2 | 3) {
                return Err(LexerAdapterError::InvalidSnapshot(format!(
                    "token kind {} has inconsistent trivia state",
                    token.kind_id
                )));
            }
            cursor = token.end;
        }
        if cursor != source_len {
            return Err(LexerAdapterError::InvalidSnapshot(format!(
                "token stream ends at {cursor}, source ends at {source_len}"
            )));
        }

        let expected_diagnostics = self
            .tokens
            .iter()
            .filter(|token| token.kind_id == 11)
            .map(|token| LexerDiagnosticSnapshot {
                code: "E1001".to_owned(),
                severity: LexerSeverity::Error,
                label_style: LexerLabelStyle::Primary,
                start: token.start,
                end: token.end,
            })
            .collect::<Vec<_>>();
        if self.diagnostics != expected_diagnostics {
            return Err(LexerAdapterError::InvalidSnapshot(
                "lexical diagnostics do not correspond exactly to Unknown tokens".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Failure while building or executing a lexer adapter.
#[derive(Debug, Error)]
pub enum LexerAdapterError {
    /// The Rust compiler core rejected an explicit adapter source graph.
    #[error(transparent)]
    Compile(#[from] CompileError),
    /// Futao adapter source did not pass its required compilation profile.
    #[error("Futao lexer source was rejected: {0}")]
    FutaoSource(String),
    /// Futao MIR execution failed.
    #[error("Futao lexer execution failed: {0}")]
    Runtime(String),
    /// The private Host bridge emitted an invalid protocol.
    #[error("invalid Futao lexer protocol: {0}")]
    Protocol(String),
    /// One adapter returned structurally invalid output.
    #[error("invalid lexer snapshot: {0}")]
    InvalidSnapshot(String),
    /// The source cannot be represented by schema 1 offsets.
    #[error("source is too large for lexer snapshot schema 1")]
    InputTooLarge,
    /// Canonical snapshot serialization failed.
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

/// Produces one canonical lexer snapshot from explicit UTF-8 source.
pub trait LexerAdapter {
    /// Returns the implementation represented by this adapter.
    fn implementation(&self) -> LexerImplementation;

    /// Lexes one source string without parsing it.
    ///
    /// # Errors
    ///
    /// Returns an adapter, execution, protocol, or snapshot validation error.
    fn lex(&self, source: &str) -> Result<LexerSnapshot, LexerAdapterError>;
}

/// Adapter for the frozen Rust reference lexer.
#[derive(Debug, Default, Clone, Copy)]
pub struct RustLexerAdapter;

impl LexerAdapter for RustLexerAdapter {
    fn implementation(&self) -> LexerImplementation {
        LexerImplementation::RustReference
    }

    fn lex(&self, source: &str) -> Result<LexerSnapshot, LexerAdapterError> {
        let lexed = lex_source(FileId::new(0), source);
        let tokens = lexed
            .tokens()
            .iter()
            .map(|token| {
                Ok(LexerTokenSnapshot {
                    kind_id: token.kind() as u16,
                    start: u32::try_from(token.range().start())
                        .map_err(|_| LexerAdapterError::InputTooLarge)?,
                    end: u32::try_from(token.range().end())
                        .map_err(|_| LexerAdapterError::InputTooLarge)?,
                    trivia: token.kind().is_trivia(),
                })
            })
            .collect::<Result<Vec<_>, LexerAdapterError>>()?;
        let diagnostics = lexed
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                let label = diagnostic.labels().first().ok_or_else(|| {
                    LexerAdapterError::InvalidSnapshot(
                        "Rust lexical diagnostic has no primary label".to_owned(),
                    )
                })?;
                Ok(LexerDiagnosticSnapshot {
                    code: diagnostic.code().as_str().to_owned(),
                    severity: match diagnostic.severity() {
                        Severity::Error => LexerSeverity::Error,
                        Severity::Warning => LexerSeverity::Warning,
                    },
                    label_style: match label.style() {
                        LabelStyle::Primary => LexerLabelStyle::Primary,
                        LabelStyle::Secondary => LexerLabelStyle::Secondary,
                    },
                    start: u32::try_from(label.span().range().start())
                        .map_err(|_| LexerAdapterError::InputTooLarge)?,
                    end: u32::try_from(label.span().range().end())
                        .map_err(|_| LexerAdapterError::InputTooLarge)?,
                })
            })
            .collect::<Result<Vec<_>, LexerAdapterError>>()?;
        let snapshot = LexerSnapshot {
            schema_version: LEXER_SNAPSHOT_SCHEMA_VERSION,
            tokens,
            diagnostics,
        };
        snapshot.validate(source)?;
        Ok(snapshot)
    }
}

/// Adapter that executes the real Futao-written lexer through verified MIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutaoLexerAdapter {
    program: MirProgram,
}

impl FutaoLexerAdapter {
    /// Compiles and validates the Futao lexer source and private Host driver.
    ///
    /// # Errors
    ///
    /// Returns an error when the pure source violates Bootstrap Profile v1 or
    /// the application-only driver cannot produce executable MIR.
    pub fn new() -> Result<Self, LexerAdapterError> {
        let profile_output = compile(&CompilerInput::with_options(
            "lexer_profile.ft",
            futao_sources("lexer_profile.ft", FUTAO_PROFILE_ENTRY),
            CompilerOptions::bootstrap_v1(),
        ))?;
        ensure_compiled("Bootstrap Profile entry", &profile_output)?;

        let driver_output = compile(&CompilerInput::new(
            "lexer_driver.ft",
            futao_sources("lexer_driver.ft", FUTAO_DRIVER_ENTRY),
        ))?;
        ensure_compiled("application driver", &driver_output)?;
        let program = driver_output.mir().cloned().ok_or_else(|| {
            LexerAdapterError::FutaoSource(
                "application driver produced no executable MIR".to_owned(),
            )
        })?;
        Ok(Self { program })
    }
}

impl LexerAdapter for FutaoLexerAdapter {
    fn implementation(&self) -> LexerImplementation {
        LexerImplementation::Futao
    }

    fn lex(&self, source: &str) -> Result<LexerSnapshot, LexerAdapterError> {
        let arguments = encoded_source_arguments(source)?;
        let execution = run_mir_with_args(&self.program, &arguments)
            .map_err(|failure| LexerAdapterError::Runtime(failure.to_string()))?;
        let snapshot = parse_futao_output(execution.output())?;
        snapshot.validate(source)?;
        Ok(snapshot)
    }
}

fn futao_sources(entry: &str, entry_source: &str) -> Vec<CompilerSource> {
    vec![
        CompilerSource::new(entry, entry_source),
        CompilerSource::new("lexer.ft", FUTAO_LEXER_SOURCE),
        CompilerSource::new("lexer_bridge.ft", FUTAO_BRIDGE_SOURCE),
    ]
}

fn ensure_compiled(role: &str, output: &crate::CompilerOutput) -> Result<(), LexerAdapterError> {
    if output.is_ok() && output.diagnostics().is_empty() {
        return Ok(());
    }
    let summary = output
        .diagnostics()
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
        .collect::<Vec<_>>()
        .join("; ");
    Err(LexerAdapterError::FutaoSource(format!(
        "{role} failed: {summary}"
    )))
}

fn encoded_source_arguments(source: &str) -> Result<Vec<String>, LexerAdapterError> {
    let _ = u32::try_from(source.len()).map_err(|_| LexerAdapterError::InputTooLarge)?;
    let mut arguments = Vec::with_capacity(source.chars().count().saturating_mul(2) + 1);
    arguments.push(source.len().to_string());
    for (offset, character) in source.char_indices() {
        arguments.push(u32::from(character).to_string());
        arguments.push(offset.to_string());
    }
    Ok(arguments)
}

fn parse_futao_output(output: &[String]) -> Result<LexerSnapshot, LexerAdapterError> {
    let mut reader = ProtocolReader::new(output);
    reader.expect("header", "FUTAO-LEXER-1")?;
    let token_count = reader.usize("token count")?;
    let mut tokens = Vec::with_capacity(token_count);
    for _ in 0..token_count {
        tokens.push(LexerTokenSnapshot {
            kind_id: reader.u16("token kind")?,
            start: reader.u32("token start")?,
            end: reader.u32("token end")?,
            trivia: reader.boolean("token trivia")?,
        });
    }

    let diagnostic_count = reader.usize("diagnostic count")?;
    let mut diagnostics = Vec::with_capacity(diagnostic_count);
    for _ in 0..diagnostic_count {
        diagnostics.push(LexerDiagnosticSnapshot {
            code: reader.next("diagnostic code")?.to_owned(),
            severity: LexerSeverity::parse(reader.next("diagnostic severity")?)?,
            label_style: LexerLabelStyle::parse(reader.next("diagnostic label style")?)?,
            start: reader.u32("diagnostic start")?,
            end: reader.u32("diagnostic end")?,
        });
    }
    reader.finish()?;

    Ok(LexerSnapshot {
        schema_version: LEXER_SNAPSHOT_SCHEMA_VERSION,
        tokens,
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

    fn next(&mut self, field: &str) -> Result<&'output str, LexerAdapterError> {
        let value = self.output.get(self.position).ok_or_else(|| {
            LexerAdapterError::Protocol(format!("missing {field} at line {}", self.position + 1))
        })?;
        self.position += 1;
        Ok(value)
    }

    fn expect(&mut self, field: &str, expected: &str) -> Result<(), LexerAdapterError> {
        let value = self.next(field)?;
        if value == expected {
            Ok(())
        } else {
            Err(LexerAdapterError::Protocol(format!(
                "{field} must be `{expected}`, found `{value}`"
            )))
        }
    }

    fn usize(&mut self, field: &str) -> Result<usize, LexerAdapterError> {
        self.next(field)?
            .parse::<usize>()
            .map_err(|_| LexerAdapterError::Protocol(format!("{field} is not an unsigned integer")))
    }

    fn u16(&mut self, field: &str) -> Result<u16, LexerAdapterError> {
        self.next(field)?
            .parse::<u16>()
            .map_err(|_| LexerAdapterError::Protocol(format!("{field} is not a u16 integer")))
    }

    fn u32(&mut self, field: &str) -> Result<u32, LexerAdapterError> {
        self.next(field)?
            .parse::<u32>()
            .map_err(|_| LexerAdapterError::Protocol(format!("{field} is not a u32 integer")))
    }

    fn boolean(&mut self, field: &str) -> Result<bool, LexerAdapterError> {
        match self.next(field)? {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(LexerAdapterError::Protocol(format!(
                "{field} is not a canonical boolean"
            ))),
        }
    }

    fn finish(self) -> Result<(), LexerAdapterError> {
        if self.position == self.output.len() {
            Ok(())
        } else {
            Err(LexerAdapterError::Protocol(format!(
                "{} trailing line(s)",
                self.output.len() - self.position
            )))
        }
    }
}

/// Snapshot area that differs between two lexer implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexerObservable {
    /// Token kind, range, order, or trivia differs.
    Tokens,
    /// Lexical diagnostic structure or order differs.
    Diagnostics,
}

/// One unsuppressed lexer differential mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexerDifference {
    observable: LexerObservable,
    reference_digest: String,
    candidate_digest: String,
}

impl LexerDifference {
    /// Returns the differing observable category.
    #[must_use]
    pub const fn observable(&self) -> LexerObservable {
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

/// Overall lexer comparison result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexerDifferentialOutcome {
    /// Both complete snapshots match.
    Match,
    /// At least one observable category differs.
    Differences,
}

/// Structured report for one stable lexer corpus case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexerDifferentialReport {
    case_id: String,
    reference: LexerImplementation,
    candidate: LexerImplementation,
    outcome: LexerDifferentialOutcome,
    differences: Vec<LexerDifference>,
    reference_snapshot: LexerSnapshot,
    candidate_snapshot: LexerSnapshot,
}

impl LexerDifferentialReport {
    /// Returns the stable corpus case identifier.
    #[must_use]
    pub fn case_id(&self) -> &str {
        &self.case_id
    }

    /// Returns the reference implementation.
    #[must_use]
    pub const fn reference(&self) -> LexerImplementation {
        self.reference
    }

    /// Returns the candidate implementation.
    #[must_use]
    pub const fn candidate(&self) -> LexerImplementation {
        self.candidate
    }

    /// Returns the comparison outcome.
    #[must_use]
    pub const fn outcome(&self) -> LexerDifferentialOutcome {
        self.outcome
    }

    /// Returns every unsuppressed observable mismatch.
    #[must_use]
    pub fn differences(&self) -> &[LexerDifference] {
        &self.differences
    }

    /// Returns true only when every observable matches.
    #[must_use]
    pub const fn is_match(&self) -> bool {
        matches!(self.outcome, LexerDifferentialOutcome::Match)
    }

    /// Returns true only for a complete match; no suppression list exists.
    #[must_use]
    pub const fn passes_gate(&self) -> bool {
        self.is_match()
    }

    /// Returns the validated reference snapshot for failure artifacts.
    #[must_use]
    pub const fn reference_snapshot(&self) -> &LexerSnapshot {
        &self.reference_snapshot
    }

    /// Returns the validated candidate snapshot for failure artifacts.
    #[must_use]
    pub const fn candidate_snapshot(&self) -> &LexerSnapshot {
        &self.candidate_snapshot
    }
}

/// Runs two real lexer implementations over one shared source string.
pub struct LexerDifferentialHarness<'adapter> {
    reference: &'adapter dyn LexerAdapter,
    candidate: &'adapter dyn LexerAdapter,
}

impl<'adapter> LexerDifferentialHarness<'adapter> {
    /// Creates a lexer-only differential harness.
    #[must_use]
    pub const fn new(
        reference: &'adapter dyn LexerAdapter,
        candidate: &'adapter dyn LexerAdapter,
    ) -> Self {
        Self {
            reference,
            candidate,
        }
    }

    /// Runs both adapters and compares every frozen lexer observable.
    ///
    /// # Errors
    ///
    /// Returns an adapter, snapshot-validation, or serialization error.
    pub fn run_case(
        &self,
        case_id: impl Into<String>,
        source: &str,
    ) -> Result<LexerDifferentialReport, LexerAdapterError> {
        let reference_snapshot = self.reference.lex(source)?;
        let candidate_snapshot = self.candidate.lex(source)?;
        reference_snapshot.validate(source)?;
        candidate_snapshot.validate(source)?;

        let reference_digest = snapshot_digest(&reference_snapshot)?;
        let candidate_digest = snapshot_digest(&candidate_snapshot)?;
        let mut differences = Vec::new();
        if reference_snapshot.tokens != candidate_snapshot.tokens {
            differences.push(LexerDifference {
                observable: LexerObservable::Tokens,
                reference_digest: reference_digest.clone(),
                candidate_digest: candidate_digest.clone(),
            });
        }
        if reference_snapshot.diagnostics != candidate_snapshot.diagnostics {
            differences.push(LexerDifference {
                observable: LexerObservable::Diagnostics,
                reference_digest,
                candidate_digest,
            });
        }
        let outcome = if differences.is_empty() {
            LexerDifferentialOutcome::Match
        } else {
            LexerDifferentialOutcome::Differences
        };

        Ok(LexerDifferentialReport {
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

fn snapshot_digest(snapshot: &LexerSnapshot) -> Result<String, LexerAdapterError> {
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(snapshot)?)
    ))
}

const fn is_token_kind(kind: u16) -> bool {
    matches!(
        kind,
        0..=38 | 64..=68 | 74 | 80..=84 | 92..=94 | 101
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_futao_output, LexerAdapterError};

    #[test]
    fn futao_protocol_rejects_missing_token_fields() {
        let output = ["FUTAO-LEXER-1", "1"].map(str::to_owned);

        let error = parse_futao_output(&output).err();

        assert!(matches!(
            error,
            Some(LexerAdapterError::Protocol(message))
                if message == "missing token kind at line 3"
        ));
    }

    #[test]
    fn futao_protocol_rejects_trailing_lines() {
        let output = ["FUTAO-LEXER-1", "0", "0", "unexpected"].map(str::to_owned);

        let error = parse_futao_output(&output).err();

        assert!(matches!(
            error,
            Some(LexerAdapterError::Protocol(message)) if message == "1 trailing line(s)"
        ));
    }
}
