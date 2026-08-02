//! Rust/Futao parser differential contract for toolchain 0.0.8.

use nexa_compiler::{
    FutaoParserAdapter, ParserAdapter, ParserCstEvent, ParserDifferentialHarness,
    ParserDifferentialOutcome, ParserImplementation, ParserObservable, RustParserAdapter,
    PARSER_SNAPSHOT_SCHEMA_VERSION,
};

#[test]
fn rust_snapshot_freezes_balanced_lossless_cst_events() -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit {}";
    let snapshot = RustParserAdapter.parse(source)?;

    assert_eq!(snapshot.schema_version(), PARSER_SNAPSHOT_SCHEMA_VERSION);
    assert!(matches!(
        snapshot.cst_events().first(),
        Some(ParserCstEvent::StartNode {
            kind_id: 39,
            offset: 0
        })
    ));
    assert!(matches!(
        snapshot.cst_events().last(),
        Some(ParserCstEvent::FinishNode { offset: 24 })
    ));
    assert!(snapshot.cst_events().iter().any(|event| matches!(
        event,
        ParserCstEvent::StartNode {
            kind_id: 40,
            offset: 0
        }
    )));
    assert!(snapshot.recovery_events().is_empty());
    assert!(snapshot.diagnostics().is_empty());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&snapshot.to_json()?)?["schemaVersion"],
        1
    );
    Ok(())
}

#[test]
fn rust_snapshot_separates_recovery_regions_from_parser_diagnostics(
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = RustParserAdapter.parse("function main(): Unit { @ }")?;

    assert_eq!(snapshot.recovery_events().len(), 1);
    assert_eq!(
        (
            snapshot.recovery_events()[0].start(),
            snapshot.recovery_events()[0].end()
        ),
        (24, 26)
    );
    assert_eq!(snapshot.diagnostics().len(), 1);
    assert_eq!(snapshot.diagnostics()[0].code(), "E1001");
    assert_eq!(
        (
            snapshot.diagnostics()[0].start(),
            snapshot.diagnostics()[0].end()
        ),
        (24, 25)
    );
    Ok(())
}

#[test]
fn rust_snapshot_preserves_generic_close_token_splitting() -> Result<(), Box<dyn std::error::Error>>
{
    let snapshot = RustParserAdapter.parse("type Box<T>= { value: T; };")?;
    let significant_tokens = snapshot
        .cst_events()
        .iter()
        .filter_map(|event| match event {
            ParserCstEvent::Token {
                kind_id,
                start,
                end,
            } if !matches!(kind_id, 2 | 3) => Some((*kind_id, *start, *end)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(significant_tokens.windows(2).any(|pair| {
        pair[0].0 == 37
            && pair[1].0 == 10
            && pair[0].2 == pair[1].1
            && pair[0].1 == 10
            && pair[1].2 == 12
    }));
    Ok(())
}

#[test]
fn futao_adapter_executes_the_real_parser_and_matches_empty_source(
) -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);

    let report = harness.run_case("accepted/empty", "")?;

    assert_eq!(report.reference(), ParserImplementation::RustReference);
    assert_eq!(report.candidate(), ParserImplementation::Futao);
    assert_eq!(report.outcome(), ParserDifferentialOutcome::Match);
    assert!(report.passes_gate());
    Ok(())
}

#[test]
fn any_parser_observable_difference_fails_without_suppression(
) -> Result<(), Box<dyn std::error::Error>> {
    let reference = SourceParser("function main(): Unit { @ }");
    let different = SourceParser("function main(): Unit { 1 }");
    let harness = ParserDifferentialHarness::new(&reference, &different);

    let report = harness.run_case("synthetic/difference", reference.0)?;

    assert_eq!(report.reference(), ParserImplementation::RustReference);
    assert_eq!(report.candidate(), ParserImplementation::Futao);
    assert_eq!(report.outcome(), ParserDifferentialOutcome::Differences);
    assert!(!report.passes_gate());
    assert_eq!(
        report
            .differences()
            .iter()
            .map(|difference| difference.observable())
            .collect::<Vec<_>>(),
        [
            ParserObservable::Cst,
            ParserObservable::Recovery,
            ParserObservable::Diagnostics
        ]
    );
    assert!(report.differences().iter().all(|difference| {
        difference.reference_digest().starts_with("sha256:")
            && difference.candidate_digest().starts_with("sha256:")
    }));
    Ok(())
}

struct SourceParser(&'static str);

impl ParserAdapter for SourceParser {
    fn implementation(&self) -> ParserImplementation {
        if self.0.contains('@') {
            ParserImplementation::RustReference
        } else {
            ParserImplementation::Futao
        }
    }

    fn parse(
        &self,
        _source: &str,
    ) -> Result<nexa_compiler::ParserSnapshot, nexa_compiler::ParserAdapterError> {
        RustParserAdapter.parse(self.0)
    }
}
