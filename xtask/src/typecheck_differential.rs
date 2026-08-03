//! Checked-in Rust/Futao type-checker expression-kernel gate.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use nexa_compiler::{
    FutaoTypeCheckerAdapter, RustTypeCheckerAdapter, TypecheckDifferentialHarness,
    TypecheckDifferentialReport, TypecheckInput, TypecheckNode, TypecheckNodeKind,
    TypecheckObservable, TypecheckType,
};
use serde::Deserialize;

const ROOTS: [(&str, &str); 2] = [
    ("accepted", "bootstrap/compiler/tests/typecheck/accepted"),
    ("rejected", "bootstrap/compiler/tests/typecheck/rejected"),
];

#[derive(Debug)]
struct CorpusCase {
    id: String,
    path: PathBuf,
    input: TypecheckInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    source_length: u32,
    nodes: Vec<FixtureNode>,
}

#[derive(Debug, Deserialize)]
struct FixtureNode {
    kind: String,
    left: Option<usize>,
    right: Option<usize>,
    extra: Option<usize>,
    expected: Option<String>,
    start: u32,
    end: u32,
}

pub(crate) fn run() -> Result<()> {
    let root = workspace_root();
    let cases = discover_cases(&root)?;
    let rust = RustTypeCheckerAdapter;
    let futao = FutaoTypeCheckerAdapter::new().context("failed to build Futao type checker")?;
    let harness = TypecheckDifferentialHarness::new(&rust, &futao);

    for case in &cases {
        let report = harness
            .run_case(&case.id, &case.input)
            .with_context(|| format!("type checker failed for {}", case.id))?;
        if report.outcome() != nexa_compiler::TypecheckDifferentialOutcome::Match {
            let artifacts = retain_mismatch(&root, case, &report)?;
            let observables = report
                .differences()
                .iter()
                .map(|difference| match difference {
                    TypecheckObservable::Types => "types",
                    TypecheckObservable::Diagnostics => "diagnostics",
                })
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "type-checker differential mismatch for {} ({observables}); artifacts: {}",
                case.id,
                artifacts.display()
            );
        }
    }

    println!(
        "Futao type-checker differential matched {} checked-in case(s)",
        cases.len()
    );
    Ok(())
}

fn discover_cases(root: &Path) -> Result<Vec<CorpusCase>> {
    let mut cases = Vec::new();
    for (category, relative_root) in ROOTS {
        let directory = root.join(relative_root);
        let mut paths = fs::read_dir(&directory)
            .with_context(|| format!("failed to read type-checker corpus {}", directory.display()))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()
            .with_context(|| {
                format!(
                    "failed to inspect type-checker corpus {}",
                    directory.display()
                )
            })?;
        paths.sort();
        ensure!(
            !paths.is_empty(),
            "type-checker corpus category is empty: {}",
            directory.display()
        );
        for path in paths {
            ensure!(
                path.is_file(),
                "type-checker corpus entry must be a file: {}",
                path.display()
            );
            ensure!(
                path.extension().and_then(|value| value.to_str()) == Some("json"),
                "type-checker fixture must use .json: {}",
                path.display()
            );
            let fixture: Fixture = serde_json::from_str(&fs::read_to_string(&path)?)
                .with_context(|| format!("invalid type-checker fixture {}", path.display()))?;
            let input = fixture_to_input(fixture)
                .with_context(|| format!("invalid type-checker fixture {}", path.display()))?;
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .context("fixture name must be UTF-8")?;
            cases.push(CorpusCase {
                id: format!("{category}/{stem}"),
                path,
                input,
            });
        }
    }
    cases.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(cases)
}

fn fixture_to_input(fixture: Fixture) -> Result<TypecheckInput> {
    let mut nodes = Vec::with_capacity(fixture.nodes.len());
    for node in fixture.nodes {
        let kind = match node.kind.as_str() {
            "int" => TypecheckNodeKind::Int,
            "bool" => TypecheckNodeKind::Bool,
            "string" => TypecheckNodeKind::String,
            "unit" => TypecheckNodeKind::Unit,
            "neg" => TypecheckNodeKind::Neg,
            "not" => TypecheckNodeKind::Not,
            "add" => TypecheckNodeKind::Add,
            "equal" => TypecheckNodeKind::Equal,
            "and" => TypecheckNodeKind::And,
            "or" => TypecheckNodeKind::Or,
            "if" => TypecheckNodeKind::If,
            "return" => TypecheckNodeKind::Return,
            value => bail!("unknown node kind `{value}`"),
        };
        let value = match kind {
            TypecheckNodeKind::Int
            | TypecheckNodeKind::Bool
            | TypecheckNodeKind::String
            | TypecheckNodeKind::Unit => TypecheckNode::literal(kind, node.start, node.end),
            TypecheckNodeKind::Neg | TypecheckNodeKind::Not => TypecheckNode::unary(
                kind,
                node.left.context("unary node requires left")?,
                node.start,
                node.end,
            ),
            TypecheckNodeKind::Add
            | TypecheckNodeKind::Equal
            | TypecheckNodeKind::And
            | TypecheckNodeKind::Or => TypecheckNode::binary(
                kind,
                node.left.context("binary node requires left")?,
                node.right.context("binary node requires right")?,
                node.start,
                node.end,
            ),
            TypecheckNodeKind::If => TypecheckNode::conditional(
                node.left.context("if node requires condition")?,
                node.right.context("if node requires then branch")?,
                node.extra.context("if node requires else branch")?,
                node.start,
                node.end,
            ),
            TypecheckNodeKind::Return => TypecheckNode::return_check(
                node.left.context("return node requires value")?,
                match node.expected.as_deref() {
                    Some("int") => TypecheckType::Int,
                    Some("bool") => TypecheckType::Bool,
                    Some("string") => TypecheckType::String,
                    Some("unit") => TypecheckType::Unit,
                    Some(value) => bail!("unknown expected type `{value}`"),
                    None => bail!("return node requires expected type"),
                },
                node.start,
                node.end,
            ),
        };
        nodes.push(value);
    }
    Ok(TypecheckInput::new(fixture.source_length, nodes))
}

fn retain_mismatch(
    root: &Path,
    case: &CorpusCase,
    report: &TypecheckDifferentialReport,
) -> Result<PathBuf> {
    let directory = root.join("target/typecheck-differential").join(&case.id);
    fs::create_dir_all(&directory)?;
    fs::copy(&case.path, directory.join("input.json"))?;
    fs::write(
        directory.join("rust-reference.json"),
        report.reference_snapshot().to_json()?,
    )?;
    fs::write(
        directory.join("futao.json"),
        report.candidate_snapshot().to_json()?,
    )?;
    Ok(directory)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[cfg(test)]
mod tests {
    #[test]
    fn checked_in_corpus_has_stable_case_order() -> Result<(), Box<dyn std::error::Error>> {
        let cases = super::discover_cases(&super::workspace_root())?;
        assert_eq!(
            cases
                .iter()
                .map(|case| case.id.as_str())
                .collect::<Vec<_>>(),
            ["accepted/basic", "rejected/basic"]
        );
        Ok(())
    }
}
