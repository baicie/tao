//! Differential adapter and phase-classification contracts.

use nexa_compiler::{
    compile, CompilerAdapter, CompilerAdapterState, CompilerImplementation, CompilerInput,
    CompilerOutput, CompilerSource, DifferentialHarness, DifferentialOutcome,
    RustReferenceCompiler,
};

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
            CompilerImplementation::Futao,
            "the complete Futao compiler adapter is scheduled for 0.0.11",
        ),
    );

    let report = harness.run(&input())?;

    assert_eq!(report.outcome(), DifferentialOutcome::Unavailable);
    assert_eq!(report.issues().len(), 1);
    assert_eq!(
        report.issues()[0].unavailable_implementation(),
        Some(CompilerImplementation::Futao)
    );
    Ok(())
}

#[test]
fn every_observed_difference_is_unclassified_and_fails_closed(
) -> Result<(), Box<dyn std::error::Error>> {
    let reference_output = compile(&input())?;
    let candidate_output = compile(&CompilerInput::new(
        "main.ft",
        [CompilerSource::new(
            "main.ft",
            "function main(): Unit { print(43); }",
        )],
    ))?;
    let reference = FixedAdapter(reference_output);
    let candidate = FixedAdapter(candidate_output);
    let harness = DifferentialHarness::new(
        CompilerAdapterState::available(&reference),
        CompilerAdapterState::available(&candidate),
    );

    let mut report = harness.run_case("synthetic/literal-change", &input())?;

    assert_eq!(report.outcome(), DifferentialOutcome::Differences);
    assert!(report.has_unclassified_differences());
    assert!(!report.is_match());
    assert!(!report.passes_gate());
    assert_eq!(report.case_id(), "synthetic/literal-change");
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

struct FixedAdapter(CompilerOutput);

impl CompilerAdapter for FixedAdapter {
    fn implementation(&self) -> CompilerImplementation {
        CompilerImplementation::RustReference
    }

    fn compile(
        &self,
        _input: &CompilerInput,
    ) -> Result<CompilerOutput, nexa_compiler::CompileError> {
        Ok(self.0.clone())
    }
}
