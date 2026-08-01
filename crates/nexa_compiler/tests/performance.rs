//! Reproducible reference-pipeline performance baselines.

use std::fmt::Write as _;
use std::time::Instant;

use nexa_compiler::{check, run_with_args};
use nexa_span::FileId;

const FUNCTION_COUNT: usize = 256;
const SAMPLE_COUNT: usize = 3;

#[test]
#[ignore = "run explicitly through `cargo xtask perf`"]
fn reference_frontend_baseline_reports_samples_without_a_machine_threshold(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = reference_program(FUNCTION_COUNT)?;
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);

    for _ in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let result = check(FileId::new(0), &source);
        samples.push(started.elapsed().as_micros());

        if !result.is_ok() {
            return Err(std::io::Error::other(format!(
                "reference program failed checking: {:?}",
                result.diagnostics()
            ))
            .into());
        }
    }

    samples.sort_unstable();
    eprintln!(
        "nexa-perf frontend bytes={} functions={} samples_us={samples:?} median_us={}",
        source.len(),
        FUNCTION_COUNT + 1,
        samples[samples.len() / 2]
    );

    Ok(())
}

#[test]
#[ignore = "run explicitly through `cargo xtask perf`"]
fn reference_full_pipeline_baseline_preserves_the_expected_result(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = reference_program(FUNCTION_COUNT)?;
    let arguments = ["10".to_owned()];
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);

    for _ in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let result = run_with_args(FileId::new(0), &source, &arguments);
        samples.push(started.elapsed().as_micros());

        if !result.is_ok() {
            return Err(std::io::Error::other(format!(
                "reference program failed execution: diagnostics={:?}, runtime_error={:?}",
                result.diagnostics(),
                result.runtime_error()
            ))
            .into());
        }
        assert_eq!(result.output(), ["265"]);
    }

    samples.sort_unstable();
    eprintln!(
        "nexa-perf full-pipeline bytes={} functions={} samples_us={samples:?} median_us={}",
        source.len(),
        FUNCTION_COUNT + 1,
        samples[samples.len() / 2]
    );

    Ok(())
}

fn reference_program(function_count: usize) -> Result<String, std::fmt::Error> {
    let mut source = String::new();
    for index in 0..function_count {
        writeln!(
            source,
            "function stage{index}(value: Int): Int {{ return value + {index}; }}"
        )?;
    }
    writeln!(source, "function main(args: String[]): Unit {{")?;
    writeln!(source, "  const input = parseInt(args[0]);")?;
    writeln!(source, "  print(stage{}(input));", function_count - 1)?;
    writeln!(source, "}}")?;
    Ok(source)
}
