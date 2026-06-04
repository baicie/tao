//! CLI integration tests for nexac.

use std::fs;
use std::process::Command;

#[test]
fn nexac_check_accepts_basic_source() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("nexa-test-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    let source = dir.join("basic.nexa");
    fs::write(&source, "let answer = 42;")?;

    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .args(["check", source.to_string_lossy().as_ref()])
        .output()?;

    assert!(
        output.status.success(),
        "nexac check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");

    Ok(())
}
