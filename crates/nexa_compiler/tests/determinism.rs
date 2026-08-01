//! Determinism and larger-module-graph integration coverage.

use std::fmt::Write as _;
use std::path::Path;

use nexa_compiler::{check_session, run_session, CompilerSession};
use nexa_mir::lower as lower_mir;
use nexa_source::MemorySourceProvider;

const MODULE_COUNT: usize = 32;

#[test]
fn compiler_pipeline_is_structurally_stable_across_provider_insertion_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let forward = chain_session(false, false)?;
    let reverse = chain_session(true, false)?;

    assert_eq!(forward.modules(), reverse.modules());
    assert_eq!(forward.edges(), reverse.edges());

    let forward_checked = check_session(&forward);
    let reverse_checked = check_session(&reverse);
    assert_eq!(forward_checked, reverse_checked);

    let forward_typed = forward_checked
        .typed()
        .ok_or_else(|| std::io::Error::other("forward graph did not type-check"))?;
    let reverse_typed = reverse_checked
        .typed()
        .ok_or_else(|| std::io::Error::other("reverse graph did not type-check"))?;
    let forward_mir = lower_mir(forward_typed)?;
    let reverse_mir = lower_mir(reverse_typed)?;
    assert_eq!(forward_mir, reverse_mir);

    let forward_run = run_session(&forward);
    let reverse_run = run_session(&reverse);
    assert_eq!(forward_run, reverse_run);
    assert_eq!(forward_run.output(), ["42"]);

    Ok(())
}

#[test]
fn cross_file_diagnostic_order_is_stable_across_provider_insertion_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let forward = chain_session(false, true)?;
    let reverse = chain_session(true, true)?;

    let forward_checked = check_session(&forward);
    let reverse_checked = check_session(&reverse);

    assert_eq!(forward_checked, reverse_checked);
    assert_eq!(forward_checked.diagnostics().len(), MODULE_COUNT - 1);
    assert!(forward_checked
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.code().as_str() == "E2001"));

    Ok(())
}

fn chain_session(
    reverse: bool,
    with_errors: bool,
) -> Result<CompilerSession<MemorySourceProvider>, Box<dyn std::error::Error>> {
    let mut sources = chain_sources(with_errors)?;
    if reverse {
        sources.reverse();
    }

    let mut provider = MemorySourceProvider::default();
    for (path, source) in sources {
        let _ = provider.insert(path, source.as_bytes());
    }

    Ok(CompilerSession::build(
        provider,
        Path::new("chain/module0.nexa"),
    )?)
}

fn chain_sources(with_errors: bool) -> Result<Vec<(String, String)>, std::fmt::Error> {
    let mut sources = Vec::with_capacity(MODULE_COUNT);
    for index in 0..MODULE_COUNT {
        let path = format!("chain/module{index}.nexa");
        let mut source = String::new();

        if index + 1 < MODULE_COUNT {
            writeln!(
                source,
                "import {{ value{} }} from \"./module{}.nexa\";",
                index + 1,
                index + 1
            )?;
        }

        if index == 0 {
            writeln!(source, "function main(): Unit {{ print(value1()); }}")?;
        } else if with_errors {
            writeln!(
                source,
                "export function value{index}(): Int {{ return missing{index}; }}"
            )?;
        } else if index + 1 == MODULE_COUNT {
            writeln!(
                source,
                "export function value{index}(): Int {{ return 42; }}"
            )?;
        } else {
            writeln!(
                source,
                "export function value{index}(): Int {{ return value{}(); }}",
                index + 1
            )?;
        }

        sources.push((path, source));
    }
    Ok(sources)
}
