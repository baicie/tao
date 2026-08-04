use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, ensure, Context, Result};
use nexa_compiler::{
    CANDIDATE_OBSERVATION_SCHEMA_VERSION, FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION,
    LEXER_SNAPSHOT_SCHEMA_VERSION, PARSER_SNAPSHOT_SCHEMA_VERSION,
    RESOLVER_SNAPSHOT_SCHEMA_VERSION, TYPECHECK_SNAPSHOT_SCHEMA_VERSION,
};
use nexa_nir::{
    ArtifactCompatibility, ArtifactErrorCode, CanonicalArtifact, NIR_ARTIFACT_MAGIC,
    NIR_SCHEMA_VERSION,
};
use nexa_parser::parse_source;
use nexa_span::FileId;
use nexa_syntax::SyntaxKind;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapManifest {
    schema_version: u32,
    toolchain_version: String,
    language_version: String,
    stage0: Stage0,
    bootstrap_profile: BootstrapProfile,
    bootstrap_stdlib: BootstrapStdlib,
    bootstrap_compiler: BootstrapCompiler,
    bootstrap_output: BootstrapOutput,
    stable_component: StableComponent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapProfile {
    id: String,
    digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stage0 {
    implementation: String,
    compiler: String,
    compiler_version: String,
    source: Stage0Source,
    build: Stage0Build,
    distribution: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stage0Source {
    repository: String,
    commit: String,
    archive_digest: String,
    cargo_lock_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stage0Build {
    rust_version: String,
    cargo_locked: bool,
    profile: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapStdlib {
    status: String,
    version: String,
    profile: String,
    tree_digest: String,
    build_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapCompiler {
    status: String,
    version: String,
    profile: String,
    manifest: String,
    manifest_schema_version: u32,
    source_roots: Vec<String>,
    tree_digest: String,
    implemented_phases: Vec<String>,
    phase_records: Vec<PhaseRecord>,
    candidate_implementation: String,
    candidate_observation_schema_version: u32,
    default_implementation: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapCompilerManifest {
    schema_version: u32,
    name: String,
    version: String,
    toolchain_version: String,
    profile: String,
    implemented_phases: Vec<String>,
    source_roots: Vec<String>,
    source_files: Vec<String>,
    tree_hash_algorithm: String,
    tree_digest: String,
    phase_records: Vec<PhaseRecord>,
    candidate_implementation: String,
    candidate_observation_schema_version: u32,
    default_implementation: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhaseRecord {
    name: String,
    profile_entry: String,
    driver_entry: String,
    observation_schema_version: u32,
    protocol_schema_version: u32,
    accepted_case_count: usize,
    accepted_corpus_digest: String,
    rejected_case_count: usize,
    rejected_corpus_digest: String,
    fuzz_seed_count: usize,
    fuzz_seed_digest: String,
    target_layout_descriptor: serde_json::Value,
}

#[derive(Clone, Copy)]
enum PhaseCaseModel {
    Files,
    SingleGraph,
}

#[derive(Clone, Copy)]
struct PhaseInputSpec {
    root: Option<&'static str>,
    extension: Option<&'static str>,
    absent_root: Option<&'static str>,
    case_model: PhaseCaseModel,
}

#[derive(Clone, Copy)]
struct PhaseSpec {
    name: &'static str,
    profile_entry: &'static str,
    driver_entry: &'static str,
    observation_schema_version: u32,
    protocol_schema_version: u32,
    accepted: PhaseInputSpec,
    rejected: PhaseInputSpec,
    fuzz: PhaseInputSpec,
}

const fn phase_files(root: &'static str, extension: &'static str) -> PhaseInputSpec {
    PhaseInputSpec {
        root: Some(root),
        extension: Some(extension),
        absent_root: None,
        case_model: PhaseCaseModel::Files,
    }
}

const fn phase_graph(root: &'static str, extension: &'static str) -> PhaseInputSpec {
    PhaseInputSpec {
        root: Some(root),
        extension: Some(extension),
        absent_root: None,
        case_model: PhaseCaseModel::SingleGraph,
    }
}

const EMPTY_PHASE_INPUT: PhaseInputSpec = PhaseInputSpec {
    root: None,
    extension: None,
    absent_root: Some("fuzz/corpus/typecheck"),
    case_model: PhaseCaseModel::Files,
};

const PHASE_SPECS: [PhaseSpec; 4] = [
    PhaseSpec {
        name: "lexer",
        profile_entry: "src/lexer_profile.ft",
        driver_entry: "src/lexer_driver.ft",
        observation_schema_version: LEXER_SNAPSHOT_SCHEMA_VERSION,
        protocol_schema_version: 1,
        accepted: phase_files("bootstrap/compiler/tests/lexer/accepted", "ft"),
        rejected: phase_files("bootstrap/compiler/tests/lexer/rejected", "ft"),
        fuzz: phase_files("fuzz/corpus/lexer", "ft"),
    },
    PhaseSpec {
        name: "parser",
        profile_entry: "src/parser_profile.ft",
        driver_entry: "src/parser_driver.ft",
        observation_schema_version: PARSER_SNAPSHOT_SCHEMA_VERSION,
        protocol_schema_version: 1,
        accepted: phase_files("bootstrap/compiler/tests/parser/accepted", "ft"),
        rejected: phase_files("bootstrap/compiler/tests/parser/rejected", "ft"),
        fuzz: phase_files("fuzz/corpus/parser", "ft"),
    },
    PhaseSpec {
        name: "resolver",
        profile_entry: "src/resolver_profile.ft",
        driver_entry: "src/resolver_driver.ft",
        observation_schema_version: RESOLVER_SNAPSHOT_SCHEMA_VERSION,
        protocol_schema_version: 2,
        accepted: phase_graph("bootstrap/compiler/tests/resolver/accepted", "ft"),
        rejected: phase_graph("bootstrap/compiler/tests/resolver/rejected", "ft"),
        fuzz: phase_files("fuzz/corpus/resolver", "ft"),
    },
    PhaseSpec {
        name: "typecheck-expression-kernel",
        profile_entry: "typecheck/typecheck_profile.ft",
        driver_entry: "typecheck/typecheck_driver.ft",
        observation_schema_version: TYPECHECK_SNAPSHOT_SCHEMA_VERSION,
        protocol_schema_version: 1,
        accepted: phase_files("bootstrap/compiler/tests/typecheck/accepted", "json"),
        rejected: phase_files("bootstrap/compiler/tests/typecheck/rejected", "json"),
        fuzz: EMPTY_PHASE_INPUT,
    },
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapStdlibSources {
    entry: String,
    source_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapOutput {
    kind: String,
    stability: String,
    consumer: String,
    comparison: String,
    artifact_magic: String,
    nir_schema_version: u32,
    verifier_crate: String,
    verifier_version: String,
    target_profile: String,
    feature_flags: Vec<String>,
    canonical_encoding: String,
    content_hash_algorithm: String,
    public_extension: Option<String>,
    signature_envelope_included: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StableComponent {
    kind: String,
    extension: String,
    includes_nir: bool,
    separate_lifecycle: bool,
}

pub(crate) fn default_manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../bootstrap/stage0/bootstrap-manifest.json")
}

pub(crate) fn run(manifest_path: &Path, rebuild: bool) -> Result<()> {
    let text = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest = parse_manifest(&text)
        .with_context(|| format!("failed to validate {}", manifest_path.display()))?;
    let root = workspace_root();
    verify_bootstrap_inputs(&manifest, &root)?;
    verify_source(&manifest, &root)?;
    verify_nir_artifacts_at(&root)?;
    if rebuild {
        rebuild_stage0(&manifest, &root)?;
    }
    println!(
        "bootstrap contract {}: stage0 {} {} at {} verified{}",
        manifest.schema_version,
        manifest.stage0.compiler,
        manifest.stage0.compiler_version,
        manifest.stage0.source.commit,
        if rebuild { " and rebuilt" } else { "" }
    );
    Ok(())
}

fn parse_manifest(text: &str) -> Result<BootstrapManifest> {
    let manifest: BootstrapManifest =
        serde_json::from_str(text).context("invalid bootstrap manifest JSON")?;
    manifest.validate()?;
    Ok(manifest)
}

impl BootstrapManifest {
    fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 4, "schemaVersion must be 4");
        expect(
            "toolchainVersion",
            &self.toolchain_version,
            env!("CARGO_PKG_VERSION"),
        )?;
        expect("languageVersion", &self.language_version, "1.0")?;

        expect("stage0.implementation", &self.stage0.implementation, "rust")?;
        expect("stage0.compiler", &self.stage0.compiler, "nexac")?;
        expect(
            "stage0.compilerVersion",
            &self.stage0.compiler_version,
            "0.0.1",
        )?;
        expect(
            "stage0.source.repository",
            &self.stage0.source.repository,
            "https://github.com/baicie/tao",
        )?;
        ensure!(
            valid_lower_hex(&self.stage0.source.commit, 40),
            "stage0.source.commit must be a full lowercase Git object ID"
        );
        validate_sha256(
            "stage0.source.archiveDigest",
            &self.stage0.source.archive_digest,
        )?;
        validate_sha256(
            "stage0.source.cargoLockDigest",
            &self.stage0.source.cargo_lock_digest,
        )?;
        expect(
            "stage0.build.rustVersion",
            &self.stage0.build.rust_version,
            "1.80.0",
        )?;
        ensure!(
            self.stage0.build.cargo_locked,
            "stage0.build.cargoLocked must be true"
        );
        expect(
            "stage0.build.profile",
            &self.stage0.build.profile,
            "release",
        )?;
        expect(
            "stage0.distribution",
            &self.stage0.distribution,
            "source-only",
        )?;
        expect(
            "bootstrapProfile.id",
            &self.bootstrap_profile.id,
            "futao-bootstrap-v1",
        )?;
        validate_sha256("bootstrapProfile.digest", &self.bootstrap_profile.digest)?;
        expect(
            "bootstrapStdlib.status",
            &self.bootstrap_stdlib.status,
            "defined",
        )?;
        expect(
            "bootstrapStdlib.version",
            &self.bootstrap_stdlib.version,
            "0.0.1",
        )?;
        expect(
            "bootstrapStdlib.profile",
            &self.bootstrap_stdlib.profile,
            &self.bootstrap_profile.id,
        )?;
        validate_sha256(
            "bootstrapStdlib.treeDigest",
            &self.bootstrap_stdlib.tree_digest,
        )?;
        validate_sha256(
            "bootstrapStdlib.buildDigest",
            &self.bootstrap_stdlib.build_digest,
        )?;
        expect(
            "bootstrapCompiler.status",
            &self.bootstrap_compiler.status,
            "resolver-differential",
        )?;
        expect(
            "bootstrapCompiler.version",
            &self.bootstrap_compiler.version,
            "0.0.3",
        )?;
        expect(
            "bootstrapCompiler.profile",
            &self.bootstrap_compiler.profile,
            &self.bootstrap_profile.id,
        )?;
        expect(
            "bootstrapCompiler.manifest",
            &self.bootstrap_compiler.manifest,
            "bootstrap/compiler/bootstrap-compiler.json",
        )?;
        ensure!(
            self.bootstrap_compiler.manifest_schema_version == 4,
            "bootstrapCompiler.manifestSchemaVersion must be 4"
        );
        ensure!(
            self.bootstrap_compiler.source_roots == ["src", "typecheck"],
            "bootstrapCompiler.sourceRoots must contain src, then typecheck"
        );
        validate_sha256(
            "bootstrapCompiler.treeDigest",
            &self.bootstrap_compiler.tree_digest,
        )?;
        ensure!(
            self.bootstrap_compiler.implemented_phases == ["lexer", "parser", "resolver"],
            "bootstrapCompiler.implementedPhases must contain lexer, parser, then resolver"
        );
        validate_phase_record_shape(&self.bootstrap_compiler.phase_records)?;
        expect(
            "bootstrapCompiler.candidateImplementation",
            &self.bootstrap_compiler.candidate_implementation,
            FUTAO_BOOTSTRAP_CANDIDATE_IMPLEMENTATION,
        )?;
        ensure!(
            self.bootstrap_compiler
                .candidate_observation_schema_version
                == CANDIDATE_OBSERVATION_SCHEMA_VERSION,
            "bootstrapCompiler.candidateObservationSchemaVersion must be {CANDIDATE_OBSERVATION_SCHEMA_VERSION}"
        );
        expect(
            "bootstrapCompiler.defaultImplementation",
            &self.bootstrap_compiler.default_implementation,
            "rust-reference",
        )?;

        expect(
            "bootstrapOutput.kind",
            &self.bootstrap_output.kind,
            "internal-nir",
        )?;
        expect(
            "bootstrapOutput.stability",
            &self.bootstrap_output.stability,
            "toolchain-internal",
        )?;
        expect(
            "bootstrapOutput.consumer",
            &self.bootstrap_output.consumer,
            "rust-verifier-backend",
        )?;
        expect(
            "bootstrapOutput.comparison",
            &self.bootstrap_output.comparison,
            "normalized-c2-c3",
        )?;
        expect(
            "bootstrapOutput.artifactMagic",
            &self.bootstrap_output.artifact_magic,
            NIR_ARTIFACT_MAGIC,
        )?;
        ensure!(
            self.bootstrap_output.nir_schema_version == NIR_SCHEMA_VERSION,
            "bootstrapOutput.nirSchemaVersion must be {NIR_SCHEMA_VERSION}"
        );
        expect(
            "bootstrapOutput.verifierCrate",
            &self.bootstrap_output.verifier_crate,
            "nexa_nir",
        )?;
        expect(
            "bootstrapOutput.verifierVersion",
            &self.bootstrap_output.verifier_version,
            env!("CARGO_PKG_VERSION"),
        )?;
        expect(
            "bootstrapOutput.targetProfile",
            &self.bootstrap_output.target_profile,
            "target-neutral-v1",
        )?;
        ensure!(
            self.bootstrap_output.feature_flags == ["n1-scalar-cfg-v1"],
            "bootstrapOutput.featureFlags must contain only n1-scalar-cfg-v1"
        );
        expect(
            "bootstrapOutput.canonicalEncoding",
            &self.bootstrap_output.canonical_encoding,
            "strict-json",
        )?;
        expect(
            "bootstrapOutput.contentHashAlgorithm",
            &self.bootstrap_output.content_hash_algorithm,
            "sha256",
        )?;
        ensure!(
            self.bootstrap_output.public_extension.is_none(),
            "bootstrapOutput.publicExtension must be null"
        );
        ensure!(
            !self.bootstrap_output.signature_envelope_included,
            "bootstrapOutput.signatureEnvelopeIncluded must be false"
        );

        expect(
            "stableComponent.kind",
            &self.stable_component.kind,
            "stable-component",
        )?;
        expect(
            "stableComponent.extension",
            &self.stable_component.extension,
            ".nexc",
        )?;
        ensure!(
            !self.stable_component.includes_nir,
            "stableComponent.includesNir must be false"
        );
        ensure!(
            self.stable_component.separate_lifecycle,
            "stableComponent.separateLifecycle must be true"
        );

        Ok(())
    }
}

fn verify_bootstrap_inputs(manifest: &BootstrapManifest, root: &Path) -> Result<()> {
    let profile_path = root.join("bootstrap/profile/bootstrap-profile-v1.json");
    let profile_bytes = std::fs::read(&profile_path)
        .with_context(|| format!("failed to read {}", profile_path.display()))?;
    verify_digest(
        "bootstrapProfile.digest",
        &manifest.bootstrap_profile.digest,
        &profile_bytes,
    )?;
    let profile: serde_json::Value = serde_json::from_slice(&profile_bytes)
        .with_context(|| format!("invalid JSON in {}", profile_path.display()))?;
    expect(
        "bootstrapProfile.id",
        json_string(&profile, "id")?,
        &manifest.bootstrap_profile.id,
    )?;
    expect(
        "bootstrapProfile.languageVersion",
        json_string(&profile, "languageVersion")?,
        &manifest.language_version,
    )?;

    let stdlib_path = root.join("bootstrap/stdlib/bootstrap-stdlib.json");
    let stdlib_bytes = std::fs::read(&stdlib_path)
        .with_context(|| format!("failed to read {}", stdlib_path.display()))?;
    let stdlib: serde_json::Value = serde_json::from_slice(&stdlib_bytes)
        .with_context(|| format!("invalid JSON in {}", stdlib_path.display()))?;
    for (field, expected) in [
        ("version", manifest.bootstrap_stdlib.version.as_str()),
        ("profile", manifest.bootstrap_stdlib.profile.as_str()),
        ("treeDigest", manifest.bootstrap_stdlib.tree_digest.as_str()),
        (
            "buildDigest",
            manifest.bootstrap_stdlib.build_digest.as_str(),
        ),
    ] {
        expect(
            &format!("bootstrapStdlib.{field}"),
            json_string(&stdlib, field)?,
            expected,
        )?;
    }
    verify_bootstrap_compiler(manifest, root)?;
    Ok(())
}

fn verify_bootstrap_compiler(manifest: &BootstrapManifest, root: &Path) -> Result<()> {
    let bootstrap_root = root.join("bootstrap");
    ensure_real_directory(&bootstrap_root, "bootstrap root")?;
    let compiler_root = bootstrap_root.join("compiler");
    ensure_real_directory(&compiler_root, "bootstrap compiler root")?;
    let compiler_path = root.join(&manifest.bootstrap_compiler.manifest);
    let compiler_metadata = std::fs::symlink_metadata(&compiler_path)
        .with_context(|| format!("failed to inspect {}", compiler_path.display()))?;
    ensure!(
        compiler_metadata.file_type().is_file(),
        "{} must be a regular file, not a symlink",
        compiler_path.display()
    );
    let compiler_bytes = std::fs::read(&compiler_path)
        .with_context(|| format!("failed to read {}", compiler_path.display()))?;
    let compiler: BootstrapCompilerManifest = serde_json::from_slice(&compiler_bytes)
        .with_context(|| format!("invalid JSON in {}", compiler_path.display()))?;

    ensure!(
        compiler.schema_version == manifest.bootstrap_compiler.manifest_schema_version,
        "compiler schemaVersion does not match bootstrapCompiler.manifestSchemaVersion"
    );
    expect("compiler.name", &compiler.name, "futao-bootstrap-compiler")?;
    expect(
        "compiler.version",
        &compiler.version,
        &manifest.bootstrap_compiler.version,
    )?;
    expect(
        "compiler.toolchainVersion",
        &compiler.toolchain_version,
        &manifest.toolchain_version,
    )?;
    expect(
        "compiler.profile",
        &compiler.profile,
        &manifest.bootstrap_compiler.profile,
    )?;
    ensure!(
        compiler.implemented_phases == manifest.bootstrap_compiler.implemented_phases,
        "compiler implementedPhases do not match the top-level contract"
    );
    validate_compiler_source_roots(&compiler.source_roots)?;
    ensure!(
        compiler.source_roots == manifest.bootstrap_compiler.source_roots,
        "compiler sourceRoots do not match the top-level contract"
    );
    validate_compiler_source_files(&compiler.source_files, &compiler.source_roots)?;
    expect(
        "compiler.treeHashAlgorithm",
        &compiler.tree_hash_algorithm,
        "sha256",
    )?;
    expect(
        "compiler.treeDigest",
        &compiler.tree_digest,
        &manifest.bootstrap_compiler.tree_digest,
    )?;
    validate_phase_record_shape(&compiler.phase_records)?;
    ensure!(
        compiler.phase_records == manifest.bootstrap_compiler.phase_records,
        "compiler phaseRecords do not match the top-level contract"
    );
    for record in &compiler.phase_records {
        ensure!(
            compiler
                .source_files
                .iter()
                .any(|source| source == &record.profile_entry),
            "compiler phase profileEntry is not bound by sourceFiles: {}",
            record.profile_entry
        );
        ensure!(
            compiler
                .source_files
                .iter()
                .any(|source| source == &record.driver_entry),
            "compiler phase driverEntry is not bound by sourceFiles: {}",
            record.driver_entry
        );
    }
    expect(
        "compiler.candidateImplementation",
        &compiler.candidate_implementation,
        &manifest.bootstrap_compiler.candidate_implementation,
    )?;
    ensure!(
        compiler.candidate_observation_schema_version
            == manifest
                .bootstrap_compiler
                .candidate_observation_schema_version,
        "compiler candidateObservationSchemaVersion does not match the top-level contract"
    );
    expect(
        "compiler.defaultImplementation",
        &compiler.default_implementation,
        &manifest.bootstrap_compiler.default_implementation,
    )?;

    reject_undeclared_compiler_entries(&compiler_root, &compiler.source_roots)?;
    let discovered = discover_compiler_sources(&compiler_root, &compiler.source_roots)?;
    ensure!(
        compiler.source_files == discovered,
        "compiler sourceFiles do not match recursively discovered source roots"
    );
    let actual = compiler_tree_digest(&compiler, &compiler_root)?;
    ensure!(
        actual == compiler.tree_digest,
        "bootstrap compiler tree digest mismatch: expected {}, found {actual}",
        compiler.tree_digest
    );
    verify_phase_inputs(&compiler.phase_records, root)?;
    Ok(())
}

fn ensure_real_directory(path: &Path, description: &str) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    ensure!(
        metadata.file_type().is_dir(),
        "{description} must be a directory, not a symlink: {}",
        path.display()
    );
    Ok(())
}

fn discover_compiler_sources(root: &Path, source_roots: &[String]) -> Result<Vec<String>> {
    let mut sources = Vec::new();
    for source_root in source_roots {
        let path = root.join(source_root);
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        ensure!(
            metadata.file_type().is_dir(),
            "bootstrap compiler source root must be a directory, not a symlink: {}",
            path.display()
        );
        discover_compiler_source_directory(root, &path, &mut sources)?;
    }
    sources.sort();
    case_folded_path_set(
        &sources,
        "compiler source paths must be unique on case-insensitive filesystems",
    )?;
    Ok(sources)
}

fn reject_undeclared_compiler_entries(root: &Path, source_roots: &[String]) -> Result<()> {
    reject_undeclared_compiler_entries_below(root, root, source_roots)
}

fn reject_undeclared_compiler_entries_below(
    compiler_root: &Path,
    directory: &Path,
    source_roots: &[String],
) -> Result<()> {
    let entries = std::fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to inspect {}", directory.display()))?;
        let path = entry.path();
        let relative = portable_relative_path(
            path.strip_prefix(compiler_root)
                .context("bootstrap compiler entry escaped its root")?,
            "bootstrap compiler entry",
        )?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        ensure!(
            !file_type.is_symlink(),
            "bootstrap compiler source entry must not be a symlink: {}",
            path.display()
        );

        if relative == "bootstrap-compiler.json" {
            ensure!(
                file_type.is_file(),
                "bootstrap compiler manifest must be a regular file"
            );
            continue;
        }
        if relative == "tests" {
            ensure!(
                file_type.is_dir(),
                "bootstrap compiler tests corpus must be a directory"
            );
            continue;
        }
        if source_roots.iter().any(|root| root == &relative) {
            ensure!(
                file_type.is_dir(),
                "bootstrap compiler source root must be a directory: {}",
                path.display()
            );
            continue;
        }
        let is_source_root_ancestor = source_roots
            .iter()
            .any(|root| root.starts_with(&format!("{relative}/")));
        ensure!(
            is_source_root_ancestor && file_type.is_dir(),
            "undeclared bootstrap compiler source entry: {}",
            path.display()
        );
        reject_undeclared_compiler_entries_below(compiler_root, &path, source_roots)?;
    }
    Ok(())
}

fn discover_compiler_source_directory(
    compiler_root: &Path,
    directory: &Path,
    sources: &mut Vec<String>,
) -> Result<()> {
    let entries = std::fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to inspect {}", directory.display()))?;
        let path = entry.path();
        let relative = portable_relative_path(
            path.strip_prefix(compiler_root)
                .context("bootstrap compiler source escaped its root")?,
            "compiler source path",
        )?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        ensure!(
            !file_type.is_symlink(),
            "bootstrap compiler source entry must not be a symlink: {}",
            path.display()
        );
        if file_type.is_dir() {
            discover_compiler_source_directory(compiler_root, &path, sources)?;
        } else {
            ensure!(
                file_type.is_file(),
                "bootstrap compiler source entry must be a regular file: {}",
                path.display()
            );
            ensure!(
                relative.ends_with(".ft"),
                "bootstrap compiler source files must use .ft: {}",
                path.display()
            );
            sources.push(relative);
        }
    }
    Ok(())
}

fn validate_phase_record_shape(records: &[PhaseRecord]) -> Result<()> {
    ensure!(
        records.len() == PHASE_SPECS.len(),
        "bootstrap compiler phaseRecords must contain exactly four records"
    );
    for (record, expected) in records.iter().zip(PHASE_SPECS) {
        expect("phaseRecords.name", &record.name, expected.name)?;
        expect(
            "phaseRecords.profileEntry",
            &record.profile_entry,
            expected.profile_entry,
        )?;
        expect(
            "phaseRecords.driverEntry",
            &record.driver_entry,
            expected.driver_entry,
        )?;
        ensure!(
            record.observation_schema_version == expected.observation_schema_version,
            "{} observationSchemaVersion must be {}",
            record.name,
            expected.observation_schema_version
        );
        ensure!(
            record.protocol_schema_version == expected.protocol_schema_version,
            "{} protocolSchemaVersion must be {}",
            record.name,
            expected.protocol_schema_version
        );
        validate_sha256(
            &format!("{}.acceptedCorpusDigest", record.name),
            &record.accepted_corpus_digest,
        )?;
        validate_sha256(
            &format!("{}.rejectedCorpusDigest", record.name),
            &record.rejected_corpus_digest,
        )?;
        validate_sha256(
            &format!("{}.fuzzSeedDigest", record.name),
            &record.fuzz_seed_digest,
        )?;
        ensure!(
            record.target_layout_descriptor.is_null(),
            "{} targetLayoutDescriptor must be null for a front-end phase",
            record.name
        );
    }
    Ok(())
}

fn verify_phase_inputs(records: &[PhaseRecord], workspace_root: &Path) -> Result<()> {
    for (record, spec) in records.iter().zip(PHASE_SPECS) {
        let accepted =
            phase_input_observation(workspace_root, spec.name, "accepted", spec.accepted)?;
        let rejected =
            phase_input_observation(workspace_root, spec.name, "rejected", spec.rejected)?;
        let fuzz = phase_input_observation(workspace_root, spec.name, "fuzz", spec.fuzz)?;

        ensure!(
            record.accepted_case_count == accepted.0,
            "{} acceptedCaseCount does not match checked-in inputs",
            record.name
        );
        ensure!(
            record.accepted_corpus_digest == accepted.1,
            "{} acceptedCorpusDigest does not match checked-in inputs: found {}",
            record.name,
            accepted.1
        );
        ensure!(
            record.rejected_case_count == rejected.0,
            "{} rejectedCaseCount does not match checked-in inputs",
            record.name
        );
        ensure!(
            record.rejected_corpus_digest == rejected.1,
            "{} rejectedCorpusDigest does not match checked-in inputs: found {}",
            record.name,
            rejected.1
        );
        ensure!(
            record.fuzz_seed_count == fuzz.0,
            "{} fuzzSeedCount does not match checked-in inputs",
            record.name
        );
        ensure!(
            record.fuzz_seed_digest == fuzz.1,
            "{} fuzzSeedDigest does not match checked-in inputs: found {}",
            record.name,
            fuzz.1
        );
    }
    Ok(())
}

fn phase_input_observation(
    workspace_root: &Path,
    phase: &str,
    category: &str,
    spec: PhaseInputSpec,
) -> Result<(usize, String)> {
    let files = match (spec.root, spec.extension, spec.absent_root) {
        (Some(relative_root), Some(extension), None) => {
            discover_phase_input_files(workspace_root, relative_root, extension)?
        }
        (None, None, Some(absent_root)) => {
            validate_portable_relative_path("absent phase input root", absent_root)?;
            let path = workspace_root.join(absent_root);
            match std::fs::symlink_metadata(&path) {
                Ok(_) => bail!(
                    "{phase} {category} input root must remain absent: {}",
                    path.display()
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to inspect absent phase input root {}",
                            path.display()
                        )
                    });
                }
            }
            Vec::new()
        }
        _ => {
            bail!("phase input root, extension, and absent root do not form a valid input contract")
        }
    };
    let case_count = match spec.case_model {
        PhaseCaseModel::Files => files.len(),
        PhaseCaseModel::SingleGraph => {
            ensure!(
                files.iter().any(|relative| relative == "main.ft"),
                "{phase} {category} source graph must contain main.ft"
            );
            1
        }
    };
    if spec.root.is_some() {
        ensure!(
            !files.is_empty(),
            "{phase} {category} phase input set must not be empty"
        );
    }

    let mut hasher = Sha256::new();
    hasher.update(b"FUTAO-BOOTSTRAP-PHASE-INPUTS-V1\0");
    hash_tree_field(&mut hasher, phase.as_bytes())?;
    hash_tree_field(&mut hasher, category.as_bytes())?;
    hash_tree_field(&mut hasher, spec.root.unwrap_or_default().as_bytes())?;
    hash_tree_field(
        &mut hasher,
        &u64::try_from(case_count)
            .context("too many phase input cases")?
            .to_be_bytes(),
    )?;
    hash_tree_field(
        &mut hasher,
        &u64::try_from(files.len())
            .context("too many phase input files")?
            .to_be_bytes(),
    )?;
    if let Some(relative_root) = spec.root {
        let input_root = workspace_root.join(relative_root);
        for relative in &files {
            let path = input_root.join(relative);
            let metadata = std::fs::symlink_metadata(&path)
                .with_context(|| format!("failed to inspect {}", path.display()))?;
            ensure!(
                metadata.file_type().is_file(),
                "phase input must be a regular file, not a symlink: {}",
                path.display()
            );
            let bytes = std::fs::read(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            std::str::from_utf8(&bytes)
                .with_context(|| format!("phase input must be UTF-8: {}", path.display()))?;
            hash_tree_field(&mut hasher, relative.as_bytes())?;
            hash_tree_field(&mut hasher, &bytes)?;
        }
    }
    Ok((case_count, format!("sha256:{:x}", hasher.finalize())))
}

fn discover_phase_input_files(
    workspace_root: &Path,
    relative_root: &str,
    extension: &str,
) -> Result<Vec<String>> {
    validate_portable_relative_path("phase input root", relative_root)?;
    ensure_real_directory_chain(workspace_root, relative_root)?;
    let input_root = workspace_root.join(relative_root);
    let mut files = Vec::new();
    discover_phase_input_files_below(&input_root, &input_root, extension, &mut files)?;
    files.sort();
    case_folded_path_set(
        &files,
        "phase input paths must be unique on case-insensitive filesystems",
    )?;
    Ok(files)
}

fn discover_phase_input_files_below(
    input_root: &Path,
    directory: &Path,
    extension: &str,
    files: &mut Vec<String>,
) -> Result<()> {
    let entries = std::fs::read_dir(directory)
        .with_context(|| format!("failed to read phase inputs {}", directory.display()))?;
    for entry in entries {
        let entry = entry
            .with_context(|| format!("failed to inspect phase inputs {}", directory.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect phase input {}", path.display()))?;
        ensure!(
            !file_type.is_symlink(),
            "phase input entry must not be a symlink: {}",
            path.display()
        );
        if file_type.is_dir() {
            discover_phase_input_files_below(input_root, &path, extension, files)?;
            continue;
        }
        ensure!(
            file_type.is_file(),
            "phase input entry must be a regular file: {}",
            path.display()
        );
        ensure!(
            path.extension().and_then(|value| value.to_str()) == Some(extension),
            "phase input entry must use .{extension}: {}",
            path.display()
        );
        let relative = portable_relative_path(
            path.strip_prefix(input_root)
                .context("phase input escaped its declared root")?,
            "phase input path",
        )?;
        files.push(relative);
    }
    Ok(())
}

fn ensure_real_directory_chain(workspace_root: &Path, relative: &str) -> Result<()> {
    let mut directory = workspace_root.to_path_buf();
    for component in relative.split('/') {
        directory.push(component);
        ensure_real_directory(&directory, "phase input directory")?;
    }
    Ok(())
}

fn validate_compiler_source_roots(source_roots: &[String]) -> Result<()> {
    ensure!(
        !source_roots.is_empty(),
        "compiler sourceRoots must not be empty"
    );
    for source_root in source_roots {
        validate_portable_relative_path("compiler sourceRoots entry", source_root).context(
            "compiler sourceRoots entries must be normalized portable relative directories",
        )?;
    }
    ensure!(
        source_roots.windows(2).all(|pair| pair[0] < pair[1]),
        "compiler sourceRoots must be unique and sorted by portable path"
    );
    let folded_roots = case_folded_path_set(
        source_roots,
        "compiler sourceRoots must be unique and sorted by portable path",
    )?;
    for source_root in &folded_roots {
        for (index, _) in source_root.match_indices('/') {
            ensure!(
                !folded_roots.contains(&source_root[..index]),
                "compiler sourceRoots must not overlap"
            );
        }
    }
    Ok(())
}

fn validate_compiler_source_files(source_files: &[String], source_roots: &[String]) -> Result<()> {
    ensure!(
        !source_files.is_empty(),
        "compiler sourceFiles must not be empty"
    );
    for source_file in source_files {
        validate_portable_relative_path("compiler sourceFiles entry", source_file)
            .context("compiler sourceFiles entries must be normalized portable `.ft` paths")?;
    }
    ensure!(
        source_files.windows(2).all(|pair| pair[0] < pair[1]),
        "compiler sourceFiles must be unique and sorted by portable path"
    );
    case_folded_path_set(
        source_files,
        "compiler sourceFiles must be unique on case-insensitive filesystems",
    )?;
    let source_root_set = source_roots
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for source_file in source_files {
        ensure!(
            source_file.ends_with(".ft"),
            "compiler sourceFiles entries must use .ft"
        );
        ensure!(
            source_file
                .match_indices('/')
                .any(|(index, _)| source_root_set.contains(&source_file[..index])),
            "compiler sourceFiles entries must be below a declared source root"
        );
    }
    Ok(())
}

fn case_folded_path_set(values: &[String], duplicate_message: &str) -> Result<BTreeSet<String>> {
    let mut folded = BTreeSet::new();
    for value in values {
        ensure!(
            folded.insert(value.to_ascii_lowercase()),
            "{}",
            duplicate_message
        );
    }
    Ok(folded)
}

fn validate_portable_relative_path(field: &str, value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.is_ascii() && !value.contains(['\\', '\0']),
        "{field} must be an ASCII portable relative path"
    );
    for component in value.split('/') {
        ensure!(
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.ends_with('.')
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
            "{field} must be a normalized portable relative path"
        );
        let basename = component
            .split_once('.')
            .map_or(component, |(basename, _)| basename);
        let basename = basename.to_ascii_uppercase();
        let reserved = matches!(basename.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || basename
                .strip_prefix("COM")
                .is_some_and(|suffix| matches!(suffix.as_bytes(), [b'1'..=b'9']))
            || basename
                .strip_prefix("LPT")
                .is_some_and(|suffix| matches!(suffix.as_bytes(), [b'1'..=b'9']));
        ensure!(!reserved, "{field} contains a reserved portable name");
    }
    Ok(())
}

fn portable_relative_path(path: &Path, field: &str) -> Result<String> {
    let mut components = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(component) = component else {
            bail!("{field} must be a normalized portable relative path");
        };
        components.push(
            component
                .to_str()
                .with_context(|| format!("{field} must be UTF-8"))?,
        );
    }
    let value = components.join("/");
    validate_portable_relative_path(field, &value)?;
    Ok(value)
}

fn compiler_tree_digest(
    compiler: &BootstrapCompilerManifest,
    compiler_root: &Path,
) -> Result<String> {
    let mut tree_hasher = Sha256::new();
    tree_hasher.update(b"FUTAO-BOOTSTRAP-COMPILER-TREE-V2\0");
    hash_tree_field(&mut tree_hasher, &compiler.schema_version.to_be_bytes())?;
    let source_root_count = u64::try_from(compiler.source_roots.len())
        .context("too many bootstrap compiler source roots")?;
    hash_tree_field(&mut tree_hasher, &source_root_count.to_be_bytes())?;
    for source_root in &compiler.source_roots {
        hash_tree_field(&mut tree_hasher, source_root.as_bytes())?;
    }
    let source_file_count = u64::try_from(compiler.source_files.len())
        .context("too many bootstrap compiler source files")?;
    hash_tree_field(&mut tree_hasher, &source_file_count.to_be_bytes())?;
    for relative in &compiler.source_files {
        let path = compiler_root.join(relative);
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        ensure!(
            metadata.file_type().is_file(),
            "{} must be a regular file, not a symlink",
            path.display()
        );
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        std::str::from_utf8(&bytes).with_context(|| format!("{} must be UTF-8", path.display()))?;
        hash_tree_field(&mut tree_hasher, relative.as_bytes())?;
        hash_tree_field(&mut tree_hasher, &bytes)?;
    }
    Ok(format!("sha256:{:x}", tree_hasher.finalize()))
}

fn hash_tree_field(hasher: &mut Sha256, bytes: &[u8]) -> Result<()> {
    let length =
        u64::try_from(bytes.len()).context("bootstrap compiler hash field is too large")?;
    hasher.update(length.to_be_bytes());
    hasher.update(bytes);
    Ok(())
}

fn json_string<'a>(value: &'a serde_json::Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("bootstrap input field {field:?} must be a string"))
}

pub(crate) fn verify_nir_artifacts() -> Result<()> {
    verify_nir_artifacts_at(&workspace_root())?;
    println!(
        "private NIR artifact schema {}: accepted and rejected fixtures verified",
        NIR_SCHEMA_VERSION
    );
    Ok(())
}

fn verify_nir_artifacts_at(root: &Path) -> Result<()> {
    let compatibility = ArtifactCompatibility::exact(env!("CARGO_PKG_VERSION"));
    let accepted = root.join("bootstrap/tests/accepted/minimal-nir.json");
    let bytes = std::fs::read(&accepted)
        .with_context(|| format!("failed to read {}", accepted.display()))?;
    CanonicalArtifact::deserialize(canonical_fixture_bytes(&bytes), &compatibility)
        .with_context(|| format!("accepted NIR fixture failed: {}", accepted.display()))?;

    for (name, expected_code) in [
        ("mutated-nir.json", ArtifactErrorCode::ContentHashMismatch),
        ("unknown-nir-field.json", ArtifactErrorCode::InvalidJson),
        (
            "unsupported-nir-schema.json",
            ArtifactErrorCode::UnsupportedSchema,
        ),
    ] {
        let path = root.join("bootstrap/tests/rejected").join(name);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let result =
            CanonicalArtifact::deserialize(canonical_fixture_bytes(&bytes), &compatibility);
        let Err(error) = result else {
            bail!(
                "rejected NIR fixture unexpectedly verified: {}",
                path.display()
            );
        };
        ensure!(
            error.code() == expected_code,
            "rejected NIR fixture {} failed with {}, expected {}",
            path.display(),
            error.code().as_str(),
            expected_code.as_str()
        );
    }
    Ok(())
}

fn canonical_fixture_bytes(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

fn expect(field: &str, actual: &str, expected: &str) -> Result<()> {
    ensure!(
        actual == expected,
        "{field} must be {expected:?}, found {actual:?}"
    );
    Ok(())
}

fn validate_sha256(field: &str, digest: &str) -> Result<()> {
    let value = digest.strip_prefix("sha256:").unwrap_or_default();
    ensure!(
        valid_lower_hex(value, 64),
        "{field} must be an algorithm-qualified lowercase SHA-256 digest"
    );
    Ok(())
}

fn verify_digest(field: &str, expected: &str, bytes: &[u8]) -> Result<()> {
    let actual = sha256(bytes);
    ensure!(
        actual == expected,
        "{field} digest mismatch: expected {expected}, found {actual}"
    );
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity("sha256:".len() + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn verify_source(manifest: &BootstrapManifest, root: &Path) -> Result<()> {
    let commit = &manifest.stage0.source.commit;
    let commit_object = format!("{commit}^{{commit}}");
    let resolved = git_output(root, &["rev-parse", &commit_object])?;
    let resolved = String::from_utf8(resolved).context("git rev-parse output was not UTF-8")?;
    ensure!(
        resolved.trim() == commit,
        "stage0.source.commit resolved to a different Git object"
    );

    let archive = git_output(root, &["archive", "--format=tar", commit])?;
    verify_digest(
        "stage0.source.archiveDigest",
        &manifest.stage0.source.archive_digest,
        &archive,
    )?;

    let cargo_lock_object = format!("{commit}:Cargo.lock");
    let cargo_lock = git_output(root, &["show", &cargo_lock_object])?;
    verify_digest(
        "stage0.source.cargoLockDigest",
        &manifest.stage0.source.cargo_lock_digest,
        &cargo_lock,
    )
}

fn git_output(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

fn rebuild_stage0(manifest: &BootstrapManifest, root: &Path) -> Result<()> {
    let worktree = std::env::temp_dir().join(format!(
        "futao-stage0-{}-{}",
        manifest.stage0.compiler_version,
        std::process::id()
    ));
    ensure!(
        !worktree.exists(),
        "temporary Stage 0 worktree already exists at {}",
        worktree.display()
    );

    let mut add = Command::new("git");
    add.arg("worktree")
        .arg("add")
        .arg("--detach")
        .arg(&worktree)
        .arg(&manifest.stage0.source.commit)
        .current_dir(root);
    run_status(&mut add, "create Stage 0 worktree")?;

    let build_result = build_and_smoke_stage0(manifest, root, &worktree);
    let mut remove = Command::new("git");
    remove
        .arg("worktree")
        .arg("remove")
        .arg("--force")
        .arg(&worktree)
        .current_dir(root);
    let cleanup_result = run_status(&mut remove, "remove Stage 0 worktree");

    build_result?;
    cleanup_result
}

fn build_and_smoke_stage0(
    manifest: &BootstrapManifest,
    root: &Path,
    worktree: &Path,
) -> Result<()> {
    let target = stage0_target_dir(worktree);
    let manifest_path = worktree.join("Cargo.toml");
    let mut build = Command::new("cargo");
    build
        .arg(format!("+{}", manifest.stage0.build.rust_version))
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--locked")
        .arg("--release")
        .arg("--package")
        .arg(&manifest.stage0.compiler)
        .arg("--target-dir")
        .arg(&target)
        .current_dir(worktree);
    run_status(&mut build, "build Stage 0")?;

    let binary = target
        .join("release")
        .join(if cfg!(windows) { "nexac.exe" } else { "nexac" });
    let output = Command::new(&binary)
        .arg("--version")
        .current_dir(worktree)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("failed to run rebuilt Stage 0 at {}", binary.display()))?;
    ensure!(
        output.status.success(),
        "rebuilt Stage 0 version smoke failed"
    );
    let expected = format!(
        "{} {}\n",
        manifest.stage0.compiler, manifest.stage0.compiler_version
    );
    ensure!(
        output.stdout == expected.as_bytes(),
        "rebuilt Stage 0 version mismatch: expected {expected:?}, found {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    ensure!(
        output.stderr.is_empty(),
        "rebuilt Stage 0 wrote to stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    verify_stage0_bootstrap_stdlib(&binary, root, worktree)?;
    Ok(())
}

fn stage0_target_dir(worktree: &Path) -> PathBuf {
    worktree.join("target")
}

fn project_source_for_stage0(source: &str) -> Result<String> {
    let parse = parse_source(FileId::new(0), source);
    ensure!(
        parse.is_ok(),
        "cannot project malformed Bootstrap Stdlib source for Stage 0"
    );

    let mut replacements = Vec::new();
    for declaration in parse
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::ImportDeclaration)
    {
        let specifier = declaration
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| token.kind() == SyntaxKind::String)
            .context("Bootstrap Stdlib import is missing its path")?;
        let text = specifier.text();
        let prefix = text
            .strip_suffix(".ft\"")
            .filter(|prefix| prefix.starts_with('"'))
            .context("Bootstrap Stdlib import must be an unescaped `.ft` path")?;
        let range = specifier.text_range();
        let start = usize::try_from(u32::from(range.start()))
            .context("Bootstrap Stdlib import offset does not fit usize")?;
        let end = usize::try_from(u32::from(range.end()))
            .context("Bootstrap Stdlib import offset does not fit usize")?;
        replacements.push((start, end, format!("{prefix}.nexa\"")));
    }

    let mut projected = String::with_capacity(source.len());
    let mut cursor = 0;
    for (start, end, replacement) in replacements {
        projected.push_str(&source[cursor..start]);
        projected.push_str(&replacement);
        cursor = end;
    }
    projected.push_str(&source[cursor..]);
    Ok(projected)
}

fn project_bootstrap_stdlib_for_stage0(root: &Path, destination: &Path) -> Result<PathBuf> {
    let manifest_path = root.join("bootstrap/stdlib/bootstrap-stdlib.json");
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: BootstrapStdlibSources = serde_json::from_str(&manifest_text)
        .with_context(|| format!("invalid JSON in {}", manifest_path.display()))?;
    ensure!(
        !manifest.source_files.is_empty(),
        "Bootstrap Stdlib sourceFiles must not be empty"
    );
    ensure!(
        manifest
            .source_files
            .windows(2)
            .all(|pair| pair[0] < pair[1]),
        "Bootstrap Stdlib sourceFiles must be unique and sorted"
    );
    ensure!(
        manifest.source_files.contains(&manifest.entry),
        "Bootstrap Stdlib entry must be present in sourceFiles"
    );

    std::fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    for relative in &manifest.source_files {
        let projected_relative = stage0_source_path(relative)?;
        let source_path = root.join("bootstrap/stdlib").join(relative);
        let metadata = std::fs::symlink_metadata(&source_path)
            .with_context(|| format!("failed to inspect {}", source_path.display()))?;
        ensure!(
            metadata.file_type().is_file(),
            "{} must be a regular file",
            source_path.display()
        );
        let source = std::fs::read_to_string(&source_path)
            .with_context(|| format!("failed to read {}", source_path.display()))?;
        let target_path = destination.join(projected_relative);
        ensure!(
            !target_path.exists(),
            "Stage 0 projection target already exists at {}",
            target_path.display()
        );
        let parent = target_path
            .parent()
            .context("Stage 0 projection target must have a parent")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        std::fs::write(&target_path, project_source_for_stage0(&source)?)
            .with_context(|| format!("failed to write {}", target_path.display()))?;
    }

    Ok(destination.join(stage0_source_path(&manifest.entry)?))
}

fn stage0_source_path(relative: &str) -> Result<PathBuf> {
    ensure!(
        relative.starts_with("src/")
            && !relative.contains(['\\', '\0'])
            && relative
                .split('/')
                .all(|component| !component.is_empty() && component != "." && component != ".."),
        "Bootstrap Stdlib source path must be normalized below src/"
    );
    let prefix = relative
        .strip_suffix(".ft")
        .context("Bootstrap Stdlib source path must end in `.ft`")?;
    Ok(PathBuf::from(format!("{prefix}.nexa")))
}

fn verify_stage0_bootstrap_stdlib(binary: &Path, root: &Path, worktree: &Path) -> Result<()> {
    let projection = worktree.join("stage0-bootstrap-stdlib");
    let entry = project_bootstrap_stdlib_for_stage0(root, &projection)?;
    let output = Command::new(binary)
        .arg("check")
        .arg(&entry)
        .current_dir(worktree)
        .stdin(Stdio::null())
        .output()
        .with_context(|| {
            format!(
                "failed to check Bootstrap Stdlib with rebuilt Stage 0 at {}",
                binary.display()
            )
        })?;
    ensure!(
        output.status.success(),
        "rebuilt Stage 0 rejected Bootstrap Stdlib: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    ensure!(
        output.stdout == b"ok\n",
        "rebuilt Stage 0 Bootstrap Stdlib check wrote unexpected stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    ensure!(
        output.stderr.is_empty(),
        "rebuilt Stage 0 Bootstrap Stdlib check wrote stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn run_status(command: &mut Command, description: &str) -> Result<()> {
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to {description}"))?;
    ensure!(status.success(), "failed to {description}");
    Ok(())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        canonical_fixture_bytes, compiler_tree_digest, discover_compiler_sources, parse_manifest,
        phase_input_observation, project_bootstrap_stdlib_for_stage0, project_source_for_stage0,
        stage0_target_dir, validate_compiler_source_roots, verify_bootstrap_compiler,
        verify_bootstrap_inputs, verify_digest, verify_source, workspace_root, PhaseCaseModel,
        PhaseInputSpec, PHASE_SPECS,
    };
    use serde_json::{json, Value};

    const ACCEPTED: &str = include_str!("../../bootstrap/stage0/bootstrap-manifest.json");
    const COMPILER: &str = include_str!("../../bootstrap/compiler/bootstrap-compiler.json");
    const PUBLIC_NIR: &str =
        include_str!("../../bootstrap/tests/rejected/public-nir-artifact.json");
    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn checked_in_stage0_manifest_defines_the_internal_bootstrap_boundary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = parse_manifest(ACCEPTED)?;

        assert_eq!(manifest.schema_version, 4);
        assert_eq!(manifest.stage0.compiler_version, "0.0.1");
        assert_eq!(manifest.bootstrap_profile.id, "futao-bootstrap-v1");
        assert_eq!(
            manifest.bootstrap_profile.digest,
            "sha256:8f906ed909566e098ee8cd5029ded3b34bfb174619d198ab68bf26e57e63a3cd"
        );
        assert_eq!(manifest.bootstrap_stdlib.status, "defined");
        assert_eq!(manifest.bootstrap_stdlib.version, "0.0.1");
        assert_eq!(manifest.bootstrap_stdlib.profile, "futao-bootstrap-v1");
        assert_eq!(
            manifest.bootstrap_stdlib.tree_digest,
            "sha256:d302a92827072fdb6c9ef85ffc53cca9472f9dfe69f604132d5acd50a6f92da2"
        );
        assert_eq!(
            manifest.bootstrap_stdlib.build_digest,
            "sha256:03c183622af98e72f667443a1995f96f0314ab8cf9c7e1eaf70dbe114175538b"
        );
        assert_eq!(manifest.bootstrap_compiler.status, "resolver-differential");
        assert_eq!(manifest.bootstrap_compiler.version, "0.0.3");
        assert_eq!(manifest.bootstrap_compiler.profile, "futao-bootstrap-v1");
        assert_eq!(manifest.bootstrap_compiler.manifest_schema_version, 4);
        assert_eq!(
            manifest.bootstrap_compiler.source_roots,
            ["src", "typecheck"]
        );
        assert_eq!(
            manifest.bootstrap_compiler.implemented_phases,
            ["lexer", "parser", "resolver"]
        );
        assert_eq!(
            manifest.bootstrap_compiler.tree_digest,
            "sha256:15e4eafa43625e26ae03af2b14d5bd02c94d4239d08fbc3a6ce0cd6bcbea3465"
        );
        assert_eq!(manifest.bootstrap_compiler.phase_records.len(), 4);
        assert_eq!(
            manifest.bootstrap_compiler.candidate_implementation,
            "futao-bootstrap-candidate"
        );
        assert_eq!(
            manifest
                .bootstrap_compiler
                .candidate_observation_schema_version,
            1
        );
        assert_eq!(
            manifest.bootstrap_compiler.default_implementation,
            "rust-reference"
        );
        assert_eq!(manifest.bootstrap_output.kind, "internal-nir");
        assert_eq!(manifest.bootstrap_output.consumer, "rust-verifier-backend");
        assert_eq!(manifest.bootstrap_output.artifact_magic, "FUTAO-NIR");
        assert_eq!(manifest.bootstrap_output.nir_schema_version, 1);
        assert_eq!(manifest.bootstrap_output.verifier_crate, "nexa_nir");
        assert_eq!(manifest.bootstrap_output.verifier_version, "0.0.10");
        assert_eq!(
            manifest.bootstrap_output.target_profile,
            "target-neutral-v1"
        );
        assert_eq!(
            manifest.bootstrap_output.feature_flags,
            ["n1-scalar-cfg-v1"]
        );
        assert_eq!(manifest.bootstrap_output.canonical_encoding, "strict-json");
        assert_eq!(manifest.bootstrap_output.content_hash_algorithm, "sha256");
        assert_eq!(manifest.bootstrap_output.public_extension, None);
        assert!(!manifest.bootstrap_output.signature_envelope_included);
        assert!(!manifest.stable_component.includes_nir);
        assert!(manifest.stable_component.separate_lifecycle);

        Ok(())
    }

    #[test]
    fn checked_in_manifests_bind_recursive_compiler_sources(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let stage0 = parse_manifest(ACCEPTED)?;
        let compiler: super::BootstrapCompilerManifest = serde_json::from_str(COMPILER)?;

        assert_eq!(compiler.schema_version, 4);
        assert_eq!(compiler.source_roots, ["src", "typecheck"]);
        assert_eq!(
            compiler.source_files,
            [
                "src/lexer.ft",
                "src/lexer_bridge.ft",
                "src/lexer_driver.ft",
                "src/lexer_profile.ft",
                "src/parser.ft",
                "src/parser_bridge.ft",
                "src/parser_driver.ft",
                "src/parser_profile.ft",
                "src/resolver.ft",
                "src/resolver_bridge.ft",
                "src/resolver_driver.ft",
                "src/resolver_profile.ft",
                "src/sequence.ft",
                "typecheck/typecheck.ft",
                "typecheck/typecheck_bridge.ft",
                "typecheck/typecheck_driver.ft",
                "typecheck/typecheck_profile.ft",
            ]
        );
        assert_eq!(
            compiler.schema_version,
            stage0.bootstrap_compiler.manifest_schema_version
        );
        assert_eq!(
            compiler.source_roots,
            stage0.bootstrap_compiler.source_roots
        );
        assert_eq!(compiler.tree_digest, stage0.bootstrap_compiler.tree_digest);
        Ok(())
    }

    #[test]
    fn checked_in_manifests_bind_phase_provenance_records() -> Result<(), Box<dyn std::error::Error>>
    {
        let compiler: Value = serde_json::from_str(COMPILER)?;
        let stage0: Value = serde_json::from_str(ACCEPTED)?;

        assert_eq!(compiler["schemaVersion"], json!(4));
        assert_eq!(stage0["schemaVersion"], json!(4));
        assert_eq!(
            compiler["phaseRecords"],
            stage0["bootstrapCompiler"]["phaseRecords"]
        );
        assert_eq!(
            compiler["candidateImplementation"],
            stage0["bootstrapCompiler"]["candidateImplementation"]
        );
        assert_eq!(
            compiler["candidateObservationSchemaVersion"],
            stage0["bootstrapCompiler"]["candidateObservationSchemaVersion"]
        );
        assert_eq!(
            compiler["candidateImplementation"],
            json!("futao-bootstrap-candidate")
        );
        assert_eq!(compiler["candidateObservationSchemaVersion"], json!(1));
        let records = compiler["phaseRecords"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("compiler phaseRecords must be an array"))?;
        assert_eq!(records.len(), 4);
        assert_eq!(records[0]["name"], "lexer");
        assert_eq!(records[1]["name"], "parser");
        assert_eq!(records[2]["name"], "resolver");
        assert_eq!(records[3]["name"], "typecheck-expression-kernel");
        for record in records {
            assert!(record["profileEntry"].is_string());
            assert!(record["driverEntry"].is_string());
            assert!(record["observationSchemaVersion"].is_u64());
            assert!(record["protocolSchemaVersion"].is_u64());
            assert!(record["acceptedCaseCount"].is_u64());
            assert!(record["acceptedCorpusDigest"].is_string());
            assert!(record["rejectedCaseCount"].is_u64());
            assert!(record["rejectedCorpusDigest"].is_string());
            assert!(record["fuzzSeedCount"].is_u64());
            assert!(record["fuzzSeedDigest"].is_string());
            assert!(record["targetLayoutDescriptor"].is_null());
        }
        Ok(())
    }

    #[test]
    fn fixture_reader_strips_exactly_one_final_line_feed() {
        assert_eq!(canonical_fixture_bytes(b"artifact\n\n"), b"artifact\n");
        assert_eq!(canonical_fixture_bytes(b"artifact"), b"artifact");
    }

    #[test]
    fn parser_rejects_a_public_nir_bootstrap_artifact() {
        let error = parse_manifest(PUBLIC_NIR).err();

        assert!(matches!(error, Some(error) if error.to_string().contains("toolchain-internal")));
    }

    #[test]
    fn parser_rejects_an_unverifiable_source_digest() {
        let invalid = ACCEPTED.replace(
            "sha256:15ea2ee9205e327293353aa6ece756ef511c9425e740f89d5b62694ae571f492",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        );
        let error = parse_manifest(&invalid).err();

        assert!(matches!(error, Some(error) if error.to_string().contains("archiveDigest")));
    }

    #[test]
    fn parser_rejects_an_unverifiable_bootstrap_profile_digest() {
        let invalid = ACCEPTED.replace(
            "sha256:8f906ed909566e098ee8cd5029ded3b34bfb174619d198ab68bf26e57e63a3cd",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        );
        let error = parse_manifest(&invalid).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("bootstrapProfile.digest")
        ));
    }

    #[test]
    fn parser_rejects_an_undefined_bootstrap_stdlib() {
        let invalid = ACCEPTED.replace("\"status\": \"defined\"", "\"status\": \"not-defined\"");
        let error = parse_manifest(&invalid).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("bootstrapStdlib.status")
        ));
    }

    #[test]
    fn parser_rejects_noncanonical_phase_record_order() -> Result<(), Box<dyn std::error::Error>> {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["phaseRecords"]
            .as_array_mut()
            .ok_or_else(|| std::io::Error::other("phaseRecords must be an array"))?
            .swap(0, 1);

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("phaseRecords.name")
        ));
        Ok(())
    }

    #[test]
    fn parser_rejects_a_wrong_phase_observation_schema() -> Result<(), Box<dyn std::error::Error>> {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["phaseRecords"][0]["observationSchemaVersion"] = json!(2);

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("lexer observationSchemaVersion must be 1")
        ));
        Ok(())
    }

    #[test]
    fn parser_rejects_a_wrong_phase_protocol_schema() -> Result<(), Box<dyn std::error::Error>> {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["phaseRecords"][2]["protocolSchemaVersion"] = json!(1);

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("resolver protocolSchemaVersion must be 2")
        ));
        Ok(())
    }

    #[test]
    fn parser_rejects_a_front_end_target_layout_descriptor(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["phaseRecords"][0]["targetLayoutDescriptor"] =
            json!("x86_64-unknown-linux-gnu");

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "lexer targetLayoutDescriptor must be null for a front-end phase"
            )
        ));
        Ok(())
    }

    #[test]
    fn parser_rejects_a_missing_target_layout_descriptor() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["phaseRecords"][0]
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("phase record must be an object"))?
            .remove("targetLayoutDescriptor");

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        let error = error.ok_or_else(|| {
            std::io::Error::other("missing targetLayoutDescriptor must be rejected")
        })?;
        let error_chain = format!("{error:#}");
        assert!(
            error_chain.contains("targetLayoutDescriptor"),
            "unexpected error: {error_chain}"
        );
        Ok(())
    }

    #[test]
    fn parser_rejects_phase_record_count_drift() -> Result<(), Box<dyn std::error::Error>> {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["phaseRecords"]
            .as_array_mut()
            .ok_or_else(|| std::io::Error::other("phaseRecords must be an array"))?
            .pop();

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("must contain exactly four records")
        ));
        Ok(())
    }

    #[test]
    fn parser_rejects_the_previous_stage0_manifest_schema() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["schemaVersion"] = json!(3);

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("schemaVersion must be 4")
        ));
        Ok(())
    }

    #[test]
    fn parser_rejects_an_unbound_compiler_manifest_schema() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["manifestSchemaVersion"] = json!(3);

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "bootstrapCompiler.manifestSchemaVersion must be 4"
            )
        ));
        Ok(())
    }

    #[test]
    fn parser_rejects_reserved_or_unbound_candidate_identity(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["candidateImplementation"] = json!("futao-self-hosted");

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("candidateImplementation")
        ));
        Ok(())
    }

    #[test]
    fn parser_rejects_an_unbound_candidate_observation_schema(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["candidateObservationSchemaVersion"] = json!(2);

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("candidateObservationSchemaVersion")
        ));
        Ok(())
    }

    #[test]
    fn parser_rejects_unbound_compiler_source_roots() -> Result<(), Box<dyn std::error::Error>> {
        let mut manifest: Value = serde_json::from_str(ACCEPTED)?;
        manifest["bootstrapCompiler"]["sourceRoots"] = json!(["src"]);

        let error = parse_manifest(&serde_json::to_string(&manifest)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("bootstrapCompiler.sourceRoots")
        ));
        Ok(())
    }

    #[test]
    fn checked_in_bootstrap_inputs_match_the_top_level_contract(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = parse_manifest(ACCEPTED)?;

        verify_bootstrap_inputs(&manifest, &workspace_root())?;

        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_accepts_recursive_declared_source_roots(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let mut manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        let nested = directory
            .path()
            .join("bootstrap/compiler/src/nested/helper.ft");
        fs::create_dir_all(directory.path().join("bootstrap/compiler/src/nested"))?;
        fs::write(&nested, "function helper(): Unit {}\n")?;
        let compiler_path = directory
            .path()
            .join("bootstrap/compiler/bootstrap-compiler.json");
        let mut compiler = read_json(&compiler_path)?;
        compiler["sourceFiles"] = json!(discover_compiler_sources(
            &directory.path().join("bootstrap/compiler"),
            &["src".to_owned(), "typecheck".to_owned()],
        )?);
        refresh_fixture_digest(directory.path(), &mut manifest, &mut compiler)?;

        verify_bootstrap_compiler(&manifest, directory.path())?;

        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_a_missing_source_root(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            fs::remove_dir_all(root.join("bootstrap/compiler/typecheck"))?;
            Ok(())
        })?;

        assert!(error.contains("typecheck"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_a_missing_source_file(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            fs::remove_file(root.join("bootstrap/compiler/src/lexer.ft"))?;
            Ok(())
        })?;

        assert!(error.contains("sourceFiles do not match recursively discovered source roots"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_a_wrong_source_extension(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            fs::rename(
                root.join("bootstrap/compiler/src/lexer.ft"),
                root.join("bootstrap/compiler/src/lexer.txt"),
            )?;
            Ok(())
        })?;

        assert!(error.contains("bootstrap compiler source files must use .ft"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_child_manifest_binding_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let schema_error = compiler_fixture_error(|root| {
            mutate_compiler_manifest(root, |compiler| {
                compiler["schemaVersion"] = json!(1);
            })?;
            Ok(())
        })?;
        let digest_error = compiler_fixture_error(|root| {
            mutate_compiler_manifest(root, |compiler| {
                compiler["treeDigest"] = json!(
                    "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                );
            })?;
            Ok(())
        })?;

        assert!(schema_error.contains("compiler schemaVersion"));
        assert!(digest_error.contains("compiler.treeDigest"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_child_phase_record_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        mutate_compiler_manifest(directory.path(), |compiler| {
            compiler["phaseRecords"][0]["acceptedCaseCount"] = json!(5);
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "compiler phaseRecords do not match the top-level contract"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_child_candidate_binding_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let identity_error = compiler_fixture_error(|root| {
            mutate_compiler_manifest(root, |compiler| {
                compiler["candidateImplementation"] = json!("futao-self-hosted");
            })?;
            Ok(())
        })?;
        let schema_error = compiler_fixture_error(|root| {
            mutate_compiler_manifest(root, |compiler| {
                compiler["candidateObservationSchemaVersion"] = json!(2);
            })?;
            Ok(())
        })?;

        assert!(identity_error.contains("compiler.candidateImplementation"));
        assert!(schema_error.contains(
            "compiler candidateObservationSchemaVersion does not match the top-level contract"
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_phase_case_count_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let mut manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        manifest.bootstrap_compiler.phase_records[0].accepted_case_count = 5;
        mutate_compiler_manifest(directory.path(), |compiler| {
            compiler["phaseRecords"][0]["acceptedCaseCount"] = json!(5);
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "lexer acceptedCaseCount does not match checked-in inputs"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_bound_phase_digest_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let mut manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        let drift = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        manifest.bootstrap_compiler.phase_records[0].accepted_corpus_digest = drift.to_owned();
        mutate_compiler_manifest(directory.path(), |compiler| {
            compiler["phaseRecords"][0]["acceptedCorpusDigest"] = json!(drift);
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "lexer acceptedCorpusDigest does not match checked-in inputs"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_a_typecheck_fuzz_seed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            let fuzz = root.join("fuzz/corpus/typecheck");
            fs::create_dir_all(&fuzz)?;
            fs::write(fuzz.join("expression.json"), b"{}\n")?;
            Ok(())
        })?;

        assert!(error.contains("typecheck-expression-kernel fuzz input root must remain absent"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_accepted_corpus_content_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            let path = root.join("bootstrap/compiler/tests/lexer/accepted/keywords.ft");
            let mut source = fs::read_to_string(&path)?;
            source.push_str("// corpus drift\n");
            fs::write(path, source)?;
            Ok(())
        })?;

        assert!(error.contains("lexer acceptedCorpusDigest does not match checked-in inputs"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_rejected_corpus_content_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            let path = root.join("bootstrap/compiler/tests/lexer/rejected/invalid-escapes.ft");
            let mut source = fs::read_to_string(&path)?;
            source.push_str("// corpus drift\n");
            fs::write(path, source)?;
            Ok(())
        })?;

        assert!(error.contains("lexer rejectedCorpusDigest does not match checked-in inputs"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_fuzz_corpus_content_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            let path = root.join("fuzz/corpus/lexer/long-boundary.ft");
            let mut source = fs::read_to_string(&path)?;
            source.push_str("// fuzz drift\n");
            fs::write(path, source)?;
            Ok(())
        })?;

        assert!(error.contains("lexer fuzzSeedDigest does not match checked-in inputs"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_a_phase_input_with_the_wrong_extension(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            fs::rename(
                root.join("bootstrap/compiler/tests/lexer/accepted/keywords.ft"),
                root.join("bootstrap/compiler/tests/lexer/accepted/keywords.txt"),
            )?;
            Ok(())
        })?;

        assert!(error.contains("phase input entry must use .ft"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_non_utf8_phase_input(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            fs::write(
                root.join("bootstrap/compiler/tests/lexer/accepted/keywords.ft"),
                [0xff, 0xfe],
            )?;
            Ok(())
        })?;

        assert!(error.contains("phase input must be UTF-8"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_compiler_provenance_rejects_a_phase_input_root_symlink(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let error = compiler_fixture_error(|root| {
            let lexer = root.join("bootstrap/compiler/tests/lexer");
            fs::rename(lexer.join("accepted"), lexer.join("accepted-target"))?;
            symlink("accepted-target", lexer.join("accepted"))?;
            Ok(())
        })?;

        assert!(error.contains("phase input directory must be a directory, not a symlink"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_an_unbound_nested_phase_input(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let error = compiler_fixture_error(|root| {
            let accepted = root.join("bootstrap/compiler/tests/lexer/accepted");
            fs::create_dir(accepted.join("nested"))?;
            fs::write(
                accepted.join("nested/additional.ft"),
                "function additional(): Unit {}\n",
            )?;
            Ok(())
        })?;

        assert!(error.contains("lexer acceptedCaseCount does not match checked-in inputs"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_compiler_provenance_rejects_a_phase_input_directory_symlink(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let error = compiler_fixture_error(|root| {
            let accepted = root.join("bootstrap/compiler/tests/lexer/accepted");
            symlink(".", accepted.join("nested-link"))?;
            Ok(())
        })?;

        assert!(error.contains("phase input entry must not be a symlink"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_compiler_provenance_rejects_a_phase_input_file_symlink(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let error = compiler_fixture_error(|root| {
            let accepted = root.join("bootstrap/compiler/tests/lexer/accepted");
            fs::remove_file(accepted.join("keywords.ft"))?;
            symlink("operators.ft", accepted.join("keywords.ft"))?;
            Ok(())
        })?;

        assert!(error.contains("phase input entry must not be a symlink"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_an_unbound_phase_entry(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let mut manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        let source = directory
            .path()
            .join("bootstrap/compiler/src/lexer_profile.ft");
        fs::remove_file(source)?;
        let compiler_path = directory
            .path()
            .join("bootstrap/compiler/bootstrap-compiler.json");
        let mut compiler = read_json(&compiler_path)?;
        compiler["sourceFiles"]
            .as_array_mut()
            .ok_or_else(|| std::io::Error::other("sourceFiles must be an array"))?
            .retain(|entry| entry != "src/lexer_profile.ft");
        refresh_fixture_digest(directory.path(), &mut manifest, &mut compiler)?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "compiler phase profileEntry is not bound by sourceFiles: src/lexer_profile.ft"
            )
        ));
        Ok(())
    }

    #[test]
    fn compiler_tree_digest_binds_schema_roots_paths_and_contents(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        copy_bootstrap_compiler_fixture(directory.path())?;
        let compiler_root = directory.path().join("bootstrap/compiler");
        let compiler_path = compiler_root.join("bootstrap-compiler.json");
        let original = read_json(&compiler_path)?;
        let baseline: super::BootstrapCompilerManifest = serde_json::from_value(original.clone())?;
        let baseline_digest = compiler_tree_digest(&baseline, &compiler_root)?;

        let mut schema = original.clone();
        schema["schemaVersion"] = json!(2);
        let schema_manifest: super::BootstrapCompilerManifest = serde_json::from_value(schema)?;
        assert_ne!(
            baseline_digest,
            compiler_tree_digest(&schema_manifest, &compiler_root)?
        );

        let mut roots = original.clone();
        roots["sourceRoots"] = json!(["typecheck", "src"]);
        let roots_manifest: super::BootstrapCompilerManifest = serde_json::from_value(roots)?;
        assert_ne!(
            baseline_digest,
            compiler_tree_digest(&roots_manifest, &compiler_root)?
        );

        let mut paths = original.clone();
        if let Some(source_files) = paths["sourceFiles"].as_array_mut() {
            source_files.swap(0, 1);
        }
        let paths_manifest: super::BootstrapCompilerManifest = serde_json::from_value(paths)?;
        assert_ne!(
            baseline_digest,
            compiler_tree_digest(&paths_manifest, &compiler_root)?
        );

        let source = compiler_root.join("src/lexer.ft");
        let mut bytes = fs::read(&source)?;
        bytes.push(b'\n');
        fs::write(&source, bytes)?;
        assert_ne!(
            baseline_digest,
            compiler_tree_digest(&baseline, &compiler_root)?
        );
        Ok(())
    }

    #[test]
    fn compiler_tree_digest_matches_the_tree_v2_fixed_vector(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        fs::create_dir(directory.path().join("src"))?;
        fs::write(
            directory.path().join("src/main.ft"),
            b"function main(): Unit {}\n",
        )?;
        let mut compiler: Value = serde_json::from_str(COMPILER)?;
        compiler["schemaVersion"] = json!(2);
        compiler["sourceRoots"] = json!(["src"]);
        compiler["sourceFiles"] = json!(["src/main.ft"]);
        let compiler: super::BootstrapCompilerManifest = serde_json::from_value(compiler)?;

        assert_eq!(
            compiler_tree_digest(&compiler, directory.path())?,
            "sha256:6f092de2710a2a723a993ac86e1d0afbb6c8d11e9efa5c565789578289bc3889"
        );
        Ok(())
    }

    #[test]
    fn phase_input_digest_matches_the_phase_inputs_v1_fixed_vector(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let root = directory.path().join("fixtures/accepted");
        fs::create_dir_all(&root)?;
        fs::write(root.join("z.ft"), b"function z(): Unit {}\n")?;
        fs::write(root.join("a.ft"), b"function a(): Unit {}\n")?;
        let spec = PhaseInputSpec {
            root: Some("fixtures/accepted"),
            extension: Some("ft"),
            absent_root: None,
            case_model: PhaseCaseModel::Files,
        };

        assert_eq!(
            phase_input_observation(directory.path(), "fixture", "accepted", spec)?,
            (
                2,
                "sha256:5d8eec01cfb530ff0a4d046d1cb14d3aac4ce5b8f4746033f1e09b4146d7e70f"
                    .to_owned(),
            )
        );
        Ok(())
    }

    #[test]
    fn typecheck_zero_fuzz_digest_matches_the_absent_input_fixed_vector(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            phase_input_observation(
                &workspace_root(),
                "typecheck-expression-kernel",
                "fuzz",
                PHASE_SPECS[3].fuzz,
            )?,
            (
                0,
                "sha256:58a78ba5d24b522c8d580b44ebf75da0bcf0539c79a293ffb4c5c2d8a397804b"
                    .to_owned(),
            )
        );
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_an_undeclared_file_in_a_source_root(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        fs::write(
            directory
                .path()
                .join("bootstrap/compiler/src/undeclared.ft"),
            "function undeclared(): Unit {}\n",
        )?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "compiler sourceFiles do not match recursively discovered source roots"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_an_undeclared_source_root(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        let source = directory
            .path()
            .join("bootstrap/compiler/lowering/lower.ft");
        fs::create_dir_all(directory.path().join("bootstrap/compiler/lowering"))?;
        fs::write(source, "function lower(): Unit {}\n")?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("undeclared bootstrap compiler source")
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_unsorted_source_roots(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        mutate_compiler_manifest(directory.path(), |compiler| {
            compiler["sourceRoots"] = json!(["typecheck", "src"]);
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "compiler sourceRoots must be unique and sorted by portable path"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_duplicate_source_roots(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        mutate_compiler_manifest(directory.path(), |compiler| {
            compiler["sourceRoots"] = json!(["src", "src", "typecheck"]);
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "compiler sourceRoots must be unique and sorted by portable path"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_overlapping_source_roots(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        mutate_compiler_manifest(directory.path(), |compiler| {
            compiler["sourceRoots"] = json!(["src", "src/nested", "typecheck"]);
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("compiler sourceRoots must not overlap")
        ));
        Ok(())
    }

    #[test]
    fn compiler_source_contract_rejects_case_insensitive_reverse_root_overlap() {
        let roots = vec!["SRC/nested".to_owned(), "src".to_owned()];

        let error = validate_compiler_source_roots(&roots).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("compiler sourceRoots must not overlap")
        ));
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_non_portable_source_roots(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        mutate_compiler_manifest(directory.path(), |compiler| {
            compiler["sourceRoots"] = json!(["../src", "typecheck"]);
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "compiler sourceRoots entries must be normalized portable relative directories"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_unsorted_source_files(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        mutate_compiler_manifest(directory.path(), |compiler| {
            if let Some(source_files) = compiler["sourceFiles"].as_array_mut() {
                source_files.swap(0, 1);
            }
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "compiler sourceFiles must be unique and sorted by portable path"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_duplicate_source_files(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        mutate_compiler_manifest(directory.path(), |compiler| {
            if let Some(sources) = compiler["sourceFiles"].as_array_mut() {
                if let Some(first) = sources.first().cloned() {
                    sources.insert(1, first);
                }
            }
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "compiler sourceFiles must be unique and sorted by portable path"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_non_portable_source_files(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        mutate_compiler_manifest(directory.path(), |compiler| {
            compiler["sourceFiles"][0] = json!("src\\lexer.ft");
        })?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "compiler sourceFiles entries must be normalized portable `.ft` paths"
            )
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_invalid_utf8_source(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        let source = directory
            .path()
            .join("bootstrap/compiler/typecheck/typecheck.ft");
        fs::write(&source, [0xff, 0xfe])?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("typecheck.ft must be UTF-8")
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_compiler_provenance_rejects_source_symlinks(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        let source = directory.path().join("bootstrap/compiler/src/lexer.ft");
        fs::remove_file(&source)?;
        symlink("sequence.ft", &source)?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains(
                "bootstrap compiler source entry must not be a symlink"
            )
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_compiler_provenance_rejects_a_source_root_symlink(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let error = compiler_fixture_error(|root| {
            let typecheck = root.join("bootstrap/compiler/typecheck");
            fs::remove_dir_all(&typecheck)?;
            symlink("../src", typecheck)?;
            Ok(())
        })?;

        assert!(error.contains("must not be a symlink"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_compiler_provenance_rejects_a_compiler_root_symlink(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let error = compiler_fixture_error(|root| {
            let compiler = root.join("bootstrap/compiler");
            let compiler_target = root.join("bootstrap/compiler-target");
            fs::rename(&compiler, &compiler_target)?;
            symlink("compiler-target", compiler)?;
            Ok(())
        })?;

        assert!(error.contains("bootstrap compiler root must be a directory, not a symlink"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_compiler_provenance_rejects_a_bootstrap_root_symlink(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let error = compiler_fixture_error(|root| {
            let bootstrap = root.join("bootstrap");
            let bootstrap_target = root.join("bootstrap-target");
            fs::rename(&bootstrap, &bootstrap_target)?;
            symlink("bootstrap-target", bootstrap)?;
            Ok(())
        })?;

        assert!(error.contains("bootstrap root must be a directory, not a symlink"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_compiler_provenance_rejects_a_nested_directory_symlink(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let error = compiler_fixture_error(|root| {
            let source_root = root.join("bootstrap/compiler/src");
            fs::create_dir(source_root.join("nested-target"))?;
            symlink("nested-target", source_root.join("nested-link"))?;
            Ok(())
        })?;

        assert!(error.contains("must not be a symlink"));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bootstrap_compiler_provenance_rejects_a_non_utf8_source_filename(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let error = compiler_fixture_error(|root| {
            let name = OsString::from_vec(vec![0xff, b'.', b'f', b't']);
            fs::write(root.join("bootstrap/compiler/src").join(name), b"source")?;
            Ok(())
        })?;

        assert!(error.contains("compiler source path must be UTF-8"));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_source_drift() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        let lexer = directory.path().join("bootstrap/compiler/src/lexer.ft");
        let mut source = fs::read_to_string(&lexer)?;
        source.push_str("// provenance drift\n");
        fs::write(&lexer, source)?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("bootstrap compiler tree digest mismatch")
        ));
        Ok(())
    }

    #[test]
    fn bootstrap_compiler_provenance_rejects_typecheck_source_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        let typecheck = directory
            .path()
            .join("bootstrap/compiler/typecheck/typecheck.ft");
        let mut source = fs::read_to_string(&typecheck)?;
        source.push_str("// provenance drift\n");
        fs::write(&typecheck, source)?;

        let error = verify_bootstrap_compiler(&manifest, directory.path()).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("bootstrap compiler tree digest mismatch")
        ));
        Ok(())
    }

    #[test]
    fn provenance_rejects_content_that_does_not_match_its_digest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expected = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

        verify_digest("fixture", expected, b"abc")?;
        let error = verify_digest("fixture", expected, b"abd").err();

        assert!(
            matches!(error, Some(error) if error.to_string().contains("fixture digest mismatch"))
        );
        Ok(())
    }

    #[test]
    fn stage0_rebuild_uses_a_fresh_target_inside_the_detached_worktree() {
        let worktree = Path::new("detached-stage0");

        assert_eq!(stage0_target_dir(worktree), worktree.join("target"));
    }

    #[test]
    fn stage0_projection_rewrites_only_ft_import_specifiers(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"import { Value } from "./value.ft";

function main(): Unit {
  const fixtureName = "value.ft";
}
"#;

        assert_eq!(
            project_source_for_stage0(source)?,
            r#"import { Value } from "./value.nexa";

function main(): Unit {
  const fixtureName = "value.ft";
}
"#
        );
        Ok(())
    }

    #[test]
    fn stage0_projection_uses_the_checked_in_stdlib_manifest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;

        let entry = project_bootstrap_stdlib_for_stage0(&workspace_root(), directory.path())?;

        assert_eq!(entry, directory.path().join("src/bootstrap.nexa"));
        assert!(directory.path().join("src/array.nexa").is_file());
        assert!(!directory.path().join("src/array.ft").exists());
        assert!(fs::read_to_string(entry)?.contains("from \"./array.nexa\""));
        Ok(())
    }

    #[test]
    fn checked_in_stage0_provenance_matches_the_pinned_git_objects(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = parse_manifest(ACCEPTED)?;

        verify_source(&manifest, &workspace_root())?;

        Ok(())
    }

    fn compiler_fixture_error(
        mutate: impl FnOnce(&Path) -> Result<(), Box<dyn std::error::Error>>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let manifest = prepare_schema_four_compiler_fixture(directory.path())?;
        mutate(directory.path())?;
        match verify_bootstrap_compiler(&manifest, directory.path()) {
            Ok(()) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "verification unexpectedly succeeded",
            )
            .into()),
            Err(error) => Ok(error.to_string()),
        }
    }

    fn prepare_schema_four_compiler_fixture(
        destination: &Path,
    ) -> Result<super::BootstrapManifest, Box<dyn std::error::Error>> {
        copy_bootstrap_compiler_fixture(destination)?;
        Ok(parse_manifest(ACCEPTED)?)
    }

    fn refresh_fixture_digest(
        destination: &Path,
        manifest: &mut super::BootstrapManifest,
        compiler: &mut Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let parsed: super::BootstrapCompilerManifest = serde_json::from_value(compiler.clone())?;
        let digest = compiler_tree_digest(&parsed, &destination.join("bootstrap/compiler"))?;
        compiler["treeDigest"] = json!(digest);
        manifest.bootstrap_compiler.tree_digest = digest;
        write_json(
            &destination.join("bootstrap/compiler/bootstrap-compiler.json"),
            compiler,
        )?;
        Ok(())
    }

    fn mutate_compiler_manifest(
        destination: &Path,
        mutate: impl FnOnce(&mut Value),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = destination.join("bootstrap/compiler/bootstrap-compiler.json");
        let mut compiler = read_json(&path)?;
        mutate(&mut compiler);
        write_json(&path, &compiler)?;
        Ok(())
    }

    fn read_json(path: &Path) -> Result<Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
    }

    fn write_json(path: &Path, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let mut bytes = serde_json::to_vec_pretty(value)?;
        bytes.push(b'\n');
        fs::write(path, bytes)?;
        Ok(())
    }

    fn copy_bootstrap_compiler_fixture(destination: &Path) -> Result<(), std::io::Error> {
        for relative in [
            "bootstrap/compiler",
            "fuzz/corpus/lexer",
            "fuzz/corpus/parser",
            "fuzz/corpus/resolver",
        ] {
            copy_source_tree(
                &workspace_root().join(relative),
                &destination.join(relative),
            )?;
        }
        Ok(())
    }

    fn copy_source_tree(source: &Path, target: &Path) -> Result<(), std::io::Error> {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let destination = target.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_source_tree(&entry.path(), &destination)?;
            } else {
                fs::copy(entry.path(), destination)?;
            }
        }
        Ok(())
    }

    struct TestDirectory {
        path: std::path::PathBuf,
    }

    impl TestDirectory {
        fn new() -> Result<Self, std::io::Error> {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nexa-bootstrap-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
