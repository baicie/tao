//! Multi-source compiler-session integration tests.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use nexa_compiler::{CompilerSession, SessionBuildError};
use nexa_diagnostics::LabelStyle;
use nexa_source::{
    MemorySourceProvider, ProvidedSource, SourceKey, SourceLoadError, SourceProvider, SourceRequest,
};

#[derive(Default)]
struct ProviderCounts {
    resolves: usize,
    loads: HashMap<SourceKey, usize>,
}

#[derive(Default)]
struct CountingProvider {
    inner: MemorySourceProvider,
    counts: Rc<RefCell<ProviderCounts>>,
}

impl CountingProvider {
    fn insert(&mut self, path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> SourceKey {
        self.inner.insert(path, bytes)
    }

    fn counts(&self) -> Rc<RefCell<ProviderCounts>> {
        Rc::clone(&self.counts)
    }
}

impl SourceProvider for CountingProvider {
    fn resolve(&mut self, request: SourceRequest<'_>) -> Result<SourceKey, SourceLoadError> {
        self.counts.borrow_mut().resolves += 1;
        self.inner.resolve(request)
    }

    fn load(&mut self, key: &SourceKey) -> Result<ProvidedSource, SourceLoadError> {
        let mut counts = self.counts.borrow_mut();
        let count = counts.loads.entry(key.clone()).or_default();
        *count += 1;
        drop(counts);
        self.inner.load(key)
    }
}

#[test]
fn session_returns_a_structured_error_for_a_missing_entry() {
    let error =
        CompilerSession::build(CountingProvider::default(), Path::new("missing.nexa")).err();

    assert!(matches!(
        error,
        Some(SessionBuildError::EntryLoad(
            SourceLoadError::NotFound { .. }
        ))
    ));
}

#[test]
fn session_returns_a_structured_error_for_non_utf8_entry_bytes() {
    let mut provider = CountingProvider::default();
    let entry = provider.insert("entry.nexa", [0xff]);
    let error = CompilerSession::build(provider, Path::new("entry.nexa")).err();

    assert!(matches!(
        error,
        Some(SessionBuildError::EntryEncoding { key, .. }) if key == entry
    ));
}

#[test]
fn session_accepts_the_relative_path_matrix_and_deduplicates_normalized_keys(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert(
        path(&["root", "dir", "main.nexa"]),
        br#"import { A } from "./a.nexa";
import { B } from "../b.nexa";
import { C } from "../../c.nexa";
import { AAgain } from "./nested/../a.nexa";"#,
    );
    let a = provider.insert(path(&["root", "dir", "a.nexa"]), b"");
    let b = provider.insert(path(&["root", "b.nexa"]), b"");
    let c = provider.insert("c.nexa", b"");
    let counts = provider.counts();

    let session = CompilerSession::build(provider, entry.as_path())?;
    let discovered = session
        .modules()
        .iter()
        .map(|module| module.key().clone())
        .collect::<Vec<_>>();

    assert_eq!(discovered, [entry, a.clone(), b, c]);
    assert_eq!(session.edges().len(), 4);
    assert_eq!(load_count(&counts, &a), 1);
    assert!(session.is_ok(), "diagnostics: {:?}", session.diagnostics());

    Ok(())
}

#[test]
fn session_discovers_a_diamond_in_source_order_and_loads_shared_once(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert(
        path(&["graph", "main.nexa"]),
        br#"import { Left } from "./left.nexa";
import { Right } from "./right.nexa";"#,
    );
    let left = provider.insert(
        path(&["graph", "left.nexa"]),
        br#"import { Shared } from "./shared.nexa";"#,
    );
    let right = provider.insert(
        path(&["graph", "right.nexa"]),
        br#"import { Shared } from "./shared.nexa";"#,
    );
    let shared = provider.insert(path(&["graph", "shared.nexa"]), b"");
    let counts = provider.counts();

    let session = CompilerSession::build(provider, entry.as_path())?;
    let modules = session
        .modules()
        .iter()
        .map(|module| (module.id().index(), module.key().clone()))
        .collect::<Vec<_>>();
    let edges = session
        .edges()
        .iter()
        .map(|edge| (edge.importer().index(), edge.imported().index()))
        .collect::<Vec<_>>();

    assert_eq!(
        modules,
        [(0, entry), (1, left), (2, shared.clone()), (3, right),]
    );
    assert_eq!(edges, [(0, 1), (1, 2), (0, 3), (3, 2)]);
    assert_eq!(load_count(&counts, &shared), 1);

    Ok(())
}

#[test]
fn session_emits_each_failed_import_but_loads_the_failed_key_once(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert(
        path(&["graph", "main.nexa"]),
        br#"import { First } from "./missing.nexa";
import { Second } from "./missing.nexa";"#,
    );
    let missing = SourceKey::new(path(&["graph", "missing.nexa"]));
    let counts = provider.counts();

    let session = CompilerSession::build(provider, entry.as_path())?;

    assert_eq!(diagnostic_count(&session, "E4001"), 2);
    assert_eq!(load_count(&counts, &missing), 1);
    assert_eq!(counts.borrow().resolves, 2);

    Ok(())
}

#[test]
fn session_reports_non_utf8_import_bytes_without_registering_a_module(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert(
        path(&["graph", "main.nexa"]),
        br#"import { Bad } from "./bad.nexa";"#,
    );
    let bad = provider.insert(path(&["graph", "bad.nexa"]), [0xff]);
    let counts = provider.counts();

    let session = CompilerSession::build(provider, entry.as_path())?;

    assert_eq!(session.modules().len(), 1);
    assert_eq!(diagnostic_count(&session, "E4001"), 1);
    assert_eq!(load_count(&counts, &bad), 1);

    Ok(())
}

#[test]
fn session_never_loads_or_parses_unreachable_provider_sources(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert("main.nexa", b"function main(): Unit {}");
    let unreachable = provider.insert("unreachable.nexa", b"@");
    let counts = provider.counts();

    let session = CompilerSession::build(provider, entry.as_path())?;

    assert_eq!(session.modules().len(), 1);
    assert!(session.diagnostics().is_empty());
    assert_eq!(load_count(&counts, &unreachable), 0);

    Ok(())
}

#[test]
fn session_rejects_invalid_paths_before_provider_resolution(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"import { Missing } from "missing.nexa";"#;
    let mut provider = CountingProvider::default();
    let entry = provider.insert("main.nexa", source.as_bytes());
    let counts = provider.counts();

    let session = CompilerSession::build(provider, entry.as_path())?;
    let diagnostic = session
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code().as_str() == "E4001")
        .ok_or_else(|| std::io::Error::other("expected E4001"))?;
    let label = diagnostic
        .labels()
        .first()
        .ok_or_else(|| std::io::Error::other("expected primary label"))?;

    assert_eq!(counts.borrow().resolves, 1);
    assert_eq!(
        &source[label.span().range().start()..label.span().range().end()],
        "\"missing.nexa\""
    );

    Ok(())
}

#[test]
fn session_does_not_resolve_a_malformed_import_string() -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert("main.nexa", br#"import { Missing } from "./bad\q.nexa";"#);
    let counts = provider.counts();

    let session = CompilerSession::build(provider, entry.as_path())?;

    assert!(diagnostic_count(&session, "E1001") >= 1);
    assert_eq!(diagnostic_count(&session, "E4001"), 0);
    assert_eq!(counts.borrow().resolves, 1);

    Ok(())
}

#[test]
fn session_loads_complete_imports_despite_independent_parse_errors_and_sorts_by_file(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry_source = br#"import { A } from "./a.nexa";
import { B } from "./b.nexa";
@"#;
    let entry = provider.insert(path(&["sort", "main.nexa"]), entry_source);
    provider.insert(path(&["sort", "a.nexa"]), b"@");
    provider.insert(path(&["sort", "b.nexa"]), b"@");

    let session = CompilerSession::build(provider, entry.as_path())?;
    let positions = session
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            diagnostic.labels().first().map(|label| {
                (
                    label.span().file().raw(),
                    label.span().range().start(),
                    diagnostic.code().as_str(),
                )
            })
        })
        .collect::<Vec<_>>();

    assert_eq!(
        positions,
        [
            Some((0, entry_source.len() - 1, "E1001")),
            Some((1, 0, "E1001")),
            Some((2, 0, "E1001"))
        ]
    );

    Ok(())
}

#[test]
fn session_emits_one_primary_only_cycle_diagnostic_for_a_self_import(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert(
        path(&["cycle", "main.nexa"]),
        br#"import { SelfValue } from "./main.nexa";"#,
    );

    let session = CompilerSession::build(provider, entry.as_path())?;
    let diagnostic = one_cycle(&session)?;

    assert_eq!(diagnostic.labels().len(), 1);
    assert_eq!(diagnostic.labels()[0].style(), LabelStyle::Primary);
    assert_eq!(diagnostic.labels()[0].span().file().raw(), 0);

    Ok(())
}

#[test]
fn session_emits_the_dfs_tree_witness_for_a_two_module_cycle(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert(
        path(&["cycle", "main.nexa"]),
        br#"import { A } from "./a.nexa";"#,
    );
    provider.insert(
        path(&["cycle", "a.nexa"]),
        br#"import { Main } from "./main.nexa";"#,
    );

    let session = CompilerSession::build(provider, entry.as_path())?;
    let diagnostic = one_cycle(&session)?;
    let labels = diagnostic
        .labels()
        .iter()
        .map(|label| (label.style(), label.span().file().raw()))
        .collect::<Vec<_>>();

    assert_eq!(
        labels,
        [(LabelStyle::Primary, 1), (LabelStyle::Secondary, 0),]
    );

    Ok(())
}

#[test]
fn session_emits_one_complete_witness_for_a_three_module_cycle(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert(
        path(&["cycle", "main.nexa"]),
        br#"import { A } from "./a.nexa";"#,
    );
    provider.insert(
        path(&["cycle", "a.nexa"]),
        br#"import { B } from "./b.nexa";"#,
    );
    provider.insert(
        path(&["cycle", "b.nexa"]),
        br#"import { Main } from "./main.nexa";"#,
    );

    let session = CompilerSession::build(provider, entry.as_path())?;
    let diagnostic = one_cycle(&session)?;
    let files = diagnostic
        .labels()
        .iter()
        .map(|label| label.span().file().raw())
        .collect::<Vec<_>>();

    assert_eq!(files, [2, 0, 1]);

    Ok(())
}

#[test]
fn session_emits_one_diagnostic_for_each_disjoint_reachable_cycle(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = CountingProvider::default();
    let entry = provider.insert(
        path(&["cycles", "main.nexa"]),
        br#"import { A } from "./a.nexa";
import { B } from "./b.nexa";"#,
    );
    provider.insert(
        path(&["cycles", "a.nexa"]),
        br#"import { AAgain } from "./a.nexa";"#,
    );
    provider.insert(
        path(&["cycles", "b.nexa"]),
        br#"import { C } from "./c.nexa";"#,
    );
    provider.insert(
        path(&["cycles", "c.nexa"]),
        br#"import { BAgain } from "./b.nexa";"#,
    );

    let session = CompilerSession::build(provider, entry.as_path())?;

    assert_eq!(diagnostic_count(&session, "E4002"), 2);

    Ok(())
}

fn path(parts: &[&str]) -> PathBuf {
    parts.iter().collect()
}

fn load_count(counts: &Rc<RefCell<ProviderCounts>>, key: &SourceKey) -> usize {
    counts.borrow().loads.get(key).copied().unwrap_or_default()
}

fn diagnostic_count<P>(session: &CompilerSession<P>, code: &str) -> usize {
    session
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == code)
        .count()
}

fn one_cycle<P>(
    session: &CompilerSession<P>,
) -> Result<&nexa_diagnostics::Diagnostic, std::io::Error> {
    let mut cycles = session
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == "E4002");
    let cycle = cycles
        .next()
        .ok_or_else(|| std::io::Error::other("expected E4002"))?;
    if cycles.next().is_some() {
        return Err(std::io::Error::other("expected exactly one E4002"));
    }
    Ok(cycle)
}
