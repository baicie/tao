//! Differential adapter and phase-classification contracts.

use nexa_compiler::{
    CandidateObservation, CompilerAdapter, CompilerAdapterState, CompilerImplementation,
    CompilerInput, CompilerObservation, CompilerSource, DifferentialHarness, DifferentialOutcome,
    RustReferenceCompiler,
};
use serde_json::{json, Value};

fn input() -> CompilerInput {
    CompilerInput::new(
        "main.ft",
        [CompilerSource::new(
            "main.ft",
            "function main(): Unit { print(42); }",
        )],
    )
}

#[test]
fn identical_real_adapters_have_no_phase_differences() -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustReferenceCompiler;
    let harness = DifferentialHarness::new(
        CompilerAdapterState::available(&rust),
        CompilerAdapterState::available(&rust),
    );

    let report = harness.run(&input())?;

    assert_eq!(report.outcome(), DifferentialOutcome::Match);
    assert!(report.issues().is_empty());
    Ok(())
}

#[test]
fn unavailable_futao_adapter_is_reported_instead_of_emulated(
) -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustReferenceCompiler;
    let harness = DifferentialHarness::new(
        CompilerAdapterState::available(&rust),
        CompilerAdapterState::unavailable(
            CompilerImplementation::FutaoBootstrapCandidate,
            "the complete Futao compiler adapter is scheduled for 0.0.11",
        ),
    );

    let report = harness.run(&input())?;

    assert_eq!(report.outcome(), DifferentialOutcome::Unavailable);
    assert_eq!(report.issues().len(), 1);
    assert_eq!(
        report.issues()[0].unavailable_implementation(),
        Some(CompilerImplementation::FutaoBootstrapCandidate)
    );
    Ok(())
}

#[test]
fn every_observed_difference_is_unclassified_and_fails_closed(
) -> Result<(), Box<dyn std::error::Error>> {
    let case_input = input();
    let rust = RustReferenceCompiler;
    let reference = rust.compile(&case_input)?;
    let mut artifacts = serde_json::to_value(reference.artifacts())?;
    let token_content = artifacts[0]["artifact"]["content"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("expected token content"))?;
    let mut token_content: Value = serde_json::from_str(token_content)?;
    token_content["value"][0]["tokens"][0]["kindId"] = json!(0);
    artifacts[0]["artifact"]["content"] = json!(serde_json::to_string(&token_content)?);
    let cst_content = artifacts[1]["artifact"]["content"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("expected CST content"))?;
    let mut cst_content: Value = serde_json::from_str(cst_content)?;
    cst_content["value"][0]["elements"][2]["kindId"] = json!(0);
    artifacts[1]["artifact"]["content"] = json!(serde_json::to_string(&cst_content)?);
    let candidate = CandidateAdapter(load_candidate(
        &case_input,
        json!([{"file": 0, "identity": "main.ft"}]),
        artifacts,
    )?);
    let harness = DifferentialHarness::new(
        CompilerAdapterState::available(&rust),
        CompilerAdapterState::available(&candidate),
    );

    let mut report = harness.run_case("synthetic/token-change", &case_input)?;

    assert_eq!(report.outcome(), DifferentialOutcome::Differences);
    assert!(report.has_unclassified_differences());
    assert!(!report.is_match());
    assert!(!report.passes_gate());
    assert_eq!(report.case_id(), "synthetic/token-change");
    assert!(report.issues().iter().all(|issue| {
        issue
            .reference_digest()
            .is_some_and(|digest| digest.starts_with("sha256:"))
            && issue
                .candidate_digest()
                .is_some_and(|digest| digest.starts_with("sha256:"))
    }));

    let phases = report
        .issues()
        .iter()
        .filter_map(nexa_compiler::DifferentialIssue::phase)
        .collect::<Vec<_>>();
    for phase in phases {
        assert!(report.classify(
            phase,
            nexa_compiler::DifferenceClassification::CanonicalizationBug
        ));
    }
    assert!(report.passes_gate());
    Ok(())
}

#[test]
fn rust_reference_observation_preserves_portable_directory_identities(
) -> Result<(), Box<dyn std::error::Error>> {
    let case_input = CompilerInput::new(
        "app/main.ft",
        [CompilerSource::new(
            "app/main.ft",
            "function main(): Unit {}",
        )],
    );
    let rust = RustReferenceCompiler;

    let observation = rust.compile(&case_input)?;

    assert_eq!(observation.sources()[0].identity(), "app/main.ft");
    Ok(())
}

#[test]
fn source_ordinals_must_bind_the_same_identity_in_both_observations(
) -> Result<(), Box<dyn std::error::Error>> {
    let case_input = CompilerInput::new(
        "main.ft",
        [
            CompilerSource::new(
                "main.ft",
                "import { a } from \"./a.ft\";\nimport { b } from \"./b.ft\";\nfunction main(): Unit { print(a() + b()); }",
            ),
            CompilerSource::new("a.ft", "export function a(): Int { return 1; }"),
            CompilerSource::new("b.ft", "export function b(): Int { return 2; }"),
        ],
    );
    let rust = RustReferenceCompiler;
    let reference = rust.compile(&case_input)?;
    assert_eq!(
        reference
            .sources()
            .iter()
            .map(|source| source.identity())
            .collect::<Vec<_>>(),
        ["main.ft", "a.ft", "b.ft"]
    );
    let mut artifacts = serde_json::to_value(reference.artifacts())?;
    for artifact_index in [0, 1] {
        let content = artifacts[artifact_index]["artifact"]["content"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("expected front-end content"))?;
        let mut content: Value = serde_json::from_str(content)?;
        let modules = content["value"]
            .as_array_mut()
            .ok_or_else(|| std::io::Error::other("expected front-end modules"))?;
        modules.swap(1, 2);
        modules[1]["module"] = json!(1);
        modules[2]["module"] = json!(2);
        artifacts[artifact_index]["artifact"]["content"] = json!(serde_json::to_string(&content)?);
    }
    let candidate = CandidateAdapter(load_candidate(
        &case_input,
        json!([
            {"file": 0, "identity": "main.ft"},
            {"file": 1, "identity": "b.ft"},
            {"file": 2, "identity": "a.ft"}
        ]),
        artifacts,
    )?);
    let harness = DifferentialHarness::new(
        CompilerAdapterState::available(&rust),
        CompilerAdapterState::available(&candidate),
    );

    assert!(matches!(
        harness.run(&case_input),
        Err(nexa_compiler::CompileError::AdapterObservationMismatch { field: "sources" })
    ));
    Ok(())
}

#[test]
fn observation_cannot_be_reused_for_different_same_length_source_bytes(
) -> Result<(), Box<dyn std::error::Error>> {
    let first_input = input();
    let second_input = CompilerInput::new(
        "main.ft",
        [CompilerSource::new(
            "main.ft",
            "function main(): Unit { print(43); }",
        )],
    );
    let rust = RustReferenceCompiler;
    let first = rust.compile(&first_input)?;
    let cached_candidate = CandidateAdapter(load_candidate(
        &first_input,
        json!([{"file": 0, "identity": "main.ft"}]),
        serde_json::to_value(first.artifacts())?,
    )?);
    let harness = DifferentialHarness::new(
        CompilerAdapterState::available(&rust),
        CompilerAdapterState::available(&cached_candidate),
    );

    assert!(matches!(
        harness.run(&second_input),
        Err(nexa_compiler::CompileError::AdapterObservationMismatch { field: "sources" })
    ));
    Ok(())
}

#[test]
fn adapter_cannot_relabel_another_implementations_observation(
) -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustReferenceCompiler;
    let rust_observation = rust.compile(&input())?;
    let relabelled = RelabelledCandidate(rust_observation);
    let harness = DifferentialHarness::new(
        CompilerAdapterState::available(&rust),
        CompilerAdapterState::available(&relabelled),
    );

    assert!(matches!(
        harness.run(&input()),
        Err(nexa_compiler::CompileError::AdapterIdentityMismatch {
            declared: "futao-bootstrap-candidate",
            observed: "rust-reference"
        })
    ));
    Ok(())
}

fn load_candidate(
    input: &CompilerInput,
    sources: Value,
    artifacts: Value,
) -> Result<CompilerObservation, Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec(&json!({
        "schemaVersion": 1,
        "implementation": "futao-bootstrap-candidate",
        "languageVersion": "1.0",
        "compilationProfile": "application",
        "status": "accepted",
        "sources": sources,
        "diagnostics": [],
        "artifacts": artifacts
    }))?;
    Ok(CandidateObservation::load(&bytes, input)?.into_compiler_observation())
}

struct CandidateAdapter(CompilerObservation);

impl CompilerAdapter for CandidateAdapter {
    fn implementation(&self) -> CompilerImplementation {
        CompilerImplementation::FutaoBootstrapCandidate
    }

    fn compile(
        &self,
        _input: &CompilerInput,
    ) -> Result<CompilerObservation, nexa_compiler::CompileError> {
        Ok(self.0.clone())
    }
}

struct RelabelledCandidate(CompilerObservation);

impl CompilerAdapter for RelabelledCandidate {
    fn implementation(&self) -> CompilerImplementation {
        CompilerImplementation::FutaoBootstrapCandidate
    }

    fn compile(
        &self,
        _input: &CompilerInput,
    ) -> Result<CompilerObservation, nexa_compiler::CompileError> {
        Ok(self.0.clone())
    }
}
