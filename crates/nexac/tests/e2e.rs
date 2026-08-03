//! End-to-end coverage for the installed-style `nexac` CLI and `.ft` sources.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ACCEPTED_CASES: &[AcceptedCase] = &[
    AcceptedCase {
        name: "control flow",
        entry: "accepted/control_flow.ft",
        arguments: &[],
        expected_stdout: "accepted/control_flow.stdout",
    },
    AcceptedCase {
        name: "generics and tagged unions",
        entry: "accepted/generics_and_unions.ft",
        arguments: &[],
        expected_stdout: "accepted/generics_and_unions.stdout",
    },
    AcceptedCase {
        name: "multi-file imports",
        entry: "accepted/modules/main.ft",
        arguments: &[],
        expected_stdout: "accepted/modules/main.stdout",
    },
    AcceptedCase {
        name: "CLI arguments",
        entry: "accepted/arguments.ft",
        arguments: &["Futao", "2.0"],
        expected_stdout: "accepted/arguments.stdout",
    },
];

const REJECTED_CASES: &[RejectedCase] = &[
    RejectedCase {
        name: "type mismatch",
        entry: "rejected/type_mismatch.ft",
        expected_diagnostics: "rejected/type_mismatch.diagnostics",
    },
    RejectedCase {
        name: "non-boolean condition",
        entry: "rejected/non_boolean_condition.ft",
        expected_diagnostics: "rejected/non_boolean_condition.diagnostics",
    },
    RejectedCase {
        name: "undefined name",
        entry: "rejected/undefined_name.ft",
        expected_diagnostics: "rejected/undefined_name.diagnostics",
    },
];

const RUNTIME_CASES: &[RuntimeCase] = &[RuntimeCase {
    name: "dependency runtime failure",
    entry: "runtime/division_by_zero/main.ft",
    expected_stdout: "runtime/division_by_zero/main.stdout",
    expected_stderr: "runtime/division_by_zero/main.stderr",
}];

#[derive(Debug)]
struct AcceptedCase {
    name: &'static str,
    entry: &'static str,
    arguments: &'static [&'static str],
    expected_stdout: &'static str,
}

#[derive(Debug)]
struct RejectedCase {
    name: &'static str,
    entry: &'static str,
    expected_diagnostics: &'static str,
}

#[derive(Debug)]
struct RuntimeCase {
    name: &'static str,
    entry: &'static str,
    expected_stdout: &'static str,
    expected_stderr: &'static str,
}

#[test]
fn accepted_ft_programs_check_and_run_through_the_cli() -> Result<(), Box<dyn std::error::Error>> {
    for case in ACCEPTED_CASES {
        let entry = fixture(case.entry);
        let checked = invoke("check", &entry, &[])?;
        assert_success(&checked, case.name, "check");
        assert_eq!(text(&checked.stdout), "ok\n", "case: {}", case.name);

        let executed = invoke("run", &entry, case.arguments);
        let executed = executed?;
        assert_success(&executed, case.name, "run");
        let expected_stdout = read_fixture(case.expected_stdout)?;
        assert_eq!(
            text(&executed.stdout),
            expected_stdout,
            "case: {}",
            case.name
        );
    }

    Ok(())
}

#[test]
fn rejected_ft_programs_report_expected_diagnostics_through_the_cli(
) -> Result<(), Box<dyn std::error::Error>> {
    for case in REJECTED_CASES {
        let entry = fixture(case.entry);
        let checked = invoke("check", &entry, &[])?;
        assert!(
            !checked.status.success(),
            "case unexpectedly passed: {}",
            case.name
        );
        assert!(checked.stdout.is_empty(), "case: {}", case.name);

        let expected_diagnostics = read_fixture(case.expected_diagnostics)?;
        for diagnostic in expected_diagnostics.lines().filter(|line| !line.is_empty()) {
            assert!(
                text(&checked.stderr).contains(diagnostic),
                "case: {}; missing diagnostic {diagnostic}; stderr: {}",
                case.name,
                text(&checked.stderr)
            );
        }
    }

    Ok(())
}

#[test]
fn runtime_ft_programs_preserve_output_and_render_dependency_locations(
) -> Result<(), Box<dyn std::error::Error>> {
    for case in RUNTIME_CASES {
        let entry = fixture(case.entry);
        let executed = invoke("run", &entry, &[])?;
        assert!(
            !executed.status.success(),
            "case unexpectedly passed: {}",
            case.name
        );
        let expected_stdout = read_fixture(case.expected_stdout)?;
        assert_eq!(
            text(&executed.stdout),
            expected_stdout,
            "case: {}",
            case.name
        );

        let expected_stderr = read_fixture(case.expected_stderr)?;
        for expected in expected_stderr.lines().filter(|line| !line.is_empty()) {
            assert!(
                text(&executed.stderr).contains(expected),
                "case: {}; missing stderr {expected}; stderr: {}",
                case.name,
                text(&executed.stderr)
            );
        }
    }

    Ok(())
}

fn invoke(command: &str, entry: &Path, arguments: &[&str]) -> std::io::Result<Output> {
    let mut process = Command::new(cli_binary());
    process.arg(command).arg(entry);
    if !arguments.is_empty() {
        process.arg("--").args(arguments);
    }
    process.output()
}

fn cli_binary() -> PathBuf {
    std::env::var_os("NEXAC_E2E_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_nexac")))
}

fn assert_success(output: &Output, case: &str, command: &str) {
    assert!(
        output.status.success(),
        "{command} failed for {case}: {}",
        text(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{command} emitted diagnostics for {case}: {}",
        text(&output.stderr)
    );
}

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/e2e")
        .join(path)
}

fn read_fixture(path: &str) -> std::io::Result<String> {
    std::fs::read_to_string(fixture(path))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
