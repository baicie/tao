//! Checked-in Rust/Futao parser differential corpus gate.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use nexa_compiler::{
    FutaoParserAdapter, ParserDifferentialHarness, ParserDifferentialReport, ParserObservable,
    RustParserAdapter,
};

const CORPUS_ROOTS: [(&str, &str); 3] = [
    ("accepted", "bootstrap/compiler/tests/parser/accepted"),
    ("rejected", "bootstrap/compiler/tests/parser/rejected"),
    ("fuzz", "fuzz/corpus/parser"),
];

#[derive(Debug)]
struct CorpusCase {
    id: String,
    path: PathBuf,
    source: String,
}

pub(crate) fn run() -> Result<()> {
    let root = workspace_root();
    let cases = discover_cases(&root)?;
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new().context("failed to build the Futao parser adapter")?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);

    for case in &cases {
        let report = harness
            .run_case(&case.id, &case.source)
            .with_context(|| format!("parser adapter failed for {}", case.id))?;
        if !report.passes_gate() {
            let artifacts = retain_mismatch(&root, case, &report)?;
            let observables = report
                .differences()
                .iter()
                .map(|difference| match difference.observable() {
                    ParserObservable::Cst => "cst",
                    ParserObservable::Recovery => "recovery",
                    ParserObservable::Diagnostics => "diagnostics",
                })
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "parser differential mismatch for {} ({observables}); artifacts: {}",
                case.id,
                artifacts.display()
            );
        }
    }

    println!(
        "Futao parser differential matched {} checked-in case(s)",
        cases.len()
    );
    Ok(())
}

fn discover_cases(root: &Path) -> Result<Vec<CorpusCase>> {
    let mut cases = Vec::new();
    let mut ids = HashSet::new();

    for (category, relative_root) in CORPUS_ROOTS {
        let corpus_root = root.join(relative_root);
        let entries = fs::read_dir(&corpus_root)
            .with_context(|| format!("failed to read parser corpus {}", corpus_root.display()))?;
        let category_start = cases.len();

        for entry in entries {
            let entry = entry.with_context(|| {
                format!("failed to inspect parser corpus {}", corpus_root.display())
            })?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to inspect parser case {}", path.display()))?;
            ensure!(
                file_type.is_file(),
                "parser corpus entry must be a file: {}",
                path.display()
            );
            ensure!(
                path.extension().and_then(|value| value.to_str()) == Some("ft"),
                "parser corpus entry must use .ft: {}",
                path.display()
            );
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .context("parser corpus filename must be valid UTF-8")?;
            let id = format!("{category}/{stem}");
            ensure!(ids.insert(id.clone()), "duplicate parser case ID `{id}`");
            let source = fs::read_to_string(&path).with_context(|| {
                format!("parser case must contain valid UTF-8: {}", path.display())
            })?;
            cases.push(CorpusCase { id, path, source });
        }

        ensure!(
            cases.len() > category_start,
            "parser differential {category} corpus is empty: {}",
            corpus_root.display()
        );
    }

    cases.sort_by(|left, right| left.id.cmp(&right.id));
    ensure!(!cases.is_empty(), "parser differential corpus is empty");
    Ok(cases)
}

fn retain_mismatch(
    root: &Path,
    case: &CorpusCase,
    report: &ParserDifferentialReport,
) -> Result<PathBuf> {
    let artifact_root = root.join("target/parser-differential").join(&case.id);
    fs::create_dir_all(&artifact_root).with_context(|| {
        format!(
            "failed to create parser mismatch directory {}",
            artifact_root.display()
        )
    })?;
    fs::write(artifact_root.join("source.ft"), &case.source)
        .context("failed to retain parser mismatch source")?;
    fs::write(
        artifact_root.join("rust-reference.json"),
        report.reference_snapshot().to_json()?,
    )
    .context("failed to retain Rust parser snapshot")?;
    fs::write(
        artifact_root.join("futao.json"),
        report.candidate_snapshot().to_json()?,
    )
    .context("failed to retain Futao parser snapshot")?;
    fs::write(
        artifact_root.join("case-path.txt"),
        case.path.display().to_string(),
    )
    .context("failed to retain parser case path")?;
    Ok(artifact_root)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use nexa_compiler::{
        ParserAdapter, ParserAdapterError, ParserDifferentialHarness, ParserImplementation,
        ParserSnapshot, RustParserAdapter,
    };

    use super::{discover_cases, retain_mismatch, CorpusCase};

    #[test]
    fn checked_in_corpus_has_frozen_cases_in_stable_order() -> Result<(), Box<dyn std::error::Error>>
    {
        let cases = discover_cases(&workspace_root())?;
        let ids = cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            [
                "accepted/closures-arrays",
                "accepted/empty",
                "accepted/expressions-precedence",
                "accepted/modules-declarations",
                "accepted/records-unions",
                "accepted/statements-control-flow",
                "accepted/trivia-utf8",
                "accepted/types-generics",
                "fuzz/empty",
                "fuzz/generic-close-transitions",
                "fuzz/nested-precedence-postfix",
                "fuzz/recovery-boundaries",
                "fuzz/utf8-trivia",
                "rejected/expression-recovery",
                "rejected/generic-close-split",
                "rejected/invalid-top-level",
                "rejected/malformed-lists",
                "rejected/match-arm-recovery",
                "rejected/mismatched-delimiters",
                "rejected/missing-delimiters",
                "rejected/record-union-recovery",
            ]
        );
        Ok(())
    }

    #[test]
    fn corpus_discovery_rejects_empty_categories_and_unknown_entries(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = temporary_root("invalid-corpus")?;
        create_corpus_directories(&root)?;
        write_case(&root, "bootstrap/compiler/tests/parser/accepted/one.ft", "")?;
        write_case(
            &root,
            "bootstrap/compiler/tests/parser/rejected/one.ft",
            "@",
        )?;

        let empty_error = discover_cases(&root).err();
        assert!(matches!(
            empty_error,
            Some(error) if error.to_string().contains("fuzz")
                && error.to_string().contains("empty")
        ));

        write_case(&root, "fuzz/corpus/parser/one.ft", "")?;
        write_case(&root, "fuzz/corpus/parser/README.md", "not a parser seed")?;
        let extension_error = discover_cases(&root).err();
        assert!(matches!(
            extension_error,
            Some(error) if error.to_string().contains("must use .ft")
        ));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn mismatch_retention_writes_source_and_both_canonical_snapshots(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = temporary_root("retention")?;
        let source = "function main(): Unit { @ }";
        let reference = FixedSourceParser {
            implementation: ParserImplementation::RustReference,
            source,
        };
        let candidate = FixedSourceParser {
            implementation: ParserImplementation::Futao,
            source: "function main(): Unit { 1 }",
        };
        let report = ParserDifferentialHarness::new(&reference, &candidate)
            .run_case("rejected/expression-recovery", source)?;
        assert!(!report.passes_gate());
        let case = CorpusCase {
            id: "rejected/expression-recovery".to_owned(),
            path: PathBuf::from("fixture.ft"),
            source: source.to_owned(),
        };

        let artifacts = retain_mismatch(&root, &case, &report)?;

        assert_eq!(fs::read_to_string(artifacts.join("source.ft"))?, source);
        assert_eq!(
            fs::read_to_string(artifacts.join("rust-reference.json"))?,
            report.reference_snapshot().to_json()?
        );
        assert_eq!(
            fs::read_to_string(artifacts.join("futao.json"))?,
            report.candidate_snapshot().to_json()?
        );
        assert_eq!(
            fs::read_to_string(artifacts.join("case-path.txt"))?,
            "fixture.ft"
        );

        fs::remove_dir_all(root)?;
        Ok(())
    }

    struct FixedSourceParser {
        implementation: ParserImplementation,
        source: &'static str,
    }

    impl ParserAdapter for FixedSourceParser {
        fn implementation(&self) -> ParserImplementation {
            self.implementation
        }

        fn parse(&self, _source: &str) -> Result<ParserSnapshot, ParserAdapterError> {
            RustParserAdapter.parse(self.source)
        }
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    fn temporary_root(name: &str) -> Result<PathBuf, std::io::Error> {
        let root = std::env::temp_dir().join(format!(
            "nexa-parser-differential-{}-{name}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn create_corpus_directories(root: &Path) -> Result<(), std::io::Error> {
        fs::create_dir_all(root.join("bootstrap/compiler/tests/parser/accepted"))?;
        fs::create_dir_all(root.join("bootstrap/compiler/tests/parser/rejected"))?;
        fs::create_dir_all(root.join("fuzz/corpus/parser"))
    }

    fn write_case(root: &Path, path: &str, source: &str) -> Result<(), std::io::Error> {
        fs::write(root.join(path), source)
    }
}
