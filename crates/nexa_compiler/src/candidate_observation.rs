//! Strict, compare-only observation boundary for compiler adapters.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt::Formatter;
use std::marker::PhantomData;

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::core::{
    validated_sources, CanonicalArtifactState, CanonicalLabelStyle, CanonicalPhase,
    CanonicalSeverity, CompilerInput, CompilerOutput, CANONICAL_DUMP_SCHEMA_VERSION,
};

/// Schema version for the complete untrusted candidate observation.
pub const CANDIDATE_OBSERVATION_SCHEMA_VERSION: u32 = 1;

/// Compiler identity used by a Stage 0-executed Futao candidate.
pub const FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION: &str = "futao-bootstrap-candidate";

/// Maximum encoded candidate observation size: 128 MiB.
pub const CANDIDATE_OBSERVATION_MAX_BYTES: usize = 128 * 1024 * 1024;

/// Maximum encoded content for one compiler phase: 64 MiB.
pub const CANDIDATE_PHASE_CONTENT_MAX_BYTES: usize = 64 * 1024 * 1024;

/// Maximum JSON nesting accepted at either protocol boundary.
pub const CANDIDATE_OBSERVATION_MAX_JSON_DEPTH: usize = 128;

/// Maximum source files represented by one candidate observation.
pub const CANDIDATE_OBSERVATION_MAX_SOURCES: usize = 1_024;

/// Maximum UTF-8 bytes in one source supplied to the candidate.
pub const CANDIDATE_OBSERVATION_MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// Maximum aggregate UTF-8 source bytes supplied to the candidate.
pub const CANDIDATE_OBSERVATION_MAX_TOTAL_SOURCE_BYTES: usize = 64 * 1024 * 1024;

/// Maximum UTF-8 bytes in one portable source identity.
pub const CANDIDATE_OBSERVATION_MAX_SOURCE_IDENTITY_BYTES: usize = 1_024;

/// Maximum structured diagnostics in one observation.
pub const CANDIDATE_OBSERVATION_MAX_DIAGNOSTICS: usize = 16_384;

/// Maximum labels attached to one structured diagnostic.
pub const CANDIDATE_OBSERVATION_MAX_LABELS_PER_DIAGNOSTIC: usize = 256;

/// Maximum labels across one complete observation.
pub const CANDIDATE_OBSERVATION_MAX_TOTAL_LABELS: usize = 65_536;

/// Maximum UTF-8 bytes in one diagnostic or label message.
pub const CANDIDATE_OBSERVATION_MAX_MESSAGE_BYTES: usize = 64 * 1024;

/// Identifies one compiler implementation participating in differential tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilerImplementation {
    /// The Rust Stage 0 reference compiler.
    RustReference,
    /// Futao source executed by Stage 0 as the pre-Stage-1 candidate.
    FutaoBootstrapCandidate,
}

impl CompilerImplementation {
    /// Returns the manifest spelling of this implementation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustReference => "rust-reference",
            Self::FutaoBootstrapCandidate => FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION,
        }
    }
}

/// Root result status of a complete candidate compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateCompilationStatus {
    /// No error diagnostic blocked typed HIR or MIR production.
    Accepted,
    /// At least one error diagnostic blocked HIR and every later phase.
    Rejected,
}

/// Validated availability of one canonical compiler phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidatePhaseStatus {
    /// Canonical phase bytes were produced.
    Produced,
    /// Error diagnostics prevented this phase from running.
    Blocked,
    /// The accepted input needs a later explicitly identified NIR slice.
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum CompilerArtifactState {
    Produced { content: String },
    SkippedDueToDiagnostics,
    Deferred { content: String },
}

/// One validated, compare-only compiler phase envelope.
///
/// The transport state, header, JSON depth, and outer payload shape are
/// validated here. HIR, MIR, and NIR content remains opaque canonical bytes;
/// only their later phase-specific verifiers may construct typed values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerArtifactObservation {
    phase: CanonicalPhase,
    artifact: CompilerArtifactState,
}

impl CompilerArtifactObservation {
    fn from_reference(phase: CanonicalPhase, artifact: &CanonicalArtifactState) -> Self {
        let artifact = match artifact {
            CanonicalArtifactState::Produced { content } => CompilerArtifactState::Produced {
                content: content.clone(),
            },
            CanonicalArtifactState::SkippedDueToDiagnostics => {
                CompilerArtifactState::SkippedDueToDiagnostics
            }
            CanonicalArtifactState::Deferred { content } => CompilerArtifactState::Deferred {
                content: content.clone(),
            },
        };
        Self { phase, artifact }
    }

    /// Returns the represented compiler phase.
    #[must_use]
    pub const fn phase(&self) -> CanonicalPhase {
        self.phase
    }

    /// Returns the validated availability state.
    #[must_use]
    pub const fn status(&self) -> CandidatePhaseStatus {
        match self.artifact {
            CompilerArtifactState::Produced { .. } => CandidatePhaseStatus::Produced,
            CompilerArtifactState::SkippedDueToDiagnostics => CandidatePhaseStatus::Blocked,
            CompilerArtifactState::Deferred { .. } => CandidatePhaseStatus::Deferred,
        }
    }

    /// Returns produced or deferred canonical phase bytes.
    #[must_use]
    pub fn content(&self) -> Option<&str> {
        match &self.artifact {
            CompilerArtifactState::Produced { content }
            | CompilerArtifactState::Deferred { content } => Some(content),
            CompilerArtifactState::SkippedDueToDiagnostics => None,
        }
    }
}

/// Immutable comparison surface returned by every complete compiler adapter.
///
/// This type deliberately contains no Rust typed HIR, MIR, or verified NIR.
/// Rust creates it from trusted compiler output; Futao can create it only by
/// passing the strict candidate loader. Equality also binds source discovery
/// ordinals and exact source bytes, so an observation cannot be reused for a
/// different input with the same paths and lengths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerObservation {
    implementation: CompilerImplementation,
    schema_version: u32,
    language_version: String,
    compilation_profile: String,
    sources: Vec<CompilerSourceObservation>,
    artifacts: Vec<CompilerArtifactObservation>,
}

impl CompilerObservation {
    /// Projects trusted Rust output onto the compare-only adapter boundary.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CompileError`] when `output` was not produced from the
    /// supplied explicit input or the input exceeds the candidate protocol.
    pub fn from_rust_output(
        input: &CompilerInput,
        output: &CompilerOutput,
    ) -> Result<Self, crate::CompileError> {
        let dumps = output.dumps();
        if dumps.language_version() != input.options().language_version().as_str()
            || dumps.compilation_profile() != input.options().profile().as_str()
        {
            return Err(crate::CompileError::ObservationInputMismatch {
                message: "language version or compilation profile differs".to_owned(),
            });
        }
        let sources = reference_sources(input, output)?;
        let artifacts = dumps
            .artifacts()
            .iter()
            .map(|artifact| {
                CompilerArtifactObservation::from_reference(artifact.phase(), artifact.state())
            })
            .collect();
        Ok(Self {
            implementation: CompilerImplementation::RustReference,
            schema_version: dumps.schema_version(),
            language_version: dumps.language_version().to_owned(),
            compilation_profile: dumps.compilation_profile().to_owned(),
            sources,
            artifacts,
        })
    }

    /// Returns the implementation that produced this validated observation.
    #[must_use]
    pub const fn implementation(&self) -> CompilerImplementation {
        self.implementation
    }

    /// Returns the canonical phase schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the selected source-language version.
    #[must_use]
    pub fn language_version(&self) -> &str {
        &self.language_version
    }

    /// Returns the selected compiler profile.
    #[must_use]
    pub fn compilation_profile(&self) -> &str {
        &self.compilation_profile
    }

    /// Returns the source table bound to canonical file ordinals.
    #[must_use]
    pub fn sources(&self) -> &[CompilerSourceObservation] {
        &self.sources
    }

    /// Returns all six artifacts in canonical phase order.
    #[must_use]
    pub fn artifacts(&self) -> &[CompilerArtifactObservation] {
        &self.artifacts
    }

    /// Returns one required canonical artifact.
    #[must_use]
    pub fn artifact(&self, phase: CanonicalPhase) -> &CompilerArtifactObservation {
        self.artifacts
            .iter()
            .find(|artifact| artifact.phase == phase)
            .unwrap_or_else(|| unreachable!("validated compiler observations contain six phases"))
    }
}

/// One source-table entry validated against exact explicit compiler input.
///
/// Its private digest binds source bytes without putting Host paths or source
/// text into canonical phase artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerSourceObservation {
    file: u32,
    identity: String,
    byte_length: usize,
    content_digest: [u8; 32],
}

impl CompilerSourceObservation {
    fn new(file: u32, identity: String, content: &str) -> Self {
        Self {
            file,
            identity,
            byte_length: content.len(),
            content_digest: Sha256::digest(content.as_bytes()).into(),
        }
    }

    /// Returns the dense source ordinal used by candidate spans.
    #[must_use]
    pub const fn file(&self) -> u32 {
        self.file
    }

    /// Returns the portable logical source identity.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Returns the exact UTF-8 byte length bound to this source ordinal.
    #[must_use]
    pub const fn byte_length(&self) -> usize {
        self.byte_length
    }
}

fn reference_sources(
    input: &CompilerInput,
    output: &CompilerOutput,
) -> Result<Vec<CompilerSourceObservation>, crate::CompileError> {
    let input_sources = validated_sources(input)?;
    validate_input_resource_bounds(&input_sources)?;
    let mut observed = BTreeSet::new();
    let mut sources = Vec::with_capacity(input_sources.len());

    for (expected_file, file) in output.sources().files().iter().enumerate() {
        let expected_file = u32::try_from(expected_file).map_err(|_| {
            crate::CompileError::ObservationInputMismatch {
                message: "reference source ordinal exceeded u32".to_owned(),
            }
        })?;
        if file.id().raw() != expected_file {
            return Err(crate::CompileError::ObservationInputMismatch {
                message: "reference source ordinals are not dense".to_owned(),
            });
        }
        let identity = file
            .key()
            .and_then(|key| key.as_path().to_str())
            .ok_or_else(|| crate::CompileError::ObservationInputMismatch {
                message: format!("reference source file {expected_file} has no portable identity"),
            })?;
        let Some(source) = input_sources
            .iter()
            .copied()
            .find(|source| source.identity() == identity)
        else {
            return Err(crate::CompileError::ObservationInputMismatch {
                message: format!("reference source `{identity}` is absent from explicit input"),
            });
        };
        if source.content() != file.text() || !observed.insert(identity) {
            return Err(crate::CompileError::ObservationInputMismatch {
                message: format!("reference source `{identity}` is duplicated or has other bytes"),
            });
        }
        sources.push(CompilerSourceObservation::new(
            expected_file,
            identity.to_owned(),
            source.content(),
        ));
    }

    for source in input_sources {
        if observed.insert(source.identity()) {
            let file = u32::try_from(sources.len()).map_err(|_| {
                crate::CompileError::ObservationInputMismatch {
                    message: "explicit source ordinal exceeded u32".to_owned(),
                }
            })?;
            sources.push(CompilerSourceObservation::new(
                file,
                source.identity().to_owned(),
                source.content(),
            ));
        }
    }
    if sources
        .first()
        .map_or(true, |source| source.identity != input.entry())
    {
        return Err(crate::CompileError::ObservationInputMismatch {
            message: "reference source file 0 is not the explicit entry".to_owned(),
        });
    }
    Ok(sources)
}

/// One source label validated against a candidate source table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateLabelObservation {
    style: CanonicalLabelStyle,
    file: u32,
    start: u32,
    end: u32,
    message: String,
}

impl CandidateLabelObservation {
    /// Returns the label role.
    #[must_use]
    pub const fn style(&self) -> CanonicalLabelStyle {
        self.style
    }

    /// Returns the source ordinal.
    #[must_use]
    pub const fn file(&self) -> u32 {
        self.file
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

    /// Returns presentation text excluded from canonical comparison.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// One complete structured diagnostic from a validated candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateDiagnosticObservation {
    code: String,
    severity: CanonicalSeverity,
    message: String,
    labels: Vec<CandidateLabelObservation>,
}

impl CandidateDiagnosticObservation {
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

    /// Returns presentation text excluded from canonical comparison.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns labels in compiler-defined order.
    #[must_use]
    pub fn labels(&self) -> &[CandidateLabelObservation] {
        &self.labels
    }
}

/// Complete candidate protocol after strict decoding and validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateObservation {
    status: CandidateCompilationStatus,
    diagnostics: Vec<CandidateDiagnosticObservation>,
    compiler: CompilerObservation,
}

impl CandidateObservation {
    /// Loads an untrusted candidate JSON observation against explicit input.
    ///
    /// # Errors
    ///
    /// Fails closed on resource limits, malformed or unknown schema fields,
    /// identity/input drift, invalid source spans, diagnostic drift, phase
    /// ordering, or an impossible produced/blocked/deferred state chain.
    pub fn load(bytes: &[u8], input: &CompilerInput) -> Result<Self, CandidateObservationError> {
        let input_sources =
            validated_sources(input).map_err(|source| CandidateObservationError {
                code: CandidateObservationErrorCode::InputMismatch,
                message: "explicit compiler input is invalid".to_owned(),
                source: Some(Box::new(source)),
            })?;
        validate_input_resource_bounds(&input_sources)?;
        validate_json_resource(
            bytes,
            CANDIDATE_OBSERVATION_MAX_BYTES,
            "candidate observation",
        )?;
        let raw: RawCandidateObservation = serde_json::from_slice(bytes).map_err(|source| {
            let code = if source.to_string().contains(RESOURCE_LIMIT_MARKER) {
                CandidateObservationErrorCode::ResourceLimit
            } else {
                CandidateObservationErrorCode::InvalidJson
            };
            CandidateObservationError {
                code,
                message: "candidate observation does not match strict schema 1 JSON".to_owned(),
                source: Some(Box::new(source)),
            }
        })?;
        validate_candidate(raw, input, &input_sources)
    }

    /// Returns the candidate observation schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        CANDIDATE_OBSERVATION_SCHEMA_VERSION
    }

    /// Returns the fixed pre-Stage-1 candidate implementation identity.
    #[must_use]
    pub const fn implementation(&self) -> &'static str {
        FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION
    }

    /// Returns the selected source-language version.
    #[must_use]
    pub fn language_version(&self) -> &str {
        self.compiler.language_version()
    }

    /// Returns the selected compiler profile.
    #[must_use]
    pub fn compilation_profile(&self) -> &str {
        self.compiler.compilation_profile()
    }

    /// Returns whether this candidate compilation was accepted or rejected.
    #[must_use]
    pub const fn status(&self) -> CandidateCompilationStatus {
        self.status
    }

    /// Returns the complete dense source table.
    #[must_use]
    pub fn sources(&self) -> &[CompilerSourceObservation] {
        self.compiler.sources()
    }

    /// Returns validated structured diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[CandidateDiagnosticObservation] {
        &self.diagnostics
    }

    /// Returns all six validated compare-only artifacts.
    #[must_use]
    pub fn artifacts(&self) -> &[CompilerArtifactObservation] {
        self.compiler.artifacts()
    }

    /// Converts this validated candidate into the only value accepted by the
    /// complete differential adapter boundary.
    #[must_use]
    pub fn into_compiler_observation(self) -> CompilerObservation {
        self.compiler
    }
}

/// Stable category for candidate observation rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateObservationErrorCode {
    /// A byte, collection, string, source, or nesting budget was exceeded.
    ResourceLimit,
    /// JSON was malformed, duplicated a field, or contained an unknown field.
    InvalidJson,
    /// The root schema version is not supported.
    UnsupportedSchema,
    /// The implementation is not the Stage 0-executed Futao candidate.
    InvalidImplementation,
    /// Language/profile or the supplied compiler input was inconsistent.
    InputMismatch,
    /// The dense source table did not match the complete explicit input.
    InvalidSourceTable,
    /// A structured diagnostic, label, span, code, or order was invalid.
    InvalidDiagnostic,
    /// The six phase entries or their state chain were invalid.
    InvalidPhaseTable,
    /// Canonical phase content had an invalid envelope or diagnostic payload.
    InvalidPhaseContent,
}

impl CandidateObservationErrorCode {
    /// Returns a stable machine-readable rejection category.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceLimit => "resource-limit",
            Self::InvalidJson => "invalid-json",
            Self::UnsupportedSchema => "unsupported-schema",
            Self::InvalidImplementation => "invalid-implementation",
            Self::InputMismatch => "input-mismatch",
            Self::InvalidSourceTable => "invalid-source-table",
            Self::InvalidDiagnostic => "invalid-diagnostic",
            Self::InvalidPhaseTable => "invalid-phase-table",
            Self::InvalidPhaseContent => "invalid-phase-content",
        }
    }
}

/// Deterministic failure returned before untrusted candidate data can compare.
#[derive(Debug, Error)]
#[error("{}: {message}", code.as_str())]
pub struct CandidateObservationError {
    code: CandidateObservationErrorCode,
    message: String,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl CandidateObservationError {
    /// Returns the stable failure category.
    #[must_use]
    pub const fn code(&self) -> CandidateObservationErrorCode {
        self.code
    }

    /// Returns deterministic rejection context.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

const RESOURCE_LIMIT_MARKER: &str = "candidate-resource-limit";
const PROTOCOL_TAG_MAX_BYTES: usize = 1_024;

struct BoundedString<const MAXIMUM: usize>(String);

impl<const MAXIMUM: usize> BoundedString<MAXIMUM> {
    fn as_str(&self) -> &str {
        &self.0
    }

    fn into_string(self) -> String {
        self.0
    }
}

impl<'de, const MAXIMUM: usize> Deserialize<'de> for BoundedString<MAXIMUM> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StringVisitor<const MAXIMUM: usize>;

        impl<const MAXIMUM: usize> Visitor<'_> for StringVisitor<MAXIMUM> {
            type Value = BoundedString<MAXIMUM>;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "a UTF-8 string of at most {MAXIMUM} bytes")
            }

            fn visit_borrowed_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(value)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() > MAXIMUM {
                    return Err(E::custom(format_args!(
                        "{RESOURCE_LIMIT_MARKER}: string bytes {} exceed {MAXIMUM}",
                        value.len()
                    )));
                }
                Ok(BoundedString(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() > MAXIMUM {
                    return Err(E::custom(format_args!(
                        "{RESOURCE_LIMIT_MARKER}: string bytes {} exceed {MAXIMUM}",
                        value.len()
                    )));
                }
                Ok(BoundedString(value))
            }
        }

        deserializer.deserialize_str(StringVisitor::<MAXIMUM>)
    }
}

struct BoundedVec<T, const MAXIMUM: usize>(Vec<T>);

impl<T, const MAXIMUM: usize> BoundedVec<T, MAXIMUM> {
    fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<'de, T, const MAXIMUM: usize> Deserialize<'de> for BoundedVec<T, MAXIMUM>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct VecVisitor<T, const MAXIMUM: usize>(PhantomData<T>);

        impl<'de, T, const MAXIMUM: usize> Visitor<'de> for VecVisitor<T, MAXIMUM>
        where
            T: Deserialize<'de>,
        {
            type Value = BoundedVec<T, MAXIMUM>;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "an array of at most {MAXIMUM} entries")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                if sequence.size_hint().is_some_and(|size| size > MAXIMUM) {
                    return Err(de::Error::custom(format_args!(
                        "{RESOURCE_LIMIT_MARKER}: array count exceeds {MAXIMUM}"
                    )));
                }
                let mut values =
                    Vec::with_capacity(sequence.size_hint().unwrap_or_default().min(MAXIMUM));
                while let Some(value) = sequence.next_element()? {
                    if values.len() == MAXIMUM {
                        return Err(de::Error::custom(format_args!(
                            "{RESOURCE_LIMIT_MARKER}: array count exceeds {MAXIMUM}"
                        )));
                    }
                    values.push(value);
                }
                Ok(BoundedVec(values))
            }
        }

        deserializer.deserialize_seq(VecVisitor::<T, MAXIMUM>(PhantomData))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawCandidateObservation {
    schema_version: u32,
    implementation: BoundedString<PROTOCOL_TAG_MAX_BYTES>,
    language_version: BoundedString<PROTOCOL_TAG_MAX_BYTES>,
    compilation_profile: BoundedString<PROTOCOL_TAG_MAX_BYTES>,
    status: RawCompilationStatus,
    sources: BoundedVec<RawSource, CANDIDATE_OBSERVATION_MAX_SOURCES>,
    diagnostics: BoundedVec<RawDiagnostic, CANDIDATE_OBSERVATION_MAX_DIAGNOSTICS>,
    artifacts: BoundedVec<RawArtifact, 6>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RawCompilationStatus {
    Accepted,
    Rejected,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawSource {
    file: u32,
    identity: BoundedString<CANDIDATE_OBSERVATION_MAX_SOURCE_IDENTITY_BYTES>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawDiagnostic {
    code: String,
    severity: RawSeverity,
    message: BoundedString<CANDIDATE_OBSERVATION_MAX_MESSAGE_BYTES>,
    labels: BoundedVec<RawLabel, CANDIDATE_OBSERVATION_MAX_LABELS_PER_DIAGNOSTIC>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum RawSeverity {
    Error,
    Warning,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawLabel {
    style: RawLabelStyle,
    file: u32,
    start: u32,
    end: u32,
    message: BoundedString<CANDIDATE_OBSERVATION_MAX_MESSAGE_BYTES>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum RawLabelStyle {
    Primary,
    Secondary,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawArtifact {
    phase: BoundedString<PROTOCOL_TAG_MAX_BYTES>,
    artifact: RawArtifactState,
}

#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
enum RawArtifactState {
    Produced {
        content: BoundedString<CANDIDATE_PHASE_CONTENT_MAX_BYTES>,
    },
    SkippedDueToDiagnostics,
    Deferred {
        content: BoundedString<CANDIDATE_PHASE_CONTENT_MAX_BYTES>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BorrowedPhaseEnvelope<'a> {
    schema_version: u32,
    phase: &'a str,
    #[serde(borrow)]
    value: &'a RawValue,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializablePhaseEnvelope<T> {
    schema_version: u32,
    phase: &'static str,
    value: T,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawDiagnosticShape {
    code: String,
    severity: RawSeverity,
    labels: Vec<RawLabelShape>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawLabelShape {
    style: RawLabelStyle,
    file: u32,
    start: u32,
    end: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BorrowedDeferredEnvelope<'a> {
    schema_version: u32,
    phase: &'a str,
    #[serde(borrow)]
    value: RawDeferredReason<'a>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawDeferredReason<'a> {
    reason_code: &'a str,
    planned_phase: &'a str,
}

fn validate_candidate(
    raw: RawCandidateObservation,
    input: &CompilerInput,
    input_sources: &[&crate::CompilerSource],
) -> Result<CandidateObservation, CandidateObservationError> {
    if raw.schema_version != CANDIDATE_OBSERVATION_SCHEMA_VERSION {
        return candidate_error(
            CandidateObservationErrorCode::UnsupportedSchema,
            format!(
                "candidate schema {} is unsupported; expected {}",
                raw.schema_version, CANDIDATE_OBSERVATION_SCHEMA_VERSION
            ),
        );
    }
    if raw.implementation.as_str() != FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION {
        return candidate_error(
            CandidateObservationErrorCode::InvalidImplementation,
            format!(
                "implementation `{}` is not the Stage 0 Futao candidate",
                raw.implementation.as_str()
            ),
        );
    }

    if raw.language_version.as_str() != input.options().language_version().as_str()
        || raw.compilation_profile.as_str() != input.options().profile().as_str()
    {
        return candidate_error(
            CandidateObservationErrorCode::InputMismatch,
            "candidate languageVersion or compilationProfile differs from explicit input",
        );
    }

    let (sources, source_texts) = validate_sources(raw.sources.into_vec(), input, input_sources)?;
    let diagnostics = validate_diagnostics(raw.diagnostics.into_vec(), &source_texts)?;
    let has_error = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == CanonicalSeverity::Error);
    let status = match raw.status {
        RawCompilationStatus::Accepted if !has_error => CandidateCompilationStatus::Accepted,
        RawCompilationStatus::Rejected if has_error => CandidateCompilationStatus::Rejected,
        RawCompilationStatus::Accepted => {
            return candidate_error(
                CandidateObservationErrorCode::InvalidDiagnostic,
                "accepted observation contains an error diagnostic",
            );
        }
        RawCompilationStatus::Rejected => {
            return candidate_error(
                CandidateObservationErrorCode::InvalidDiagnostic,
                "rejected observation contains no error diagnostic",
            );
        }
    };
    let artifacts = validate_artifacts(raw.artifacts.into_vec(), status, &diagnostics)?;
    let compiler = CompilerObservation {
        implementation: CompilerImplementation::FutaoBootstrapCandidate,
        schema_version: CANONICAL_DUMP_SCHEMA_VERSION,
        language_version: raw.language_version.into_string(),
        compilation_profile: raw.compilation_profile.into_string(),
        sources,
        artifacts,
    };
    Ok(CandidateObservation {
        status,
        diagnostics,
        compiler,
    })
}

fn validate_input_resource_bounds(
    sources: &[&crate::CompilerSource],
) -> Result<(), CandidateObservationError> {
    ensure_at_most(
        sources.len(),
        CANDIDATE_OBSERVATION_MAX_SOURCES,
        "source count",
    )?;
    let mut total = 0_usize;
    for source in sources {
        ensure_at_most(
            source.identity().len(),
            CANDIDATE_OBSERVATION_MAX_SOURCE_IDENTITY_BYTES,
            "source identity bytes",
        )?;
        ensure_at_most(
            source.content().len(),
            CANDIDATE_OBSERVATION_MAX_SOURCE_BYTES,
            "single source bytes",
        )?;
        total = total
            .checked_add(source.content().len())
            .ok_or_else(|| resource_error("total source byte count overflowed"))?;
    }
    ensure_at_most(
        total,
        CANDIDATE_OBSERVATION_MAX_TOTAL_SOURCE_BYTES,
        "total source bytes",
    )
}

fn validate_sources<'a>(
    raw_sources: Vec<RawSource>,
    input: &CompilerInput,
    input_sources: &[&'a crate::CompilerSource],
) -> Result<(Vec<CompilerSourceObservation>, Vec<&'a str>), CandidateObservationError> {
    if raw_sources.len() != input_sources.len() {
        return candidate_error(
            CandidateObservationErrorCode::InvalidSourceTable,
            format!(
                "candidate source table has {} entries; explicit input has {}",
                raw_sources.len(),
                input_sources.len()
            ),
        );
    }
    if raw_sources.first().map_or(true, |source| {
        source.file != 0 || source.identity.as_str() != input.entry()
    }) {
        return candidate_error(
            CandidateObservationErrorCode::InvalidSourceTable,
            "candidate source file 0 must be the explicit entry",
        );
    }

    let mut identities = BTreeSet::new();
    let mut sources = Vec::with_capacity(raw_sources.len());
    let mut source_texts = Vec::with_capacity(raw_sources.len());
    for (expected_file, raw) in raw_sources.into_iter().enumerate() {
        let expected_file = u32::try_from(expected_file).map_err(|_| {
            resource_error("candidate source ordinal exceeded the schema u32 range")
        })?;
        if raw.file != expected_file {
            return candidate_error(
                CandidateObservationErrorCode::InvalidSourceTable,
                format!(
                    "candidate source ordinal {} is not dense file {expected_file}",
                    raw.file
                ),
            );
        }
        let identity = raw.identity.into_string();
        if !identities.insert(identity.clone()) {
            return candidate_error(
                CandidateObservationErrorCode::InvalidSourceTable,
                format!("candidate source `{identity}` is duplicated"),
            );
        }
        let Some(source) = input_sources
            .iter()
            .copied()
            .find(|source| source.identity() == identity)
        else {
            return candidate_error(
                CandidateObservationErrorCode::InvalidSourceTable,
                format!("candidate source `{identity}` was not supplied in explicit input"),
            );
        };
        source_texts.push(source.content());
        sources.push(CompilerSourceObservation::new(
            raw.file,
            identity,
            source.content(),
        ));
    }
    Ok((sources, source_texts))
}

fn validate_diagnostics(
    raw_diagnostics: Vec<RawDiagnostic>,
    sources: &[&str],
) -> Result<Vec<CandidateDiagnosticObservation>, CandidateObservationError> {
    ensure_at_most(
        raw_diagnostics.len(),
        CANDIDATE_OBSERVATION_MAX_DIAGNOSTICS,
        "diagnostic count",
    )?;
    let mut total_labels = 0_usize;
    let mut diagnostics = Vec::with_capacity(raw_diagnostics.len());
    for raw in raw_diagnostics {
        if !valid_diagnostic_code(&raw.code, raw.severity) {
            return candidate_error(
                CandidateObservationErrorCode::InvalidDiagnostic,
                format!(
                    "diagnostic code `{}` does not match its severity or [EW][0-9]{{4}}",
                    raw.code
                ),
            );
        }
        if raw.message.as_str().is_empty() {
            return candidate_error(
                CandidateObservationErrorCode::InvalidDiagnostic,
                "diagnostic message must not be empty",
            );
        }
        ensure_at_most(
            raw.message.as_str().len(),
            CANDIDATE_OBSERVATION_MAX_MESSAGE_BYTES,
            "diagnostic message bytes",
        )?;
        let raw_labels = raw.labels.into_vec();
        if raw_labels.is_empty() {
            return candidate_error(
                CandidateObservationErrorCode::InvalidDiagnostic,
                "diagnostic must have one primary label",
            );
        }
        ensure_at_most(
            raw_labels.len(),
            CANDIDATE_OBSERVATION_MAX_LABELS_PER_DIAGNOSTIC,
            "per-diagnostic label count",
        )?;
        total_labels = total_labels
            .checked_add(raw_labels.len())
            .ok_or_else(|| resource_error("total diagnostic label count overflowed"))?;
        if total_labels > CANDIDATE_OBSERVATION_MAX_TOTAL_LABELS {
            return candidate_error(
                CandidateObservationErrorCode::ResourceLimit,
                "total diagnostic label count exceeds the schema bound",
            );
        }

        let mut labels = Vec::with_capacity(raw_labels.len());
        for (index, raw_label) in raw_labels.into_iter().enumerate() {
            let expected_style = if index == 0 {
                RawLabelStyle::Primary
            } else {
                RawLabelStyle::Secondary
            };
            if raw_label.style != expected_style {
                return candidate_error(
                    CandidateObservationErrorCode::InvalidDiagnostic,
                    "the first diagnostic label must be primary and every later label secondary",
                );
            }
            ensure_at_most(
                raw_label.message.as_str().len(),
                CANDIDATE_OBSERVATION_MAX_MESSAGE_BYTES,
                "diagnostic label message bytes",
            )?;
            let Some(source) = usize::try_from(raw_label.file)
                .ok()
                .and_then(|file| sources.get(file))
            else {
                return candidate_error(
                    CandidateObservationErrorCode::InvalidDiagnostic,
                    format!(
                        "diagnostic label references unknown source file {}",
                        raw_label.file
                    ),
                );
            };
            let start = usize::try_from(raw_label.start)
                .map_err(|_| resource_error("diagnostic start did not fit usize"))?;
            let end = usize::try_from(raw_label.end)
                .map_err(|_| resource_error("diagnostic end did not fit usize"))?;
            if start > end
                || end > source.len()
                || !source.is_char_boundary(start)
                || !source.is_char_boundary(end)
            {
                return candidate_error(
                    CandidateObservationErrorCode::InvalidDiagnostic,
                    format!(
                        "diagnostic span {}:{}..{} is not an in-bounds UTF-8 range",
                        raw_label.file, raw_label.start, raw_label.end
                    ),
                );
            }
            labels.push(CandidateLabelObservation {
                style: canonical_label_style(raw_label.style),
                file: raw_label.file,
                start: raw_label.start,
                end: raw_label.end,
                message: raw_label.message.into_string(),
            });
        }
        diagnostics.push(CandidateDiagnosticObservation {
            code: raw.code,
            severity: canonical_severity(raw.severity),
            message: raw.message.into_string(),
            labels,
        });
    }

    if diagnostics
        .windows(2)
        .any(|pair| candidate_diagnostic_order(&pair[0], &pair[1]) == Ordering::Greater)
    {
        return candidate_error(
            CandidateObservationErrorCode::InvalidDiagnostic,
            "candidate diagnostics are not in canonical total order",
        );
    }
    Ok(diagnostics)
}

fn validate_artifacts(
    raw_artifacts: Vec<RawArtifact>,
    status: CandidateCompilationStatus,
    diagnostics: &[CandidateDiagnosticObservation],
) -> Result<Vec<CompilerArtifactObservation>, CandidateObservationError> {
    if raw_artifacts.len() != CanonicalPhase::ALL.len() {
        return candidate_error(
            CandidateObservationErrorCode::InvalidPhaseTable,
            format!(
                "candidate has {} artifacts; expected {}",
                raw_artifacts.len(),
                CanonicalPhase::ALL.len()
            ),
        );
    }

    let mut artifacts = Vec::with_capacity(CanonicalPhase::ALL.len());
    for (raw, expected_phase) in raw_artifacts.into_iter().zip(CanonicalPhase::ALL) {
        if raw.phase.as_str() != phase_name(expected_phase) {
            return candidate_error(
                CandidateObservationErrorCode::InvalidPhaseTable,
                format!(
                    "candidate phase `{}` is out of order; expected `{}`",
                    raw.phase.as_str(),
                    phase_name(expected_phase)
                ),
            );
        }
        let artifact = match raw.artifact {
            RawArtifactState::Produced { content } => {
                let content = content.into_string();
                validate_phase_content(expected_phase, &content, false)?;
                CompilerArtifactState::Produced { content }
            }
            RawArtifactState::SkippedDueToDiagnostics => {
                CompilerArtifactState::SkippedDueToDiagnostics
            }
            RawArtifactState::Deferred { content } => {
                let content = content.into_string();
                validate_phase_content(expected_phase, &content, true)?;
                CompilerArtifactState::Deferred { content }
            }
        };
        artifacts.push(CompilerArtifactObservation {
            phase: expected_phase,
            artifact,
        });
    }

    if artifacts[..3]
        .iter()
        .any(|artifact| artifact.status() != CandidatePhaseStatus::Produced)
    {
        return candidate_error(
            CandidateObservationErrorCode::InvalidPhaseTable,
            "tokens, CST, and diagnostics must always be produced",
        );
    }
    match status {
        CandidateCompilationStatus::Accepted => {
            if artifacts[3].status() != CandidatePhaseStatus::Produced
                || artifacts[4].status() != CandidatePhaseStatus::Produced
                || !matches!(
                    artifacts[5].status(),
                    CandidatePhaseStatus::Produced | CandidatePhaseStatus::Deferred
                )
            {
                return candidate_error(
                    CandidateObservationErrorCode::InvalidPhaseTable,
                    "accepted observations require produced HIR/MIR and produced or deferred NIR",
                );
            }
        }
        CandidateCompilationStatus::Rejected => {
            if artifacts[3..]
                .iter()
                .any(|artifact| artifact.status() != CandidatePhaseStatus::Blocked)
            {
                return candidate_error(
                    CandidateObservationErrorCode::InvalidPhaseTable,
                    "rejected observations require HIR, MIR, and NIR to be skipped",
                );
            }
        }
    }

    validate_diagnostic_artifact(&artifacts[2], diagnostics)?;
    Ok(artifacts)
}

fn validate_phase_content(
    phase: CanonicalPhase,
    content: &str,
    deferred: bool,
) -> Result<(), CandidateObservationError> {
    validate_json_resource(
        content.as_bytes(),
        CANDIDATE_PHASE_CONTENT_MAX_BYTES,
        "phase content",
    )?;
    if deferred {
        if phase != CanonicalPhase::Nir {
            return candidate_error(
                CandidateObservationErrorCode::InvalidPhaseTable,
                "only NIR may be deferred",
            );
        }
        let deferred: BorrowedDeferredEnvelope<'_> =
            serde_json::from_str(content).map_err(|source| CandidateObservationError {
                code: CandidateObservationErrorCode::InvalidPhaseContent,
                message: "deferred NIR content must contain reasonCode and plannedPhase".to_owned(),
                source: Some(Box::new(source)),
            })?;
        validate_phase_header(phase, deferred.schema_version, deferred.phase)?;
        if deferred.value.reason_code.is_empty()
            || deferred.value.reason_code.len() > 1_024
            || deferred.value.planned_phase.is_empty()
            || deferred.value.planned_phase.len() > 1_024
        {
            return candidate_error(
                CandidateObservationErrorCode::InvalidPhaseContent,
                "deferred NIR reasonCode or plannedPhase is empty or oversized",
            );
        }
        return Ok(());
    }

    let envelope: BorrowedPhaseEnvelope<'_> =
        serde_json::from_str(content).map_err(|source| CandidateObservationError {
            code: CandidateObservationErrorCode::InvalidPhaseContent,
            message: format!("{} content is not strict phase JSON", phase_name(phase)),
            source: Some(Box::new(source)),
        })?;
    validate_phase_header(phase, envelope.schema_version, envelope.phase)?;
    let valid_shape = match phase {
        CanonicalPhase::Tokens | CanonicalPhase::Cst | CanonicalPhase::Diagnostics => {
            envelope.value.get().starts_with('[')
        }
        CanonicalPhase::Hir | CanonicalPhase::Mir | CanonicalPhase::Nir => {
            envelope.value.get().starts_with('{')
        }
    };
    if !valid_shape {
        return candidate_error(
            CandidateObservationErrorCode::InvalidPhaseContent,
            format!(
                "{} content has the wrong top-level value shape",
                phase_name(phase)
            ),
        );
    }
    Ok(())
}

fn validate_phase_header(
    phase: CanonicalPhase,
    schema_version: u32,
    observed_phase: &str,
) -> Result<(), CandidateObservationError> {
    if schema_version != CANONICAL_DUMP_SCHEMA_VERSION || observed_phase != phase_name(phase) {
        return candidate_error(
            CandidateObservationErrorCode::InvalidPhaseContent,
            format!(
                "{} content header must use schema {} and the same phase",
                phase_name(phase),
                CANONICAL_DUMP_SCHEMA_VERSION
            ),
        );
    }
    Ok(())
}

fn validate_diagnostic_artifact(
    artifact: &CompilerArtifactObservation,
    diagnostics: &[CandidateDiagnosticObservation],
) -> Result<(), CandidateObservationError> {
    let Some(content) = artifact.content() else {
        return candidate_error(
            CandidateObservationErrorCode::InvalidPhaseTable,
            "diagnostic artifact is not produced",
        );
    };
    let expected = diagnostics
        .iter()
        .map(|diagnostic| RawDiagnosticShape {
            code: diagnostic.code.clone(),
            severity: raw_severity(diagnostic.severity),
            labels: diagnostic
                .labels
                .iter()
                .map(|label| RawLabelShape {
                    style: raw_label_style(label.style),
                    file: label.file,
                    start: label.start,
                    end: label.end,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let expected = serde_json::to_string(&SerializablePhaseEnvelope {
        schema_version: CANONICAL_DUMP_SCHEMA_VERSION,
        phase: phase_name(CanonicalPhase::Diagnostics),
        value: expected,
    })
    .map_err(|source| CandidateObservationError {
        code: CandidateObservationErrorCode::InvalidPhaseContent,
        message: "diagnostics projection could not be encoded".to_owned(),
        source: Some(Box::new(source)),
    })?;
    if content != expected {
        return candidate_error(
            CandidateObservationErrorCode::InvalidPhaseContent,
            "diagnostics phase is not the exact presentation-free structured projection",
        );
    }
    Ok(())
}

fn validate_json_resource(
    bytes: &[u8],
    maximum: usize,
    resource: &str,
) -> Result<(), CandidateObservationError> {
    if bytes.is_empty() {
        return candidate_error(
            CandidateObservationErrorCode::ResourceLimit,
            format!("{resource} byte length is zero"),
        );
    }
    ensure_at_most(bytes.len(), maximum, &format!("{resource} byte length"))?;
    if !json_depth_within(bytes, CANDIDATE_OBSERVATION_MAX_JSON_DEPTH) {
        return candidate_error(
            CandidateObservationErrorCode::ResourceLimit,
            format!(
                "{resource} exceeds JSON nesting depth {}",
                CANDIDATE_OBSERVATION_MAX_JSON_DEPTH
            ),
        );
    }
    Ok(())
}

fn json_depth_within(bytes: &[u8], maximum: usize) -> bool {
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.saturating_add(1);
                if depth > maximum {
                    return false;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    true
}

fn ensure_at_most(
    actual: usize,
    maximum: usize,
    resource: &str,
) -> Result<(), CandidateObservationError> {
    if actual > maximum {
        return candidate_error(
            CandidateObservationErrorCode::ResourceLimit,
            format!("{resource} {actual} exceeds {maximum}"),
        );
    }
    Ok(())
}

fn resource_error(message: impl Into<String>) -> CandidateObservationError {
    CandidateObservationError {
        code: CandidateObservationErrorCode::ResourceLimit,
        message: message.into(),
        source: None,
    }
}

fn candidate_error<T>(
    code: CandidateObservationErrorCode,
    message: impl Into<String>,
) -> Result<T, CandidateObservationError> {
    Err(CandidateObservationError {
        code,
        message: message.into(),
        source: None,
    })
}

fn valid_diagnostic_code(code: &str, severity: RawSeverity) -> bool {
    let bytes = code.as_bytes();
    bytes.len() == 5
        && bytes[0]
            == match severity {
                RawSeverity::Error => b'E',
                RawSeverity::Warning => b'W',
            }
        && bytes[1..].iter().all(u8::is_ascii_digit)
}

const fn canonical_severity(severity: RawSeverity) -> CanonicalSeverity {
    match severity {
        RawSeverity::Error => CanonicalSeverity::Error,
        RawSeverity::Warning => CanonicalSeverity::Warning,
    }
}

const fn raw_severity(severity: CanonicalSeverity) -> RawSeverity {
    match severity {
        CanonicalSeverity::Error => RawSeverity::Error,
        CanonicalSeverity::Warning => RawSeverity::Warning,
    }
}

const fn canonical_label_style(style: RawLabelStyle) -> CanonicalLabelStyle {
    match style {
        RawLabelStyle::Primary => CanonicalLabelStyle::Primary,
        RawLabelStyle::Secondary => CanonicalLabelStyle::Secondary,
    }
}

const fn raw_label_style(style: CanonicalLabelStyle) -> RawLabelStyle {
    match style {
        CanonicalLabelStyle::Primary => RawLabelStyle::Primary,
        CanonicalLabelStyle::Secondary => RawLabelStyle::Secondary,
    }
}

fn candidate_diagnostic_order(
    left: &CandidateDiagnosticObservation,
    right: &CandidateDiagnosticObservation,
) -> Ordering {
    diagnostic_location(left)
        .cmp(&diagnostic_location(right))
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| severity_order(left.severity).cmp(&severity_order(right.severity)))
        .then_with(|| {
            left.labels
                .iter()
                .map(candidate_label_key)
                .cmp(right.labels.iter().map(candidate_label_key))
        })
        .then_with(|| left.message.cmp(&right.message))
}

fn diagnostic_location(diagnostic: &CandidateDiagnosticObservation) -> (u32, u32, u32) {
    diagnostic
        .labels
        .first()
        .map_or((u32::MAX, u32::MAX, u32::MAX), |label| {
            (label.file, label.start, label.end)
        })
}

const fn severity_order(severity: CanonicalSeverity) -> u8 {
    match severity {
        CanonicalSeverity::Error => 0,
        CanonicalSeverity::Warning => 1,
    }
}

fn candidate_label_key(label: &CandidateLabelObservation) -> (u8, u32, u32, u32, &str) {
    let style = match label.style {
        CanonicalLabelStyle::Primary => 0,
        CanonicalLabelStyle::Secondary => 1,
    };
    (style, label.file, label.start, label.end, &label.message)
}

const fn phase_name(phase: CanonicalPhase) -> &'static str {
    match phase {
        CanonicalPhase::Tokens => "tokens",
        CanonicalPhase::Cst => "cst",
        CanonicalPhase::Diagnostics => "diagnostics",
        CanonicalPhase::Hir => "hir",
        CanonicalPhase::Mir => "mir",
        CanonicalPhase::Nir => "nir",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_at_most, json_depth_within, validate_json_resource, CandidateObservationErrorCode,
        CANDIDATE_OBSERVATION_MAX_JSON_DEPTH,
    };

    #[test]
    fn every_count_bound_accepts_exact_and_rejects_one_over() {
        assert!(ensure_at_most(8, 8, "test count").is_ok());
        assert!(matches!(
            ensure_at_most(9, 8, "test count"),
            Err(error) if error.code() == CandidateObservationErrorCode::ResourceLimit
        ));
    }

    #[test]
    fn encoded_byte_bound_accepts_exact_and_rejects_one_over_before_decoding() {
        assert!(validate_json_resource(b"[]", 2, "test JSON").is_ok());
        assert!(matches!(
            validate_json_resource(b"[]", 1, "test JSON"),
            Err(error) if error.code() == CandidateObservationErrorCode::ResourceLimit
        ));
    }

    #[test]
    fn json_depth_ignores_delimiters_inside_strings_and_rejects_one_over() {
        let exact = format!(
            "{}\"{{[ignored]}}\"{}",
            "[".repeat(CANDIDATE_OBSERVATION_MAX_JSON_DEPTH),
            "]".repeat(CANDIDATE_OBSERVATION_MAX_JSON_DEPTH)
        );
        assert!(json_depth_within(
            exact.as_bytes(),
            CANDIDATE_OBSERVATION_MAX_JSON_DEPTH
        ));

        let over = format!(
            "{}{}",
            "[".repeat(CANDIDATE_OBSERVATION_MAX_JSON_DEPTH + 1),
            "]".repeat(CANDIDATE_OBSERVATION_MAX_JSON_DEPTH + 1)
        );
        assert!(!json_depth_within(
            over.as_bytes(),
            CANDIDATE_OBSERVATION_MAX_JSON_DEPTH
        ));
    }
}
