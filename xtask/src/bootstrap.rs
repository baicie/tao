use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapManifest {
    schema_version: u32,
    toolchain_version: String,
    language_version: String,
    stage0: Stage0,
    bootstrap_stdlib: BootstrapStdlib,
    bootstrap_output: BootstrapOutput,
    stable_component: StableComponent,
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
    version: Option<String>,
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapOutput {
    kind: String,
    stability: String,
    consumer: String,
    comparison: String,
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
    verify_source(&manifest, &root)?;
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
        expect("toolchainVersion", &self.toolchain_version, "0.0.2")?;
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
            "bootstrapStdlib.status",
            &self.bootstrap_stdlib.status,
            "not-defined",
        )?;
        ensure!(
            self.bootstrap_stdlib.version.is_none(),
            "bootstrapStdlib.version must be null while status is not-defined"
        );
        ensure!(
            self.bootstrap_stdlib.digest.is_none(),
            "bootstrapStdlib.digest must be null while status is not-defined"
        );

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
            "rust-backend",
        )?;
        expect(
            "bootstrapOutput.comparison",
            &self.bootstrap_output.comparison,
            "normalized-c2-c3",
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
    let target = root
        .join("target/bootstrap")
        .join(format!("stage0-{}", manifest.stage0.compiler_version));
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
    use super::{parse_manifest, verify_digest, verify_source, workspace_root};

    const ACCEPTED: &str = include_str!("../../bootstrap/stage0/bootstrap-manifest.json");
    const PUBLIC_NIR: &str =
        include_str!("../../bootstrap/tests/rejected/public-nir-artifact.json");

    #[test]
    fn checked_in_stage0_manifest_defines_the_internal_bootstrap_boundary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = parse_manifest(ACCEPTED)?;

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.stage0.compiler_version, "0.0.1");
        assert_eq!(manifest.bootstrap_stdlib.version, None);
        assert_eq!(manifest.bootstrap_output.kind, "internal-nir");
        assert_eq!(manifest.bootstrap_output.public_extension, None);
        assert!(!manifest.stable_component.includes_nir);

        Ok(())
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
    fn checked_in_stage0_provenance_matches_the_pinned_git_objects(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = parse_manifest(ACCEPTED)?;

        verify_source(&manifest, &workspace_root())?;

        Ok(())
    }
}
