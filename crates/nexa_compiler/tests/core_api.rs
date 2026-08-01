//! Explicit-input compiler core and canonical output contracts.

use nexa_compiler::{
    compile, CanonicalArtifactStatus, CanonicalPhase, CompilerInput, CompilerOutput, CompilerSource,
};
use nexa_nir::{ArtifactCompatibility, CanonicalArtifact as NirArtifact};
use serde_json::{json, Value};

const FUTAO_MAIN: &str = include_str!("fixtures/differential/accepted/main.ft");
const FUTAO_SUPPORT: &str = include_str!("fixtures/differential/accepted/support.ft");

fn accepted_output() -> Result<CompilerOutput, nexa_compiler::CompileError> {
    compile(&CompilerInput::new(
        "app/main.ft",
        [
            CompilerSource::new("app/main.ft", FUTAO_MAIN),
            CompilerSource::new("app/support.ft", FUTAO_SUPPORT),
        ],
    ))
}

fn phase_value(
    output: &CompilerOutput,
    phase: CanonicalPhase,
) -> Result<Value, Box<dyn std::error::Error>> {
    let content = output
        .dumps()
        .artifact(phase)
        .content()
        .ok_or_else(|| std::io::Error::other("expected produced canonical artifact"))?;
    let envelope: Value = serde_json::from_str(content)?;
    envelope
        .get("value")
        .cloned()
        .ok_or_else(|| std::io::Error::other("canonical envelope is missing value").into())
}

#[test]
fn explicit_core_contains_no_host_or_nondeterministic_inputs() {
    let core = include_str!("../src/core.rs");
    for forbidden in [
        "std::fs",
        "std::env",
        "SystemTime",
        "Instant::now",
        "thread_rng",
        "current_dir",
    ] {
        assert!(
            !core.contains(forbidden),
            "compiler core must not use `{forbidden}`"
        );
    }
}

#[test]
fn explicit_core_compiles_futao_sources_without_host_state(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = accepted_output()?;

    assert!(output.is_ok(), "diagnostics: {:?}", output.diagnostics());
    assert!(output.mir().is_some());
    for phase in [
        CanonicalPhase::Tokens,
        CanonicalPhase::Cst,
        CanonicalPhase::Diagnostics,
        CanonicalPhase::Hir,
        CanonicalPhase::Mir,
    ] {
        assert_eq!(
            output.dumps().artifact(phase).status(),
            CanonicalArtifactStatus::Available
        );
    }
    assert_eq!(
        output.dumps().artifact(CanonicalPhase::Nir).status(),
        CanonicalArtifactStatus::Available
    );
    assert!(output.nir().is_some());
    let artifact = output
        .nir_artifact()
        .ok_or_else(|| std::io::Error::other("expected internal NIR artifact"))?;
    NirArtifact::deserialize(
        artifact,
        &ArtifactCompatibility::exact(env!("CARGO_PKG_VERSION")),
    )?;

    Ok(())
}

#[test]
fn canonical_nir_schema_has_a_small_exact_golden() -> Result<(), Box<dyn std::error::Error>> {
    let value = phase_value(&accepted_output()?, CanonicalPhase::Nir)?;
    let functions = value["module"]["functions"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("expected NIR function array"))?;

    assert_eq!(
        serde_json::json!({
            "magic": value["magic"].clone(),
            "schema": value["nirSchemaVersion"].clone(),
            "target": value["targetProfile"].clone(),
            "functions": functions.iter().map(|function| function["name"].clone()).collect::<Vec<_>>()
        }),
        json!({
            "magic": "FUTAO-NIR",
            "schema": 1,
            "target": "target-neutral-v1",
            "functions": ["m0::main", "m1::answer"]
        })
    );
    Ok(())
}

#[test]
fn unsupported_post_n1_mir_is_deferred_without_fabricating_nir(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = compile(&CompilerInput::new(
        "main.ft",
        [CompilerSource::new(
            "main.ft",
            "function main(): Unit { print(\"later layout\"); }",
        )],
    ))?;

    assert!(output.is_ok());
    assert!(output.nir().is_none());
    assert!(output.nir_artifact().is_none());
    assert_eq!(
        output.dumps().artifact(CanonicalPhase::Nir).status(),
        CanonicalArtifactStatus::Deferred
    );
    assert_eq!(
        phase_value(&output, CanonicalPhase::Nir)?,
        json!({"reasonCode": "unsupported-type", "plannedPhase": "ADR-003 N4"})
    );
    Ok(())
}

#[test]
fn canonical_token_schema_has_a_small_exact_golden() -> Result<(), Box<dyn std::error::Error>> {
    let value = phase_value(&accepted_output()?, CanonicalPhase::Tokens)?;
    let tokens = value[0]["tokens"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("expected token array"))?;

    assert_eq!(
        Value::Array(tokens[..4].to_vec()),
        json!([
            {"kindId": 92, "start": 0, "end": 6, "text": "import"},
            {"kindId": 2, "start": 6, "end": 7, "text": " "},
            {"kindId": 6, "start": 7, "end": 8, "text": "{"},
            {"kindId": 2, "start": 8, "end": 9, "text": " "}
        ])
    );
    Ok(())
}

#[test]
fn canonical_cst_schema_has_a_small_exact_golden() -> Result<(), Box<dyn std::error::Error>> {
    let value = phase_value(&accepted_output()?, CanonicalPhase::Cst)?;
    let elements = value[0]["elements"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("expected CST element array"))?;

    assert_eq!(
        Value::Array(elements[..4].to_vec()),
        json!([
            {"depth": 0, "element": "node", "kindId": 39, "start": 0, "end": 85},
            {"depth": 1, "element": "node", "kindId": 95, "start": 0, "end": 38},
            {"depth": 2, "element": "token", "kindId": 92, "start": 0, "end": 6, "text": "import"},
            {"depth": 2, "element": "token", "kindId": 2, "start": 6, "end": 7, "text": " "}
        ])
    );
    Ok(())
}

#[test]
fn canonical_diagnostic_schema_has_a_small_exact_golden() -> Result<(), Box<dyn std::error::Error>>
{
    let source = include_str!("fixtures/differential/rejected/invalid_import.ft");
    let output = compile(&CompilerInput::new(
        "rejected/main.ft",
        [CompilerSource::new("rejected/main.ft", source)],
    ))?;

    assert_eq!(
        phase_value(&output, CanonicalPhase::Diagnostics)?,
        json!([{
            "code": "E4001",
            "severity": "error",
            "labels": [{"style": "primary", "file": 0, "start": 23, "end": 37}]
        }])
    );
    Ok(())
}

#[test]
fn canonical_hir_schema_has_a_small_exact_golden() -> Result<(), Box<dyn std::error::Error>> {
    let value = phase_value(&accepted_output()?, CanonicalPhase::Hir)?;
    let actual = json!({
        "entryModule": value["entryModule"].clone(),
        "moduleFunctionNames": [
            value["modules"][0]["functions"][0]["name"]["text"].clone(),
            value["modules"][1]["functions"][0]["name"]["text"].clone()
        ],
        "functions": value["facts"]["functions"].clone(),
        "calls": value["facts"]["calls"].clone()
    });

    assert_eq!(
        actual,
        json!({
            "entryModule": 0,
            "moduleFunctionNames": ["main", "answer"],
            "functions": [
                {
                    "id": {"module": 0, "index": 0},
                    "typeParameters": [],
                    "parameters": [],
                    "parameterTypes": [],
                    "returnType": {"kind": "unit"},
                    "localCount": 0
                },
                {
                    "id": {"module": 1, "index": 0},
                    "typeParameters": [],
                    "parameters": [],
                    "parameterTypes": [],
                    "returnType": {"kind": "int"},
                    "localCount": 0
                }
            ],
            "calls": [{
                "span": {"source": 0, "start": 72, "end": 80},
                "function": {"module": 1, "index": 0},
                "typeArguments": []
            }]
        })
    );
    Ok(())
}

#[test]
fn canonical_mir_schema_has_a_small_exact_golden() -> Result<(), Box<dyn std::error::Error>> {
    let value = phase_value(&accepted_output()?, CanonicalPhase::Mir)?;
    let actual = json!({
        "entryModule": value["entryModule"].clone(),
        "entryFunction": value["entryFunction"].clone(),
        "mainFrame": {
            "id": value["functions"][0]["id"].clone(),
            "name": value["functions"][0]["name"].clone(),
            "parameters": value["functions"][0]["parameters"].clone(),
            "localCount": value["functions"][0]["localCount"].clone(),
            "returnType": value["functions"][0]["returnType"].clone(),
            "entryBlock": value["functions"][0]["entryBlock"].clone()
        },
        "supportReturn": value["functions"][1]["blocks"][0]["terminator"].clone()
    });

    assert_eq!(
        actual,
        json!({
            "entryModule": 0,
            "entryFunction": {"module": 0, "index": 0},
            "mainFrame": {
                "id": {"module": 0, "index": 0},
                "name": "main",
                "parameters": [],
                "localCount": 2,
                "returnType": {"kind": "unit"},
                "entryBlock": 0
            },
            "supportReturn": {
                "kind": "return",
                "value": {"kind": "integer", "value": 42, "span": {"source": 1, "start": 41, "end": 43}},
                "span": {"source": 1, "start": 34, "end": 44}
            }
        })
    );
    Ok(())
}

#[test]
fn explicit_core_retains_nexa_and_mixed_extension_compatibility(
) -> Result<(), Box<dyn std::error::Error>> {
    for input in [
        CompilerInput::new(
            "legacy/main.nexa",
            [CompilerSource::new(
                "legacy/main.nexa",
                "function main(): Unit {}",
            )],
        ),
        CompilerInput::new(
            "mixed/main.ft",
            [
                CompilerSource::new(
                    "mixed/main.ft",
                    "import { answer } from \"./support.nexa\";\nfunction main(): Unit { print(answer()); }",
                ),
                CompilerSource::new(
                    "mixed/support.nexa",
                    "export function answer(): Int { return 42; }",
                ),
            ],
        ),
    ] {
        let output = compile(&input)?;
        assert!(output.is_ok(), "diagnostics: {:?}", output.diagnostics());
    }

    Ok(())
}

#[test]
fn canonical_output_ignores_input_order_and_logical_root_spelling(
) -> Result<(), Box<dyn std::error::Error>> {
    let first = compile(&CompilerInput::new(
        "first/main.ft",
        [
            CompilerSource::new("first/main.ft", FUTAO_MAIN),
            CompilerSource::new("first/support.ft", FUTAO_SUPPORT),
        ],
    ))?;
    let reordered = compile(&CompilerInput::new(
        "second/main.ft",
        [
            CompilerSource::new("second/support.ft", FUTAO_SUPPORT),
            CompilerSource::new("second/main.ft", FUTAO_MAIN),
        ],
    ))?;

    assert_eq!(first.diagnostics(), reordered.diagnostics());
    assert_eq!(first.dumps(), reordered.dumps());

    Ok(())
}

#[test]
fn canonical_output_is_repeatable_for_large_graphs_and_randomized_maps(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut sources = Vec::new();
    for index in 0..32 {
        let content = if index == 31 {
            format!("export function value{index}(): Int {{ return 42; }}")
        } else {
            format!(
                "import {{ value{} }} from \"./module{}.ft\";\nexport function value{index}(): Int {{ return value{}(); }}",
                index + 1,
                index + 1,
                index + 1
            )
        };
        sources.push(CompilerSource::new(
            format!("chain/module{index}.ft"),
            content,
        ));
    }
    let forward = compile(&CompilerInput::new("chain/module0.ft", sources.clone()))?;
    sources.reverse();
    let reverse = compile(&CompilerInput::new("chain/module0.ft", sources))?;
    let repeated = compile(&CompilerInput::new(
        "chain/module0.ft",
        (0..32).map(|index| {
            let content = if index == 31 {
                format!("export function value{index}(): Int {{ return 42; }}")
            } else {
                format!(
                    "import {{ value{} }} from \"./module{}.ft\";\nexport function value{index}(): Int {{ return value{}(); }}",
                    index + 1,
                    index + 1,
                    index + 1
                )
            };
            CompilerSource::new(format!("chain/module{index}.ft"), content)
        }),
    ))?;

    assert_eq!(forward.dumps(), reverse.dumps());
    assert_eq!(forward.dumps(), repeated.dumps());
    Ok(())
}

#[test]
fn rejected_canonical_output_excludes_provider_specific_paths(
) -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "import { answer } from \"./missing.ft\";\nfunction main(): Unit { print(answer()); }";
    let first = compile(&CompilerInput::new(
        "one/main.ft",
        [CompilerSource::new("one/main.ft", source)],
    ))?;
    let second = compile(&CompilerInput::new(
        "another/root/main.ft",
        [CompilerSource::new("another/root/main.ft", source)],
    ))?;

    assert_eq!(first.diagnostics(), second.diagnostics());
    assert_eq!(first.dumps(), second.dumps());
    assert!(!first.dumps().to_json()?.contains("one/"));
    assert!(!second.dumps().to_json()?.contains("another/"));
    Ok(())
}

#[test]
fn rejected_fixture_has_canonical_code_and_byte_span() -> Result<(), Box<dyn std::error::Error>> {
    let source = include_str!("fixtures/differential/rejected/invalid_import.ft");
    let output = compile(&CompilerInput::new(
        "rejected/main.ft",
        [CompilerSource::new("rejected/main.ft", source)],
    ))?;
    let diagnostic = output
        .diagnostics()
        .first()
        .ok_or_else(|| std::io::Error::other("expected a diagnostic"))?;
    let label = diagnostic
        .labels()
        .first()
        .ok_or_else(|| std::io::Error::other("expected a diagnostic label"))?;
    let expected_start = u32::try_from(
        source
            .find("\"./support.ts\"")
            .ok_or_else(|| std::io::Error::other("fixture path literal is missing"))?,
    )?;
    let expected_end = expected_start + u32::try_from("\"./support.ts\"".len())?;

    assert_eq!(diagnostic.code(), "E4001");
    assert_eq!(label.file(), 0);
    assert_eq!(label.start(), expected_start);
    assert_eq!(label.end(), expected_end);
    assert_eq!(
        output.dumps().artifact(CanonicalPhase::Hir).status(),
        CanonicalArtifactStatus::BlockedByDiagnostics
    );

    Ok(())
}

#[test]
fn explicit_input_rejects_non_canonical_duplicate_and_missing_sources() {
    let invalid = compile(&CompilerInput::new(
        "app/../main.ft",
        [CompilerSource::new(
            "app/../main.ft",
            "function main(): Unit {}",
        )],
    ));
    assert!(matches!(
        invalid,
        Err(nexa_compiler::CompileError::InvalidSourceIdentity { .. })
    ));

    let duplicate = compile(&CompilerInput::new(
        "main.ft",
        [
            CompilerSource::new("main.ft", "function main(): Unit {}"),
            CompilerSource::new("main.ft", "function main(): Unit { print(1); }"),
        ],
    ));
    assert!(matches!(
        duplicate,
        Err(nexa_compiler::CompileError::DuplicateSource { .. })
    ));

    let missing = compile(&CompilerInput::new(
        "main.ft",
        [CompilerSource::new(
            "support.ft",
            "export function answer(): Int { return 42; }",
        )],
    ));
    assert!(matches!(
        missing,
        Err(nexa_compiler::CompileError::MissingEntry { .. })
    ));
}
