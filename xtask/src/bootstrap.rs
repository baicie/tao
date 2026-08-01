use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapManifest {
    schema_version: u32,
    toolchain_version: String,
    language_version: String,
    stage0: Stage0,
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

pub(crate) fn run(manifest_path: &Path) -> Result<()> {
    let text = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest = parse_manifest(&text)
        .with_context(|| format!("failed to validate {}", manifest_path.display()))?;
    println!(
        "bootstrap contract {}: stage0 {} {} at {}",
        manifest.schema_version,
        manifest.stage0.compiler,
        manifest.stage0.compiler_version,
        manifest.stage0.source.commit
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

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

#[cfg(test)]
mod tests {
    use super::parse_manifest;

    const ACCEPTED: &str = include_str!("../../bootstrap/stage0/bootstrap-manifest.json");
    const PUBLIC_NIR: &str =
        include_str!("../../bootstrap/tests/rejected/public-nir-artifact.json");

    #[test]
    fn checked_in_stage0_manifest_defines_the_internal_bootstrap_boundary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = parse_manifest(ACCEPTED)?;

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.stage0.compiler_version, "0.0.1");
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
}
