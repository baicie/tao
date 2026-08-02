use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, ensure, Context, Result};
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
        ensure!(self.schema_version == 1, "schemaVersion must be 1");
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
            "https://github.com/baicie/nexa",
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
        canonical_fixture_bytes, parse_manifest, project_bootstrap_stdlib_for_stage0,
        project_source_for_stage0, stage0_target_dir, verify_bootstrap_inputs, verify_digest,
        verify_source, workspace_root,
    };

    const ACCEPTED: &str = include_str!("../../bootstrap/stage0/bootstrap-manifest.json");
    const PUBLIC_NIR: &str =
        include_str!("../../bootstrap/tests/rejected/public-nir-artifact.json");
    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn checked_in_stage0_manifest_defines_the_internal_bootstrap_boundary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = parse_manifest(ACCEPTED)?;

        assert_eq!(manifest.schema_version, 1);
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
        assert_eq!(manifest.bootstrap_output.kind, "internal-nir");
        assert_eq!(manifest.bootstrap_output.consumer, "rust-verifier-backend");
        assert_eq!(manifest.bootstrap_output.artifact_magic, "FUTAO-NIR");
        assert_eq!(manifest.bootstrap_output.nir_schema_version, 1);
        assert_eq!(manifest.bootstrap_output.verifier_crate, "nexa_nir");
        assert_eq!(manifest.bootstrap_output.verifier_version, "0.0.6");
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
    fn checked_in_bootstrap_inputs_match_the_top_level_contract(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = parse_manifest(ACCEPTED)?;

        verify_bootstrap_inputs(&manifest, &workspace_root())?;

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
