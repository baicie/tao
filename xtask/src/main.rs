//! Repository automation tasks for Nexa.

mod bootstrap;
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
        Task::ReleaseCheck => release_check()?,
        Task::BootstrapContract {
            manifest,
            rebuild_stage0,
        } => {
            let manifest = manifest.unwrap_or_else(bootstrap::default_manifest_path);
            bootstrap::run(&manifest, rebuild_stage0)?;
        }
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
        .args(["dump", "examples/futao-2-full-stack/baseline-1.0/main.ft"])
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
    let dump: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("canonical dump was not valid JSON")?;
    ensure!(
        dump.get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            == Some(1),
        "canonical dump schemaVersion must be 1"
    );
    ensure!(
        dump.get("artifacts")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|artifacts| artifacts.len() == 6),
        "canonical dump must contain all six compiler phases"
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
