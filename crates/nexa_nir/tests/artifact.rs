//! Internal NIR artifact compatibility, integrity, and canonicalization coverage.

#![expect(
    clippy::expect_used,
    reason = "test setup failures should stop at the violated fixture assumption"
)]

use nexa_nir::{
    ArtifactCompatibility, ArtifactErrorCode, ArtifactMetadata, BlockId, CanonicalArtifact,
    FunctionId, InstructionId, ModuleBuilder, NirSpan, NirType, Operation, Terminator, TypeId,
    TypedValue, ValueId, Verifier, NIR_ARTIFACT_MAGIC, NIR_SCHEMA_VERSION,
};
use serde_json::Value;

const MAIN: FunctionId = FunctionId::new(0);
const ENTRY: BlockId = BlockId::new(0);
const SPAN: NirSpan = NirSpan::new(0, 4, 6);

fn verified_module(reverse_insertion: bool) -> nexa_nir::VerifiedModule {
    let mut module = ModuleBuilder::new("app/main").expect("module identity is valid");
    let types = [
        (TypeId::new(0), NirType::I64),
        (TypeId::new(1), NirType::Unit),
    ];
    if reverse_insertion {
        for (id, ty) in types.into_iter().rev() {
            module.define_type(id, ty).expect("type id is unique");
        }
    } else {
        for (id, ty) in types {
            module.define_type(id, ty).expect("type id is unique");
        }
    }
    module
        .define_function(MAIN, "main", [], TypeId::new(0), ENTRY)
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [])
        .expect("entry block is unique");
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(0),
            Some(TypedValue::new(ValueId::new(0), TypeId::new(0))),
            Operation::ConstI64 { value: 42 },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .set_terminator(
            MAIN,
            ENTRY,
            Terminator::Return {
                value: Some(ValueId::new(0)),
            },
            SPAN,
        )
        .expect("terminator is unique");
    Verifier::verify(module.finish()).expect("module should verify")
}

fn metadata() -> ArtifactMetadata {
    ArtifactMetadata::new("0.0.5", "target-neutral-v1", ["ssa-v1", "ownership-v1"])
        .expect("metadata is canonical")
}

fn compatibility() -> ArtifactCompatibility {
    ArtifactCompatibility::exact("0.0.5")
}

#[test]
fn canonical_artifact_round_trip_preserves_exact_bytes() {
    let bytes = CanonicalArtifact::serialize(&verified_module(false), &metadata())
        .expect("verified NIR serializes");
    let decoded = CanonicalArtifact::deserialize(&bytes, &compatibility())
        .expect("canonical artifact should load");
    let round_trip =
        CanonicalArtifact::serialize(&decoded, &metadata()).expect("decoded NIR serializes");

    assert_eq!(round_trip, bytes);
}

#[test]
fn canonical_artifact_contains_the_required_private_header() {
    let bytes = CanonicalArtifact::serialize(&verified_module(false), &metadata())
        .expect("verified NIR serializes");
    let value: Value = serde_json::from_slice(&bytes).expect("artifact is JSON");

    assert_eq!(
        (
            value["magic"].as_str(),
            value["nirSchemaVersion"].as_u64(),
            value["compilerVersion"].as_str(),
            value["targetProfile"].as_str(),
        ),
        (
            Some(NIR_ARTIFACT_MAGIC),
            Some(u64::from(NIR_SCHEMA_VERSION)),
            Some("0.0.5"),
            Some("target-neutral-v1"),
        )
    );
}

#[test]
fn canonical_bytes_ignore_builder_insertion_order() {
    let forward = CanonicalArtifact::serialize(&verified_module(false), &metadata())
        .expect("forward module serializes");
    let reverse = CanonicalArtifact::serialize(&verified_module(true), &metadata())
        .expect("reverse module serializes");

    assert_eq!(reverse, forward);
}

#[test]
fn artifact_rejects_an_unsupported_schema_version() {
    let bytes = CanonicalArtifact::serialize(&verified_module(false), &metadata())
        .expect("verified NIR serializes");
    let mut value: Value = serde_json::from_slice(&bytes).expect("artifact is JSON");
    value["nirSchemaVersion"] = serde_json::json!(99);
    let bytes = serde_json::to_vec(&value).expect("mutated artifact serializes");

    let error = CanonicalArtifact::deserialize(&bytes, &compatibility())
        .expect_err("unknown schema versions must fail closed");

    assert_eq!(error.code(), ArtifactErrorCode::UnsupportedSchema);
}

#[test]
fn artifact_rejects_an_incompatible_compiler_version() {
    let bytes = CanonicalArtifact::serialize(&verified_module(false), &metadata())
        .expect("verified NIR serializes");

    let error = CanonicalArtifact::deserialize(&bytes, &ArtifactCompatibility::exact("0.0.4"))
        .expect_err("compiler-specific NIR must fail closed");

    assert_eq!(error.code(), ArtifactErrorCode::IncompatibleCompiler);
}

#[test]
fn artifact_rejects_unknown_json_fields() {
    let bytes = CanonicalArtifact::serialize(&verified_module(false), &metadata())
        .expect("verified NIR serializes");
    let mut value: Value = serde_json::from_slice(&bytes).expect("artifact is JSON");
    value["publicExtension"] = serde_json::json!(".nexc");
    let bytes = serde_json::to_vec(&value).expect("mutated artifact serializes");

    let error = CanonicalArtifact::deserialize(&bytes, &compatibility())
        .expect_err("unknown fields must fail closed");

    assert_eq!(error.code(), ArtifactErrorCode::InvalidJson);
}

#[test]
fn artifact_rejects_mutated_content() {
    let bytes = CanonicalArtifact::serialize(&verified_module(false), &metadata())
        .expect("verified NIR serializes");
    let mut value: Value = serde_json::from_slice(&bytes).expect("artifact is JSON");
    value["module"]["functions"][0]["blocks"][0]["instructions"][0]["operation"]["value"] =
        serde_json::json!(43);
    let bytes = serde_json::to_vec(&value).expect("mutated artifact serializes");

    let error = CanonicalArtifact::deserialize(&bytes, &compatibility())
        .expect_err("content mutation must invalidate the digest");

    assert_eq!(error.code(), ArtifactErrorCode::ContentHashMismatch);
}

#[test]
fn artifact_rejects_noncanonical_json_even_when_semantics_match() {
    let bytes = CanonicalArtifact::serialize(&verified_module(false), &metadata())
        .expect("verified NIR serializes");
    let value: Value = serde_json::from_slice(&bytes).expect("artifact is JSON");
    let pretty = serde_json::to_vec_pretty(&value).expect("pretty artifact serializes");

    let error = CanonicalArtifact::deserialize(&pretty, &compatibility())
        .expect_err("noncanonical encodings must fail closed");

    assert_eq!(error.code(), ArtifactErrorCode::NonCanonicalEncoding);
}
