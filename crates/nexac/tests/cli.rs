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
