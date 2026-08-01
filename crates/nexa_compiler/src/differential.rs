//! Differential comparison over canonical compiler outputs.

use std::fmt::{Debug, Formatter};

use sha2::{Digest, Sha256};

use crate::{compile, CanonicalPhase, CompileError, CompilerInput, CompilerOutput};

/// Identifies one compiler implementation participating in differential tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilerImplementation {
    /// The Rust Stage 0 reference compiler.
    RustReference,
    /// The self-hosted Futao compiler.
    Futao,
}

impl CompilerImplementation {
    /// Returns the manifest spelling of this implementation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustReference => "rust-reference",
            Self::Futao => "futao-self-hosted",
        }
    }
}

/// Compiles one explicit input into canonical, structured output.
///
/// Implementations must not synthesize another implementation's result. When
/// an implementation does not exist yet, represent it with
/// [`CompilerAdapterState::Unavailable`] instead of implementing this trait.
pub trait CompilerAdapter {
    /// Returns the implementation represented by this adapter.
    fn implementation(&self) -> CompilerImplementation;

    /// Compiles an explicit, in-memory compiler input.
    ///
    /// # Errors
    ///
    /// Returns [`CompileError`] when the input contract cannot be satisfied.
    fn compile(&self, input: &CompilerInput) -> Result<CompilerOutput, CompileError>;
}

/// Adapter for the real Rust reference compiler.
#[derive(Debug, Default, Clone, Copy)]
pub struct RustReferenceCompiler;

impl CompilerAdapter for RustReferenceCompiler {
    fn implementation(&self) -> CompilerImplementation {
        CompilerImplementation::RustReference
    }

    fn compile(&self, input: &CompilerInput) -> Result<CompilerOutput, CompileError> {
        compile(input)
    }
}

/// Availability of one compiler adapter at a self-hosting milestone.
pub enum CompilerAdapterState<'a> {
    /// A real adapter that can be executed by the harness.
    Available(&'a dyn CompilerAdapter),
    /// An implementation that does not exist at the current milestone.
    Unavailable {
        /// The missing implementation.
        implementation: CompilerImplementation,
        /// A concrete reason the implementation cannot be run.
        reason: String,
    },
}

impl<'a> CompilerAdapterState<'a> {
    /// Marks a real compiler adapter as available.
    #[must_use]
    pub const fn available(adapter: &'a dyn CompilerAdapter) -> Self {
        Self::Available(adapter)
    }

    /// Records that a compiler implementation cannot be run.
    #[must_use]
    pub fn unavailable(implementation: CompilerImplementation, reason: impl Into<String>) -> Self {
        Self::Unavailable {
            implementation,
            reason: reason.into(),
        }
    }

    /// Returns the compiler implementation represented by this state.
    #[must_use]
    pub fn implementation(&self) -> CompilerImplementation {
        match self {
            Self::Available(adapter) => adapter.implementation(),
            Self::Unavailable { implementation, .. } => *implementation,
        }
    }

    /// Returns why this implementation is unavailable, when applicable.
    #[must_use]
    pub fn unavailable_reason(&self) -> Option<&str> {
        match self {
            Self::Available(_) => None,
            Self::Unavailable { reason, .. } => Some(reason),
        }
    }
}

impl Debug for CompilerAdapterState<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Available(adapter) => formatter
                .debug_tuple("Available")
                .field(&adapter.implementation())
                .finish(),
            Self::Unavailable {
                implementation,
                reason,
            } => formatter
                .debug_struct("Unavailable")
                .field("implementation", implementation)
                .field("reason", reason)
                .finish(),
        }
    }
}

/// Classification assigned to one observed compiler difference.
///
/// `Unclassified` is deliberately distinct from the five accepted ADR-000
/// classifications. A report containing it must not pass a differential gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DifferenceClassification {
    /// No reviewed explanation has been assigned to this difference.
    Unclassified,
    /// The Rust reference implementation violates the language contract.
    RustCompilerBug,
    /// The Futao implementation violates the language contract.
    FutaoCompilerBug,
    /// The language specification does not determine the expected behavior.
    SpecificationAmbiguity,
    /// Canonicalization exposes or erases the wrong observable information.
    CanonicalizationBug,
    /// Only non-contractual diagnostic wording differs.
    AllowedDiagnosticTextDifference,
}

/// Stable discriminant for one differential report issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DifferentialIssueKind {
    /// A requested compiler implementation could not be run.
    Unavailable,
    /// Canonical output differs for one compiler phase.
    Difference,
}

/// One issue discovered while attempting a differential comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DifferentialIssue {
    /// A compiler implementation could not be run.
    Unavailable {
        /// The missing implementation.
        implementation: CompilerImplementation,
        /// A concrete reason the implementation cannot be run.
        reason: String,
    },
    /// Canonical output differs for one compiler phase.
    Difference {
        /// The canonical phase whose output differs.
        phase: CanonicalPhase,
        /// The reviewed classification, or `Unclassified` by default.
        classification: DifferenceClassification,
        /// SHA-256 of the reference artifact's canonical JSON state.
        reference_digest: String,
        /// SHA-256 of the candidate artifact's canonical JSON state.
        candidate_digest: String,
    },
}

impl DifferentialIssue {
    /// Returns the stable kind of this issue.
    #[must_use]
    pub const fn kind(&self) -> DifferentialIssueKind {
        match self {
            Self::Unavailable { .. } => DifferentialIssueKind::Unavailable,
            Self::Difference { .. } => DifferentialIssueKind::Difference,
        }
    }

    /// Returns the unavailable implementation represented by this issue.
    #[must_use]
    pub const fn unavailable_implementation(&self) -> Option<CompilerImplementation> {
        match self {
            Self::Unavailable { implementation, .. } => Some(*implementation),
            Self::Difference { .. } => None,
        }
    }

    /// Returns the reason an implementation is unavailable.
    #[must_use]
    pub fn unavailable_reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable { reason, .. } => Some(reason),
            Self::Difference { .. } => None,
        }
    }

    /// Returns the phase associated with a canonical output difference.
    #[must_use]
    pub const fn phase(&self) -> Option<CanonicalPhase> {
        match self {
            Self::Unavailable { .. } => None,
            Self::Difference { phase, .. } => Some(*phase),
        }
    }

    /// Returns the classification assigned to a canonical output difference.
    #[must_use]
    pub const fn classification(&self) -> Option<DifferenceClassification> {
        match self {
            Self::Unavailable { .. } => None,
            Self::Difference { classification, .. } => Some(*classification),
        }
    }

    /// Returns the reference artifact digest for a phase difference.
    #[must_use]
    pub fn reference_digest(&self) -> Option<&str> {
        match self {
            Self::Unavailable { .. } => None,
            Self::Difference {
                reference_digest, ..
            } => Some(reference_digest),
        }
    }

    /// Returns the candidate artifact digest for a phase difference.
    #[must_use]
    pub fn candidate_digest(&self) -> Option<&str> {
        match self {
            Self::Unavailable { .. } => None,
            Self::Difference {
                candidate_digest, ..
            } => Some(candidate_digest),
        }
    }
}

/// Overall result of attempting one differential comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferentialOutcome {
    /// Every available canonical phase has identical output.
    Match,
    /// At least one canonical phase differs.
    Differences,
    /// At least one requested compiler implementation is unavailable.
    Unavailable,
}

/// Structured result of running two compiler adapters over one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifferentialReport {
    case_id: String,
    reference: CompilerImplementation,
    candidate: CompilerImplementation,
    outcome: DifferentialOutcome,
    issues: Vec<DifferentialIssue>,
}

impl DifferentialReport {
    /// Returns the stable corpus case identifier supplied to the harness.
    #[must_use]
    pub fn case_id(&self) -> &str {
        &self.case_id
    }

    /// Returns the reference implementation identity.
    #[must_use]
    pub const fn reference(&self) -> CompilerImplementation {
        self.reference
    }

    /// Returns the candidate implementation identity.
    #[must_use]
    pub const fn candidate(&self) -> CompilerImplementation {
        self.candidate
    }

    /// Returns the overall comparison outcome.
    #[must_use]
    pub const fn outcome(&self) -> DifferentialOutcome {
        self.outcome
    }

    /// Returns unavailable implementations and per-phase differences.
    #[must_use]
    pub fn issues(&self) -> &[DifferentialIssue] {
        &self.issues
    }

    /// Returns true only when both adapters ran and every phase matched.
    #[must_use]
    pub const fn is_match(&self) -> bool {
        matches!(self.outcome, DifferentialOutcome::Match)
    }

    /// Returns true when any phase has not received an accepted classification.
    #[must_use]
    pub fn has_unclassified_differences(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.classification() == Some(DifferenceClassification::Unclassified))
    }

    /// Returns true when the run is available and has no unclassified difference.
    #[must_use]
    pub fn passes_gate(&self) -> bool {
        self.outcome != DifferentialOutcome::Unavailable && !self.has_unclassified_differences()
    }

    /// Assigns a reviewed ADR-000 classification to one differing phase.
    ///
    /// Returns false when the phase did not differ, `Unclassified` was supplied,
    /// or an allowed text classification was requested for structural output.
    /// Diagnostic text is excluded from canonical diagnostic artifacts, so a
    /// structural phase difference can never be text-only.
    pub fn classify(
        &mut self,
        phase: CanonicalPhase,
        classification: DifferenceClassification,
    ) -> bool {
        if matches!(
            classification,
            DifferenceClassification::Unclassified
                | DifferenceClassification::AllowedDiagnosticTextDifference
        ) {
            return false;
        }
        let Some(issue) = self
            .issues
            .iter_mut()
            .find(|issue| issue.phase() == Some(phase))
        else {
            return false;
        };
        let DifferentialIssue::Difference {
            classification: current,
            ..
        } = issue
        else {
            return false;
        };
        *current = classification;
        true
    }
}

/// Runs two compiler implementations over the same explicit input.
#[derive(Debug)]
pub struct DifferentialHarness<'a> {
    reference: CompilerAdapterState<'a>,
    candidate: CompilerAdapterState<'a>,
}

impl<'a> DifferentialHarness<'a> {
    /// Creates a harness from two explicit compiler availability states.
    #[must_use]
    pub const fn new(
        reference: CompilerAdapterState<'a>,
        candidate: CompilerAdapterState<'a>,
    ) -> Self {
        Self {
            reference,
            candidate,
        }
    }

    /// Runs both real adapters and compares every canonical compiler phase.
    ///
    /// Unavailable implementations are reported without fabricating compiler
    /// output. Every observed phase difference starts as `Unclassified`.
    ///
    /// # Errors
    ///
    /// Returns the first [`CompileError`] produced by an available adapter.
    pub fn run(&self, input: &CompilerInput) -> Result<DifferentialReport, CompileError> {
        self.run_case("anonymous", input)
    }

    /// Runs both adapters for one stable corpus case identifier.
    ///
    /// # Errors
    ///
    /// Returns the first [`CompileError`] produced by an available adapter.
    pub fn run_case(
        &self,
        case_id: impl Into<String>,
        input: &CompilerInput,
    ) -> Result<DifferentialReport, CompileError> {
        let case_id = case_id.into();
        let reference_implementation = self.reference.implementation();
        let candidate_implementation = self.candidate.implementation();
        let unavailable = self.unavailable_issues();
        if !unavailable.is_empty() {
            return Ok(DifferentialReport {
                case_id,
                reference: reference_implementation,
                candidate: candidate_implementation,
                outcome: DifferentialOutcome::Unavailable,
                issues: unavailable,
            });
        }

        let (
            CompilerAdapterState::Available(reference),
            CompilerAdapterState::Available(candidate),
        ) = (&self.reference, &self.candidate)
        else {
            return Ok(DifferentialReport {
                case_id,
                reference: reference_implementation,
                candidate: candidate_implementation,
                outcome: DifferentialOutcome::Unavailable,
                issues: self.unavailable_issues(),
            });
        };

        let reference_output = reference.compile(input)?;
        let candidate_output = candidate.compile(input)?;
        let issues = CanonicalPhase::ALL
            .iter()
            .copied()
            .filter_map(|phase| {
                let reference = reference_output.dumps().artifact(phase);
                let candidate = candidate_output.dumps().artifact(phase);
                (reference != candidate).then_some((phase, reference, candidate))
            })
            .map(|(phase, reference, candidate)| {
                Ok(DifferentialIssue::Difference {
                    phase,
                    classification: DifferenceClassification::Unclassified,
                    reference_digest: artifact_digest(reference)?,
                    candidate_digest: artifact_digest(candidate)?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let outcome = if issues.is_empty() {
            DifferentialOutcome::Match
        } else {
            DifferentialOutcome::Differences
        };

        Ok(DifferentialReport {
            case_id,
            reference: reference_implementation,
            candidate: candidate_implementation,
            outcome,
            issues,
        })
    }

    fn unavailable_issues(&self) -> Vec<DifferentialIssue> {
        [&self.reference, &self.candidate]
            .iter()
            .filter_map(|state| match state {
                CompilerAdapterState::Available(_) => None,
                CompilerAdapterState::Unavailable {
                    implementation,
                    reason,
                } => Some(DifferentialIssue::Unavailable {
                    implementation: *implementation,
                    reason: reason.clone(),
                }),
            })
            .collect()
    }
}

fn artifact_digest(artifact: &crate::CanonicalArtifact) -> Result<String, CompileError> {
    let encoded = serde_json::to_vec(artifact)?;
    Ok(format!("sha256:{:x}", Sha256::digest(encoded)))
}
