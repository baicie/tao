//! Repository automation tasks for Nexa.

mod bootstrap;
mod bootstrap_profile;
mod conformance;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, ensure, Context, Result};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "cargo xtask")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Debug, Subcommand)]
enum Task {
    /// Run formatting, linting, tests, and Rust documentation.
    Check,
    /// Format all Rust code.
    Fmt,
    /// Run locked Clippy with warnings denied.
    Lint,
    /// Run locked workspace tests.
    Test,
    /// Build locked Rust documentation.
    Doc,
    /// Run security and dependency policy checks if tools are installed.
    Security,
    /// Run the versioned language conformance corpus.
    Conformance {
        /// Manifest to run. Relative paths are resolved from the current directory.
        #[arg(long, value_name = "PATH")]
        manifest: Option<PathBuf>,
    },
    /// Replay parser robustness seeds and check the fuzz target with Rust 1.80.
    FuzzSmoke,
    /// Run release-mode reference performance workloads without timing thresholds.
    Perf,
    /// Validate the bounded ownership/storage kernel and its release workload.
    StorageKernel,
    /// Run every required local Language 1.0 release-candidate gate.
    ReleaseCheck,
    /// Validate the pinned Stage 0 and internal bootstrap artifact contract.
    BootstrapContract {
        /// Manifest to validate. Relative paths are resolved from the current directory.
        #[arg(long, value_name = "PATH")]
        manifest: Option<PathBuf>,
        /// Rebuild and smoke-test Stage 0 from the pinned source commit.
        #[arg(long)]
        rebuild_stage0: bool,
    },
    /// Validate accepted and rejected private NIR artifact fixtures.
    NirArtifact,
    /// Validate Futao Bootstrap Profile v1 and stdlib version transitions.
    BootstrapProfile,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Task::Check => check()?,
        Task::Fmt => run("cargo", &["fmt", "--all"])?,
        Task::Lint => lint()?,
        Task::Test => test()?,
        Task::Doc => doc()?,
        Task::Security => {
            run_optional("cargo-deny", &["check"])?;
            run_optional("cargo-audit", &["audit"])?;
            run_optional("cargo-machete", &["--with-metadata"])?;
        }
        Task::Conformance { manifest } => {
            let manifest = manifest.unwrap_or_else(conformance::default_manifest_path);
            conformance::run(&manifest)?;
        }
        Task::FuzzSmoke => fuzz_smoke()?,
        Task::Perf => perf()?,
        Task::StorageKernel => storage_kernel()?,
        Task::ReleaseCheck => release_check()?,
        Task::BootstrapContract {
            manifest,
            rebuild_stage0,
        } => {
            let manifest = manifest.unwrap_or_else(bootstrap::default_manifest_path);
            bootstrap::run(&manifest, rebuild_stage0)?;
        }
        Task::NirArtifact => bootstrap::verify_nir_artifacts()?,
        Task::BootstrapProfile => bootstrap_profile::run()?,
    }

    Ok(())
}

fn check() -> Result<()> {
    run("cargo", &["fmt", "--all", "--", "--check"])?;
    lint()?;
    test()?;
    doc()?;
    bootstrap::run(&bootstrap::default_manifest_path(), false)
}

fn lint() -> Result<()> {
    run(
        "cargo",
        &[
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn test() -> Result<()> {
    run(
        "cargo",
        &[
            "test",
            "--locked",
            "--workspace",
            "--all-targets",
            "--all-features",
        ],
    )
}

fn doc() -> Result<()> {
    run(
        "cargo",
        &[
            "doc",
            "--locked",
            "--workspace",
            "--all-features",
            "--no-deps",
        ],
    )
}

fn fuzz_smoke() -> Result<()> {
    run(
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "nexa_parser",
            "--test",
            "robustness",
        ],
    )?;
    run(
        "cargo",
        &[
            "+1.80.0",
            "check",
            "--locked",
            "--manifest-path",
            "fuzz/Cargo.toml",
        ],
    )
}

fn perf() -> Result<()> {
    run(
        "cargo",
        &[
            "+1.80.0",
            "test",
            "--locked",
            "--release",
            "-p",
            "nexa_compiler",
            "--test",
            "performance",
            "--",
            "--ignored",
            "--nocapture",
        ],
    )
}

fn storage_kernel() -> Result<()> {
    run(
        "cargo",
        &[
            "+1.80.0",
            "test",
            "--locked",
            "-p",
            "nexa_storage",
            "--all-targets",
            "--all-features",
        ],
    )?;
    run(
        "cargo",
        &[
            "+1.80.0",
            "test",
            "--locked",
            "--release",
            "-p",
            "nexa_storage",
            "--test",
            "workload",
            "--",
            "--ignored",
            "--nocapture",
        ],
    )
}

fn release_check() -> Result<()> {
    check()?;
    bootstrap::run(&bootstrap::default_manifest_path(), true)?;
    run(
        "cargo",
        &[
            "+1.80.0",
            "check",
            "--locked",
            "--workspace",
            "--all-targets",
            "--all-features",
        ],
    )?;
    conformance::run(&conformance::default_manifest_path())?;
    fuzz_smoke()?;
    perf()?;
    storage_kernel()?;
    run("cargo", &["build", "--locked", "--release", "-p", "nexac"])?;

    let binary = release_binary();
    run_expect_stdout(
        &binary,
        &["--version"],
        &format!("nexac {}\n", env!("CARGO_PKG_VERSION")),
    )?;
    run_expect_stdout(
        &binary,
        &["check", "examples/practical-core/main.nexa"],
        "ok\n",
    )?;
    run_expect_stdout(
        &binary,
        &["run", "examples/practical-core/main.nexa", "--", "20"],
        "42\n5\n",
    )?;
    run_canonical_dump_smoke(&binary)?;
    run(pnpm_program(), &["--dir", "docs", "build"])
}

fn run_canonical_dump_smoke(binary: &Path) -> Result<()> {
    let output = Command::new(binary)
        .args([
            "dump",
            "crates/nexa_compiler/tests/fixtures/differential/accepted/main.ft",
        ])
        .current_dir(workspace_root())
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("failed to run {} dump", binary.display()))?;
    ensure!(
        output.status.success(),
        "canonical dump smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(output.stderr.is_empty(), "canonical dump wrote stderr");
    validate_canonical_dump(&output.stdout)
}

fn validate_canonical_dump(bytes: &[u8]) -> Result<()> {
    let dump: serde_json::Value =
        serde_json::from_slice(bytes).context("canonical dump was not valid JSON")?;
    ensure!(
        dump.get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            == Some(1),
        "canonical dump schemaVersion must be 1"
    );
    ensure!(
        matches!(
            dump.get("compilationProfile")
                .and_then(serde_json::Value::as_str),
            Some("application" | "futao-bootstrap-v1")
        ),
        "canonical dump compilationProfile must be a known profile"
    );
    let Some(artifacts) = dump.get("artifacts").and_then(serde_json::Value::as_array) else {
        bail!("canonical dump artifacts must be an array");
    };
    ensure!(
        artifacts.len() == 6,
        "canonical dump must contain all six compiler phases"
    );
    let nir_state = artifacts
        .iter()
        .find(|artifact| artifact.get("phase").and_then(serde_json::Value::as_str) == Some("nir"))
        .and_then(|artifact| artifact.pointer("/artifact/state"))
        .and_then(serde_json::Value::as_str);
    ensure!(
        nir_state == Some("produced"),
        "canonical dump NIR phase must be produced"
    );
    Ok(())
}

const fn pnpm_program() -> &'static str {
    if cfg!(windows) {
        "pnpm.cmd"
    } else {
        "pnpm"
    }
}

fn release_binary() -> PathBuf {
    workspace_root()
        .join("target")
        .join("release")
        .join(if cfg!(windows) { "nexac.exe" } else { "nexac" })
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn run(cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd)
        .args(args)
        .current_dir(workspace_root())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to run {cmd} {}", args.join(" ")))?;

    if !status.success() {
        bail!("command failed: {cmd} {}", args.join(" "));
    }

    Ok(())
}

fn run_expect_stdout(cmd: &Path, args: &[&str], expected: &str) -> Result<()> {
    let output = Command::new(cmd)
        .args(args)
        .current_dir(workspace_root())
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("failed to run {} {}", cmd.display(), args.join(" ")))?;
    let stdout = String::from_utf8(output.stdout).context("command stdout was not UTF-8")?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        bail!(
            "command failed: {} {}; stderr: {stderr}",
            cmd.display(),
            args.join(" ")
        );
    }
    if stdout != expected {
        bail!(
            "unexpected stdout from {} {}: expected {expected:?}, found {stdout:?}",
            cmd.display(),
            args.join(" ")
        );
    }
    if !stderr.is_empty() {
        bail!(
            "unexpected stderr from {} {}: {stderr}",
            cmd.display(),
            args.join(" ")
        );
    }

    Ok(())
}

fn run_optional(cmd: &str, args: &[&str]) -> Result<()> {
    if Command::new(cmd)
        .arg("--version")
        .current_dir(workspace_root())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        eprintln!("skip {cmd}: command not installed");
        return Ok(());
    }

    run(cmd, args)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate_canonical_dump;

    fn dump_with_nir_state(state: &str) -> Result<Vec<u8>, serde_json::Error> {
        let phases = ["tokens", "cst", "diagnostics", "hir", "mir", "nir"];
        serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "compilationProfile": "application",
            "artifacts": phases.map(|phase| json!({
                "phase": phase,
                "artifact": {"state": if phase == "nir" { state } else { "produced" }}
            }))
        }))
    }

    #[test]
    fn release_dump_accepts_a_produced_nir_phase() -> Result<(), Box<dyn std::error::Error>> {
        validate_canonical_dump(&dump_with_nir_state("produced")?)?;

        Ok(())
    }

    #[test]
    fn release_dump_rejects_a_deferred_nir_phase() -> Result<(), Box<dyn std::error::Error>> {
        let error = validate_canonical_dump(&dump_with_nir_state("deferred")?).err();

        assert!(matches!(error, Some(error) if error.to_string().contains("must be produced")));
        Ok(())
    }

    #[test]
    fn release_dump_rejects_a_missing_compilation_profile() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut dump: serde_json::Value =
            serde_json::from_slice(&dump_with_nir_state("produced")?)?;
        let _ = dump
            .as_object_mut()
            .ok_or("canonical dump fixture must be an object")?
            .remove("compilationProfile");
        let error = validate_canonical_dump(&serde_json::to_vec(&dump)?).err();

        assert!(matches!(
            error,
            Some(error) if error.to_string().contains("compilationProfile")
        ));
        Ok(())
    }
}
