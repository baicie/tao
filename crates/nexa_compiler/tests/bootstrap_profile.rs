//! Futao Bootstrap Profile v1 acceptance and rejection coverage.

use nexa_compiler::{compile, CompilerInput, CompilerOptions, CompilerSource};

fn compile_bootstrap(
    source: &str,
) -> Result<nexa_compiler::CompilerOutput, nexa_compiler::CompileError> {
    compile(&CompilerInput::with_options(
        "main.ft",
        [CompilerSource::new("main.ft", source)],
        CompilerOptions::bootstrap_v1(),
    ))
}

fn diagnostic_codes(output: &nexa_compiler::CompilerOutput) -> Vec<&str> {
    output
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code())
        .collect()
}

#[test]
fn bootstrap_profile_accepts_the_frozen_pure_language_subset(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = compile_bootstrap(
        r#"
type Option<T> =
  | Some(value: T)
  | None();

function map<T, U>(values: T[], transform: (value: T) => U): U[] {
  const mapped: U[] = [];
  for (const value of values) {
    transform(value);
  }
  if (values.length === 0) {
    return mapped;
  }
  return [transform(values[0])];
}

function main(): Unit {
  const result: Option<Int> = Option.Some(42);
  match (result) {
    case Option.Some(value) => value;
    case Option.None() => 0;
  };
}
"#,
    )?;

    assert!(output.is_ok(), "{:?}", output.diagnostics());
    assert!(output.diagnostics().is_empty());
    Ok(())
}

#[test]
fn bootstrap_profile_rejects_mutation_and_unbounded_loop_forms(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = compile_bootstrap(
        r#"
function main(): Unit {
  let value = 0;
  while (value < 1) {
    value = value + 1;
    break;
  }
}
"#,
    )?;

    assert!(!output.is_ok());
    assert_eq!(
        diagnostic_codes(&output),
        ["E6201", "E6202", "E6201", "E6202"]
    );
    Ok(())
}

#[test]
fn bootstrap_profile_rejects_ambient_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = compile_bootstrap(
        r#"
function main(): Unit {
  print("not pure");
}
"#,
    )?;

    assert!(!output.is_ok());
    assert_eq!(diagnostic_codes(&output), ["E6203"]);
    Ok(())
}

#[test]
fn bootstrap_profile_rejects_legacy_source_identity() {
    let error = compile(&CompilerInput::with_options(
        "main.nexa",
        [CompilerSource::new("main.nexa", "function main(): Unit {}")],
        CompilerOptions::bootstrap_v1(),
    ))
    .err();

    assert!(matches!(
        error,
        Some(error) if error.to_string().contains("bootstrap profile requires `.ft`")
    ));
}

#[test]
fn application_profile_preserves_language_1_0_behavior() -> Result<(), Box<dyn std::error::Error>> {
    let output = compile(&CompilerInput::new(
        "main.nexa",
        [CompilerSource::new(
            "main.nexa",
            r#"
function main(): Unit {
  let value = 1;
  value = value + 1;
  print(value);
}
"#,
        )],
    ))?;

    assert!(output.is_ok(), "{:?}", output.diagnostics());
    Ok(())
}
