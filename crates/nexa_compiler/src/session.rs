//! Multi-source loading, parsing, and module-graph construction.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label, Severity};
use nexa_hir::ModuleId;
use nexa_parser::{parse_source, Parse};
use nexa_source::{
    ProvidedSource, SourceKey, SourceLoadError, SourceMap, SourceMapError, SourceProvider,
    SourceRequest,
};
use nexa_span::{FileId, SourceSpan, TextRange};
use nexa_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use thiserror::Error;

const IMPORT_SOURCE_ERROR: DiagnosticCode = DiagnosticCode::new("E4001");
const MODULE_CYCLE: DiagnosticCode = DiagnosticCode::new("E4002");

/// A source module loaded and parsed once within one compiler session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionModule {
    id: ModuleId,
    key: SourceKey,
    file: FileId,
    parse: Parse,
}

impl SessionModule {
    /// Returns the stable first-discovery module identifier.
    #[must_use]
    pub const fn id(&self) -> ModuleId {
        self.id
    }

    /// Returns the provider-defined canonical source identity.
    #[must_use]
    pub const fn key(&self) -> &SourceKey {
        &self.key
    }

    /// Returns the source-map file identifier for this module.
    #[must_use]
    pub const fn file(&self) -> FileId {
        self.file
    }

    /// Returns this module's one retained parse result.
    #[must_use]
    pub const fn parse(&self) -> &Parse {
        &self.parse
    }
}

/// One successfully resolved source-level import edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEdge {
    importer: ModuleId,
    imported: ModuleId,
    specifier: String,
    path_span: SourceSpan,
}

impl ImportEdge {
    /// Returns the module containing the import declaration.
    #[must_use]
    pub const fn importer(&self) -> ModuleId {
        self.importer
    }

    /// Returns the module selected by the canonical provider key.
    #[must_use]
    pub const fn imported(&self) -> ModuleId {
        self.imported
    }

    /// Returns the decoded source-level relative import spelling.
    #[must_use]
    pub fn specifier(&self) -> &str {
        &self.specifier
    }

    /// Returns the complete quoted path-token span.
    #[must_use]
    pub const fn path_span(&self) -> SourceSpan {
        self.path_span
    }
}

/// A source-less failure that prevents construction of a compiler session.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionBuildError {
    /// The provider could not resolve the command-line entry request.
    #[error("failed to resolve entry source: {0}")]
    EntryResolve(#[source] SourceLoadError),
    /// The provider could not load the resolved command-line entry source.
    #[error("failed to load entry source: {0}")]
    EntryLoad(#[source] SourceLoadError),
    /// The command-line entry bytes were not valid UTF-8.
    #[error("entry source `{display_path}` is not valid UTF-8")]
    EntryEncoding {
        /// The canonical identity of the invalid entry source.
        key: SourceKey,
        /// The provider path suitable for an entry-level error message.
        display_path: PathBuf,
    },
    /// The session could not allocate another stable source identifier.
    #[error(transparent)]
    SourceMap(#[from] SourceMapError),
}

/// A complete reachable source graph owned by one compiler invocation.
pub struct CompilerSession<P> {
    provider: P,
    sources: SourceMap,
    modules: Vec<SessionModule>,
    edges: Vec<ImportEdge>,
    diagnostics: Vec<Diagnostic>,
    source_states: HashMap<SourceKey, SourceState>,
    resolution_cache: HashMap<ImportRequest, Result<SourceKey, SourceLoadError>>,
    visit_states: Vec<VisitState>,
    parent_edges: Vec<Option<usize>>,
    back_edges: Vec<usize>,
}

impl<P> CompilerSession<P> {
    /// Returns modules in deterministic depth-first discovery order.
    #[must_use]
    pub fn modules(&self) -> &[SessionModule] {
        &self.modules
    }

    /// Returns successful import occurrences in deterministic traversal order.
    #[must_use]
    pub fn edges(&self) -> &[ImportEdge] {
        &self.edges
    }

    /// Returns the session-owned source map used by every diagnostic span.
    #[must_use]
    pub const fn sources(&self) -> &SourceMap {
        &self.sources
    }

    /// Returns globally stable-sorted parse and module diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns the retained parse result for a stable module identifier.
    #[must_use]
    pub fn parse(&self, module: ModuleId) -> Option<&Parse> {
        self.modules.get(module.index()).map(SessionModule::parse)
    }

    /// Returns a loaded module by its canonical provider key.
    #[must_use]
    pub fn module_by_key(&self, key: &SourceKey) -> Option<&SessionModule> {
        let SourceState::Loaded(module) = self.source_states.get(key)? else {
            return None;
        };
        self.modules.get(module.index())
    }

    /// Returns true when loading and parsing produced no error diagnostics.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity() == Severity::Error)
    }
}

impl<P: SourceProvider> CompilerSession<P> {
    /// Resolves an entry source and builds its complete reachable module graph.
    ///
    /// Import failures are retained as source-spanned diagnostics. This method
    /// returns an error only when the entry cannot be resolved, loaded, or
    /// decoded before a source span exists, or when a `FileId` cannot be
    /// allocated.
    ///
    /// # Errors
    ///
    /// Returns [`SessionBuildError`] for source-less entry failures or source
    /// identifier exhaustion.
    pub fn build(mut provider: P, entry: &Path) -> Result<Self, SessionBuildError> {
        let entry_key = provider
            .resolve(SourceRequest::Entry(entry))
            .map_err(SessionBuildError::EntryResolve)?;
        let provided = provider
            .load(&entry_key)
            .map_err(SessionBuildError::EntryLoad)?;
        let (display_path, bytes) =
            checked_provided_parts(&entry_key, provided).map_err(SessionBuildError::EntryLoad)?;
        let text = String::from_utf8(bytes).map_err(|_| SessionBuildError::EntryEncoding {
            key: entry_key.clone(),
            display_path: display_path.clone(),
        })?;

        let mut session = Self {
            provider,
            sources: SourceMap::default(),
            modules: Vec::new(),
            edges: Vec::new(),
            diagnostics: Vec::new(),
            source_states: HashMap::new(),
            resolution_cache: HashMap::new(),
            visit_states: Vec::new(),
            parent_edges: Vec::new(),
            back_edges: Vec::new(),
        };
        let entry_module = session.register_module(entry_key, display_path, text)?;
        debug_assert_eq!(entry_module, ModuleId::ENTRY);
        session.visit_module(entry_module)?;
        session.diagnostics.extend(session.cycle_diagnostics());
        session.diagnostics.sort_by_key(super::diagnostic_position);
        Ok(session)
    }

    fn register_module(
        &mut self,
        key: SourceKey,
        display_path: PathBuf,
        text: String,
    ) -> Result<ModuleId, SessionBuildError> {
        let file = self
            .sources
            .add_provided(key.clone(), display_path, text.clone())?;
        let parse = parse_source(file, &text);
        self.diagnostics.extend_from_slice(parse.diagnostics());

        let module = ModuleId::new(self.modules.len());
        self.modules.push(SessionModule {
            id: module,
            key: key.clone(),
            file,
            parse,
        });
        let _ = self.source_states.insert(key, SourceState::Loaded(module));
        self.visit_states.push(VisitState::Unvisited);
        self.parent_edges.push(None);
        Ok(module)
    }

    fn visit_module(&mut self, module: ModuleId) -> Result<(), SessionBuildError> {
        let Some(state) = self.visit_states.get_mut(module.index()) else {
            return Ok(());
        };
        if *state != VisitState::Unvisited {
            return Ok(());
        }
        *state = VisitState::Active;

        let Some(source) = self.modules.get(module.index()) else {
            return Ok(());
        };
        let importer_key = source.key.clone();
        let imports = import_sites(source.file, &source.parse);

        for import in imports {
            if !import_path_is_valid(&import.specifier) {
                self.diagnostics.push(invalid_path_diagnostic(&import));
                continue;
            }

            let target_key = match self.resolve_import(&importer_key, &import.specifier) {
                Ok(key) => key,
                Err(error) => {
                    self.diagnostics
                        .push(import_failure_diagnostic(&import, &error));
                    continue;
                }
            };
            let (target, is_new) = match self.module_for_import(&target_key)? {
                LoadOutcome::Loaded { module, is_new } => (module, is_new),
                LoadOutcome::Failed(error) => {
                    self.diagnostics
                        .push(import_failure_diagnostic(&import, &error));
                    continue;
                }
            };

            let edge_index = self.edges.len();
            self.edges.push(ImportEdge {
                importer: module,
                imported: target,
                specifier: import.specifier,
                path_span: import.path_span,
            });

            if is_new {
                if let Some(parent) = self.parent_edges.get_mut(target.index()) {
                    *parent = Some(edge_index);
                }
                self.visit_module(target)?;
            } else if self
                .visit_states
                .get(target.index())
                .is_some_and(|state| *state == VisitState::Active)
            {
                self.back_edges.push(edge_index);
            }
        }

        if let Some(state) = self.visit_states.get_mut(module.index()) {
            *state = VisitState::Finished;
        }
        Ok(())
    }

    fn resolve_import(
        &mut self,
        importer: &SourceKey,
        specifier: &str,
    ) -> Result<SourceKey, SourceLoadError> {
        let request = ImportRequest {
            importer: importer.clone(),
            specifier: specifier.to_owned(),
        };
        if let Some(cached) = self.resolution_cache.get(&request) {
            return cached.clone();
        }

        let resolved = self.provider.resolve(SourceRequest::Import {
            importer,
            specifier,
        });
        let _ = self.resolution_cache.insert(request, resolved.clone());
        resolved
    }

    fn module_for_import(&mut self, key: &SourceKey) -> Result<LoadOutcome, SessionBuildError> {
        if let Some(state) = self.source_states.get(key) {
            return Ok(match state {
                SourceState::Loaded(module) => LoadOutcome::Loaded {
                    module: *module,
                    is_new: false,
                },
                SourceState::Failed(error) => LoadOutcome::Failed(error.clone()),
            });
        }

        let provided = match self.provider.load(key) {
            Ok(provided) => provided,
            Err(error) => {
                let _ = self
                    .source_states
                    .insert(key.clone(), SourceState::Failed(error.clone()));
                return Ok(LoadOutcome::Failed(error));
            }
        };
        let (display_path, bytes) = match checked_provided_parts(key, provided) {
            Ok(parts) => parts,
            Err(error) => {
                let _ = self
                    .source_states
                    .insert(key.clone(), SourceState::Failed(error.clone()));
                return Ok(LoadOutcome::Failed(error));
            }
        };
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                let error = SourceLoadError::Load {
                    key: key.clone(),
                    message: "source bytes are not valid UTF-8".to_owned(),
                };
                let _ = self
                    .source_states
                    .insert(key.clone(), SourceState::Failed(error.clone()));
                return Ok(LoadOutcome::Failed(error));
            }
        };
        let module = self.register_module(key.clone(), display_path, text)?;
        Ok(LoadOutcome::Loaded {
            module,
            is_new: true,
        })
    }

    fn cycle_diagnostics(&self) -> Vec<Diagnostic> {
        let components = strongly_connected_components(self.modules.len(), &self.edges);
        let mut component_by_module = vec![None; self.modules.len()];
        for (component_index, component) in components.iter().enumerate() {
            for module in component {
                if let Some(slot) = component_by_module.get_mut(module.index()) {
                    *slot = Some(component_index);
                }
            }
        }

        let mut diagnostics = Vec::new();
        for (component_index, component) in components.iter().enumerate() {
            if !component_is_cyclic(component, &self.edges) {
                continue;
            }

            let witness = self.back_edges.iter().copied().find(|edge_index| {
                self.edges.get(*edge_index).is_some_and(|edge| {
                    component_by_module
                        .get(edge.importer.index())
                        .copied()
                        .flatten()
                        == Some(component_index)
                        && component_by_module
                            .get(edge.imported.index())
                            .copied()
                            .flatten()
                            == Some(component_index)
                })
            });
            let Some(witness) = witness else {
                continue;
            };
            let Some(closing_edge) = self.edges.get(witness) else {
                continue;
            };
            let mut diagnostic = Diagnostic::error(MODULE_CYCLE, "cyclic module import")
                .with_label(Label::primary(
                    closing_edge.path_span,
                    "this import closes a module cycle",
                ));
            for edge_index in self.tree_path_edges(closing_edge.imported, closing_edge.importer) {
                if let Some(edge) = self.edges.get(edge_index) {
                    diagnostic = diagnostic.with_label(Label::secondary(
                        edge.path_span,
                        "cycle continues through this import",
                    ));
                }
            }
            diagnostics.push(diagnostic);
        }
        diagnostics
    }

    fn tree_path_edges(&self, ancestor: ModuleId, descendant: ModuleId) -> Vec<usize> {
        let mut reversed = Vec::new();
        let mut current = descendant;

        for _ in 0..self.modules.len() {
            if current == ancestor {
                reversed.reverse();
                return reversed;
            }
            let Some(parent_edge) = self.parent_edges.get(current.index()).copied().flatten()
            else {
                return Vec::new();
            };
            let Some(edge) = self.edges.get(parent_edge) else {
                return Vec::new();
            };
            reversed.push(parent_edge);
            current = edge.importer;
        }
        Vec::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ImportRequest {
    importer: SourceKey,
    specifier: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceState {
    Loaded(ModuleId),
    Failed(SourceLoadError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unvisited,
    Active,
    Finished,
}

enum LoadOutcome {
    Loaded { module: ModuleId, is_new: bool },
    Failed(SourceLoadError),
}

struct ImportSite {
    specifier: String,
    path_span: SourceSpan,
}

fn checked_provided_parts(
    expected: &SourceKey,
    provided: ProvidedSource,
) -> Result<(PathBuf, Vec<u8>), SourceLoadError> {
    let (actual, display_path, bytes) = provided.into_parts();
    if &actual != expected {
        return Err(SourceLoadError::Load {
            key: expected.clone(),
            message: format!("source provider returned key `{actual}` while loading `{expected}`"),
        });
    }
    Ok((display_path, bytes))
}

fn import_sites(file: FileId, parse: &Parse) -> Vec<ImportSite> {
    parse
        .syntax()
        .children()
        .filter(|node| node.kind() == SyntaxKind::ImportDeclaration)
        .filter_map(|node| complete_import_site(file, &node))
        .collect()
}

fn complete_import_site(file: FileId, node: &SyntaxNode) -> Option<ImportSite> {
    const EXPECTED_TOKENS: [SyntaxKind; 6] = [
        SyntaxKind::ImportKw,
        SyntaxKind::LBrace,
        SyntaxKind::RBrace,
        SyntaxKind::FromKw,
        SyntaxKind::String,
        SyntaxKind::Semicolon,
    ];

    let direct_tokens = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect::<Vec<_>>();
    if !direct_tokens
        .iter()
        .map(SyntaxToken::kind)
        .eq(EXPECTED_TOKENS)
    {
        return None;
    }
    let mut children = node.children();
    let list = children
        .next()
        .filter(|child| child.kind() == SyntaxKind::ImportList)?;
    if children.next().is_some() || !import_list_is_complete(&list) {
        return None;
    }

    let path = direct_tokens
        .iter()
        .find(|token| token.kind() == SyntaxKind::String)?;
    Some(ImportSite {
        specifier: decode_string_token(path)?,
        path_span: syntax_token_span(file, path)?,
    })
}

fn import_list_is_complete(list: &SyntaxNode) -> bool {
    let mut expects_name = true;
    let mut name_count = 0;

    for element in list.children_with_tokens() {
        let Some(token) = element.into_token() else {
            return false;
        };
        if token.kind().is_trivia() {
            continue;
        }
        match (expects_name, token.kind()) {
            (true, SyntaxKind::Ident) => {
                expects_name = false;
                name_count += 1;
            }
            (false, SyntaxKind::Comma) => expects_name = true,
            _ => return false,
        }
    }
    name_count > 0 && !expects_name
}

fn decode_string_token(token: &SyntaxToken) -> Option<String> {
    let contents = token.text().strip_prefix('"')?.strip_suffix('"')?;
    let mut value = String::with_capacity(contents.len());
    let mut characters = contents.chars();

    while let Some(character) = characters.next() {
        if character != '\\' {
            value.push(character);
            continue;
        }
        value.push(match characters.next()? {
            '"' => '"',
            '\\' => '\\',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            _ => return None,
        });
    }
    Some(value)
}

fn syntax_token_span(file: FileId, token: &SyntaxToken) -> Option<SourceSpan> {
    let range = token.text_range();
    let start = usize::try_from(u32::from(range.start())).ok()?;
    let end = usize::try_from(u32::from(range.end())).ok()?;
    Some(SourceSpan::new(file, TextRange::new(start, end)))
}

fn import_path_is_valid(path: &str) -> bool {
    if !(path.starts_with("./") || path.starts_with("../")) || path.contains(['\\', '\0', '?', '#'])
    {
        return false;
    }
    let mut components = path.split('/');
    let Some(first) = components.next() else {
        return false;
    };
    if first != "." && first != ".." {
        return false;
    }
    let remaining = components.collect::<Vec<_>>();
    if remaining.is_empty() || remaining.iter().any(|component| component.is_empty()) {
        return false;
    }
    remaining.last().is_some_and(|component| {
        [".ft", ".nexa"].iter().any(|extension| {
            component
                .strip_suffix(extension)
                .is_some_and(|stem| !stem.is_empty())
        })
    })
}

fn invalid_path_diagnostic(import: &ImportSite) -> Diagnostic {
    Diagnostic::error(
        IMPORT_SOURCE_ERROR,
        format!("invalid import path `{}`", import.specifier),
    )
    .with_label(Label::primary(
        import.path_span,
        "expected a relative path ending in `.ft` or `.nexa`",
    ))
}

fn import_failure_diagnostic(import: &ImportSite, _error: &SourceLoadError) -> Diagnostic {
    Diagnostic::error(
        IMPORT_SOURCE_ERROR,
        format!("could not load import `{}`", import.specifier),
    )
    .with_label(Label::primary(
        import.path_span,
        "imported source could not be loaded",
    ))
}

fn component_is_cyclic(component: &[ModuleId], edges: &[ImportEdge]) -> bool {
    component.len() > 1
        || component.first().is_some_and(|module| {
            edges
                .iter()
                .any(|edge| edge.importer == *module && edge.imported == *module)
        })
}

fn strongly_connected_components(module_count: usize, edges: &[ImportEdge]) -> Vec<Vec<ModuleId>> {
    let mut adjacency = vec![Vec::new(); module_count];
    for edge in edges {
        if let Some(neighbors) = adjacency.get_mut(edge.importer.index()) {
            neighbors.push(edge.imported);
        }
    }
    Tarjan::new(adjacency).components()
}

struct Tarjan {
    adjacency: Vec<Vec<ModuleId>>,
    next_index: usize,
    indices: Vec<Option<usize>>,
    low_links: Vec<usize>,
    stack: Vec<ModuleId>,
    on_stack: Vec<bool>,
    components: Vec<Vec<ModuleId>>,
}

impl Tarjan {
    fn new(adjacency: Vec<Vec<ModuleId>>) -> Self {
        let module_count = adjacency.len();
        Self {
            adjacency,
            next_index: 0,
            indices: vec![None; module_count],
            low_links: vec![0; module_count],
            stack: Vec::new(),
            on_stack: vec![false; module_count],
            components: Vec::new(),
        }
    }

    fn components(mut self) -> Vec<Vec<ModuleId>> {
        for index in 0..self.adjacency.len() {
            if self.indices[index].is_none() {
                self.visit(ModuleId::new(index));
            }
        }
        self.components
    }

    fn visit(&mut self, module: ModuleId) {
        let module_index = module.index();
        let discovery_index = self.next_index;
        self.next_index += 1;
        self.indices[module_index] = Some(discovery_index);
        self.low_links[module_index] = discovery_index;
        self.stack.push(module);
        self.on_stack[module_index] = true;

        for neighbor_index in 0..self.adjacency[module_index].len() {
            let neighbor = self.adjacency[module_index][neighbor_index];
            if self.indices[neighbor.index()].is_none() {
                self.visit(neighbor);
                self.low_links[module_index] =
                    self.low_links[module_index].min(self.low_links[neighbor.index()]);
            } else if self.on_stack[neighbor.index()] {
                if let Some(index) = self.indices[neighbor.index()] {
                    self.low_links[module_index] = self.low_links[module_index].min(index);
                }
            }
        }

        if self.low_links[module_index] != discovery_index {
            return;
        }
        let mut component = Vec::new();
        while let Some(member) = self.stack.pop() {
            self.on_stack[member.index()] = false;
            component.push(member);
            if member == module {
                break;
            }
        }
        self.components.push(component);
    }
}

#[cfg(test)]
mod tests {
    use super::import_path_is_valid;

    #[test]
    fn import_path_validation_accepts_the_portable_relative_matrix() {
        for path in [
            "./a.ft",
            "../a.ft",
            "./a.nexa",
            "../a.nexa",
            "../../a.nexa",
            "./dir/../a.nexa",
            ".././a.nexa",
        ] {
            assert!(import_path_is_valid(path), "expected `{path}` to be valid");
        }
    }

    #[test]
    fn import_path_validation_rejects_non_portable_or_incomplete_paths() {
        for path in [
            "a.nexa",
            "a",
            "/a.nexa",
            "C:/a.nexa",
            "https://example.com/a.nexa",
            ".\\a.nexa",
            "./a.FT",
            "./a.NEXA",
            "./a",
            "./",
            "./a.nexa/",
            "./a//b.nexa",
            "./a.nexa?query",
            "./a.nexa#fragment",
            "./a\0.nexa",
        ] {
            assert!(
                !import_path_is_valid(path),
                "expected `{path}` to be invalid"
            );
        }
    }
}
