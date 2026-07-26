//! CLI integration tests for nexac.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn nexac_check_accepts_basic_source() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("accepted/let_statement.nexa"))
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
        .arg(fixture("rejected/missing_name.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains("E1001"), "stderr: {stderr}");
    assert!(stderr.contains("expected binding name"), "stderr: {stderr}");
    assert!(stderr.contains(":1:5:"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_parse_prints_the_concrete_syntax_tree() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("parse")
        .arg(fixture("accepted/let_statement.nexa"))
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        stdout,
        concat!(
            "SourceFile@0..17\n",
            "  LetStatement@0..16\n",
            "    LetKw@0..3 \"let\"\n",
            "    Whitespace@3..4 \" \"\n",
            "    Ident@4..10 \"answer\"\n",
            "    Whitespace@10..11 \" \"\n",
            "    Eq@11..12 \"=\"\n",
            "    Whitespace@12..13 \" \"\n",
            "    Int@13..15 \"42\"\n",
            "    Semicolon@15..16 \";\"\n",
            "  Whitespace@16..17 \"\\n\"\n",
        )
    );

    Ok(())
}

#[test]
fn nexac_parse_reports_invalid_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("parse")
        .arg(fixture("rejected/missing_name.nexa"))
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stdout.contains("LetStatement@0..9"), "stdout: {stdout}");
    assert!(stderr.contains("E1001"), "stderr: {stderr}");
    assert!(stderr.contains("expected binding name"), "stderr: {stderr}");
    assert!(stderr.contains(":1:5:"), "stderr: {stderr}");

    Ok(())
}

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(path)
}
