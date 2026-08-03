//! Checked-in Rust/Futao resolver differential corpus gate.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use nexa_compiler::{
    CompilerInput, CompilerSource, FutaoResolverAdapter, ResolverDifferentialHarness,
    ResolverDifferentialReport, ResolverObservable, RustResolverAdapter,
};

const GRAPH_ROOTS: [(&str, &str); 2] = [
    ("accepted", "bootstrap/compiler/tests/resolver/accepted"),
    ("rejected", "bootstrap/compiler/tests/resolver/rejected"),
];
const FUZZ_ROOT: &str = "fuzz/corpus/resolver";

#[derive(Debug)]
struct CorpusCase {
    id: String,
    paths: Vec<PathBuf>,
    input: CompilerInput,
}

pub(crate) fn run() -> Result<()> {
    let root = workspace_root();
    let cases = discover_cases(&root)?;
    let rust = RustResolverAdapter;
    let futao =
        FutaoResolverAdapter::new().context("failed to build the Futao resolver adapter")?;
    let harness = ResolverDifferentialHarness::new(&rust, &futao);

    for case in &cases {
        let report = harness
            .run_case(&case.id, &case.input)
            .with_context(|| format!("resolver adapter failed for {}", case.id))?;
        if !report.passes_gate() {
            let artifacts = retain_mismatch(&root, case, &report)?;
            let observables = report
                .differences()
                .iter()
                .map(|difference| match difference.observable() {
                    ResolverObservable::ModuleGraph => "module-graph",
                    ResolverObservable::Symbols => "symbols",
                    ResolverObservable::Scopes => "scopes",
                    ResolverObservable::Bindings => "bindings",
                    ResolverObservable::Names => "names",
                    ResolverObservable::Diagnostics => "diagnostics",
                })
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "resolver differential mismatch for {} ({observables}); artifacts: {}",
                case.id,
                artifacts.display()
            );
        }
    }

    println!(
        "Futao resolver differential matched {} checked-in case(s)",
        cases.len()
    );
    Ok(())
}

fn discover_cases(root: &Path) -> Result<Vec<CorpusCase>> {
    let mut cases = Vec::new();
    let mut ids = HashSet::new();

    for (category, relative_root) in GRAPH_ROOTS {
        let graph_root = root.join(relative_root);
        let case = read_graph_case(&graph_root, category)?;
        ensure!(
            ids.insert(case.id.clone()),
            "duplicate resolver case ID `{}`",
            case.id
        );
        cases.push(case);
    }

    let fuzz_root = root.join(FUZZ_ROOT);
    let entries = fs::read_dir(&fuzz_root)
        .with_context(|| format!("failed to read resolver corpus {}", fuzz_root.display()))?;
    let mut fuzz_paths = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| {
            format!("failed to inspect resolver corpus {}", fuzz_root.display())
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect resolver case {}", path.display()))?;
        ensure!(
            file_type.is_file(),
            "resolver corpus entry must be a file: {}",
            path.display()
        );
        ensure!(
            path.extension().and_then(|value| value.to_str()) == Some("ft"),
            "resolver corpus entry must use .ft: {}",
            path.display()
        );
        fuzz_paths.push(path);
    }
    fuzz_paths.sort();
    ensure!(
        !fuzz_paths.is_empty(),
        "resolver differential fuzz corpus is empty"
    );
    for path in fuzz_paths {
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .context("resolver corpus filename must be valid UTF-8")?;
        let id = format!("fuzz/{stem}");
        ensure!(ids.insert(id.clone()), "duplicate resolver case ID `{id}`");
        let source = fs::read_to_string(&path).with_context(|| {
            format!("resolver case must contain valid UTF-8: {}", path.display())
        })?;
        cases.push(CorpusCase {
            id,
            paths: vec![path],
            input: CompilerInput::new("main.ft", [CompilerSource::new("main.ft", source)]),
        });
    }

    cases.sort_by(|left, right| left.id.cmp(&right.id));
    ensure!(!cases.is_empty(), "resolver differential corpus is empty");
    Ok(cases)
}

fn read_graph_case(graph_root: &Path, category: &str) -> Result<CorpusCase> {
    let entries = fs::read_dir(graph_root)
        .with_context(|| format!("failed to read resolver corpus {}", graph_root.display()))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| {
            format!("failed to inspect resolver corpus {}", graph_root.display())
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect resolver case {}", path.display()))?;
        ensure!(
            file_type.is_file(),
            "resolver corpus entry must be a file: {}",
            path.display()
        );
        ensure!(
            path.extension().and_then(|value| value.to_str()) == Some("ft"),
            "resolver corpus entry must use .ft: {}",
            path.display()
        );
        paths.push(path);
    }
    paths.sort();
    ensure!(
        !paths.is_empty(),
        "resolver {category} corpus is empty: {}",
        graph_root.display()
    );
    ensure!(
        paths
            .iter()
            .any(|path| path.file_name().and_then(|name| name.to_str()) == Some("main.ft")),
        "resolver {category} corpus must contain main.ft"
    );

    let mut sources = Vec::with_capacity(paths.len());
    for path in &paths {
        let identity = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("resolver source filename must be valid UTF-8")?;
        let source = fs::read_to_string(path).with_context(|| {
            format!("resolver case must contain valid UTF-8: {}", path.display())
        })?;
        sources.push(CompilerSource::new(identity, source));
    }
    Ok(CorpusCase {
        id: format!("{category}/core"),
        paths,
        input: CompilerInput::new("main.ft", sources),
    })
}

fn retain_mismatch(
    root: &Path,
    case: &CorpusCase,
    report: &ResolverDifferentialReport,
) -> Result<PathBuf> {
    let artifact_root = root.join("target/resolver-differential").join(&case.id);
    fs::create_dir_all(artifact_root.join("sources")).with_context(|| {
        format!(
            "failed to create resolver mismatch directory {}",
            artifact_root.display()
        )
    })?;
    for (path, source) in case.paths.iter().zip(case.input.sources()) {
        let identity = source.identity();
        fs::write(
            artifact_root.join("sources").join(identity),
            source.content(),
        )
        .with_context(|| format!("failed to retain resolver source {}", path.display()))?;
    }
    fs::write(
        artifact_root.join("rust-reference.json"),
        report.reference_snapshot().to_json()?,
    )
    .context("failed to retain Rust resolver snapshot")?;
    fs::write(
        artifact_root.join("futao.json"),
        report.candidate_snapshot().to_json()?,
    )
    .context("failed to retain Futao resolver snapshot")?;
    fs::write(
        artifact_root.join("case-paths.txt"),
        case.paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .context("failed to retain resolver case paths")?;
    Ok(artifact_root)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[cfg(test)]
mod tests {
    use super::discover_cases;

    #[test]
    fn checked_in_corpus_has_stable_resolver_case_order() -> Result<(), Box<dyn std::error::Error>>
    {
        let cases = discover_cases(&super::workspace_root())?;
        let ids = cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "accepted/core",
                "fuzz/data-declarations",
                "fuzz/empty",
                "fuzz/nested-functions",
                "fuzz/scopes",
                "rejected/core",
            ]
        );
        Ok(())
    }
}
