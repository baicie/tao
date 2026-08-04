//! Strict candidate-observation boundary for the 0.0.11 compiler core.

// Rust 1.80 predates stable `#[expect]`; invalid fixture setup should stop immediately.
#![allow(clippy::expect_used)]

use nexa_compiler::{
    compile, CandidateCompilationStatus, CandidateObservation, CandidateObservationErrorCode,
    CandidatePhaseStatus, CanonicalPhase, CompilerImplementation, CompilerInput, CompilerOptions,
    CompilerSource, CANDIDATE_OBSERVATION_MAX_DIAGNOSTICS, CANDIDATE_OBSERVATION_MAX_JSON_DEPTH,
    CANDIDATE_OBSERVATION_MAX_LABELS_PER_DIAGNOSTIC, CANDIDATE_OBSERVATION_MAX_MESSAGE_BYTES,
    CANDIDATE_OBSERVATION_MAX_SOURCES, CANDIDATE_OBSERVATION_MAX_TOTAL_LABELS,
    CANDIDATE_OBSERVATION_SCHEMA_VERSION, FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION,
};
use serde::Serialize;
use serde_json::{json, Value};

const MAIN_SOURCE: &str = "function main(): Unit {}";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ObservationFixture {
    schema_version: u32,
    implementation: String,
    language_version: String,
    compilation_profile: String,
    status: &'static str,
    sources: Vec<SourceFixture>,
    diagnostics: Vec<DiagnosticFixture>,
    artifacts: Vec<ArtifactFixture>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceFixture {
    file: u32,
    identity: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticFixture {
    code: String,
    severity: &'static str,
    message: String,
    labels: Vec<LabelFixture>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LabelFixture {
    style: &'static str,
    file: u32,
    start: u32,
    end: u32,
    message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactFixture {
    phase: &'static str,
    artifact: ArtifactStateFixture,
}

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum ArtifactStateFixture {
    Produced { content: String },
    SkippedDueToDiagnostics,
    Deferred { content: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseEnvelope<'a, T> {
    schema_version: u32,
    phase: &'a str,
    value: T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticShapeFixture {
    code: &'static str,
    severity: &'static str,
    labels: Vec<LabelShapeFixture>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LabelShapeFixture {
    style: &'static str,
    file: u32,
    start: u32,
    end: u32,
}

fn input() -> CompilerInput {
    CompilerInput::new("main.ft", [CompilerSource::new("main.ft", MAIN_SOURCE)])
}

fn bootstrap_input() -> CompilerInput {
    CompilerInput::with_options(
        "main.ft",
        [CompilerSource::new("main.ft", MAIN_SOURCE)],
        CompilerOptions::bootstrap_v1(),
    )
}

fn phase_content(phase: &'static str, value: Value) -> String {
    typed_phase_content(phase, value)
}

fn typed_phase_content<T>(phase: &'static str, value: T) -> String
where
    T: Serialize,
{
    serde_json::to_string(&PhaseEnvelope {
        schema_version: 1,
        phase,
        value,
    })
    .expect("fixture phase envelope serializes")
}

fn produced_phase(phase: &'static str, value: Value) -> ArtifactFixture {
    ArtifactFixture {
        phase,
        artifact: ArtifactStateFixture::Produced {
            content: phase_content(phase, value),
        },
    }
}

fn reference_phase_content(input: &CompilerInput, phase: CanonicalPhase) -> String {
    compile(input)
        .expect("reference fixture compiles")
        .dumps()
        .artifact(phase)
        .content()
        .expect("front-end reference artifact is produced")
        .to_owned()
}

fn replace_frontend_artifacts(fixture: &mut ObservationFixture, input: &CompilerInput) {
    for (index, phase) in [CanonicalPhase::Tokens, CanonicalPhase::Cst]
        .into_iter()
        .enumerate()
    {
        fixture.artifacts[index].artifact = ArtifactStateFixture::Produced {
            content: reference_phase_content(input, phase),
        };
    }
}

fn mutate_phase_content(
    fixture: &mut ObservationFixture,
    phase_index: usize,
    mutate: impl FnOnce(&mut Value),
) {
    let ArtifactStateFixture::Produced { content } = &mut fixture.artifacts[phase_index].artifact
    else {
        unreachable!("front-end fixture artifact is produced")
    };
    let mut value: Value = serde_json::from_str(content).expect("phase fixture is JSON");
    mutate(&mut value);
    *content = serde_json::to_string(&value).expect("mutated phase fixture serializes");
}

fn accepted_fixture() -> ObservationFixture {
    let case_input = input();
    let mut fixture = ObservationFixture {
        schema_version: CANDIDATE_OBSERVATION_SCHEMA_VERSION,
        implementation: FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION.to_owned(),
        language_version: "1.0".to_owned(),
        compilation_profile: "application".to_owned(),
        status: "accepted",
        sources: vec![SourceFixture {
            file: 0,
            identity: "main.ft".to_owned(),
        }],
        diagnostics: Vec::new(),
        artifacts: vec![
            produced_phase("tokens", json!([])),
            produced_phase("cst", json!([])),
            produced_phase("diagnostics", json!([])),
            produced_phase("hir", json!({})),
            produced_phase("mir", json!({})),
            produced_phase("nir", json!({})),
        ],
    };
    replace_frontend_artifacts(&mut fixture, &case_input);
    fixture
}

fn rejected_fixture() -> ObservationFixture {
    let diagnostic = DiagnosticFixture {
        code: "E1001".to_owned(),
        severity: "error",
        message: "unexpected token".to_owned(),
        labels: vec![LabelFixture {
            style: "primary",
            file: 0,
            start: 0,
            end: 1,
            message: "remove this token".to_owned(),
        }],
    };
    let diagnostic_shape = vec![DiagnosticShapeFixture {
        code: "E1001",
        severity: "error",
        labels: vec![LabelShapeFixture {
            style: "primary",
            file: 0,
            start: 0,
            end: 1,
        }],
    }];
    let case_input = CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "@")]);
    let mut fixture = ObservationFixture {
        schema_version: CANDIDATE_OBSERVATION_SCHEMA_VERSION,
        implementation: FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION.to_owned(),
        language_version: "1.0".to_owned(),
        compilation_profile: "application".to_owned(),
        status: "rejected",
        sources: vec![SourceFixture {
            file: 0,
            identity: "main.ft".to_owned(),
        }],
        diagnostics: vec![diagnostic],
        artifacts: vec![
            produced_phase("tokens", json!([])),
            produced_phase("cst", json!([])),
            ArtifactFixture {
                phase: "diagnostics",
                artifact: ArtifactStateFixture::Produced {
                    content: typed_phase_content("diagnostics", diagnostic_shape),
                },
            },
            ArtifactFixture {
                phase: "hir",
                artifact: ArtifactStateFixture::SkippedDueToDiagnostics,
            },
            ArtifactFixture {
                phase: "mir",
                artifact: ArtifactStateFixture::SkippedDueToDiagnostics,
            },
            ArtifactFixture {
                phase: "nir",
                artifact: ArtifactStateFixture::SkippedDueToDiagnostics,
            },
        ],
    };
    replace_frontend_artifacts(&mut fixture, &case_input);
    fixture
}

fn warning_fixture() -> ObservationFixture {
    let mut fixture = accepted_fixture();
    fixture.diagnostics = vec![DiagnosticFixture {
        code: "W0001".to_owned(),
        severity: "warning",
        message: "candidate warning".to_owned(),
        labels: vec![LabelFixture {
            style: "primary",
            file: 0,
            start: 0,
            end: 0,
            message: "warning location".to_owned(),
        }],
    }];
    fixture.artifacts[2] = ArtifactFixture {
        phase: "diagnostics",
        artifact: ArtifactStateFixture::Produced {
            content: typed_phase_content(
                "diagnostics",
                vec![DiagnosticShapeFixture {
                    code: "W0001",
                    severity: "warning",
                    labels: vec![LabelShapeFixture {
                        style: "primary",
                        file: 0,
                        start: 0,
                        end: 0,
                    }],
                }],
            ),
        },
    };
    fixture
}

fn encode(fixture: &ObservationFixture) -> Vec<u8> {
    serde_json::to_vec(fixture).expect("observation fixture serializes")
}

fn error_code(
    fixture: &ObservationFixture,
    input: &CompilerInput,
) -> CandidateObservationErrorCode {
    CandidateObservation::load(&encode(fixture), input)
        .expect_err("mutated observation must fail closed")
        .code()
}

#[test]
fn valid_candidate_observation_exposes_only_validated_protocol_data(
) -> Result<(), Box<dyn std::error::Error>> {
    let observation = CandidateObservation::load(&encode(&accepted_fixture()), &input())?;

    assert_eq!(
        observation.schema_version(),
        CANDIDATE_OBSERVATION_SCHEMA_VERSION
    );
    assert_eq!(
        observation.implementation(),
        FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION
    );
    assert_eq!(observation.language_version(), "1.0");
    assert_eq!(observation.compilation_profile(), "application");
    assert_eq!(observation.status(), CandidateCompilationStatus::Accepted);
    assert_eq!(observation.sources().len(), 1);
    assert_eq!(observation.sources()[0].identity(), "main.ft");
    assert_eq!(observation.sources()[0].byte_length(), MAIN_SOURCE.len());
    assert!(observation.diagnostics().is_empty());
    assert_eq!(observation.artifacts().len(), 6);
    assert!(observation
        .artifacts()
        .iter()
        .all(|phase| phase.status() == CandidatePhaseStatus::Produced));
    Ok(())
}

#[test]
fn error_diagnostics_require_all_later_phases_to_be_blocked(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "@")]);
    let observation = CandidateObservation::load(&encode(&rejected_fixture()), &source)?;

    assert_eq!(observation.diagnostics().len(), 1);
    assert_eq!(observation.diagnostics()[0].code(), "E1001");
    assert_eq!(observation.diagnostics()[0].labels()[0].file(), 0);
    assert_eq!(
        observation
            .artifacts()
            .iter()
            .skip(3)
            .map(|phase| phase.status())
            .collect::<Vec<_>>(),
        [
            CandidatePhaseStatus::Blocked,
            CandidatePhaseStatus::Blocked,
            CandidatePhaseStatus::Blocked,
        ]
    );
    Ok(())
}

#[test]
fn valid_observation_can_defer_only_nir_with_a_structured_reason(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = accepted_fixture();
    fixture.artifacts[5].artifact = ArtifactStateFixture::Deferred {
        content: phase_content(
            "nir",
            json!({
                "reasonCode": "NIR_STRING",
                "plannedPhase": "0.0.11-nir-string-array"
            }),
        ),
    };

    let observation = CandidateObservation::load(&encode(&fixture), &input())?;

    assert_eq!(
        observation.artifacts()[5].status(),
        CandidatePhaseStatus::Deferred
    );
    assert!(observation.artifacts()[5]
        .content()
        .is_some_and(|content| content.contains("NIR_STRING")));
    Ok(())
}

#[test]
fn warning_only_observation_remains_accepted_and_produces_later_phases(
) -> Result<(), Box<dyn std::error::Error>> {
    let observation = CandidateObservation::load(&encode(&warning_fixture()), &input())?;

    assert_eq!(observation.status(), CandidateCompilationStatus::Accepted);
    assert_eq!(observation.diagnostics().len(), 1);
    assert!(observation
        .artifacts()
        .iter()
        .skip(3)
        .all(|artifact| artifact.status() == CandidatePhaseStatus::Produced));
    Ok(())
}

#[test]
fn implementation_identity_reserves_self_hosted_until_stage_one() {
    assert_eq!(
        CompilerImplementation::FutaoBootstrapCandidate.as_str(),
        FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION
    );

    let mut fixture = accepted_fixture();
    fixture.implementation = "futao-self-hosted".to_owned();
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidImplementation
    );
}

#[test]
fn observation_rejects_unknown_fields_and_schema_versions() {
    let mut value = serde_json::to_value(accepted_fixture()).expect("fixture is JSON");
    value["hostPath"] = json!("/tmp/main.ft");
    let bytes = serde_json::to_vec(&value).expect("mutated fixture serializes");
    let error =
        CandidateObservation::load(&bytes, &input()).expect_err("unknown fields must fail closed");
    assert_eq!(error.code(), CandidateObservationErrorCode::InvalidJson);

    let mut value = serde_json::to_value(accepted_fixture()).expect("fixture is JSON");
    value["artifacts"][0]["artifact"]["hostCallback"] = json!(true);
    let bytes = serde_json::to_vec(&value).expect("mutated fixture serializes");
    let error = CandidateObservation::load(&bytes, &input())
        .expect_err("unknown artifact fields must fail closed");
    assert_eq!(error.code(), CandidateObservationErrorCode::InvalidJson);

    let mut fixture = accepted_fixture();
    fixture.schema_version += 1;
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::UnsupportedSchema
    );
}

#[test]
fn token_and_cst_payloads_reject_malformed_protocol_data() {
    let mut fixture = accepted_fixture();
    mutate_phase_content(&mut fixture, 0, |content| {
        content["value"] = json!([]);
    });
    mutate_phase_content(&mut fixture, 1, |content| {
        content["value"] = json!([]);
    });
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseContent
    );

    let mut fixture = accepted_fixture();
    mutate_phase_content(&mut fixture, 0, |content| {
        content["value"][0]["tokens"][0]["hostPath"] = json!("/tmp/main.ft");
    });
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseContent
    );

    let mut fixture = accepted_fixture();
    mutate_phase_content(&mut fixture, 0, |content| {
        content["value"][0]["tokens"][0]["kindId"] = json!(39);
    });
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseContent
    );

    let mut fixture = accepted_fixture();
    mutate_phase_content(&mut fixture, 0, |content| {
        content["value"][0]["tokens"][0]["end"] = json!(u32::MAX);
    });
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseContent
    );

    let mut fixture = accepted_fixture();
    mutate_phase_content(&mut fixture, 0, |content| {
        content["value"][0]["tokens"][0]["text"] = json!("fabricated");
    });
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseContent
    );

    let mut fixture = accepted_fixture();
    mutate_phase_content(&mut fixture, 1, |content| {
        content["value"][0]["elements"][0]["text"] = json!("fabricated");
    });
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseContent
    );
}

#[test]
fn resource_limit_marker_cannot_be_spoofed_by_unknown_json() {
    let mut value = serde_json::to_value(accepted_fixture()).expect("fixture is JSON");
    value["candidate-resource-limit"] = json!(true);
    let bytes = serde_json::to_vec(&value).expect("mutated fixture serializes");

    let error = CandidateObservation::load(&bytes, &input())
        .expect_err("unknown root fields are schema errors, not resource errors");

    assert_eq!(error.code(), CandidateObservationErrorCode::InvalidJson);
}

#[test]
fn observation_must_match_the_explicit_language_and_profile() {
    let mut fixture = accepted_fixture();
    fixture.language_version = "2.0".to_owned();
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InputMismatch
    );

    let mut fixture = accepted_fixture();
    fixture.compilation_profile = "application".to_owned();
    assert_eq!(
        error_code(&fixture, &bootstrap_input()),
        CandidateObservationErrorCode::InputMismatch
    );
}

#[test]
fn source_table_must_cover_the_exact_input_and_start_with_the_entry() {
    let multi_input = CompilerInput::new(
        "main.ft",
        [
            CompilerSource::new("main.ft", MAIN_SOURCE),
            CompilerSource::new("support.ft", "export function value(): Int { return 1; }"),
        ],
    );
    let mut fixture = accepted_fixture();
    fixture.sources.push(SourceFixture {
        file: 1,
        identity: "support.ft".to_owned(),
    });
    fixture.sources.swap(0, 1);
    assert_eq!(
        error_code(&fixture, &multi_input),
        CandidateObservationErrorCode::InvalidSourceTable
    );

    let mut fixture = accepted_fixture();
    fixture.sources[0].file = 1;
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidSourceTable
    );

    let mut fixture = accepted_fixture();
    fixture.sources.push(SourceFixture {
        file: 1,
        identity: "main.ft".to_owned(),
    });
    assert_eq!(
        error_code(&fixture, &multi_input),
        CandidateObservationErrorCode::InvalidSourceTable
    );

    let mut fixture = accepted_fixture();
    fixture.sources.push(SourceFixture {
        file: 1,
        identity: "other.ft".to_owned(),
    });
    assert_eq!(
        error_code(&fixture, &multi_input),
        CandidateObservationErrorCode::InvalidSourceTable
    );
}

#[test]
fn all_six_phases_are_required_once_in_canonical_order() {
    let mut fixture = accepted_fixture();
    fixture.artifacts.swap(0, 1);
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseTable
    );

    let mut fixture = accepted_fixture();
    let _ = fixture.artifacts.pop();
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseTable
    );
}

#[test]
fn phase_state_chain_rejects_fabricated_or_derivative_output() {
    let mut fixture = rejected_fixture();
    fixture.artifacts[3] = produced_phase("hir", json!({}));
    assert_eq!(
        error_code(
            &fixture,
            &CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "@")])
        ),
        CandidateObservationErrorCode::InvalidPhaseTable
    );

    let mut fixture = accepted_fixture();
    fixture.artifacts[3].artifact = ArtifactStateFixture::SkippedDueToDiagnostics;
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseTable
    );

    let mut fixture = accepted_fixture();
    fixture.artifacts[4].artifact = ArtifactStateFixture::Deferred {
        content: phase_content("mir", json!({})),
    };
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseTable
    );
}

#[test]
fn every_phase_content_envelope_must_name_its_outer_phase_and_schema() {
    let mut fixture = accepted_fixture();
    fixture.artifacts[0].artifact = ArtifactStateFixture::Produced {
        content: phase_content("cst", json!([])),
    };
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseContent
    );

    let mut fixture = accepted_fixture();
    fixture.artifacts[0].artifact = ArtifactStateFixture::Produced {
        content: "{\"schemaVersion\":2,\"phase\":\"tokens\",\"value\":[]}".to_owned(),
    };
    assert_eq!(
        error_code(&fixture, &input()),
        CandidateObservationErrorCode::InvalidPhaseContent
    );
}

#[test]
fn structured_diagnostics_and_diagnostic_phase_must_match_exactly() {
    let source = CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "@")]);
    let mut fixture = rejected_fixture();
    fixture.diagnostics[0].code = "E2001".to_owned();
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidPhaseContent
    );

    let mut fixture = rejected_fixture();
    fixture.diagnostics[0].severity = "warning";
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidDiagnostic
    );
}

#[test]
fn diagnostic_spans_must_reference_exact_utf8_source_boundaries() {
    let utf8_input = CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "涛")]);
    let mut fixture = rejected_fixture();
    fixture.diagnostics[0].labels[0].end = 1;
    let ArtifactStateFixture::Produced { content } = &mut fixture.artifacts[2].artifact else {
        unreachable!("diagnostic fixture produces its diagnostic phase")
    };
    *content = phase_content(
        "diagnostics",
        json!([{
            "code": "E1001",
            "severity": "error",
            "labels": [{
                "style": "primary",
                "file": 0,
                "start": 0,
                "end": 1
            }]
        }]),
    );
    assert_eq!(
        error_code(&fixture, &utf8_input),
        CandidateObservationErrorCode::InvalidDiagnostic
    );

    let source = CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "@")]);
    let mut fixture = rejected_fixture();
    fixture.diagnostics[0].labels[0].file = 1;
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidDiagnostic
    );

    let mut fixture = rejected_fixture();
    fixture.diagnostics[0].labels[0].start = 1;
    fixture.diagnostics[0].labels[0].end = 0;
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidDiagnostic
    );

    let mut fixture = rejected_fixture();
    fixture.diagnostics[0].labels[0].end = 2;
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidDiagnostic
    );
}

#[test]
fn diagnostics_require_stable_codes_and_one_ordered_primary_label() {
    let source = CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "@")]);
    let mut fixture = rejected_fixture();
    fixture.diagnostics[0].code = "ERROR".to_owned();
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidDiagnostic
    );

    let mut fixture = rejected_fixture();
    fixture.diagnostics[0].labels[0].style = "secondary";
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidDiagnostic
    );

    let mut fixture = rejected_fixture();
    let mut extra = fixture.diagnostics[0].labels[0].clone();
    extra.style = "primary";
    fixture.diagnostics[0].labels.push(extra);
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidDiagnostic
    );
}

#[test]
fn candidate_resource_dimensions_reject_one_over_the_frozen_bound() {
    let source = CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "@")]);

    let mut fixture = rejected_fixture();
    fixture.diagnostics =
        vec![fixture.diagnostics[0].clone(); CANDIDATE_OBSERVATION_MAX_DIAGNOSTICS + 1];
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::ResourceLimit
    );

    let mut fixture = rejected_fixture();
    let mut secondary = fixture.diagnostics[0].labels[0].clone();
    secondary.style = "secondary";
    fixture.diagnostics[0].labels =
        vec![secondary; CANDIDATE_OBSERVATION_MAX_LABELS_PER_DIAGNOSTIC + 1];
    fixture.diagnostics[0].labels[0].style = "primary";
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::ResourceLimit
    );

    let mut fixture = rejected_fixture();
    fixture.diagnostics[0].message = "x".repeat(CANDIDATE_OBSERVATION_MAX_MESSAGE_BYTES + 1);
    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::ResourceLimit
    );

    let sources = (0..=CANDIDATE_OBSERVATION_MAX_SOURCES)
        .map(|index| {
            let identity = if index == 0 {
                "main.ft".to_owned()
            } else {
                format!("source-{index}.ft")
            };
            CompilerSource::new(identity, "")
        })
        .collect::<Vec<_>>();
    let oversized_input = CompilerInput::new("main.ft", sources);
    assert_eq!(
        error_code(&accepted_fixture(), &oversized_input),
        CandidateObservationErrorCode::ResourceLimit
    );
}

#[test]
fn aggregate_label_bound_stops_before_decoding_later_invalid_data() {
    let source = CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "@")]);
    let mut fixture = rejected_fixture();
    let mut diagnostic = fixture.diagnostics[0].clone();
    let mut secondary = diagnostic.labels[0].clone();
    secondary.style = "secondary";
    diagnostic.labels = vec![secondary; CANDIDATE_OBSERVATION_MAX_LABELS_PER_DIAGNOSTIC];
    diagnostic.labels[0].style = "primary";
    let diagnostics_at_bound =
        CANDIDATE_OBSERVATION_MAX_TOTAL_LABELS / CANDIDATE_OBSERVATION_MAX_LABELS_PER_DIAGNOSTIC;
    fixture.diagnostics = vec![diagnostic; diagnostics_at_bound];

    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidPhaseContent,
        "the exact aggregate label bound must reach phase validation"
    );

    let mut value = serde_json::to_value(fixture).expect("fixture is JSON");
    let diagnostics = value["diagnostics"]
        .as_array_mut()
        .expect("diagnostics fixture is an array");
    diagnostics.push(json!({
        "code": "E1001",
        "severity": "error",
        "message": "one over",
        "labels": [{
            "style": "primary",
            "file": 0,
            "start": 0,
            "end": 1,
            "message": "one over"
        }]
    }));
    diagnostics.push(json!({"unexpected": true}));
    let bytes = serde_json::to_vec(&value).expect("mutated fixture serializes");

    let error = CandidateObservation::load(&bytes, &source)
        .expect_err("aggregate label overflow must stop before later invalid data");

    assert_eq!(error.code(), CandidateObservationErrorCode::ResourceLimit);
}

#[test]
fn phase_payload_depth_accepts_the_exact_bound_and_rejects_one_over() {
    fn nested_hir_content(value_depth: usize) -> String {
        let value = format!(
            "{}{}{}",
            "{\"nested\":".repeat(value_depth.saturating_sub(1)),
            "{}",
            "}".repeat(value_depth.saturating_sub(1))
        );
        format!(r#"{{"schemaVersion":1,"phase":"hir","value":{value}}}"#)
    }

    let mut exact = accepted_fixture();
    exact.artifacts[3].artifact = ArtifactStateFixture::Produced {
        content: nested_hir_content(CANDIDATE_OBSERVATION_MAX_JSON_DEPTH - 1),
    };
    CandidateObservation::load(&encode(&exact), &input())
        .expect("the exact iterative JSON depth bound must be accepted");

    let mut over = accepted_fixture();
    over.artifacts[3].artifact = ArtifactStateFixture::Produced {
        content: nested_hir_content(CANDIDATE_OBSERVATION_MAX_JSON_DEPTH),
    };
    assert_eq!(
        error_code(&over, &input()),
        CandidateObservationErrorCode::ResourceLimit
    );
}

#[test]
fn diagnostics_must_use_the_canonical_total_order() {
    let source = CompilerInput::new("main.ft", [CompilerSource::new("main.ft", "@@")]);
    let mut fixture = rejected_fixture();
    let mut later = fixture.diagnostics[0].clone();
    later.labels[0].start = 1;
    later.labels[0].end = 2;
    fixture.diagnostics.insert(0, later);
    let shapes = json!([
        {
            "code": "E1001",
            "severity": "error",
            "labels": [{"style": "primary", "file": 0, "start": 1, "end": 2}]
        },
        {
            "code": "E1001",
            "severity": "error",
            "labels": [{"style": "primary", "file": 0, "start": 0, "end": 1}]
        }
    ]);
    fixture.artifacts[2] = produced_phase("diagnostics", shapes);

    assert_eq!(
        error_code(&fixture, &source),
        CandidateObservationErrorCode::InvalidDiagnostic
    );
}
