//! CLI integration tests for nexac.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn nexac_check_accepts_a_language_core_program() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("accepted/language_core.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "nexac check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");

    Ok(())
}

#[test]
fn nexac_check_accepts_stateful_control_flow() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("accepted/stateful_control_flow.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "nexac check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");

    Ok(())
}

#[test]
fn nexac_check_rejects_invalid_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("rejected/missing_function_name.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains("E1001"), "stderr: {stderr}");
    assert!(
        stderr.contains("expected function name"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":1:10:"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_check_rejects_invalid_semantics() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("rejected/non_boolean_condition.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains("E3002"), "stderr: {stderr}");
    assert!(
        stderr.contains("if condition must have type `Bool`"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":2:7:"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_check_rejects_an_undefined_name() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/undefined_name.nexa", "E2001")
}

#[test]
fn nexac_check_rejects_a_duplicate_binding() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/duplicate_binding.nexa", "E2002")
}

#[test]
fn nexac_check_rejects_an_incorrect_call_arity() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/incorrect_call_arity.nexa", "E2003")
}

#[test]
fn nexac_check_rejects_a_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/type_mismatch.nexa", "E3001")
}

#[test]
fn nexac_check_rejects_an_invalid_return() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/invalid_return.nexa", "E3003")
}

#[test]
fn nexac_check_rejects_immutable_assignments() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/immutable_assignment.nexa", "E2004", ":2:3:")
}

#[test]
fn nexac_check_rejects_break_outside_a_loop() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/break_outside_loop.nexa", "E3004", ":2:3:")
}

#[test]
fn nexac_check_rejects_continue_outside_a_loop() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/continue_outside_loop.nexa", "E3004", ":2:3:")
}

#[test]
fn nexac_check_rejects_an_assignment_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/assignment_type_mismatch.nexa", "E3001")
}

#[test]
fn nexac_check_rejects_a_non_boolean_while_condition() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/non_boolean_while.nexa", "E3002")
}

#[test]
fn nexac_run_executes_main_and_prints_its_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/language_core.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");

    Ok(())
}

#[test]
fn nexac_run_executes_stateful_control_flow() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/stateful_control_flow.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "12\n");

    Ok(())
}

#[test]
fn nexac_run_reports_runtime_failures_with_a_source_location(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("rejected/division_by_zero.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        stderr.contains("runtime error: division by zero"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":2:9"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_run_keeps_output_emitted_before_a_runtime_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("rejected/output_before_runtime_failure.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
    assert!(
        stderr.contains("runtime error: division by zero"),
        "stderr: {stderr}"
    );

    Ok(())
}

#[test]
fn nexac_run_reports_the_execution_step_limit_with_a_source_location(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("rejected/execution_step_limit.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
    assert!(
        stderr.contains("runtime error: execution step limit of 100000 exceeded"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":3:15"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_parse_prints_the_concrete_syntax_tree() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("parse")
        .arg(fixture("accepted/language_core.nexa"))
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.starts_with("SourceFile@0.."), "stdout: {stdout}");
    assert!(
        stdout.contains("FunctionDeclaration@0.."),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("ConstDeclaration@"), "stdout: {stdout}");
    assert!(stdout.contains("CallExpression@"), "stdout: {stdout}");

    Ok(())
}

#[test]
fn nexac_parse_reports_invalid_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("parse")
        .arg(fixture("rejected/missing_function_name.nexa"))
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stdout.contains("FunctionDeclaration@0.."),
        "stdout: {stdout}"
    );
    assert!(stderr.contains("E1001"), "stderr: {stderr}");
    assert!(
        stderr.contains("expected function name"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":1:10:"), "stderr: {stderr}");

    Ok(())
}

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(path)
}

fn assert_check_rejects(
    fixture_path: &str,
    diagnostic_code: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_with_location(fixture_path, diagnostic_code, None)
}

fn assert_check_rejects_at(
    fixture_path: &str,
    diagnostic_code: &str,
    source_location: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_with_location(fixture_path, diagnostic_code, Some(source_location))
}

fn assert_check_rejects_with_location(
    fixture_path: &str,
    diagnostic_code: &str,
    source_location: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture(fixture_path))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains(diagnostic_code), "stderr: {stderr}");
    if let Some(source_location) = source_location {
        assert!(stderr.contains(source_location), "stderr: {stderr}");
    }

    Ok(())
}
