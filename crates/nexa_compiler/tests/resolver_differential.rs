//! Resolver-only canonical snapshot and differential contracts.

use nexa_compiler::{
    CompilerInput, CompilerSource, FutaoResolverAdapter, ResolverAdapter,
    ResolverDifferentialHarness, ResolverDifferentialOutcome, ResolverImplementation,
    RustResolverAdapter, RESOLVER_SNAPSHOT_SCHEMA_VERSION,
};

const ACCEPTED_MAIN: &str =
    include_str!("../../../bootstrap/compiler/tests/resolver/accepted/main.ft");
const ACCEPTED_MATH: &str =
    include_str!("../../../bootstrap/compiler/tests/resolver/accepted/math.ft");
const ACCEPTED_MODEL: &str =
    include_str!("../../../bootstrap/compiler/tests/resolver/accepted/model.ft");
const REJECTED_LIBRARY: &str =
    include_str!("../../../bootstrap/compiler/tests/resolver/rejected/library.ft");
const REJECTED_MAIN: &str =
    include_str!("../../../bootstrap/compiler/tests/resolver/rejected/main.ft");

#[test]
fn rust_resolver_snapshot_freezes_the_complete_observable_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let adapter = RustResolverAdapter;
    let snapshot = adapter.resolve(&accepted_input(false))?;

    assert_eq!(
        adapter.implementation(),
        ResolverImplementation::RustReference
    );
    assert_eq!(snapshot.schema_version(), RESOLVER_SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(
        snapshot
            .modules()
            .iter()
            .map(|module| (module.id(), module.identity()))
            .collect::<Vec<_>>(),
        [(0, "main.ft"), (1, "model.ft"), (2, "math.ft")]
    );
    assert_eq!(
        snapshot
            .edges()
            .iter()
            .map(|edge| (edge.importer(), edge.imported()))
            .collect::<Vec<_>>(),
        [(0, 1), (1, 2)]
    );
    assert_eq!(snapshot.symbols().len(), 4);
    assert_eq!(snapshot.scopes().len(), 4);
    assert_eq!(snapshot.bindings().len(), 5);
    assert!(!snapshot.names().is_empty());
    assert!(snapshot.diagnostics().is_empty());
    assert!(snapshot.to_json()?.contains("\"schemaVersion\":1"));

    Ok(())
}

#[test]
fn rust_resolver_snapshot_keeps_rejected_diagnostics_and_cycle_witnesses(
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = RustResolverAdapter.resolve(&CompilerInput::new(
        "main.ft",
        [
            CompilerSource::new("main.ft", REJECTED_MAIN),
            CompilerSource::new("library.ft", REJECTED_LIBRARY),
        ],
    ))?;

    assert_eq!(
        snapshot
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        ["E4004", "E4003", "E2002", "E2002", "E2001", "E4002"]
    );
    let cycle = snapshot
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "E4002")
        .ok_or_else(|| std::io::Error::other("expected E4002"))?;
    assert_eq!(cycle.labels().len(), 2);
    assert_eq!(cycle.labels()[0].span().source(), 1);
    assert_eq!(cycle.labels()[1].span().source(), 0);

    Ok(())
}

#[test]
fn rust_resolver_snapshot_is_independent_of_explicit_source_collection_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let adapter = RustResolverAdapter;

    assert_eq!(
        adapter.resolve(&accepted_input(false))?,
        adapter.resolve(&accepted_input(true))?
    );
    Ok(())
}

#[test]
fn futao_resolver_matches_the_accepted_graph() -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustResolverAdapter;
    let futao = FutaoResolverAdapter::new()?;
    let harness = ResolverDifferentialHarness::new(&rust, &futao);

    let report = harness.run_case("accepted/core", &accepted_input(false))?;

    assert_eq!(
        report.outcome(),
        ResolverDifferentialOutcome::Match,
        "resolver snapshots diverged: {:?}\nreference={}\ncandidate={}",
        report.differences(),
        report.reference_snapshot().to_json()?,
        report.candidate_snapshot().to_json()?
    );
    assert!(report.passes_gate());
    Ok(())
}

#[test]
fn futao_resolver_matches_rejected_diagnostics_and_cycle() -> Result<(), Box<dyn std::error::Error>>
{
    let rust = RustResolverAdapter;
    let futao = FutaoResolverAdapter::new()?;
    let harness = ResolverDifferentialHarness::new(&rust, &futao);
    let input = CompilerInput::new(
        "main.ft",
        [
            CompilerSource::new("main.ft", REJECTED_MAIN),
            CompilerSource::new("library.ft", REJECTED_LIBRARY),
        ],
    );

    let report = harness.run_case("rejected/core", &input)?;

    assert_eq!(
        report.outcome(),
        ResolverDifferentialOutcome::Match,
        "resolver snapshots diverged: {:?}\nreference={}\ncandidate={}",
        report.differences(),
        report.reference_snapshot().to_json()?,
        report.candidate_snapshot().to_json()?
    );
    assert_eq!(report.candidate_snapshot().diagnostics().len(), 6);
    Ok(())
}

#[test]
fn futao_resolver_matches_data_declaration_names_and_type_parameters(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"type Pair<T> = {
  left: T;
  right: T;
};

type Choice<T> =
  | Some(value: T)
  | None();

function main<T>(value: T): T {
  match(value) {
    case Choice.Some(payload) => payload;
    default => value;
  };
}
"#;
    let rust = RustResolverAdapter;
    let futao = FutaoResolverAdapter::new()?;
    let report = ResolverDifferentialHarness::new(&rust, &futao).run_case(
        "accepted/data-declarations",
        &CompilerInput::new("main.ft", [CompilerSource::new("main.ft", source)]),
    )?;

    assert_eq!(
        report.outcome(),
        ResolverDifferentialOutcome::Match,
        "resolver snapshots diverged: {:?}\nreference={}\ncandidate={}",
        report.differences(),
        report.reference_snapshot().to_json()?,
        report.candidate_snapshot().to_json()?
    );
    Ok(())
}

fn accepted_input(reverse: bool) -> CompilerInput {
    let mut sources = vec![
        CompilerSource::new("main.ft", ACCEPTED_MAIN),
        CompilerSource::new("math.ft", ACCEPTED_MATH),
        CompilerSource::new("model.ft", ACCEPTED_MODEL),
    ];
    if reverse {
        sources.reverse();
    }
    CompilerInput::new("main.ft", sources)
}
