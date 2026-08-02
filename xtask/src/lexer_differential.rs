//! Checked-in Rust/Futao lexer differential corpus gate.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use nexa_compiler::{
    FutaoLexerAdapter, LexerDifferentialHarness, LexerDifferentialReport, LexerObservable,
    RustLexerAdapter,
};

const CORPUS_ROOTS: [(&str, &str); 3] = [
    ("accepted", "bootstrap/compiler/tests/lexer/accepted"),
    ("rejected", "bootstrap/compiler/tests/lexer/rejected"),
    ("fuzz", "fuzz/corpus/lexer"),
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
    let rust = RustLexerAdapter;
    let futao = FutaoLexerAdapter::new().context("failed to build the Futao lexer adapter")?;
    let harness = LexerDifferentialHarness::new(&rust, &futao);

    for case in &cases {
        let report = harness
            .run_case(&case.id, &case.source)
            .with_context(|| format!("lexer adapter failed for {}", case.id))?;
        if !report.passes_gate() {
            let artifacts = retain_mismatch(&root, case, &report)?;
            let observables = report
                .differences()
                .iter()
                .map(|difference| match difference.observable() {
                    LexerObservable::Tokens => "tokens",
                    LexerObservable::Diagnostics => "diagnostics",
                })
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "lexer differential mismatch for {} ({observables}); artifacts: {}",
                case.id,
                artifacts.display()
            );
        }
    }

    println!(
        "Futao lexer differential matched {} checked-in case(s)",
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
            .with_context(|| format!("failed to read lexer corpus {}", corpus_root.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| {
                format!("failed to inspect lexer corpus {}", corpus_root.display())
            })?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to inspect lexer case {}", path.display()))?;
            ensure!(
                file_type.is_file(),
                "lexer corpus entry must be a file: {}",
                path.display()
            );
            ensure!(
                path.extension().and_then(|value| value.to_str()) == Some("ft"),
                "lexer corpus entry must use .ft: {}",
                path.display()
            );
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .context("lexer corpus filename must be valid UTF-8")?;
            let id = format!("{category}/{stem}");
            ensure!(ids.insert(id.clone()), "duplicate lexer case ID `{id}`");
            let source = fs::read_to_string(&path).with_context(|| {
                format!("lexer case must contain valid UTF-8: {}", path.display())
            })?;
            cases.push(CorpusCase { id, path, source });
        }
    }

    cases.sort_by(|left, right| left.id.cmp(&right.id));
    ensure!(!cases.is_empty(), "lexer differential corpus is empty");
    Ok(cases)
}

fn retain_mismatch(
    root: &Path,
    case: &CorpusCase,
    report: &LexerDifferentialReport,
) -> Result<PathBuf> {
    let artifact_root = root.join("target/lexer-differential").join(&case.id);
    fs::create_dir_all(&artifact_root).with_context(|| {
        format!(
            "failed to create lexer mismatch directory {}",
            artifact_root.display()
        )
    })?;
    fs::write(artifact_root.join("source.ft"), &case.source)
        .context("failed to retain lexer mismatch source")?;
    fs::write(
        artifact_root.join("rust-reference.json"),
        report.reference_snapshot().to_json()?,
    )
    .context("failed to retain Rust lexer snapshot")?;
    fs::write(
        artifact_root.join("futao.json"),
        report.candidate_snapshot().to_json()?,
    )
    .context("failed to retain Futao lexer snapshot")?;
    fs::write(
        artifact_root.join("case-path.txt"),
        case.path.display().to_string(),
    )
    .context("failed to retain lexer case path")?;
    Ok(artifact_root)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[cfg(test)]
mod tests {
    use super::{discover_cases, workspace_root};

    #[test]
    fn checked_in_corpus_has_all_required_categories_in_stable_order(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cases = discover_cases(&workspace_root())?;
        let ids = cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();

        assert!(ids.iter().any(|id| id.starts_with("accepted/")));
        assert!(ids.iter().any(|id| id.starts_with("rejected/")));
        assert!(ids.iter().any(|id| id.starts_with("fuzz/")));
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        Ok(())
    }
}
