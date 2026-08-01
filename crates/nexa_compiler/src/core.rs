//! Pure explicit-input compiler boundary and canonical phase output.

use std::collections::BTreeSet;
use std::path::Path;

use nexa_diagnostics::{Diagnostic, LabelStyle, Severity};
use nexa_mir::{lower as lower_mir, MirLoweringError, MirProgram};
use nexa_source::{MemorySourceProvider, SourceMap};
use nexa_span::SourceSpan;
use rowan::{NodeOrToken, WalkEvent};
use serde::Serialize;
use thiserror::Error;

use crate::{canonical, check_session, CheckResult, CompilerSession, SessionBuildError};

/// Schema version for every canonical compiler phase dump emitted by this toolchain.
pub const CANONICAL_DUMP_SCHEMA_VERSION: u32 = 1;

const NIR_PLANNED_VERSION: &str = "0.0.5";

/// Source-language contract selected for an explicit compiler invocation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LanguageVersion {
    /// The frozen Nexa Language 1.0 compatibility baseline used during bootstrap.
    #[default]
    Nexa1_0,
}

impl LanguageVersion {
    /// Returns the manifest spelling of this language contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nexa1_0 => "1.0",
        }
    }
}

/// Deterministic options understood by the current compiler core.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CompilerOptions {
    language_version: LanguageVersion,
}

impl CompilerOptions {
    /// Creates options for the frozen Language 1.0 compatibility baseline.
    #[must_use]
    pub const fn language_1_0() -> Self {
        Self {
            language_version: LanguageVersion::Nexa1_0,
        }
    }

    /// Returns the selected source-language contract.
    #[must_use]
    pub const fn language_version(self) -> LanguageVersion {
        self.language_version
    }
}

/// One explicit UTF-8 source supplied to the pure compiler core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerSource {
    identity: String,
    content: String,
}

impl CompilerSource {
    /// Creates one source with a canonical, portable logical identity.
    #[must_use]
    pub fn new(identity: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            identity: identity.into(),
            content: content.into(),
        }
    }

    /// Returns the caller-supplied logical source identity.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Returns the complete UTF-8 source text.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// Complete explicit input for one deterministic compiler-core invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerInput {
    entry: String,
    sources: Vec<CompilerSource>,
    options: CompilerOptions,
}

impl CompilerInput {
    /// Creates compiler input from an entry identity and an unordered source collection.
    #[must_use]
    pub fn new(
        entry: impl Into<String>,
        sources: impl IntoIterator<Item = CompilerSource>,
    ) -> Self {
        Self {
            entry: entry.into(),
            sources: sources.into_iter().collect(),
            options: CompilerOptions::default(),
        }
    }

    /// Creates compiler input with explicit deterministic compiler options.
    #[must_use]
    pub fn with_options(
        entry: impl Into<String>,
        sources: impl IntoIterator<Item = CompilerSource>,
        options: CompilerOptions,
    ) -> Self {
        Self {
            entry: entry.into(),
            sources: sources.into_iter().collect(),
            options,
        }
    }

    /// Returns the logical identity of the entry source.
    #[must_use]
    pub fn entry(&self) -> &str {
        &self.entry
    }

    /// Returns every explicit source. Collection order has no semantic meaning.
    #[must_use]
    pub fn sources(&self) -> &[CompilerSource] {
        &self.sources
    }

    /// Returns the explicit deterministic compiler options.
    #[must_use]
    pub const fn options(&self) -> CompilerOptions {
        self.options
    }
}

/// A failure before the compiler can return ordinary source diagnostics.
#[derive(Debug, Error)]
pub enum CompileError {
    /// A source identity was not already in portable canonical form.
    #[error("source identity `{identity}` is invalid: {reason}")]
    InvalidSourceIdentity {
        /// The rejected source identity.
        identity: String,
        /// The stable validation reason.
        reason: &'static str,
    },
    /// More than one source used the same canonical identity.
    #[error("source identity `{identity}` was provided more than once")]
    DuplicateSource {
        /// The duplicated source identity.
        identity: String,
    },
    /// The selected entry identity was absent from the explicit source collection.
    #[error("entry source `{entry}` was not provided")]
    MissingEntry {
        /// The missing entry identity.
        entry: String,
    },
    /// Session construction failed before a source-spanned result existed.
    #[error(transparent)]
    Session(#[from] SessionBuildError),
    /// Validated HIR violated a MIR lowering invariant.
    #[error(transparent)]
    Mir(#[from] MirLoweringError),
    /// A canonical phase value could not be serialized.
    #[error("failed to serialize canonical compiler output: {0}")]
    Canonical(#[from] serde_json::Error),
    /// A source byte offset exceeded the canonical 32-bit span contract.
    #[error("source byte offset exceeded the canonical u32 range")]
    CanonicalOffsetOverflow,
}

/// A compiler phase represented in the differential output contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CanonicalPhase {
    /// Lossless lexer tokens.
    Tokens,
    /// Lossless concrete syntax tree.
    Cst,
    /// Ordered structured diagnostics.
    Diagnostics,
    /// Typed high-level IR and semantic facts.
    Hir,
    /// Complete control-flow MIR.
    Mir,
    /// Target-neutral internal bootstrap IR.
    Nir,
}

impl CanonicalPhase {
    pub(crate) const ALL: [Self; 6] = [
        Self::Tokens,
        Self::Cst,
        Self::Diagnostics,
        Self::Hir,
        Self::Mir,
        Self::Nir,
    ];
}

/// Availability state for one canonical phase artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalArtifactStatus {
    /// The artifact contains canonical JSON.
    Available,
    /// Earlier source diagnostics prevented this phase from running.
    BlockedByDiagnostics,
    /// The phase does not exist in the current toolchain.
    NotImplemented,
}

/// Discriminated state of one canonical compiler phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum CanonicalArtifactState {
    /// Canonical JSON was produced for this phase.
    Produced {
        /// The versioned canonical JSON envelope.
        content: String,
    },
    /// Source diagnostics prevented this phase from executing.
    SkippedDueToDiagnostics,
    /// The phase is intentionally absent until a later milestone.
    Unavailable {
        /// The first planned toolchain milestone for this phase.
        planned_version: String,
    },
}

/// One phase entry in a canonical compiler dump.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalArtifact {
    phase: CanonicalPhase,
    artifact: CanonicalArtifactState,
}

impl CanonicalArtifact {
    fn produced(phase: CanonicalPhase, content: String) -> Self {
        Self {
            phase,
            artifact: CanonicalArtifactState::Produced { content },
        }
    }

    fn blocked(phase: CanonicalPhase) -> Self {
        Self {
            phase,
            artifact: CanonicalArtifactState::SkippedDueToDiagnostics,
        }
    }

    fn unavailable(phase: CanonicalPhase, planned_version: &str) -> Self {
        Self {
            phase,
            artifact: CanonicalArtifactState::Unavailable {
                planned_version: planned_version.to_owned(),
            },
        }
    }

    /// Returns the represented compiler phase.
    #[must_use]
    pub const fn phase(&self) -> CanonicalPhase {
        self.phase
    }

    /// Returns the phase availability without conflating absence reasons.
    #[must_use]
    pub const fn status(&self) -> CanonicalArtifactStatus {
        match self.artifact {
            CanonicalArtifactState::Produced { .. } => CanonicalArtifactStatus::Available,
            CanonicalArtifactState::SkippedDueToDiagnostics => {
                CanonicalArtifactStatus::BlockedByDiagnostics
            }
            CanonicalArtifactState::Unavailable { .. } => CanonicalArtifactStatus::NotImplemented,
        }
    }

    /// Returns canonical JSON when this phase was produced.
    #[must_use]
    pub fn content(&self) -> Option<&str> {
        match &self.artifact {
            CanonicalArtifactState::Produced { content } => Some(content),
            CanonicalArtifactState::SkippedDueToDiagnostics
            | CanonicalArtifactState::Unavailable { .. } => None,
        }
    }

    /// Returns the complete discriminated artifact state.
    #[must_use]
    pub const fn state(&self) -> &CanonicalArtifactState {
        &self.artifact
    }
}

/// Canonical severity independent from implementation-specific formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CanonicalSeverity {
    /// A blocking error.
    Error,
    /// A non-blocking warning.
    Warning,
}

/// Canonical role of one diagnostic label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CanonicalLabelStyle {
    /// The primary source location.
    Primary,
    /// A related source location.
    Secondary,
}

/// One canonical diagnostic label using discovery-stable source ordinals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalLabel {
    style: CanonicalLabelStyle,
    file: u32,
    start: u32,
    end: u32,
    message: String,
}

impl CanonicalLabel {
    /// Returns the label role.
    #[must_use]
    pub const fn style(&self) -> CanonicalLabelStyle {
        self.style
    }

    /// Returns the source discovery ordinal.
    #[must_use]
    pub const fn file(&self) -> u32 {
        self.file
    }

    /// Returns the inclusive byte start.
    #[must_use]
    pub const fn start(&self) -> u32 {
        self.start
    }

    /// Returns the exclusive byte end.
    #[must_use]
    pub const fn end(&self) -> u32 {
        self.end
    }

    /// Returns the source-label text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// One canonical structured diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalDiagnostic {
    code: String,
    severity: CanonicalSeverity,
    message: String,
    labels: Vec<CanonicalLabel>,
}

impl CanonicalDiagnostic {
    /// Returns the stable diagnostic code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the diagnostic severity.
    #[must_use]
    pub const fn severity(&self) -> CanonicalSeverity {
        self.severity
    }

    /// Returns the implementation diagnostic text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns labels in their compiler-defined order.
    #[must_use]
    pub fn labels(&self) -> &[CanonicalLabel] {
        &self.labels
    }
}

/// Ordered canonical artifacts for all currently planned compiler phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalDumps {
    schema_version: u32,
    language_version: String,
    artifacts: Vec<CanonicalArtifact>,
}

impl CanonicalDumps {
    /// Returns the dump schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the source-language version represented by these artifacts.
    #[must_use]
    pub fn language_version(&self) -> &str {
        &self.language_version
    }

    /// Returns all phase artifacts in the fixed canonical order.
    #[must_use]
    pub fn artifacts(&self) -> &[CanonicalArtifact] {
        &self.artifacts
    }

    /// Returns one required phase artifact.
    ///
    /// Every [`CanonicalDumps`] value is constructed with exactly one entry for
    /// every [`CanonicalPhase`], so this lookup cannot be absent.
    #[must_use]
    pub fn artifact(&self, phase: CanonicalPhase) -> &CanonicalArtifact {
        self.artifacts
            .iter()
            .find(|artifact| artifact.phase == phase)
            .unwrap_or_else(|| unreachable!("canonical phase table is complete"))
    }

    /// Serializes the entire ordered phase collection as canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the in-memory schema cannot be encoded.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Structured result of a complete explicit compiler-core invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerOutput {
    sources: SourceMap,
    checked: CheckResult,
    mir: Option<MirProgram>,
    diagnostics: Vec<CanonicalDiagnostic>,
    dumps: CanonicalDumps,
}

impl CompilerOutput {
    /// Returns true when semantic analysis and MIR lowering both succeeded.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.checked.is_ok() && self.mir.is_some()
    }

    /// Returns the source map whose stable ordinals back all output spans.
    #[must_use]
    pub const fn sources(&self) -> &SourceMap {
        &self.sources
    }

    /// Returns the existing structured front-end result.
    #[must_use]
    pub const fn checked(&self) -> &CheckResult {
        &self.checked
    }

    /// Returns complete MIR when the source graph passed semantic analysis.
    #[must_use]
    pub const fn mir(&self) -> Option<&MirProgram> {
        self.mir.as_ref()
    }

    /// Returns diagnostics normalized to source discovery ordinals and byte spans.
    #[must_use]
    pub fn diagnostics(&self) -> &[CanonicalDiagnostic] {
        &self.diagnostics
    }

    /// Returns every versioned canonical phase artifact.
    #[must_use]
    pub const fn dumps(&self) -> &CanonicalDumps {
        &self.dumps
    }
}

/// Compiles a complete explicit source collection without consulting Host state.
///
/// Source identities must already be normalized portable relative paths. The
/// function does not read files, environment variables, clocks, randomness,
/// process state, or the current working directory.
///
/// # Errors
///
/// Returns [`CompileError`] for an invalid input collection, a source-less
/// session failure, an internal MIR invariant failure, or serialization failure.
pub fn compile(input: &CompilerInput) -> Result<CompilerOutput, CompileError> {
    let sources = validated_sources(input)?;
    let mut provider = MemorySourceProvider::default();
    for source in sources {
        let _ = provider.insert(source.identity(), source.content().as_bytes());
    }
    let session = CompilerSession::build(provider, Path::new(input.entry()))?;
    compile_session(&session)
}

/// Produces the same structured output for a Host-loaded compiler session.
///
/// This adapter exists for shells such as `nexac`; deterministic compiler code
/// should prefer [`compile`] and explicit inputs.
///
/// # Errors
///
/// Returns [`CompileError`] for a MIR invariant or canonical serialization failure.
pub fn compile_session<P>(session: &CompilerSession<P>) -> Result<CompilerOutput, CompileError> {
    let checked = check_session(session);
    let mir = checked.typed().map(lower_mir).transpose()?;
    let mut diagnostics = checked
        .diagnostics()
        .iter()
        .map(canonical_diagnostic)
        .collect::<Result<Vec<_>, _>>()?;
    diagnostics.sort_by(canonical_diagnostic_order);
    let dumps = canonical_dumps(session, &checked, mir.as_ref(), &diagnostics)?;

    Ok(CompilerOutput {
        sources: session.sources().clone(),
        checked,
        mir,
        diagnostics,
        dumps,
    })
}

fn validated_sources(input: &CompilerInput) -> Result<Vec<&CompilerSource>, CompileError> {
    validate_identity(input.entry())?;
    let mut sources = input.sources().iter().collect::<Vec<_>>();
    sources.sort_by(|left, right| left.identity().cmp(right.identity()));

    let mut identities = BTreeSet::new();
    for source in &sources {
        validate_identity(source.identity())?;
        if !identities.insert(source.identity()) {
            return Err(CompileError::DuplicateSource {
                identity: source.identity().to_owned(),
            });
        }
    }
    if !identities.contains(input.entry()) {
        return Err(CompileError::MissingEntry {
            entry: input.entry().to_owned(),
        });
    }
    Ok(sources)
}

fn validate_identity(identity: &str) -> Result<(), CompileError> {
    let invalid = |reason| CompileError::InvalidSourceIdentity {
        identity: identity.to_owned(),
        reason,
    };
    if identity.is_empty() {
        return Err(invalid("identity must not be empty"));
    }
    if identity.starts_with('/') || identity.contains(['\\', '\0', '?', '#', ':']) {
        return Err(invalid("identity must be a portable relative path"));
    }
    if identity
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(invalid("identity must already be lexically normalized"));
    }
    if !source_extension_is_supported(identity) {
        return Err(invalid("identity must end in `.ft` or `.nexa`"));
    }
    Ok(())
}

fn source_extension_is_supported(identity: &str) -> bool {
    [".ft", ".nexa"].iter().any(|extension| {
        identity
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(extension))
            .is_some_and(|stem| !stem.is_empty())
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseEnvelope<T> {
    schema_version: u32,
    phase: CanonicalPhase,
    value: T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalModuleTokens {
    module: u32,
    tokens: Vec<CanonicalToken>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalToken {
    kind_id: u16,
    start: u32,
    end: u32,
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalModuleCst {
    module: u32,
    elements: Vec<CanonicalCstElement>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalCstElement {
    depth: u32,
    element: CanonicalCstElementKind,
    kind_id: u16,
    start: u32,
    end: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum CanonicalCstElementKind {
    Node,
    Token,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalDiagnosticShape<'a> {
    code: &'a str,
    severity: CanonicalSeverity,
    labels: Vec<CanonicalLabelShape>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalLabelShape {
    style: CanonicalLabelStyle,
    file: u32,
    start: u32,
    end: u32,
}

fn canonical_dumps<P>(
    session: &CompilerSession<P>,
    checked: &CheckResult,
    mir: Option<&MirProgram>,
    diagnostics: &[CanonicalDiagnostic],
) -> Result<CanonicalDumps, CompileError> {
    let tokens = session
        .modules()
        .iter()
        .map(|module| {
            Ok(CanonicalModuleTokens {
                module: u32::try_from(module.id().index())
                    .map_err(|_| CompileError::CanonicalOffsetOverflow)?,
                tokens: module
                    .parse()
                    .tokens()
                    .iter()
                    .map(|token| {
                        Ok(CanonicalToken {
                            kind_id: token.kind() as u16,
                            start: u32::try_from(token.range().start())
                                .map_err(|_| CompileError::CanonicalOffsetOverflow)?,
                            end: u32::try_from(token.range().end())
                                .map_err(|_| CompileError::CanonicalOffsetOverflow)?,
                            text: token.text().to_owned(),
                        })
                    })
                    .collect::<Result<Vec<_>, CompileError>>()?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let cst = session
        .modules()
        .iter()
        .map(|module| {
            let mut depth = 0;
            let mut elements = Vec::new();
            for event in module.parse().syntax().preorder_with_tokens() {
                match event {
                    WalkEvent::Enter(element) => {
                        let range = element.text_range();
                        let kind = element.kind();
                        let (element_kind, text) = match element {
                            NodeOrToken::Node(_) => (CanonicalCstElementKind::Node, None),
                            NodeOrToken::Token(token) => (
                                CanonicalCstElementKind::Token,
                                Some(token.text().to_owned()),
                            ),
                        };
                        elements.push(CanonicalCstElement {
                            depth,
                            element: element_kind,
                            kind_id: kind as u16,
                            start: u32::from(range.start()),
                            end: u32::from(range.end()),
                            text,
                        });
                        depth += 1;
                    }
                    WalkEvent::Leave(_) => depth -= 1,
                }
            }
            Ok(CanonicalModuleCst {
                module: u32::try_from(module.id().index())
                    .map_err(|_| CompileError::CanonicalOffsetOverflow)?,
                elements,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let diagnostic_shapes = diagnostics
        .iter()
        .map(|diagnostic| CanonicalDiagnosticShape {
            code: diagnostic.code(),
            severity: diagnostic.severity(),
            labels: diagnostic
                .labels()
                .iter()
                .map(|label| CanonicalLabelShape {
                    style: label.style(),
                    file: label.file(),
                    start: label.start(),
                    end: label.end(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    let mut artifacts = vec![
        CanonicalArtifact::produced(
            CanonicalPhase::Tokens,
            serialize_phase(CanonicalPhase::Tokens, tokens)?,
        ),
        CanonicalArtifact::produced(
            CanonicalPhase::Cst,
            serialize_phase(CanonicalPhase::Cst, cst)?,
        ),
        CanonicalArtifact::produced(
            CanonicalPhase::Diagnostics,
            serialize_phase(CanonicalPhase::Diagnostics, diagnostic_shapes)?,
        ),
    ];
    if let Some(typed) = checked.typed() {
        let value = canonical::hir_value(typed)?;
        artifacts.push(CanonicalArtifact::produced(
            CanonicalPhase::Hir,
            serialize_phase(CanonicalPhase::Hir, value)?,
        ));
    } else {
        artifacts.push(CanonicalArtifact::blocked(CanonicalPhase::Hir));
    }
    if let Some(mir) = mir {
        let value = canonical::mir_value(mir)?;
        artifacts.push(CanonicalArtifact::produced(
            CanonicalPhase::Mir,
            serialize_phase(CanonicalPhase::Mir, value)?,
        ));
    } else {
        artifacts.push(CanonicalArtifact::blocked(CanonicalPhase::Mir));
    }
    artifacts.push(CanonicalArtifact::unavailable(
        CanonicalPhase::Nir,
        NIR_PLANNED_VERSION,
    ));

    Ok(CanonicalDumps {
        schema_version: CANONICAL_DUMP_SCHEMA_VERSION,
        language_version: LanguageVersion::Nexa1_0.as_str().to_owned(),
        artifacts,
    })
}

fn serialize_phase<T>(phase: CanonicalPhase, value: T) -> Result<String, serde_json::Error>
where
    T: Serialize,
{
    serde_json::to_string(&PhaseEnvelope {
        schema_version: CANONICAL_DUMP_SCHEMA_VERSION,
        phase,
        value,
    })
}

fn canonical_diagnostic(diagnostic: &Diagnostic) -> Result<CanonicalDiagnostic, CompileError> {
    Ok(CanonicalDiagnostic {
        code: diagnostic.code().as_str().to_owned(),
        severity: match diagnostic.severity() {
            Severity::Error => CanonicalSeverity::Error,
            Severity::Warning => CanonicalSeverity::Warning,
        },
        message: diagnostic.message().to_owned(),
        labels: diagnostic
            .labels()
            .iter()
            .map(|label| {
                let span = label.span();
                Ok(CanonicalLabel {
                    style: match label.style() {
                        LabelStyle::Primary => CanonicalLabelStyle::Primary,
                        LabelStyle::Secondary => CanonicalLabelStyle::Secondary,
                    },
                    file: span.file().raw(),
                    start: u32::try_from(span.range().start())
                        .map_err(|_| CompileError::CanonicalOffsetOverflow)?,
                    end: u32::try_from(span.range().end())
                        .map_err(|_| CompileError::CanonicalOffsetOverflow)?,
                    message: label.message().to_owned(),
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?,
    })
}

fn canonical_diagnostic_order(
    left: &CanonicalDiagnostic,
    right: &CanonicalDiagnostic,
) -> std::cmp::Ordering {
    let location = |diagnostic: &CanonicalDiagnostic| {
        diagnostic
            .labels
            .first()
            .map_or((u32::MAX, u32::MAX, u32::MAX), |label| {
                (label.file, label.start, label.end)
            })
    };
    location(left)
        .cmp(&location(right))
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| severity_order(left.severity).cmp(&severity_order(right.severity)))
        .then_with(|| canonical_labels_order(&left.labels, &right.labels))
        .then_with(|| left.message.cmp(&right.message))
}

const fn severity_order(severity: CanonicalSeverity) -> u8 {
    match severity {
        CanonicalSeverity::Error => 0,
        CanonicalSeverity::Warning => 1,
    }
}

fn canonical_labels_order(left: &[CanonicalLabel], right: &[CanonicalLabel]) -> std::cmp::Ordering {
    left.iter()
        .map(canonical_label_key)
        .cmp(right.iter().map(canonical_label_key))
}

fn canonical_label_key(label: &CanonicalLabel) -> (u8, u32, u32, u32, &str) {
    let style = match label.style {
        CanonicalLabelStyle::Primary => 0,
        CanonicalLabelStyle::Secondary => 1,
    };
    (style, label.file, label.start, label.end, &label.message)
}

pub(crate) fn span_position(span: SourceSpan) -> (u32, usize, usize) {
    (span.file().raw(), span.range().start(), span.range().end())
}
