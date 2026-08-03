//! Resolver-only canonical snapshots for the third self-hosted compiler slice.

use std::collections::{HashMap, HashSet};

use nexa_diagnostics::{LabelStyle, Severity};
use nexa_hir::{
    lower_module, resolve_modules, Builtin, DefId, NameResolution, ResolvedImport,
    ResolverScopeKind as HirScopeKind, TypeParameterOwner, Visibility,
};
use nexa_mir::{run_with_args_and_step_limit as run_mir_with_args, MirProgram, RuntimeFailure};
use nexa_source::SourceMap;
use nexa_span::SourceSpan;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::core::{explicit_session, validated_sources};
use crate::{compile, CompileError, CompilerInput, CompilerOptions, CompilerSource};

/// Canonical schema used by resolver-only differential snapshots.
pub const RESOLVER_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

const FUTAO_LEXER_SOURCE: &str = include_str!("../../../bootstrap/compiler/src/lexer.ft");
const FUTAO_PARSER_SOURCE: &str = include_str!("../../../bootstrap/compiler/src/parser.ft");
const FUTAO_RESOLVER_SOURCE: &str = include_str!("../../../bootstrap/compiler/src/resolver.ft");
const FUTAO_SEQUENCE_SOURCE: &str = include_str!("../../../bootstrap/compiler/src/sequence.ft");
const FUTAO_BRIDGE_SOURCE: &str =
    include_str!("../../../bootstrap/compiler/src/resolver_bridge.ft");
const FUTAO_PROFILE_ENTRY: &str =
    include_str!("../../../bootstrap/compiler/src/resolver_profile.ft");
const FUTAO_DRIVER_ENTRY: &str = include_str!("../../../bootstrap/compiler/src/resolver_driver.ft");
// These are bounded tool budgets; the Language 1.0 runtime default is unchanged.
const FUTAO_RESOLVER_STEP_LIMIT: usize = 64_000_000;
const FUTAO_RESOLVER_SELF_GRAPH_STEP_LIMIT: usize = 1_000_000_000;

/// Identifies one implementation participating in resolver differential tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolverImplementation {
    /// The Rust Stage 0 resolver.
    RustReference,
    /// The Futao Bootstrap Profile resolver.
    Futao,
}

impl ResolverImplementation {
    /// Returns the stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustReference => "rust-reference",
            Self::Futao => "futao-bootstrap-v1",
        }
    }
}

/// One canonical source range in module-discovery coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverSpanSnapshot {
    source: u32,
    start: u32,
    end: u32,
}

impl ResolverSpanSnapshot {
    /// Returns the stable source/module ordinal.
    #[must_use]
    pub const fn source(self) -> u32 {
        self.source
    }

    /// Returns the inclusive UTF-8 byte start.
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    /// Returns the exclusive UTF-8 byte end.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }
}

/// One module in deterministic depth-first discovery order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverModuleSnapshot {
    id: u32,
    identity: String,
    entry: bool,
}

impl ResolverModuleSnapshot {
    /// Returns the stable discovery ordinal.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns the canonical logical source identity.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Returns true for the compiler entry module.
    #[must_use]
    pub const fn is_entry(&self) -> bool {
        self.entry
    }
}

/// One resolved import edge in deterministic traversal order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverEdgeSnapshot {
    importer: u32,
    imported: u32,
    specifier: String,
    path_span: ResolverSpanSnapshot,
}

impl ResolverEdgeSnapshot {
    /// Returns the importing module ordinal.
    #[must_use]
    pub const fn importer(&self) -> u32 {
        self.importer
    }

    /// Returns the imported module ordinal.
    #[must_use]
    pub const fn imported(&self) -> u32 {
        self.imported
    }

    /// Returns the decoded relative import spelling.
    #[must_use]
    pub fn specifier(&self) -> &str {
        &self.specifier
    }

    /// Returns the exact path-literal range.
    #[must_use]
    pub const fn path_span(&self) -> ResolverSpanSnapshot {
        self.path_span
    }
}

/// Kind-safe category of a top-level symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolverSymbolKind {
    /// A source function.
    Function,
    /// A nominal record.
    Record,
    /// A nominal tagged union.
    Union,
}

/// Canonical declaration visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolverVisibility {
    /// Visible only inside the declaring module.
    Private,
    /// Available to explicit named imports.
    Exported,
}

/// One stable top-level definition identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverSymbolSnapshot {
    kind: ResolverSymbolKind,
    module: u32,
    index: u32,
    visibility: ResolverVisibility,
    name_span: ResolverSpanSnapshot,
}

impl ResolverSymbolSnapshot {
    /// Returns the declaration category.
    #[must_use]
    pub const fn kind(&self) -> ResolverSymbolKind {
        self.kind
    }

    /// Returns the defining module ordinal.
    #[must_use]
    pub const fn module(&self) -> u32 {
        self.module
    }

    /// Returns the declaration-order index within its kind and module.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Returns the import visibility.
    #[must_use]
    pub const fn visibility(&self) -> ResolverVisibility {
        self.visibility
    }

    /// Returns the exact declaration-name range.
    #[must_use]
    pub const fn name_span(&self) -> ResolverSpanSnapshot {
        self.name_span
    }
}

/// Canonical lexical scope category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolverScopeKind {
    /// A source function body.
    Function,
    /// A nested statement block.
    Block,
    /// A `for...of` body.
    For,
    /// One match arm.
    MatchArm,
    /// One arrow function.
    Closure,
}

/// One stable lexical scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverScopeSnapshot {
    module: u32,
    function: u32,
    index: u32,
    parent: Option<u32>,
    kind: ResolverScopeKind,
    span: ResolverSpanSnapshot,
}

impl ResolverScopeSnapshot {
    /// Returns the owning module ordinal.
    #[must_use]
    pub const fn module(&self) -> u32 {
        self.module
    }

    /// Returns the owning function index.
    #[must_use]
    pub const fn function(&self) -> u32 {
        self.function
    }

    /// Returns the preorder scope index within the function.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Returns the immediately enclosing scope index.
    #[must_use]
    pub const fn parent(&self) -> Option<u32> {
        self.parent
    }

    /// Returns the construct that introduced this scope.
    #[must_use]
    pub const fn kind(&self) -> ResolverScopeKind {
        self.kind
    }

    /// Returns the complete source range governed by this scope.
    #[must_use]
    pub const fn span(&self) -> ResolverSpanSnapshot {
        self.span
    }
}

/// One accepted function-local binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverBindingSnapshot {
    module: u32,
    function: u32,
    local: u32,
    scope: u32,
    name_span: ResolverSpanSnapshot,
    mutable: bool,
}

impl ResolverBindingSnapshot {
    /// Returns the owning module ordinal.
    #[must_use]
    pub const fn module(&self) -> u32 {
        self.module
    }

    /// Returns the owning function index.
    #[must_use]
    pub const fn function(&self) -> u32 {
        self.function
    }

    /// Returns the stable local slot index.
    #[must_use]
    pub const fn local(&self) -> u32 {
        self.local
    }

    /// Returns the declaration scope index.
    #[must_use]
    pub const fn scope(&self) -> u32 {
        self.scope
    }

    /// Returns the exact declaration-name range.
    #[must_use]
    pub const fn name_span(&self) -> ResolverSpanSnapshot {
        self.name_span
    }

    /// Returns whether assignment is allowed after initialization.
    #[must_use]
    pub const fn is_mutable(&self) -> bool {
        self.mutable
    }
}

/// Canonical semantic target assigned to a source name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResolverTargetSnapshot {
    /// A function-local slot.
    Local {
        /// Owning module ordinal.
        module: u32,
        /// Owning function index.
        function: u32,
        /// Local slot index.
        local: u32,
    },
    /// A source function.
    Function {
        /// Defining module ordinal.
        module: u32,
        /// Function declaration index.
        index: u32,
    },
    /// A nominal record.
    Record {
        /// Defining module ordinal.
        module: u32,
        /// Record declaration index.
        index: u32,
    },
    /// A nominal record field.
    Field {
        /// Defining module ordinal.
        module: u32,
        /// Owning record index.
        record: u32,
        /// Field declaration index.
        index: u32,
    },
    /// A nominal tagged union.
    Union {
        /// Defining module ordinal.
        module: u32,
        /// Union declaration index.
        index: u32,
    },
    /// A tagged-union variant.
    Variant {
        /// Defining module ordinal.
        module: u32,
        /// Owning union index.
        union: u32,
        /// Variant declaration index.
        index: u32,
    },
    /// A named union payload.
    Payload {
        /// Defining module ordinal.
        module: u32,
        /// Owning union index.
        union: u32,
        /// Owning variant index.
        variant: u32,
        /// Payload declaration index.
        index: u32,
    },
    /// A generic type parameter.
    TypeParameter {
        /// Kind of declaration owning this parameter.
        owner_kind: ResolverSymbolKind,
        /// Defining module ordinal.
        module: u32,
        /// Owning declaration index.
        owner: u32,
        /// Parameter declaration index.
        index: u32,
    },
    /// A compiler-provided operation.
    Builtin {
        /// Stable builtin operation name.
        name: String,
    },
}

/// One resolved source-name occurrence or declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverNameSnapshot {
    span: ResolverSpanSnapshot,
    target: ResolverTargetSnapshot,
}

impl ResolverNameSnapshot {
    /// Returns the exact source-name range.
    #[must_use]
    pub const fn span(&self) -> ResolverSpanSnapshot {
        self.span
    }

    /// Returns the stable semantic target.
    #[must_use]
    pub const fn target(&self) -> &ResolverTargetSnapshot {
        &self.target
    }
}

/// Canonical resolver diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolverSeverity {
    /// An error diagnostic.
    Error,
    /// A warning diagnostic.
    Warning,
}

/// Canonical resolver diagnostic label role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolverLabelStyle {
    /// The primary source label.
    Primary,
    /// A supporting source label.
    Secondary,
}

/// One canonical resolver diagnostic label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverLabelSnapshot {
    style: ResolverLabelStyle,
    span: ResolverSpanSnapshot,
}

impl ResolverLabelSnapshot {
    /// Returns the canonical label role.
    #[must_use]
    pub const fn style(&self) -> ResolverLabelStyle {
        self.style
    }

    /// Returns the exact labeled range.
    #[must_use]
    pub const fn span(&self) -> ResolverSpanSnapshot {
        self.span
    }
}

/// One canonical resolver-only diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverDiagnosticSnapshot {
    code: String,
    severity: ResolverSeverity,
    labels: Vec<ResolverLabelSnapshot>,
}

impl ResolverDiagnosticSnapshot {
    /// Returns the stable diagnostic code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the canonical severity.
    #[must_use]
    pub const fn severity(&self) -> ResolverSeverity {
        self.severity
    }

    /// Returns labels in primary-then-witness order.
    #[must_use]
    pub fn labels(&self) -> &[ResolverLabelSnapshot] {
        &self.labels
    }
}

/// Complete canonical output of one resolver implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverSnapshot {
    schema_version: u32,
    modules: Vec<ResolverModuleSnapshot>,
    edges: Vec<ResolverEdgeSnapshot>,
    symbols: Vec<ResolverSymbolSnapshot>,
    scopes: Vec<ResolverScopeSnapshot>,
    bindings: Vec<ResolverBindingSnapshot>,
    names: Vec<ResolverNameSnapshot>,
    diagnostics: Vec<ResolverDiagnosticSnapshot>,
}

impl ResolverSnapshot {
    /// Returns the resolver snapshot schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns modules in deterministic discovery order.
    #[must_use]
    pub fn modules(&self) -> &[ResolverModuleSnapshot] {
        &self.modules
    }

    /// Returns resolved import edges in traversal order.
    #[must_use]
    pub fn edges(&self) -> &[ResolverEdgeSnapshot] {
        &self.edges
    }

    /// Returns top-level symbols in source order.
    #[must_use]
    pub fn symbols(&self) -> &[ResolverSymbolSnapshot] {
        &self.symbols
    }

    /// Returns lexical scopes in function/preorder order.
    #[must_use]
    pub fn scopes(&self) -> &[ResolverScopeSnapshot] {
        &self.scopes
    }

    /// Returns accepted local bindings in declaration order.
    #[must_use]
    pub fn bindings(&self) -> &[ResolverBindingSnapshot] {
        &self.bindings
    }

    /// Returns resolved names in stable source order.
    #[must_use]
    pub fn names(&self) -> &[ResolverNameSnapshot] {
        &self.names
    }

    /// Returns resolver-only diagnostics in stable source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[ResolverDiagnosticSnapshot] {
        &self.diagnostics
    }

    /// Serializes this snapshot as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the in-memory DTO cannot be serialized.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    fn validate(&self, source_lengths: &[usize]) -> Result<(), ResolverAdapterError> {
        if self.schema_version != RESOLVER_SNAPSHOT_SCHEMA_VERSION {
            return Err(ResolverAdapterError::InvalidSnapshot(
                "unsupported schemaVersion".to_owned(),
            ));
        }
        if self.modules.is_empty()
            || self.modules.iter().enumerate().any(|(index, module)| {
                usize::try_from(module.id).ok() != Some(index) || module.entry != (index == 0)
            })
        {
            return Err(ResolverAdapterError::InvalidSnapshot(
                "modules must be non-empty, contiguous, and entry-first".to_owned(),
            ));
        }
        let identities = self
            .modules
            .iter()
            .map(|module| module.identity.as_str())
            .collect::<HashSet<_>>();
        if identities.len() != self.modules.len() || source_lengths.len() != self.modules.len() {
            return Err(ResolverAdapterError::InvalidSnapshot(
                "module identities and source lengths must be unique and complete".to_owned(),
            ));
        }
        for span in self.all_spans() {
            validate_span(span, source_lengths)?;
        }
        let module_count = self.modules.len();
        for edge in &self.edges {
            if !is_module_id(edge.importer, module_count)
                || !is_module_id(edge.imported, module_count)
                || edge.path_span.source != edge.importer
                || edge.specifier.is_empty()
            {
                return Err(ResolverAdapterError::InvalidSnapshot(
                    "import edge has an invalid endpoint, path span, or specifier".to_owned(),
                ));
            }
        }

        let mut symbols = HashSet::new();
        let mut next_symbol = HashMap::new();
        for symbol in &self.symbols {
            let group = (symbol.module, symbol.kind);
            let expected = next_symbol.entry(group).or_insert(0);
            if !is_module_id(symbol.module, module_count)
                || symbol.name_span.source != symbol.module
                || symbol.index != *expected
                || !symbols.insert((symbol.kind, symbol.module, symbol.index))
            {
                return Err(ResolverAdapterError::InvalidSnapshot(
                    "symbols must reference their module and use contiguous kind-local ids"
                        .to_owned(),
                ));
            }
            *expected += 1;
        }
        if !spans_are_strictly_ordered(self.symbols.iter().map(|symbol| symbol.name_span)) {
            return Err(ResolverAdapterError::InvalidSnapshot(
                "symbols are outside canonical source order".to_owned(),
            ));
        }

        let mut scopes = HashSet::new();
        let mut next_scope = HashMap::new();
        let mut previous_scope_owner = None;
        for scope in &self.scopes {
            let owner = (scope.module, scope.function);
            let expected = next_scope.entry(owner).or_insert(0);
            let parent_is_valid = if scope.index == 0 {
                scope.parent.is_none() && scope.kind == ResolverScopeKind::Function
            } else {
                scope.parent.is_some_and(|parent| parent < scope.index)
            };
            if !symbols.contains(&(ResolverSymbolKind::Function, scope.module, scope.function))
                || scope.span.source != scope.module
                || scope.index != *expected
                || previous_scope_owner.is_some_and(|previous| previous > owner)
                || !parent_is_valid
                || !scopes.insert((scope.module, scope.function, scope.index))
            {
                return Err(ResolverAdapterError::InvalidSnapshot(
                    "scopes must use canonical owners, contiguous ids, and earlier parents"
                        .to_owned(),
                ));
            }
            previous_scope_owner = Some(owner);
            *expected += 1;
        }

        let mut bindings = HashSet::new();
        let mut next_local = HashMap::new();
        let mut previous_binding_owner = None;
        for binding in &self.bindings {
            let owner = (binding.module, binding.function);
            let expected = next_local.entry(owner).or_insert(0);
            if !symbols.contains(&(
                ResolverSymbolKind::Function,
                binding.module,
                binding.function,
            )) || binding.name_span.source != binding.module
                || binding.local != *expected
                || previous_binding_owner.is_some_and(|previous| previous > owner)
                || !scopes.contains(&(binding.module, binding.function, binding.scope))
                || !bindings.insert((binding.module, binding.function, binding.local))
            {
                return Err(ResolverAdapterError::InvalidSnapshot(
                    "bindings must use canonical owners, scopes, and contiguous local ids"
                        .to_owned(),
                ));
            }
            previous_binding_owner = Some(owner);
            *expected += 1;
        }

        if !spans_are_strictly_ordered(self.names.iter().map(|name| name.span)) {
            return Err(ResolverAdapterError::InvalidSnapshot(
                "resolved names are outside canonical source order".to_owned(),
            ));
        }
        for name in &self.names {
            if !target_is_valid(&name.target, name.span, &symbols, &bindings) {
                return Err(ResolverAdapterError::InvalidSnapshot(
                    "resolved name targets an unknown semantic identity".to_owned(),
                ));
            }
        }
        if !child_identity_indices_are_contiguous(&self.names, &symbols) {
            return Err(ResolverAdapterError::InvalidSnapshot(
                "child semantic identities must use contiguous owner-local ids".to_owned(),
            ));
        }

        for diagnostic in &self.diagnostics {
            if !matches!(
                diagnostic.code.as_str(),
                "E2001" | "E2002" | "E4002" | "E4003" | "E4004" | "E4005"
            ) || diagnostic.labels.is_empty()
                || diagnostic.labels[0].style != ResolverLabelStyle::Primary
                || diagnostic.labels[1..]
                    .iter()
                    .any(|label| label.style != ResolverLabelStyle::Secondary)
            {
                return Err(ResolverAdapterError::InvalidSnapshot(
                    "resolver diagnostic is outside the frozen code/label contract".to_owned(),
                ));
            }
        }
        if !diagnostics_are_strictly_ordered(&self.diagnostics) {
            return Err(ResolverAdapterError::InvalidSnapshot(
                "resolver diagnostics are outside canonical source order".to_owned(),
            ));
        }
        Ok(())
    }

    fn all_spans(&self) -> impl Iterator<Item = ResolverSpanSnapshot> + '_ {
        self.edges
            .iter()
            .map(|edge| edge.path_span)
            .chain(self.symbols.iter().map(|symbol| symbol.name_span))
            .chain(self.scopes.iter().map(|scope| scope.span))
            .chain(self.bindings.iter().map(|binding| binding.name_span))
            .chain(self.names.iter().map(|name| name.span))
            .chain(
                self.diagnostics
                    .iter()
                    .flat_map(|diagnostic| diagnostic.labels.iter().map(|label| label.span)),
            )
    }
}

fn is_module_id(id: u32, module_count: usize) -> bool {
    usize::try_from(id).is_ok_and(|index| index < module_count)
}

fn spans_are_strictly_ordered(spans: impl Iterator<Item = ResolverSpanSnapshot>) -> bool {
    let mut previous = None;
    for span in spans {
        let current = (span.source, span.start, span.end);
        if previous.is_some_and(|previous| previous >= current) {
            return false;
        }
        previous = Some(current);
    }
    true
}

fn diagnostics_are_strictly_ordered(diagnostics: &[ResolverDiagnosticSnapshot]) -> bool {
    diagnostics.windows(2).all(|window| {
        let left = &window[0];
        let right = &window[1];
        let left_span = left.labels[0].span;
        let right_span = right.labels[0].span;
        (
            left_span.source,
            left_span.start,
            left_span.end,
            left.code.as_str(),
        ) < (
            right_span.source,
            right_span.start,
            right_span.end,
            right.code.as_str(),
        )
    })
}

fn target_is_valid(
    target: &ResolverTargetSnapshot,
    span: ResolverSpanSnapshot,
    symbols: &HashSet<(ResolverSymbolKind, u32, u32)>,
    bindings: &HashSet<(u32, u32, u32)>,
) -> bool {
    match target {
        ResolverTargetSnapshot::Local {
            module,
            function,
            local,
        } => span.source == *module && bindings.contains(&(*module, *function, *local)),
        ResolverTargetSnapshot::Function { module, index } => {
            symbols.contains(&(ResolverSymbolKind::Function, *module, *index))
        }
        ResolverTargetSnapshot::Record { module, index } => {
            symbols.contains(&(ResolverSymbolKind::Record, *module, *index))
        }
        ResolverTargetSnapshot::Field { module, record, .. } => {
            symbols.contains(&(ResolverSymbolKind::Record, *module, *record))
        }
        ResolverTargetSnapshot::Union { module, index } => {
            symbols.contains(&(ResolverSymbolKind::Union, *module, *index))
        }
        ResolverTargetSnapshot::Variant { module, union, .. }
        | ResolverTargetSnapshot::Payload { module, union, .. } => {
            symbols.contains(&(ResolverSymbolKind::Union, *module, *union))
        }
        ResolverTargetSnapshot::TypeParameter {
            owner_kind,
            module,
            owner,
            ..
        } => span.source == *module && symbols.contains(&(*owner_kind, *module, *owner)),
        ResolverTargetSnapshot::Builtin { name } => matches!(
            name.as_str(),
            "print"
                | "array-length"
                | "string-length"
                | "array-append"
                | "array-concat"
                | "to-string"
                | "parse-int"
        ),
    }
}

fn child_identity_indices_are_contiguous(
    names: &[ResolverNameSnapshot],
    symbols: &HashSet<(ResolverSymbolKind, u32, u32)>,
) -> bool {
    let mut fields = HashMap::<(u32, u32), HashSet<u32>>::new();
    let mut variants = HashMap::<(u32, u32), HashSet<u32>>::new();
    let mut variant_ids = HashSet::new();
    let mut payloads = HashMap::<(u32, u32, u32), HashSet<u32>>::new();
    let mut type_parameters = HashMap::<(ResolverSymbolKind, u32, u32), HashSet<u32>>::new();

    for name in names {
        match &name.target {
            ResolverTargetSnapshot::Field {
                module,
                record,
                index,
            } => {
                if !symbols.contains(&(ResolverSymbolKind::Record, *module, *record)) {
                    return false;
                }
                fields.entry((*module, *record)).or_default().insert(*index);
            }
            ResolverTargetSnapshot::Variant {
                module,
                union,
                index,
            } => {
                if !symbols.contains(&(ResolverSymbolKind::Union, *module, *union)) {
                    return false;
                }
                variants
                    .entry((*module, *union))
                    .or_default()
                    .insert(*index);
                variant_ids.insert((*module, *union, *index));
            }
            ResolverTargetSnapshot::Payload {
                module,
                union,
                variant,
                index,
            } => {
                payloads
                    .entry((*module, *union, *variant))
                    .or_default()
                    .insert(*index);
            }
            ResolverTargetSnapshot::TypeParameter {
                owner_kind,
                module,
                owner,
                index,
            } => {
                if !symbols.contains(&(*owner_kind, *module, *owner)) {
                    return false;
                }
                type_parameters
                    .entry((*owner_kind, *module, *owner))
                    .or_default()
                    .insert(*index);
            }
            _ => {}
        }
    }

    if payloads
        .keys()
        .any(|identity| !variant_ids.contains(identity))
    {
        return false;
    }

    fn are_contiguous<K>(groups: &HashMap<K, HashSet<u32>>) -> bool
    where
        K: Eq + std::hash::Hash,
    {
        groups.values().all(|indices| {
            let mut sorted = indices.iter().copied().collect::<Vec<_>>();
            sorted.sort_unstable();
            sorted
                .iter()
                .enumerate()
                .all(|(expected, actual)| *actual == expected as u32)
        })
    }

    are_contiguous(&fields)
        && are_contiguous(&variants)
        && are_contiguous(&payloads)
        && are_contiguous(&type_parameters)
}

/// Failure while building or executing a resolver adapter.
#[derive(Debug, Error)]
pub enum ResolverAdapterError {
    /// Explicit compiler input was invalid or session construction failed.
    #[error(transparent)]
    Compile(#[from] CompileError),
    /// A parser or lowering diagnostic prevents resolver execution.
    #[error("resolver input is not syntax-valid and graph-complete: {0}")]
    InvalidInput(String),
    /// A syntax-valid CST could not be lowered to HIR.
    #[error(transparent)]
    Lowering(#[from] nexa_hir::LoweringError),
    /// Futao adapter source did not pass its required compilation profile.
    #[error("Futao resolver source was rejected: {0}")]
    FutaoSource(String),
    /// Futao MIR execution failed.
    #[error("Futao resolver execution failed: {failure} at {location}")]
    Runtime {
        /// Complete interpreter failure, including its structured source span.
        #[source]
        failure: RuntimeFailure,
        /// Human-readable source path, line, column, file ID, and byte range.
        location: String,
    },
    /// The private Host bridge emitted an invalid protocol.
    #[error("invalid Futao resolver protocol: {0}")]
    Protocol(String),
    /// A stable identity or index exceeded schema 1.
    #[error("resolver input exceeds schema 1 integer limits")]
    InputTooLarge,
    /// One adapter returned structurally invalid output.
    #[error("invalid resolver snapshot: {0}")]
    InvalidSnapshot(String),
    /// Canonical snapshot serialization failed.
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

/// Produces one canonical resolver snapshot from an explicit source graph.
pub trait ResolverAdapter {
    /// Returns the implementation represented by this adapter.
    fn implementation(&self) -> ResolverImplementation;

    /// Resolves a complete parse-valid explicit source graph.
    ///
    /// # Errors
    ///
    /// Returns an input, lowering, protocol, execution, or validation error.
    fn resolve(&self, input: &CompilerInput) -> Result<ResolverSnapshot, ResolverAdapterError>;
}

/// Snapshot area that differs between two resolver implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolverObservable {
    /// Module identities, entry selection, or import edges differ.
    ModuleGraph,
    /// Top-level definition identity or visibility differs.
    Symbols,
    /// Lexical scope identity, kind, span, or parent differs.
    Scopes,
    /// Accepted local identity, scope, span, or mutability differs.
    Bindings,
    /// A source-name occurrence or semantic target differs.
    Names,
    /// Resolver-only diagnostic structure or ordering differs.
    Diagnostics,
}

/// One unsuppressed resolver differential mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverDifference {
    observable: ResolverObservable,
    reference_digest: String,
    candidate_digest: String,
}

impl ResolverDifference {
    /// Returns the differing observable category.
    #[must_use]
    pub const fn observable(&self) -> ResolverObservable {
        self.observable
    }

    /// Returns the complete reference snapshot digest.
    #[must_use]
    pub fn reference_digest(&self) -> &str {
        &self.reference_digest
    }

    /// Returns the complete candidate snapshot digest.
    #[must_use]
    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }
}

/// Overall resolver comparison result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverDifferentialOutcome {
    /// Both complete snapshots match.
    Match,
    /// At least one observable category differs.
    Differences,
}

/// Structured report for one stable resolver corpus case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverDifferentialReport {
    case_id: String,
    reference: ResolverImplementation,
    candidate: ResolverImplementation,
    outcome: ResolverDifferentialOutcome,
    differences: Vec<ResolverDifference>,
    reference_snapshot: ResolverSnapshot,
    candidate_snapshot: ResolverSnapshot,
}

impl ResolverDifferentialReport {
    /// Returns the stable corpus case identifier.
    #[must_use]
    pub fn case_id(&self) -> &str {
        &self.case_id
    }

    /// Returns the reference implementation.
    #[must_use]
    pub const fn reference(&self) -> ResolverImplementation {
        self.reference
    }

    /// Returns the candidate implementation.
    #[must_use]
    pub const fn candidate(&self) -> ResolverImplementation {
        self.candidate
    }

    /// Returns the comparison outcome.
    #[must_use]
    pub const fn outcome(&self) -> ResolverDifferentialOutcome {
        self.outcome
    }

    /// Returns every unsuppressed observable mismatch.
    #[must_use]
    pub fn differences(&self) -> &[ResolverDifference] {
        &self.differences
    }

    /// Returns true only when every observable matches.
    #[must_use]
    pub const fn is_match(&self) -> bool {
        matches!(self.outcome, ResolverDifferentialOutcome::Match)
    }

    /// Returns true only for a complete match; no suppression list exists.
    #[must_use]
    pub const fn passes_gate(&self) -> bool {
        self.is_match()
    }

    /// Returns the validated reference snapshot for failure artifacts.
    #[must_use]
    pub const fn reference_snapshot(&self) -> &ResolverSnapshot {
        &self.reference_snapshot
    }

    /// Returns the validated candidate snapshot for failure artifacts.
    #[must_use]
    pub const fn candidate_snapshot(&self) -> &ResolverSnapshot {
        &self.candidate_snapshot
    }
}

/// Runs two resolver implementations over one shared explicit source graph.
pub struct ResolverDifferentialHarness<'adapter> {
    reference: &'adapter dyn ResolverAdapter,
    candidate: &'adapter dyn ResolverAdapter,
}

impl<'adapter> ResolverDifferentialHarness<'adapter> {
    /// Creates a resolver-only differential harness.
    #[must_use]
    pub const fn new(
        reference: &'adapter dyn ResolverAdapter,
        candidate: &'adapter dyn ResolverAdapter,
    ) -> Self {
        Self {
            reference,
            candidate,
        }
    }

    /// Runs both adapters and compares every frozen resolver observable.
    ///
    /// # Errors
    ///
    /// Returns an input, adapter, snapshot-validation, or serialization error.
    pub fn run_case(
        &self,
        case_id: impl Into<String>,
        input: &CompilerInput,
    ) -> Result<ResolverDifferentialReport, ResolverAdapterError> {
        let reference_snapshot = self.reference.resolve(input)?;
        let candidate_snapshot = self.candidate.resolve(input)?;
        validate_snapshot_against_input(input, &reference_snapshot)?;
        validate_snapshot_against_input(input, &candidate_snapshot)?;

        let reference_digest = snapshot_digest(&reference_snapshot)?;
        let candidate_digest = snapshot_digest(&candidate_snapshot)?;
        let mut differences = Vec::new();
        if reference_snapshot.modules != candidate_snapshot.modules
            || reference_snapshot.edges != candidate_snapshot.edges
        {
            differences.push(ResolverDifference {
                observable: ResolverObservable::ModuleGraph,
                reference_digest: reference_digest.clone(),
                candidate_digest: candidate_digest.clone(),
            });
        }
        if reference_snapshot.symbols != candidate_snapshot.symbols {
            differences.push(ResolverDifference {
                observable: ResolverObservable::Symbols,
                reference_digest: reference_digest.clone(),
                candidate_digest: candidate_digest.clone(),
            });
        }
        if reference_snapshot.scopes != candidate_snapshot.scopes {
            differences.push(ResolverDifference {
                observable: ResolverObservable::Scopes,
                reference_digest: reference_digest.clone(),
                candidate_digest: candidate_digest.clone(),
            });
        }
        if reference_snapshot.bindings != candidate_snapshot.bindings {
            differences.push(ResolverDifference {
                observable: ResolverObservable::Bindings,
                reference_digest: reference_digest.clone(),
                candidate_digest: candidate_digest.clone(),
            });
        }
        if reference_snapshot.names != candidate_snapshot.names {
            differences.push(ResolverDifference {
                observable: ResolverObservable::Names,
                reference_digest: reference_digest.clone(),
                candidate_digest: candidate_digest.clone(),
            });
        }
        if reference_snapshot.diagnostics != candidate_snapshot.diagnostics {
            differences.push(ResolverDifference {
                observable: ResolverObservable::Diagnostics,
                reference_digest,
                candidate_digest,
            });
        }
        let outcome = if differences.is_empty() {
            ResolverDifferentialOutcome::Match
        } else {
            ResolverDifferentialOutcome::Differences
        };

        Ok(ResolverDifferentialReport {
            case_id: case_id.into(),
            reference: self.reference.implementation(),
            candidate: self.candidate.implementation(),
            outcome,
            differences,
            reference_snapshot,
            candidate_snapshot,
        })
    }
}

fn validate_snapshot_against_input(
    input: &CompilerInput,
    snapshot: &ResolverSnapshot,
) -> Result<(), ResolverAdapterError> {
    let sources = validated_sources(input)?;
    if snapshot
        .modules
        .first()
        .map(ResolverModuleSnapshot::identity)
        != Some(input.entry())
    {
        return Err(ResolverAdapterError::InvalidSnapshot(
            "module 0 identity must equal the explicit entry identity".to_owned(),
        ));
    }
    let source_lengths = snapshot
        .modules
        .iter()
        .map(|module| {
            sources
                .iter()
                .find(|source| source.identity() == module.identity())
                .map(|source| source.content().len())
                .ok_or_else(|| {
                    ResolverAdapterError::InvalidSnapshot(format!(
                        "module identity `{}` is absent from the explicit source collection",
                        module.identity()
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    snapshot.validate(&source_lengths)
}

fn snapshot_digest(snapshot: &ResolverSnapshot) -> Result<String, ResolverAdapterError> {
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(snapshot)?)
    ))
}

fn session_source_lengths<P>(
    session: &crate::CompilerSession<P>,
) -> Result<Vec<usize>, ResolverAdapterError> {
    session
        .modules()
        .iter()
        .map(|module| {
            session
                .sources()
                .file(module.file())
                .map(|source| source.text().len())
                .ok_or_else(|| {
                    ResolverAdapterError::InvalidInput(
                        "session source map is incomplete".to_owned(),
                    )
                })
        })
        .collect()
}

/// Adapter for the frozen Rust reference resolver.
#[derive(Debug, Default, Clone, Copy)]
pub struct RustResolverAdapter;

impl ResolverAdapter for RustResolverAdapter {
    fn implementation(&self) -> ResolverImplementation {
        ResolverImplementation::RustReference
    }

    fn resolve(&self, input: &CompilerInput) -> Result<ResolverSnapshot, ResolverAdapterError> {
        let session = explicit_session(input)?;
        if !session.diagnostics().is_empty() {
            let summary = session
                .diagnostics()
                .iter()
                .map(|diagnostic| diagnostic.code().as_str())
                .collect::<Vec<_>>()
                .join(",");
            return Err(ResolverAdapterError::InvalidInput(summary));
        }

        let programs = session
            .modules()
            .iter()
            .map(|module| {
                lower_module(module.id(), module.file(), &module.parse().syntax())
                    .map_err(ResolverAdapterError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let links = session
            .edges()
            .iter()
            .map(|edge| ResolvedImport::new(edge.importer(), edge.path_span(), edge.imported()))
            .collect::<Vec<_>>();
        let resolution = resolve_modules(&programs, &links);

        let modules = session
            .modules()
            .iter()
            .map(|module| {
                Ok(ResolverModuleSnapshot {
                    id: schema_u32(module.id().index())?,
                    identity: module.key().to_string(),
                    entry: module.id().index() == 0,
                })
            })
            .collect::<Result<Vec<_>, ResolverAdapterError>>()?;
        let edges = session
            .edges()
            .iter()
            .map(|edge| {
                Ok(ResolverEdgeSnapshot {
                    importer: schema_u32(edge.importer().index())?,
                    imported: schema_u32(edge.imported().index())?,
                    specifier: edge.specifier().to_owned(),
                    path_span: resolver_span(edge.path_span())?,
                })
            })
            .collect::<Result<Vec<_>, ResolverAdapterError>>()?;
        let symbols = resolution
            .symbols()
            .iter()
            .map(symbol_snapshot)
            .collect::<Result<Vec<_>, _>>()?;
        let scopes = resolution
            .scopes()
            .iter()
            .map(|scope| {
                let id = scope.id();
                Ok(ResolverScopeSnapshot {
                    module: schema_u32(id.owner().module().index())?,
                    function: schema_u32(id.owner().index())?,
                    index: schema_u32(id.index())?,
                    parent: id_option_u32(scope.parent().map(|parent| parent.index()))?,
                    kind: match scope.kind() {
                        HirScopeKind::Function => ResolverScopeKind::Function,
                        HirScopeKind::Block => ResolverScopeKind::Block,
                        HirScopeKind::For => ResolverScopeKind::For,
                        HirScopeKind::MatchArm => ResolverScopeKind::MatchArm,
                        HirScopeKind::Closure => ResolverScopeKind::Closure,
                    },
                    span: resolver_span(scope.span())?,
                })
            })
            .collect::<Result<Vec<_>, ResolverAdapterError>>()?;
        let bindings = resolution
            .bindings()
            .iter()
            .map(|binding| {
                Ok(ResolverBindingSnapshot {
                    module: schema_u32(binding.owner().module().index())?,
                    function: schema_u32(binding.owner().index())?,
                    local: schema_u32(binding.local().index())?,
                    scope: schema_u32(binding.scope().index())?,
                    name_span: resolver_span(binding.name_span())?,
                    mutable: binding.is_mutable(),
                })
            })
            .collect::<Result<Vec<_>, ResolverAdapterError>>()?;
        let names = resolution
            .names()
            .iter()
            .map(|name| {
                Ok(ResolverNameSnapshot {
                    span: resolver_span(name.span())?,
                    target: target_snapshot(name.owner(), name.resolution())?,
                })
            })
            .collect::<Result<Vec<_>, ResolverAdapterError>>()?;
        let diagnostics = resolution
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                Ok(ResolverDiagnosticSnapshot {
                    code: diagnostic.code().as_str().to_owned(),
                    severity: match diagnostic.severity() {
                        Severity::Error => ResolverSeverity::Error,
                        Severity::Warning => ResolverSeverity::Warning,
                    },
                    labels: diagnostic
                        .labels()
                        .iter()
                        .map(|label| {
                            Ok(ResolverLabelSnapshot {
                                style: match label.style() {
                                    LabelStyle::Primary => ResolverLabelStyle::Primary,
                                    LabelStyle::Secondary => ResolverLabelStyle::Secondary,
                                },
                                span: resolver_span(label.span())?,
                            })
                        })
                        .collect::<Result<Vec<_>, ResolverAdapterError>>()?,
                })
            })
            .collect::<Result<Vec<_>, ResolverAdapterError>>()?;
        let snapshot = ResolverSnapshot {
            schema_version: RESOLVER_SNAPSHOT_SCHEMA_VERSION,
            modules,
            edges,
            symbols,
            scopes,
            bindings,
            names,
            diagnostics,
        };
        let source_lengths = session_source_lengths(&session)?;
        snapshot.validate(&source_lengths)?;
        Ok(snapshot)
    }
}

/// Adapter that executes the real Futao-written resolver through verified MIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutaoResolverAdapter {
    program: MirProgram,
    sources: SourceMap,
}

impl FutaoResolverAdapter {
    /// Compiles the pure resolver under Bootstrap Profile v1 and its private Host driver.
    ///
    /// # Errors
    ///
    /// Returns an error when either source graph is rejected or produces no executable MIR.
    pub fn new() -> Result<Self, ResolverAdapterError> {
        let profile_output = compile(&CompilerInput::with_options(
            "resolver_profile.ft",
            futao_resolver_sources("resolver_profile.ft", FUTAO_PROFILE_ENTRY),
            CompilerOptions::bootstrap_v1(),
        ))?;
        ensure_futao_resolver_compiled("Bootstrap Profile entry", &profile_output)?;

        let driver_output = compile(&CompilerInput::new(
            "resolver_driver.ft",
            futao_resolver_sources("resolver_driver.ft", FUTAO_DRIVER_ENTRY),
        ))?;
        ensure_futao_resolver_compiled("application driver", &driver_output)?;
        let sources = driver_output.sources().clone();
        let program = driver_output.mir().cloned().ok_or_else(|| {
            ResolverAdapterError::FutaoSource(
                "application driver produced no executable MIR".to_owned(),
            )
        })?;
        Ok(Self { program, sources })
    }

    /// Resolves the complete checked-in bootstrap compiler graph with its larger fixed budget.
    ///
    /// # Errors
    ///
    /// Returns an input, execution, protocol, or snapshot-validation error.
    pub fn resolve_self_hosting_graph(
        &self,
        input: &CompilerInput,
    ) -> Result<ResolverSnapshot, ResolverAdapterError> {
        self.resolve_with_step_limit(input, FUTAO_RESOLVER_SELF_GRAPH_STEP_LIMIT)
    }

    fn resolve_with_step_limit(
        &self,
        input: &CompilerInput,
        step_limit: usize,
    ) -> Result<ResolverSnapshot, ResolverAdapterError> {
        let _ = validated_sources(input)?;
        let arguments = encoded_resolver_arguments(input)?;
        let execution =
            run_mir_with_args(&self.program, &arguments, step_limit).map_err(|failure| {
                let location = resolver_runtime_location(&self.sources, failure.error().span());
                ResolverAdapterError::Runtime { failure, location }
            })?;
        let snapshot = parse_futao_resolver_output(execution.output())?;
        validate_snapshot_against_input(input, &snapshot)?;
        Ok(snapshot)
    }
}

impl ResolverAdapter for FutaoResolverAdapter {
    fn implementation(&self) -> ResolverImplementation {
        ResolverImplementation::Futao
    }

    fn resolve(&self, input: &CompilerInput) -> Result<ResolverSnapshot, ResolverAdapterError> {
        self.resolve_with_step_limit(input, FUTAO_RESOLVER_STEP_LIMIT)
    }
}

fn futao_resolver_sources(entry: &str, entry_source: &str) -> Vec<CompilerSource> {
    vec![
        CompilerSource::new(entry, entry_source),
        CompilerSource::new("lexer.ft", FUTAO_LEXER_SOURCE),
        CompilerSource::new("parser.ft", FUTAO_PARSER_SOURCE),
        CompilerSource::new("resolver.ft", FUTAO_RESOLVER_SOURCE),
        CompilerSource::new("resolver_bridge.ft", FUTAO_BRIDGE_SOURCE),
        CompilerSource::new("sequence.ft", FUTAO_SEQUENCE_SOURCE),
    ]
}

fn ensure_futao_resolver_compiled(
    role: &str,
    output: &crate::CompilerOutput,
) -> Result<(), ResolverAdapterError> {
    if output.is_ok() && output.diagnostics().is_empty() {
        return Ok(());
    }
    let summary = output
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            let span = diagnostic
                .labels()
                .first()
                .map(|label| {
                    format!(
                        " @source{}:{}..{}",
                        label.file(),
                        label.start(),
                        label.end()
                    )
                })
                .unwrap_or_default();
            format!("{}: {}{}", diagnostic.code(), diagnostic.message(), span)
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(ResolverAdapterError::FutaoSource(format!(
        "{role} failed: {summary}"
    )))
}

fn encoded_resolver_arguments(input: &CompilerInput) -> Result<Vec<String>, ResolverAdapterError> {
    let sources = validated_sources(input)?;
    let mut arguments = Vec::new();
    push_code_points(&mut arguments, input.entry())?;
    arguments.push(sources.len().to_string());
    for source in sources {
        push_code_points(&mut arguments, source.identity())?;
        let text = source.content();
        let byte_length =
            u32::try_from(text.len()).map_err(|_| ResolverAdapterError::InputTooLarge)?;
        let pair_count =
            u32::try_from(text.chars().count()).map_err(|_| ResolverAdapterError::InputTooLarge)?;
        arguments.push(byte_length.to_string());
        arguments.push(pair_count.to_string());
        for (offset, character) in text.char_indices() {
            arguments.push(u32::from(character).to_string());
            arguments.push(offset.to_string());
        }
    }
    Ok(arguments)
}

fn push_code_points(output: &mut Vec<String>, value: &str) -> Result<(), ResolverAdapterError> {
    let count =
        u32::try_from(value.chars().count()).map_err(|_| ResolverAdapterError::InputTooLarge)?;
    output.push(count.to_string());
    output.extend(
        value
            .chars()
            .map(|character| u32::from(character).to_string()),
    );
    Ok(())
}

fn resolver_runtime_location(sources: &SourceMap, span: SourceSpan) -> String {
    let range = span.range();
    if let Some((file, location)) = sources.location(span) {
        format!(
            "{}:{}:{} (source #{}, bytes {}..{})",
            file.path().display(),
            location.line(),
            location.column(),
            span.file().raw(),
            range.start(),
            range.end()
        )
    } else {
        format!(
            "<unknown> (source #{}, bytes {}..{})",
            span.file().raw(),
            range.start(),
            range.end()
        )
    }
}

fn parse_futao_resolver_output(
    output: &[String],
) -> Result<ResolverSnapshot, ResolverAdapterError> {
    let mut reader = ResolverProtocolReader::new(output);
    reader.expect("header", "FUTAO-RESOLVER-2")?;
    match reader.next("resolver status")? {
        "ok" => {}
        "invalid-input" => {
            return Err(ResolverAdapterError::Protocol(
                "resolver rejected Host-validated input".to_owned(),
            ));
        }
        "parser-invalid" => {
            return Err(ResolverAdapterError::Protocol(
                "resolver parser rejected syntax-valid input".to_owned(),
            ));
        }
        status => {
            return Err(ResolverAdapterError::Protocol(format!(
                "unknown resolver status `{status}`"
            )));
        }
    }

    let module_count = reader.count("module count")?;
    let mut modules = Vec::with_capacity(module_count);
    for expected_id in 0..module_count {
        let id = reader.u32("module id")?;
        let identity = reader.code_points("module identity")?;
        if usize::try_from(id).ok() != Some(expected_id) {
            return Err(ResolverAdapterError::Protocol(
                "module ids must be contiguous and discovery ordered".to_owned(),
            ));
        }
        modules.push(ResolverModuleSnapshot {
            id,
            identity,
            entry: expected_id == 0,
        });
    }

    let edge_count = reader.count("edge count")?;
    let mut edges = Vec::with_capacity(edge_count);
    for _ in 0..edge_count {
        let importer = reader.u32("edge importer")?;
        edges.push(ResolverEdgeSnapshot {
            importer,
            imported: reader.u32("edge imported")?,
            specifier: reader.code_points("edge specifier")?,
            path_span: ResolverSpanSnapshot {
                source: importer,
                start: reader.u32("edge path start")?,
                end: reader.u32("edge path end")?,
            },
        });
    }

    let symbol_count = reader.count("symbol count")?;
    let mut symbols = Vec::with_capacity(symbol_count);
    for _ in 0..symbol_count {
        symbols.push(ResolverSymbolSnapshot {
            kind: parse_symbol_kind(reader.next("symbol kind")?)?,
            module: reader.u32("symbol module")?,
            index: reader.u32("symbol index")?,
            visibility: parse_visibility(reader.next("symbol visibility")?)?,
            name_span: read_resolver_span(&mut reader, "symbol name span")?,
        });
    }

    let scope_count = reader.count("scope count")?;
    let mut scopes = Vec::with_capacity(scope_count);
    for _ in 0..scope_count {
        let module = reader.u32("scope module")?;
        let function = reader.u32("scope function")?;
        let index = reader.u32("scope index")?;
        let parent = match reader.next("scope parent tag")? {
            "none" => None,
            "some" => Some(reader.u32("scope parent")?),
            tag => {
                return Err(ResolverAdapterError::Protocol(format!(
                    "unknown scope parent tag `{tag}`"
                )));
            }
        };
        scopes.push(ResolverScopeSnapshot {
            module,
            function,
            index,
            parent,
            kind: parse_scope_kind(reader.next("scope kind")?)?,
            span: read_resolver_span(&mut reader, "scope span")?,
        });
    }

    let binding_count = reader.count("binding count")?;
    let mut bindings = Vec::with_capacity(binding_count);
    for _ in 0..binding_count {
        bindings.push(ResolverBindingSnapshot {
            module: reader.u32("binding module")?,
            function: reader.u32("binding function")?,
            local: reader.u32("binding local")?,
            scope: reader.u32("binding scope")?,
            name_span: read_resolver_span(&mut reader, "binding name span")?,
            mutable: reader.boolean("binding mutable")?,
        });
    }

    let name_count = reader.count("resolved name count")?;
    let mut names = Vec::with_capacity(name_count);
    for _ in 0..name_count {
        names.push(ResolverNameSnapshot {
            span: read_resolver_span(&mut reader, "resolved name span")?,
            target: read_resolver_target(&mut reader)?,
        });
    }

    let diagnostic_count = reader.count("diagnostic count")?;
    let mut diagnostics = Vec::with_capacity(diagnostic_count);
    for _ in 0..diagnostic_count {
        let code = reader.next("diagnostic code")?.to_owned();
        let severity = parse_resolver_severity(reader.next("diagnostic severity")?)?;
        let label_count = reader.count("diagnostic label count")?;
        let mut labels = Vec::with_capacity(label_count);
        for _ in 0..label_count {
            labels.push(ResolverLabelSnapshot {
                style: parse_label_style(reader.next("diagnostic label style")?)?,
                span: read_resolver_span(&mut reader, "diagnostic label span")?,
            });
        }
        diagnostics.push(ResolverDiagnosticSnapshot {
            code,
            severity,
            labels,
        });
    }
    reader.finish()?;

    Ok(ResolverSnapshot {
        schema_version: RESOLVER_SNAPSHOT_SCHEMA_VERSION,
        modules,
        edges,
        symbols,
        scopes,
        bindings,
        names,
        diagnostics,
    })
}

fn read_resolver_span(
    reader: &mut ResolverProtocolReader<'_>,
    role: &str,
) -> Result<ResolverSpanSnapshot, ResolverAdapterError> {
    Ok(ResolverSpanSnapshot {
        source: reader.u32(&format!("{role} source"))?,
        start: reader.u32(&format!("{role} start"))?,
        end: reader.u32(&format!("{role} end"))?,
    })
}

fn read_resolver_target(
    reader: &mut ResolverProtocolReader<'_>,
) -> Result<ResolverTargetSnapshot, ResolverAdapterError> {
    Ok(match reader.next("resolved target kind")? {
        "local" => ResolverTargetSnapshot::Local {
            module: reader.u32("local target module")?,
            function: reader.u32("local target function")?,
            local: reader.u32("local target index")?,
        },
        "function" => ResolverTargetSnapshot::Function {
            module: reader.u32("function target module")?,
            index: reader.u32("function target index")?,
        },
        "record" => ResolverTargetSnapshot::Record {
            module: reader.u32("record target module")?,
            index: reader.u32("record target index")?,
        },
        "field" => ResolverTargetSnapshot::Field {
            module: reader.u32("field target module")?,
            record: reader.u32("field target record")?,
            index: reader.u32("field target index")?,
        },
        "union" => ResolverTargetSnapshot::Union {
            module: reader.u32("union target module")?,
            index: reader.u32("union target index")?,
        },
        "variant" => ResolverTargetSnapshot::Variant {
            module: reader.u32("variant target module")?,
            union: reader.u32("variant target union")?,
            index: reader.u32("variant target index")?,
        },
        "payload" => ResolverTargetSnapshot::Payload {
            module: reader.u32("payload target module")?,
            union: reader.u32("payload target union")?,
            variant: reader.u32("payload target variant")?,
            index: reader.u32("payload target index")?,
        },
        "type-parameter" => {
            let owner_kind = match reader.u32("type parameter owner kind")? {
                0 => ResolverSymbolKind::Function,
                1 => ResolverSymbolKind::Record,
                2 => ResolverSymbolKind::Union,
                value => {
                    return Err(ResolverAdapterError::Protocol(format!(
                        "unknown type parameter owner kind `{value}`"
                    )));
                }
            };
            ResolverTargetSnapshot::TypeParameter {
                owner_kind,
                module: reader.u32("type parameter target module")?,
                owner: reader.u32("type parameter target owner")?,
                index: reader.u32("type parameter target index")?,
            }
        }
        "print" => ResolverTargetSnapshot::Builtin {
            name: "print".to_owned(),
        },
        "array-length" => ResolverTargetSnapshot::Builtin {
            name: "array-length".to_owned(),
        },
        "string-length" => ResolverTargetSnapshot::Builtin {
            name: "string-length".to_owned(),
        },
        "array-append" => ResolverTargetSnapshot::Builtin {
            name: "array-append".to_owned(),
        },
        "array-concat" => ResolverTargetSnapshot::Builtin {
            name: "array-concat".to_owned(),
        },
        "to-string" => ResolverTargetSnapshot::Builtin {
            name: "to-string".to_owned(),
        },
        "parse-int" => ResolverTargetSnapshot::Builtin {
            name: "parse-int".to_owned(),
        },
        kind => {
            return Err(ResolverAdapterError::Protocol(format!(
                "unknown resolved target kind `{kind}`"
            )));
        }
    })
}

fn parse_symbol_kind(value: &str) -> Result<ResolverSymbolKind, ResolverAdapterError> {
    match value {
        "function" => Ok(ResolverSymbolKind::Function),
        "record" => Ok(ResolverSymbolKind::Record),
        "union" => Ok(ResolverSymbolKind::Union),
        _ => Err(ResolverAdapterError::Protocol(format!(
            "unknown symbol kind `{value}`"
        ))),
    }
}

fn parse_visibility(value: &str) -> Result<ResolverVisibility, ResolverAdapterError> {
    match value {
        "private" => Ok(ResolverVisibility::Private),
        "exported" => Ok(ResolverVisibility::Exported),
        _ => Err(ResolverAdapterError::Protocol(format!(
            "unknown visibility `{value}`"
        ))),
    }
}

fn parse_scope_kind(value: &str) -> Result<ResolverScopeKind, ResolverAdapterError> {
    match value {
        "function" => Ok(ResolverScopeKind::Function),
        "block" => Ok(ResolverScopeKind::Block),
        "for" => Ok(ResolverScopeKind::For),
        "match-arm" => Ok(ResolverScopeKind::MatchArm),
        "closure" => Ok(ResolverScopeKind::Closure),
        _ => Err(ResolverAdapterError::Protocol(format!(
            "unknown scope kind `{value}`"
        ))),
    }
}

fn parse_resolver_severity(value: &str) -> Result<ResolverSeverity, ResolverAdapterError> {
    match value {
        "error" => Ok(ResolverSeverity::Error),
        "warning" => Ok(ResolverSeverity::Warning),
        _ => Err(ResolverAdapterError::Protocol(format!(
            "unknown diagnostic severity `{value}`"
        ))),
    }
}

fn parse_label_style(value: &str) -> Result<ResolverLabelStyle, ResolverAdapterError> {
    match value {
        "primary" => Ok(ResolverLabelStyle::Primary),
        "secondary" => Ok(ResolverLabelStyle::Secondary),
        _ => Err(ResolverAdapterError::Protocol(format!(
            "unknown diagnostic label style `{value}`"
        ))),
    }
}

struct ResolverProtocolReader<'output> {
    output: &'output [String],
    position: usize,
}

impl<'output> ResolverProtocolReader<'output> {
    const fn new(output: &'output [String]) -> Self {
        Self {
            output,
            position: 0,
        }
    }

    fn next(&mut self, field: &str) -> Result<&'output str, ResolverAdapterError> {
        let value = self.output.get(self.position).ok_or_else(|| {
            ResolverAdapterError::Protocol(format!("missing {field} at line {}", self.position + 1))
        })?;
        self.position += 1;
        Ok(value)
    }

    fn expect(&mut self, field: &str, expected: &str) -> Result<(), ResolverAdapterError> {
        let value = self.next(field)?;
        if value == expected {
            Ok(())
        } else {
            Err(ResolverAdapterError::Protocol(format!(
                "{field} must be `{expected}`, found `{value}`"
            )))
        }
    }

    fn usize(&mut self, field: &str) -> Result<usize, ResolverAdapterError> {
        self.next(field)?.parse::<usize>().map_err(|_| {
            ResolverAdapterError::Protocol(format!("{field} is not an unsigned integer"))
        })
    }

    fn count(&mut self, field: &str) -> Result<usize, ResolverAdapterError> {
        let count = self.usize(field)?;
        let remaining_lines = self.output.len().saturating_sub(self.position);
        if count > remaining_lines {
            return Err(ResolverAdapterError::Protocol(format!(
                "{field} {count} exceeds {remaining_lines} remaining protocol line(s)"
            )));
        }
        Ok(count)
    }

    fn code_points(&mut self, field: &str) -> Result<String, ResolverAdapterError> {
        let count = self.count(field)?;
        let mut value = String::new();
        for _ in 0..count {
            let point = self.u32(&format!("{field} code point"))?;
            let character = char::from_u32(point).ok_or_else(|| {
                ResolverAdapterError::Protocol(format!(
                    "{field} contains invalid Unicode scalar `{point}`"
                ))
            })?;
            value.push(character);
        }
        Ok(value)
    }

    fn u32(&mut self, field: &str) -> Result<u32, ResolverAdapterError> {
        let value = self.next(field)?;
        value.parse::<u32>().map_err(|_| {
            ResolverAdapterError::Protocol(format!("{field} is not a u32 integer: `{value}`"))
        })
    }

    fn boolean(&mut self, field: &str) -> Result<bool, ResolverAdapterError> {
        match self.next(field)? {
            "true" => Ok(true),
            "false" => Ok(false),
            value => Err(ResolverAdapterError::Protocol(format!(
                "{field} is not a boolean: `{value}`"
            ))),
        }
    }

    fn finish(self) -> Result<(), ResolverAdapterError> {
        if self.position == self.output.len() {
            Ok(())
        } else {
            Err(ResolverAdapterError::Protocol(format!(
                "{} trailing line(s)",
                self.output.len() - self.position
            )))
        }
    }
}

fn symbol_snapshot(
    symbol: &nexa_hir::ResolverSymbol,
) -> Result<ResolverSymbolSnapshot, ResolverAdapterError> {
    let (kind, module, index) = match symbol.definition() {
        DefId::Function(function) => (
            ResolverSymbolKind::Function,
            function.module(),
            function.index(),
        ),
        DefId::Record(record) => (ResolverSymbolKind::Record, record.module(), record.index()),
        DefId::Union(union) => (ResolverSymbolKind::Union, union.module(), union.index()),
    };
    Ok(ResolverSymbolSnapshot {
        kind,
        module: schema_u32(module.index())?,
        index: schema_u32(index)?,
        visibility: match symbol.visibility() {
            Visibility::Private => ResolverVisibility::Private,
            Visibility::Exported => ResolverVisibility::Exported,
        },
        name_span: resolver_span(symbol.name_span())?,
    })
}

fn target_snapshot(
    owner: Option<nexa_hir::FunctionId>,
    resolution: NameResolution,
) -> Result<ResolverTargetSnapshot, ResolverAdapterError> {
    Ok(match resolution {
        NameResolution::Local(local) => {
            let owner = owner.ok_or_else(|| {
                ResolverAdapterError::InvalidSnapshot(
                    "local target has no containing function".to_owned(),
                )
            })?;
            ResolverTargetSnapshot::Local {
                module: schema_u32(owner.module().index())?,
                function: schema_u32(owner.index())?,
                local: schema_u32(local.index())?,
            }
        }
        NameResolution::Function(function) => ResolverTargetSnapshot::Function {
            module: schema_u32(function.module().index())?,
            index: schema_u32(function.index())?,
        },
        NameResolution::Record(record) => ResolverTargetSnapshot::Record {
            module: schema_u32(record.module().index())?,
            index: schema_u32(record.index())?,
        },
        NameResolution::Field(field) => ResolverTargetSnapshot::Field {
            module: schema_u32(field.record().module().index())?,
            record: schema_u32(field.record().index())?,
            index: schema_u32(field.index())?,
        },
        NameResolution::Union(union) => ResolverTargetSnapshot::Union {
            module: schema_u32(union.module().index())?,
            index: schema_u32(union.index())?,
        },
        NameResolution::Variant(variant) => ResolverTargetSnapshot::Variant {
            module: schema_u32(variant.union().module().index())?,
            union: schema_u32(variant.union().index())?,
            index: schema_u32(variant.index())?,
        },
        NameResolution::Payload(payload) => ResolverTargetSnapshot::Payload {
            module: schema_u32(payload.variant().union().module().index())?,
            union: schema_u32(payload.variant().union().index())?,
            variant: schema_u32(payload.variant().index())?,
            index: schema_u32(payload.index())?,
        },
        NameResolution::TypeParameter(parameter) => {
            let (owner_kind, module, owner_index) = match parameter.owner() {
                TypeParameterOwner::Function(function) => (
                    ResolverSymbolKind::Function,
                    function.module(),
                    function.index(),
                ),
                TypeParameterOwner::Record(record) => {
                    (ResolverSymbolKind::Record, record.module(), record.index())
                }
                TypeParameterOwner::Union(union) => {
                    (ResolverSymbolKind::Union, union.module(), union.index())
                }
            };
            ResolverTargetSnapshot::TypeParameter {
                owner_kind,
                module: schema_u32(module.index())?,
                owner: schema_u32(owner_index)?,
                index: schema_u32(parameter.index())?,
            }
        }
        NameResolution::Builtin(builtin) => ResolverTargetSnapshot::Builtin {
            name: builtin_name(builtin).to_owned(),
        },
    })
}

const fn builtin_name(builtin: Builtin) -> &'static str {
    match builtin {
        Builtin::Print => "print",
        Builtin::ArrayLength => "array-length",
        Builtin::StringLength => "string-length",
        Builtin::ArrayAppend => "array-append",
        Builtin::ArrayConcat => "array-concat",
        Builtin::ToString => "to-string",
        Builtin::ParseInt => "parse-int",
    }
}

fn resolver_span(span: SourceSpan) -> Result<ResolverSpanSnapshot, ResolverAdapterError> {
    Ok(ResolverSpanSnapshot {
        source: span.file().raw(),
        start: schema_u32(span.range().start())?,
        end: schema_u32(span.range().end())?,
    })
}

fn schema_u32(value: usize) -> Result<u32, ResolverAdapterError> {
    u32::try_from(value).map_err(|_| ResolverAdapterError::InputTooLarge)
}

fn id_option_u32(value: Option<usize>) -> Result<Option<u32>, ResolverAdapterError> {
    value.map(schema_u32).transpose()
}

fn validate_span(
    span: ResolverSpanSnapshot,
    source_lengths: &[usize],
) -> Result<(), ResolverAdapterError> {
    let source = usize::try_from(span.source).map_err(|_| ResolverAdapterError::InputTooLarge)?;
    let start = usize::try_from(span.start).map_err(|_| ResolverAdapterError::InputTooLarge)?;
    let end = usize::try_from(span.end).map_err(|_| ResolverAdapterError::InputTooLarge)?;
    let Some(source_len) = source_lengths.get(source).copied() else {
        return Err(ResolverAdapterError::InvalidSnapshot(
            "span references an unknown source".to_owned(),
        ));
    };
    if start > end || end > source_len {
        return Err(ResolverAdapterError::InvalidSnapshot(
            "span is reversed or outside its source".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_futao_resolver_output, ResolverAdapter, ResolverAdapterError,
        ResolverBindingSnapshot, ResolverDifferentialHarness, ResolverDifferentialOutcome,
        ResolverEdgeSnapshot, ResolverImplementation, ResolverModuleSnapshot, ResolverNameSnapshot,
        ResolverObservable, ResolverScopeKind, ResolverScopeSnapshot, ResolverSnapshot,
        ResolverSpanSnapshot, ResolverSymbolKind, ResolverSymbolSnapshot, ResolverTargetSnapshot,
        ResolverVisibility, RESOLVER_SNAPSHOT_SCHEMA_VERSION,
    };
    use crate::{CompilerInput, CompilerSource};

    #[test]
    fn snapshot_rejects_an_edge_to_an_unknown_module() {
        let mut snapshot = minimal_snapshot();
        snapshot.edges.push(ResolverEdgeSnapshot {
            importer: 0,
            imported: 1,
            specifier: "./missing.ft".to_owned(),
            path_span: span(0, 8, 22),
        });

        assert!(snapshot.validate(&[64]).is_err());
    }

    #[test]
    fn snapshot_rejects_discontinuous_symbol_scope_and_local_ids() {
        let mut symbol = minimal_snapshot();
        symbol.symbols[0].index = 1;
        assert!(symbol.validate(&[64]).is_err());

        let mut scope = minimal_snapshot();
        scope.scopes[0].index = 1;
        assert!(scope.validate(&[64]).is_err());

        let mut binding = minimal_snapshot();
        binding.bindings[0].local = 1;
        assert!(binding.validate(&[64]).is_err());
    }

    #[test]
    fn snapshot_rejects_invalid_scope_parents_and_binding_scopes() {
        let mut parent = minimal_snapshot();
        parent.scopes.push(ResolverScopeSnapshot {
            module: 0,
            function: 0,
            index: 1,
            parent: Some(1),
            kind: ResolverScopeKind::Block,
            span: span(0, 16, 32),
        });
        assert!(parent.validate(&[64]).is_err());

        let mut binding = minimal_snapshot();
        binding.bindings[0].scope = 1;
        assert!(binding.validate(&[64]).is_err());
    }

    #[test]
    fn snapshot_rejects_a_name_targeting_an_unknown_definition() {
        let mut snapshot = minimal_snapshot();
        snapshot.names[0].target = ResolverTargetSnapshot::Function {
            module: 0,
            index: 1,
        };

        assert!(snapshot.validate(&[64]).is_err());
    }

    #[test]
    fn snapshot_rejects_non_contiguous_child_identity_indices(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"type Pair<T> = {
  left: T;
  right: T;
};

type Choice<T> =
  | Some(value: T)
  | None();

function main<T>(value: T): T {
  match(value) {
    case Choice.Some(payload) => payload;
    default => value;
  };
}
"#;
        let baseline = super::RustResolverAdapter.resolve(&CompilerInput::new(
            "main.ft",
            [CompilerSource::new("main.ft", source)],
        ))?;

        let mut field = baseline.clone();
        let field_name = field
            .names
            .iter_mut()
            .find(|name| matches!(name.target, ResolverTargetSnapshot::Field { .. }))
            .ok_or_else(|| std::io::Error::other("expected a field target"))?;
        if let ResolverTargetSnapshot::Field { index, .. } = &mut field_name.target {
            *index = 99;
        }
        assert!(field.validate(&[source.len()]).is_err());

        let mut variant = baseline.clone();
        let variant_name = variant
            .names
            .iter_mut()
            .find(|name| matches!(name.target, ResolverTargetSnapshot::Variant { .. }))
            .ok_or_else(|| std::io::Error::other("expected a variant target"))?;
        if let ResolverTargetSnapshot::Variant { index, .. } = &mut variant_name.target {
            *index = 99;
        }
        assert!(variant.validate(&[source.len()]).is_err());

        let mut payload = baseline.clone();
        let payload_name = payload
            .names
            .iter_mut()
            .find(|name| matches!(name.target, ResolverTargetSnapshot::Payload { .. }))
            .ok_or_else(|| std::io::Error::other("expected a payload target"))?;
        if let ResolverTargetSnapshot::Payload { index, .. } = &mut payload_name.target {
            *index = 99;
        }
        assert!(payload.validate(&[source.len()]).is_err());

        let mut type_parameter = baseline;
        let type_parameter_name = type_parameter
            .names
            .iter_mut()
            .find(|name| matches!(name.target, ResolverTargetSnapshot::TypeParameter { .. }))
            .ok_or_else(|| std::io::Error::other("expected a type-parameter target"))?;
        if let ResolverTargetSnapshot::TypeParameter { index, .. } = &mut type_parameter_name.target
        {
            *index = 99;
        }
        assert!(type_parameter.validate(&[source.len()]).is_err());

        Ok(())
    }

    #[test]
    fn snapshot_rejects_names_outside_canonical_source_order() {
        let mut snapshot = minimal_snapshot();
        snapshot.names.swap(0, 1);

        assert!(snapshot.validate(&[64]).is_err());
    }

    #[test]
    fn differential_harness_classifies_every_unsuppressed_observable(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let reference = SnapshotAdapter {
            implementation: ResolverImplementation::RustReference,
            snapshot: minimal_snapshot(),
        };
        let mut candidate_snapshot = minimal_snapshot();
        candidate_snapshot.modules.push(ResolverModuleSnapshot {
            id: 1,
            identity: "candidate.ft".to_owned(),
            entry: false,
        });
        candidate_snapshot.symbols[0].visibility = ResolverVisibility::Exported;
        candidate_snapshot.scopes[0].span.end -= 1;
        candidate_snapshot.bindings[0].mutable = true;
        candidate_snapshot.names[1].target = ResolverTargetSnapshot::Function {
            module: 0,
            index: 0,
        };
        candidate_snapshot
            .diagnostics
            .push(super::ResolverDiagnosticSnapshot {
                code: "E2001".to_owned(),
                severity: super::ResolverSeverity::Error,
                labels: vec![super::ResolverLabelSnapshot {
                    style: super::ResolverLabelStyle::Primary,
                    span: span(0, 11, 12),
                }],
            });
        let candidate = SnapshotAdapter {
            implementation: ResolverImplementation::Futao,
            snapshot: candidate_snapshot,
        };
        let harness = ResolverDifferentialHarness::new(&reference, &candidate);
        let report = harness.run_case(
            "all-observables",
            &CompilerInput::new(
                "main.ft",
                [
                    CompilerSource::new("main.ft", SOURCE),
                    CompilerSource::new("candidate.ft", SOURCE),
                ],
            ),
        )?;

        assert_eq!(
            report
                .differences()
                .iter()
                .map(|difference| difference.observable())
                .collect::<Vec<_>>(),
            [
                ResolverObservable::ModuleGraph,
                ResolverObservable::Symbols,
                ResolverObservable::Scopes,
                ResolverObservable::Bindings,
                ResolverObservable::Names,
                ResolverObservable::Diagnostics,
            ]
        );
        assert_eq!(report.outcome(), ResolverDifferentialOutcome::Differences);
        assert!(!report.passes_gate());

        Ok(())
    }

    #[test]
    fn futao_protocol_rejects_a_count_larger_than_remaining_lines() {
        let output = [
            "FUTAO-RESOLVER-2".to_owned(),
            "ok".to_owned(),
            "0".to_owned(),
            "0".to_owned(),
            usize::MAX.to_string(),
        ];

        let error = parse_futao_resolver_output(&output).err();

        assert!(
            matches!(error, Some(ResolverAdapterError::Protocol(message)) if message.contains("symbol count") && message.contains("remaining protocol line"))
        );
    }

    #[test]
    fn futao_protocol_rejects_unknown_target_tags() {
        let output = [
            "FUTAO-RESOLVER-2",
            "ok",
            "0",
            "0",
            "1",
            "function",
            "0",
            "0",
            "private",
            "0",
            "0",
            "0",
            "0",
            "0",
            "1",
            "0",
            "0",
            "0",
            "mystery",
        ]
        .map(str::to_owned);

        let error = parse_futao_resolver_output(&output).err();

        assert!(
            matches!(error, Some(ResolverAdapterError::Protocol(message)) if message.contains("unknown resolved target kind"))
        );
    }

    #[test]
    fn futao_protocol_rejects_trailing_lines() {
        let output = [
            "FUTAO-RESOLVER-2",
            "ok",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "unexpected",
        ]
        .map(str::to_owned);

        let error = parse_futao_resolver_output(&output).err();

        assert!(
            matches!(error, Some(ResolverAdapterError::Protocol(message)) if message == "1 trailing line(s)")
        );
    }

    #[test]
    fn futao_driver_rejects_unknown_enum_values_instead_of_emitting_legal_tags(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let main_start = super::FUTAO_DRIVER_ENTRY
            .find("function main(arguments: String[]): Unit {")
            .ok_or_else(|| std::io::Error::other("resolver driver main was not found"))?;
        let probe = format!(
            "{}function main(arguments: String[]): Unit {{\n  print(symbolKindName(99));\n  print(visibilityName(99));\n  print(scopeKindName(99));\n  print(diagnosticCode(99));\n  emitLabel({{ style: 99, span: {{ source: 0, start: 0, end: 0 }} }});\n}}\n",
            &super::FUTAO_DRIVER_ENTRY[..main_start]
        );
        let output = super::compile(&CompilerInput::new(
            "resolver_driver_probe.ft",
            super::futao_resolver_sources("resolver_driver_probe.ft", &probe),
        ))?;
        super::ensure_futao_resolver_compiled("driver probe", &output)?;
        let program = output
            .mir()
            .ok_or_else(|| std::io::Error::other("driver probe produced no MIR"))?;
        let execution = super::run_mir_with_args(program, &[], 1_000_000)?;
        assert_eq!(
            execution.output(),
            [
                "invalid-symbol-kind",
                "invalid-visibility",
                "invalid-scope-kind",
                "invalid-diagnostic",
                "invalid-label-style",
                "0",
                "0",
                "0"
            ]
        );
        Ok(())
    }

    #[derive(Clone)]
    struct SnapshotAdapter {
        implementation: ResolverImplementation,
        snapshot: ResolverSnapshot,
    }

    impl ResolverAdapter for SnapshotAdapter {
        fn implementation(&self) -> ResolverImplementation {
            self.implementation
        }

        fn resolve(
            &self,
            _input: &CompilerInput,
        ) -> Result<ResolverSnapshot, ResolverAdapterError> {
            Ok(self.snapshot.clone())
        }
    }

    fn minimal_snapshot() -> ResolverSnapshot {
        ResolverSnapshot {
            schema_version: RESOLVER_SNAPSHOT_SCHEMA_VERSION,
            modules: vec![ResolverModuleSnapshot {
                id: 0,
                identity: "main.ft".to_owned(),
                entry: true,
            }],
            edges: Vec::new(),
            symbols: vec![ResolverSymbolSnapshot {
                kind: ResolverSymbolKind::Function,
                module: 0,
                index: 0,
                visibility: ResolverVisibility::Private,
                name_span: span(0, 0, 4),
            }],
            scopes: vec![ResolverScopeSnapshot {
                module: 0,
                function: 0,
                index: 0,
                parent: None,
                kind: ResolverScopeKind::Function,
                span: span(0, 0, SOURCE.len() as u32),
            }],
            bindings: vec![ResolverBindingSnapshot {
                module: 0,
                function: 0,
                local: 0,
                scope: 0,
                name_span: span(0, 5, 10),
                mutable: false,
            }],
            names: vec![
                ResolverNameSnapshot {
                    span: span(0, 0, 4),
                    target: ResolverTargetSnapshot::Function {
                        module: 0,
                        index: 0,
                    },
                },
                ResolverNameSnapshot {
                    span: span(0, 5, 10),
                    target: ResolverTargetSnapshot::Local {
                        module: 0,
                        function: 0,
                        local: 0,
                    },
                },
            ],
            diagnostics: Vec::new(),
        }
    }

    const fn span(source: u32, start: u32, end: u32) -> ResolverSpanSnapshot {
        ResolverSpanSnapshot { source, start, end }
    }

    const SOURCE: &str = "function main(): Unit { const x = 1; }";
}
