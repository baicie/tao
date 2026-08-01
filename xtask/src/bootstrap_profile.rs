use std::path::Path;

use anyhow::{ensure, Context, Result};
use serde::Deserialize;

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

pub(crate) fn run() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let profile_path = root.join("bootstrap/profile/bootstrap-profile-v1.json");
    let upgrade_path = root.join("bootstrap/tests/accepted/bootstrap-stdlib-upgrade.json");
    let rollback_path = root.join("bootstrap/tests/rejected/bootstrap-stdlib-rollback.json");

    let profile = parse_file(&profile_path, parse_profile)?;
    let upgrade = parse_file(&upgrade_path, parse_transition)?;
    let rollback_text = read(&rollback_path)?;
    ensure!(
        parse_transition(&rollback_text).is_err(),
        "{} must remain a rejected rollback fixture",
        rollback_path.display()
    );

    println!(
        "bootstrap profile {}: language {}, {} -> {} upgrade verified",
        profile.id, profile.language_version, upgrade.installed.version, upgrade.candidate.version
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
        ensure!(false, "{field} must use the sha256 algorithm");
        unreachable!();
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

#[cfg(test)]
mod tests {
    use super::{parse_profile, parse_transition};

    const PROFILE: &str = include_str!("../../bootstrap/profile/bootstrap-profile-v1.json");
    const UPGRADE: &str =
        include_str!("../../bootstrap/tests/accepted/bootstrap-stdlib-upgrade.json");
    const ROLLBACK: &str =
        include_str!("../../bootstrap/tests/rejected/bootstrap-stdlib-rollback.json");

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
}
