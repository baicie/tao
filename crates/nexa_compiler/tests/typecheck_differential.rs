//! Differential coverage for the first Futao type-checker expression kernel.

use nexa_compiler::{
    FutaoTypeCheckerAdapter, RustTypeCheckerAdapter, TypecheckAdapter,
    TypecheckDifferentialHarness, TypecheckDifferentialOutcome, TypecheckInput, TypecheckNode,
    TypecheckNodeKind, TypecheckType, TYPECHECK_SNAPSHOT_SCHEMA_VERSION,
};

fn accepted_input() -> TypecheckInput {
    TypecheckInput::new(
        13,
        vec![
            TypecheckNode::literal(TypecheckNodeKind::Int, 0, 1),
            TypecheckNode::literal(TypecheckNodeKind::Int, 2, 3),
            TypecheckNode::binary(TypecheckNodeKind::Add, 0, 1, 0, 3),
            TypecheckNode::literal(TypecheckNodeKind::Bool, 4, 8),
            TypecheckNode::literal(TypecheckNodeKind::Bool, 9, 13),
            TypecheckNode::binary(TypecheckNodeKind::And, 3, 4, 4, 13),
            TypecheckNode::conditional(3, 2, 2, 4, 13),
            TypecheckNode::return_check(6, TypecheckType::Int, 0, 13),
        ],
    )
}

fn rejected_input() -> TypecheckInput {
    TypecheckInput::new(
        8,
        vec![
            TypecheckNode::literal(TypecheckNodeKind::Bool, 0, 4),
            TypecheckNode::literal(TypecheckNodeKind::Int, 5, 6),
            TypecheckNode::binary(TypecheckNodeKind::Add, 0, 1, 0, 6),
            TypecheckNode::literal(TypecheckNodeKind::Int, 7, 8),
            TypecheckNode::conditional(3, 1, 1, 7, 8),
            TypecheckNode::return_check(1, TypecheckType::Bool, 5, 6),
        ],
    )
}

#[test]
fn rust_type_checker_freezes_the_expression_kernel_schema() -> Result<(), Box<dyn std::error::Error>>
{
    let snapshot = RustTypeCheckerAdapter.check(&accepted_input())?;
    assert_eq!(snapshot.schema_version(), TYPECHECK_SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(
        snapshot.types(),
        &["int", "int", "int", "bool", "bool", "bool", "int", "unit"]
    );
    assert!(snapshot.diagnostics().is_empty());
    Ok(())
}

#[test]
fn futao_type_checker_matches_accepted_and_rejected_inputs(
) -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustTypeCheckerAdapter;
    let futao = FutaoTypeCheckerAdapter::new()?;
    let harness = TypecheckDifferentialHarness::new(&rust, &futao);

    for (id, input) in [
        ("accepted/basic", accepted_input()),
        ("rejected/basic", rejected_input()),
    ] {
        let report = harness.run_case(id, &input)?;
        assert_eq!(report.outcome(), TypecheckDifferentialOutcome::Match);
    }

    let rejected = rust.check(&rejected_input())?;
    assert_eq!(
        rejected
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        ["E3001", "E3003", "E3002"]
    );
    Ok(())
}

#[test]
fn futao_type_checker_accepts_the_maximum_diagnostic_budget(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut nodes = vec![
        TypecheckNode::literal(TypecheckNodeKind::Bool, 0, 1),
        TypecheckNode::literal(TypecheckNodeKind::Int, 1, 2),
    ];
    while nodes.len() < 1024 {
        nodes.push(TypecheckNode::conditional(1, 0, 1, 0, 2));
    }
    let input = TypecheckInput::new(2, nodes);
    let futao = FutaoTypeCheckerAdapter::new()?;
    let snapshot = futao.check(&input)?;

    assert_eq!(snapshot.types().len(), 1024);
    assert_eq!(snapshot.diagnostics().len(), 2044);
    Ok(())
}
