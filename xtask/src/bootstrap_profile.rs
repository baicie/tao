use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, ensure, Context, Result};
use nexa_compiler::{
    compile, run_session, CompilerInput, CompilerOptions, CompilerSession, CompilerSource,
};
use nexa_source::MemorySourceProvider;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const PROFILE_ID: &str = "futao-bootstrap-v1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapProfile {
    schema_version: u32,
    id: String,
    language_version: String,
    source_extension: String,
    declarations: Vec<String>,
    statements: Vec<String>,
    values: Vec<String>,
    pure_builtins: Vec<String>,
    ownership: OwnershipProfile,
    iteration_orders: Vec<String>,
    denied_syntax: Vec<String>,
    denied_capabilities: Vec<String>,
    host_effects: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnershipProfile {
    copy: Vec<String>,
    immutable_owned: Vec<String>,
    source_operations: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StdlibTransition {
    schema_version: u32,
    profile: String,
    installed: VersionedDigest,
    candidate: VersionedDigest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VersionedDigest {
    version: String,
    digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapStdlibManifest {
    schema_version: u32,
    name: String,
    version: String,
    profile: String,
    entry: String,
    source_files: Vec<String>,
    required_capabilities: Vec<String>,
    iteration_policy: String,
    tree_hash_algorithm: String,
    tree_digest: String,
    canonical_dump_schema_version: u32,
    build_digest: String,
}

pub(crate) fn run() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let profile_path = root.join("bootstrap/profile/bootstrap-profile-v1.json");
    let upgrade_path = root.join("bootstrap/tests/accepted/bootstrap-stdlib-upgrade.json");
    let rollback_path = root.join("bootstrap/tests/rejected/bootstrap-stdlib-rollback.json");
    let stdlib_manifest_path = root.join("bootstrap/stdlib/bootstrap-stdlib.json");

    let profile = parse_file(&profile_path, parse_profile)?;
    let upgrade = parse_file(&upgrade_path, parse_transition)?;
    let rollback_text = read(&rollback_path)?;
    ensure!(
        parse_transition(&rollback_text).is_err(),
        "{} must remain a rejected rollback fixture",
        rollback_path.display()
    );
    let stdlib = parse_file(&stdlib_manifest_path, parse_stdlib_manifest)?;
    verify_stdlib(&root, &stdlib)?;
    let behavior = run_stdlib_fixture(&root, &stdlib, "tests/array-string.ft")?;
    ensure!(
        behavior == ["10", "1", "2", "3", "1", "0", "2", "futao"],
        "bootstrap stdlib array/string fixture output changed"
    );
    let large_behavior = run_stdlib_fixture(&root, &stdlib, "tests/large-collections.ft")?;
    ensure!(
        large_behavior
            == [
                "80", "2", "3160", "0", "79", "79", "79", "80", "79", "800", "79", "800", "true",
                "true", "true",
            ],
        "bootstrap stdlib large-collection fixture output changed"
    );

    println!(
        "bootstrap profile {}: language {}, stdlib {} and {} -> {} upgrade verified",
        profile.id,
        profile.language_version,
        stdlib.version,
        upgrade.installed.version,
        upgrade.candidate.version
    );
    Ok(())
}

fn parse_file<T>(path: &Path, parse: impl FnOnce(&str) -> Result<T>) -> Result<T> {
    let text = read(path)?;
    parse(&text).with_context(|| format!("failed to validate {}", path.display()))
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn parse_profile(text: &str) -> Result<BootstrapProfile> {
    let profile: BootstrapProfile =
        serde_json::from_str(text).context("invalid bootstrap profile JSON")?;
    profile.validate()?;
    Ok(profile)
}

impl BootstrapProfile {
    fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "schemaVersion must be 1");
        ensure!(self.id == PROFILE_ID, "id must be {PROFILE_ID}");
        ensure!(
            self.language_version == "1.0",
            "languageVersion must be 1.0"
        );
        ensure!(
            self.source_extension == ".ft",
            "sourceExtension must be .ft"
        );
        expect_list(
            "declarations",
            &self.declarations,
            &[
                "record",
                "tagged-union",
                "function",
                "generic-function",
                "generic-type",
                "closure",
                "explicit-relative-import",
            ],
        )?;
        expect_list(
            "statements",
            &self.statements,
            &["const", "if", "for-of", "return", "expression"],
        )?;
        expect_list(
            "values",
            &self.values,
            &[
                "Int",
                "Bool",
                "String",
                "Unit",
                "Array<T>",
                "Option<T>",
                "Result<T,E>",
                "record",
                "tagged-union",
                "function",
            ],
        )?;
        expect_list(
            "pureBuiltins",
            &self.pure_builtins,
            &[
                "array.length",
                "array.append",
                "array.concat",
                "string.length",
                "toString",
                "parseInt",
            ],
        )?;
        expect_list(
            "ownership.copy",
            &self.ownership.copy,
            &["Int", "Bool", "Unit"],
        )?;
        expect_list(
            "ownership.immutableOwned",
            &self.ownership.immutable_owned,
            &["String", "Array<T>", "record", "tagged-union", "closure"],
        )?;
        ensure!(
            self.ownership.source_operations.is_empty(),
            "ownership.sourceOperations must be empty until source-level ownership ships"
        );
        expect_list(
            "iterationOrders",
            &self.iteration_orders,
            &["array-index", "source-order", "sorted-key"],
        )?;
        expect_list(
            "deniedSyntax",
            &self.denied_syntax,
            &["let", "assignment", "while", "break", "continue"],
        )?;
        expect_list(
            "deniedCapabilities",
            &self.denied_capabilities,
            &[
                "filesystem",
                "network",
                "process",
                "clock",
                "environment",
                "random",
                "thread",
                "async",
                "ui",
                "dynamic-loading",
                "plugin",
                "reflection",
            ],
        )?;
        ensure!(
            self.host_effects.is_empty(),
            "hostEffects must be empty for the compiler core"
        );
        Ok(())
    }
}

fn expect_list(field: &str, actual: &[String], expected: &[&str]) -> Result<()> {
    ensure!(
        actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied()),
        "{field} must match the frozen ordered set"
    );
    Ok(())
}

fn parse_transition(text: &str) -> Result<StdlibTransition> {
    let transition: StdlibTransition =
        serde_json::from_str(text).context("invalid bootstrap stdlib transition JSON")?;
    transition.validate()?;
    Ok(transition)
}

impl StdlibTransition {
    fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "schemaVersion must be 1");
        ensure!(self.profile == PROFILE_ID, "profile must be {PROFILE_ID}");
        validate_sha256("installed.digest", &self.installed.digest)?;
        validate_sha256("candidate.digest", &self.candidate.digest)?;
        let installed = parse_version("installed.version", &self.installed.version)?;
        let candidate = parse_version("candidate.version", &self.candidate.version)?;
        ensure!(
            candidate > installed,
            "candidate.version must advance beyond installed.version"
        );
        Ok(())
    }
}

fn parse_version(field: &str, value: &str) -> Result<(u64, u64, u64)> {
    let parts = value
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("{field} must contain three unsigned integers"))?;
    ensure!(parts.len() == 3, "{field} must contain three components");
    ensure!(
        value == format!("{}.{}.{}", parts[0], parts[1], parts[2]),
        "{field} must use canonical decimal components"
    );
    Ok((parts[0], parts[1], parts[2]))
}

fn validate_sha256(field: &str, value: &str) -> Result<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        bail!("{field} must use the sha256 algorithm");
    };
    ensure!(
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "{field} must contain 64 lowercase hexadecimal digits"
    );
    ensure!(
        hex.bytes().any(|byte| byte != b'0'),
        "{field} must not be zero"
    );
    Ok(())
}

fn parse_stdlib_manifest(text: &str) -> Result<BootstrapStdlibManifest> {
    let manifest: BootstrapStdlibManifest =
        serde_json::from_str(text).context("invalid bootstrap stdlib manifest JSON")?;
    manifest.validate()?;
    Ok(manifest)
}

impl BootstrapStdlibManifest {
    fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "schemaVersion must be 1");
        ensure!(
            self.name == "futao-bootstrap",
            "name must be futao-bootstrap"
        );
        let _ = parse_version("version", &self.version)?;
        ensure!(self.profile == PROFILE_ID, "profile must be {PROFILE_ID}");
        validate_source_path("entry", &self.entry)?;
        ensure!(
            !self.source_files.is_empty(),
            "sourceFiles must not be empty"
        );
        ensure!(
            self.source_files.windows(2).all(|pair| pair[0] < pair[1]),
            "sourceFiles must be unique and sorted by portable path"
        );
        for source in &self.source_files {
            validate_source_path("sourceFiles", source)?;
        }
        ensure!(
            self.source_files.contains(&self.entry),
            "entry must be present in sourceFiles"
        );
        ensure!(
            self.required_capabilities.is_empty(),
            "requiredCapabilities must be empty"
        );
        ensure!(
            self.iteration_policy == "source-order",
            "iterationPolicy must be source-order"
        );
        ensure!(
            self.tree_hash_algorithm == "sha256",
            "treeHashAlgorithm must be sha256"
        );
        validate_sha256("treeDigest", &self.tree_digest)?;
        ensure!(
            self.canonical_dump_schema_version == 1,
            "canonicalDumpSchemaVersion must be 1"
        );
        validate_sha256("buildDigest", &self.build_digest)?;
        Ok(())
    }
}

fn validate_source_path(field: &str, value: &str) -> Result<()> {
    ensure!(
        value.starts_with("src/") && value.ends_with(".ft"),
        "{field} entries must be `.ft` files below src/"
    );
    ensure!(
        !value.contains(['\\', '\0'])
            && value
                .split('/')
                .all(|component| !component.is_empty() && component != "." && component != ".."),
        "{field} entries must be normalized portable paths"
    );
    Ok(())
}

fn verify_stdlib(root: &Path, manifest: &BootstrapStdlibManifest) -> Result<()> {
    let stdlib_root = root.join("bootstrap/stdlib");
    let discovered = discover_stdlib_sources(&stdlib_root)?;
    ensure!(
        discovered == manifest.source_files,
        "bootstrap stdlib sourceFiles do not match the checked-in src directory"
    );

    let mut tree_hasher = Sha256::new();
    tree_hasher.update(b"FUTAO-BOOTSTRAP-STDLIB\0");
    let mut sources = Vec::with_capacity(manifest.source_files.len());
    for relative in &manifest.source_files {
        let path = stdlib_root.join(relative);
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        ensure!(
            metadata.file_type().is_file(),
            "{} must be a file",
            path.display()
        );
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        hash_field(&mut tree_hasher, relative.as_bytes())?;
        hash_field(&mut tree_hasher, &bytes)?;
        let content = String::from_utf8(bytes)
            .with_context(|| format!("{} must be UTF-8", path.display()))?;
        sources.push(CompilerSource::new(relative, content));
    }
    verify_hash(
        "bootstrap stdlib tree",
        &manifest.tree_digest,
        tree_hasher.finalize(),
    )?;

    let forward = compile_stdlib(manifest, sources.clone())?;
    sources.reverse();
    let reverse = compile_stdlib(manifest, sources)?;
    ensure!(
        forward == reverse,
        "bootstrap stdlib canonical build changed with source insertion order"
    );

    let mut build_hasher = Sha256::new();
    build_hasher.update(b"FUTAO-BOOTSTRAP-STDLIB-BUILD\0");
    build_hasher.update(forward.as_bytes());
    verify_hash(
        "bootstrap stdlib build",
        &manifest.build_digest,
        build_hasher.finalize(),
    )
}

fn run_stdlib_fixture(
    root: &Path,
    manifest: &BootstrapStdlibManifest,
    fixture: &str,
) -> Result<Vec<String>> {
    validate_fixture_path(fixture)?;
    let stdlib_root = root.join("bootstrap/stdlib");
    let mut provider = MemorySourceProvider::default();
    for relative in &manifest.source_files {
        let path = stdlib_root.join(relative);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let _ = provider.insert(relative, bytes);
    }
    let fixture_path = stdlib_root.join(fixture);
    let fixture_bytes = std::fs::read(&fixture_path)
        .with_context(|| format!("failed to read {}", fixture_path.display()))?;
    let _ = provider.insert(fixture, fixture_bytes);

    let session = CompilerSession::build(provider, Path::new(fixture))
        .with_context(|| format!("failed to load bootstrap stdlib fixture {fixture}"))?;
    let output = run_session(&session);
    if !output.is_ok() {
        let diagnostics = output
            .diagnostics()
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
            .collect::<Vec<_>>()
            .join("; ");
        let runtime = output.runtime_error().map_or_else(String::new, |error| {
            format!("; runtime: {}", error.message())
        });
        bail!("bootstrap stdlib fixture {fixture} failed: {diagnostics}{runtime}");
    }
    Ok(output.output().to_vec())
}

fn validate_fixture_path(value: &str) -> Result<()> {
    ensure!(
        value.starts_with("tests/") && value.ends_with(".ft"),
        "bootstrap stdlib fixture must be a `.ft` file below tests/"
    );
    ensure!(
        !value.contains(['\\', '\0'])
            && value
                .split('/')
                .all(|component| !component.is_empty() && component != "." && component != ".."),
        "bootstrap stdlib fixture must be a normalized portable path"
    );
    Ok(())
}

fn discover_stdlib_sources(stdlib_root: &Path) -> Result<Vec<String>> {
    let source_root = stdlib_root.join("src");
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(&source_root)
        .with_context(|| format!("failed to read {}", source_root.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read {} entry", source_root.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        ensure!(
            file_type.is_file(),
            "bootstrap stdlib src entries must be regular files"
        );
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("bootstrap stdlib source name must be UTF-8"))?;
        validate_source_path("discovered source", &format!("src/{name}"))?;
        sources.push(format!("src/{name}"));
    }
    sources.sort();
    Ok(sources)
}

fn compile_stdlib(
    manifest: &BootstrapStdlibManifest,
    sources: Vec<CompilerSource>,
) -> Result<String> {
    let output = compile(&CompilerInput::with_options(
        &manifest.entry,
        sources,
        CompilerOptions::bootstrap_v1(),
    ))
    .map_err(|error| {
        anyhow::anyhow!("bootstrap stdlib compiler-core invocation failed: {error:?}")
    })?;
    if !output.is_ok() {
        let summary = output
            .diagnostics()
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
            .collect::<Vec<_>>()
            .join("; ");
        bail!("bootstrap stdlib did not compile: {summary}");
    }
    output
        .dumps()
        .to_json()
        .context("failed to serialize bootstrap stdlib canonical build")
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) -> Result<()> {
    let length = u64::try_from(bytes.len()).context("bootstrap stdlib hash field is too large")?;
    hasher.update(length.to_be_bytes());
    hasher.update(bytes);
    Ok(())
}

fn verify_hash(field: &str, expected: &str, digest: impl IntoIterator<Item = u8>) -> Result<()> {
    let mut actual = String::with_capacity(71);
    actual.push_str("sha256:");
    for byte in digest {
        let _ = write!(actual, "{byte:02x}");
    }
    ensure!(
        actual == expected,
        "{field} digest mismatch: expected {expected}, found {actual}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_profile, parse_stdlib_manifest, parse_transition, run_stdlib_fixture, verify_stdlib,
    };

    const PROFILE: &str = include_str!("../../bootstrap/profile/bootstrap-profile-v1.json");
    const UPGRADE: &str =
        include_str!("../../bootstrap/tests/accepted/bootstrap-stdlib-upgrade.json");
    const ROLLBACK: &str =
        include_str!("../../bootstrap/tests/rejected/bootstrap-stdlib-rollback.json");
    const STDLIB: &str = include_str!("../../bootstrap/stdlib/bootstrap-stdlib.json");

    #[test]
    fn checked_in_profile_freezes_the_pure_capability_surface(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let profile = parse_profile(PROFILE)?;

        assert_eq!(profile.id, "futao-bootstrap-v1");
        assert_eq!(profile.source_extension, ".ft");
        assert!(profile.host_effects.is_empty());
        assert!(profile
            .denied_capabilities
            .contains(&"filesystem".to_owned()));
        assert!(profile.denied_capabilities.contains(&"network".to_owned()));
        assert!(profile.denied_capabilities.contains(&"clock".to_owned()));
        assert!(profile.denied_capabilities.contains(&"process".to_owned()));
        Ok(())
    }

    #[test]
    fn checked_in_upgrade_fixture_moves_forward_within_profile(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let transition = parse_transition(UPGRADE)?;

        assert_eq!(transition.installed.version, "0.0.0");
        assert_eq!(transition.candidate.version, "0.0.1");
        Ok(())
    }

    #[test]
    fn checked_in_rollback_fixture_is_rejected() {
        let error = parse_transition(ROLLBACK).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("must advance")
        ));
    }

    #[test]
    fn checked_in_stdlib_is_content_addressed_and_builds_deterministically(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let manifest = parse_stdlib_manifest(STDLIB)?;

        verify_stdlib(&root, &manifest)?;
        Ok(())
    }

    #[test]
    fn stdlib_manifest_rejects_a_host_capability() {
        let invalid = STDLIB.replace(
            "\"requiredCapabilities\": []",
            "\"requiredCapabilities\": [\"filesystem\"]",
        );
        let error = parse_stdlib_manifest(&invalid).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("requiredCapabilities")
        ));
    }

    #[test]
    fn array_and_string_apis_have_deterministic_observable_behavior(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let manifest = parse_stdlib_manifest(STDLIB)?;

        assert_eq!(
            run_stdlib_fixture(&root, &manifest, "tests/array-string.ft")?,
            ["10", "1", "2", "3", "1", "0", "2", "futao"]
        );
        Ok(())
    }

    #[test]
    fn collection_apis_preserve_stable_insertion_order_and_updates(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let manifest = parse_stdlib_manifest(STDLIB)?;

        assert_eq!(
            run_stdlib_fixture(&root, &manifest, "tests/collections.ft")?,
            ["3", "2", "b", "a", "1", "20", "10", "true", "true", "true"]
        );
        Ok(())
    }

    #[test]
    fn stdlib_operations_cross_the_reference_call_depth_boundary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let manifest = parse_stdlib_manifest(STDLIB)?;

        assert_eq!(
            run_stdlib_fixture(&root, &manifest, "tests/large-collections.ft")?,
            [
                "80", "2", "3160", "0", "79", "79", "79", "80", "79", "800", "79", "800", "true",
                "true", "true",
            ]
        );
        Ok(())
    }

    #[test]
    fn arena_ids_reject_foreign_stale_and_invalid_access() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let manifest = parse_stdlib_manifest(STDLIB)?;

        assert_eq!(
            run_stdlib_fixture(&root, &manifest, "tests/arena.ft")?,
            [
                "0",
                "1",
                "0",
                "first",
                "first",
                "1",
                "stale-generation",
                "wrong-arena",
                "invalid-index"
            ]
        );
        Ok(())
    }
}
