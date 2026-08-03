//! Name and module resolution independent of static type checking.

use std::collections::HashMap;

use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::SourceSpan;

use crate::{
    ArrowBody, AssignmentStatement, Block, Builtin, ConstDeclaration, DefId, Expression, FieldId,
    ForOfStatement, FunctionId, IfStatement, LetDeclaration, LocalId, MatchPattern, ModuleId, Name,
    NameResolution, PayloadId, Program, RecordId, ResolvedImport, ReturnStatement, Statement,
    TypeParameter, TypeParameterId, TypeParameterOwner, TypeReference, TypeReferenceKind, UnionId,
    VariantId, Visibility, WhileStatement,
};

const UNDEFINED_NAME: DiagnosticCode = DiagnosticCode::new("E2001");
const DUPLICATE_NAME: DiagnosticCode = DiagnosticCode::new("E2002");
const MODULE_CYCLE: DiagnosticCode = DiagnosticCode::new("E4002");
const MISSING_EXPORT: DiagnosticCode = DiagnosticCode::new("E4003");
const PRIVATE_EXPORT: DiagnosticCode = DiagnosticCode::new("E4004");
const EXPORT_COLLISION: DiagnosticCode = DiagnosticCode::new("E4005");

/// Complete observable result of resolving a lowered module graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    symbols: Vec<ResolverSymbol>,
    scopes: Vec<ResolverScope>,
    bindings: Vec<ResolvedBinding>,
    names: Vec<ResolvedName>,
    diagnostics: Vec<Diagnostic>,
}

impl Resolution {
    /// Returns top-level symbols in stable module and source order.
    #[must_use]
    pub fn symbols(&self) -> &[ResolverSymbol] {
        &self.symbols
    }

    /// Returns lexical scopes in stable function and preorder traversal order.
    #[must_use]
    pub fn scopes(&self) -> &[ResolverScope] {
        &self.scopes
    }

    /// Returns accepted local bindings in stable declaration order.
    #[must_use]
    pub fn bindings(&self) -> &[ResolvedBinding] {
        &self.bindings
    }

    /// Returns all resolved source names in stable source order.
    #[must_use]
    pub fn names(&self) -> &[ResolvedName] {
        &self.names
    }

    /// Returns the semantic target assigned to an exact source-name span.
    #[must_use]
    pub fn name_resolution(&self, span: SourceSpan) -> Option<NameResolution> {
        self.names
            .binary_search_by_key(&span_position(span), |name| span_position(name.span))
            .ok()
            .and_then(|index| self.names.get(index))
            .map(|name| name.resolution)
    }

    /// Returns resolver-only diagnostics in stable source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// One stable top-level source definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolverSymbol {
    definition: DefId,
    name_span: SourceSpan,
    visibility: Visibility,
}

impl ResolverSymbol {
    /// Returns the kind-safe, module-owned definition identity.
    #[must_use]
    pub const fn definition(self) -> DefId {
        self.definition
    }

    /// Returns the declaration-name token span.
    #[must_use]
    pub const fn name_span(self) -> SourceSpan {
        self.name_span
    }

    /// Returns whether the symbol is available to named imports.
    #[must_use]
    pub const fn visibility(self) -> Visibility {
        self.visibility
    }
}

/// Stable identifier for one lexical scope within a source function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResolverScopeId {
    owner: FunctionId,
    index: usize,
}

impl ResolverScopeId {
    const fn new(owner: FunctionId, index: usize) -> Self {
        Self { owner, index }
    }

    /// Returns the source function containing the scope.
    #[must_use]
    pub const fn owner(self) -> FunctionId {
        self.owner
    }

    /// Returns the preorder index within the owning function.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }
}

/// The source construct that introduces a lexical scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverScopeKind {
    /// A source function body and its parameters.
    Function,
    /// A nested statement block.
    Block,
    /// A `for...of` binding and body.
    For,
    /// One match arm and its pattern bindings.
    MatchArm,
    /// One arrow function and its parameters.
    Closure,
}

/// One lexical scope and its stable parent relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolverScope {
    id: ResolverScopeId,
    parent: Option<ResolverScopeId>,
    kind: ResolverScopeKind,
    span: SourceSpan,
}

impl ResolverScope {
    /// Returns the stable scope identity.
    #[must_use]
    pub const fn id(self) -> ResolverScopeId {
        self.id
    }

    /// Returns the containing source function.
    #[must_use]
    pub const fn owner(self) -> FunctionId {
        self.id.owner
    }

    /// Returns the immediately enclosing lexical scope.
    #[must_use]
    pub const fn parent(self) -> Option<ResolverScopeId> {
        self.parent
    }

    /// Returns the construct that introduced this scope.
    #[must_use]
    pub const fn kind(self) -> ResolverScopeKind {
        self.kind
    }

    /// Returns the complete source range governed by this scope.
    #[must_use]
    pub const fn span(self) -> SourceSpan {
        self.span
    }
}

/// One accepted function-local binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedBinding {
    owner: FunctionId,
    local: LocalId,
    scope: ResolverScopeId,
    name_span: SourceSpan,
    mutable: bool,
}

impl ResolvedBinding {
    /// Returns the source function owning the local slot.
    #[must_use]
    pub const fn owner(self) -> FunctionId {
        self.owner
    }

    /// Returns the stable function-local slot identity.
    #[must_use]
    pub const fn local(self) -> LocalId {
        self.local
    }

    /// Returns the lexical scope containing the declaration.
    #[must_use]
    pub const fn scope(self) -> ResolverScopeId {
        self.scope
    }

    /// Returns the exact declaration-name token span.
    #[must_use]
    pub const fn name_span(self) -> SourceSpan {
        self.name_span
    }

    /// Returns whether assignment is permitted after initialization.
    #[must_use]
    pub const fn is_mutable(self) -> bool {
        self.mutable
    }
}

/// One source name assigned to a stable semantic target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedName {
    span: SourceSpan,
    owner: Option<FunctionId>,
    resolution: NameResolution,
}

impl ResolvedName {
    /// Returns the exact source-name token span.
    #[must_use]
    pub const fn span(self) -> SourceSpan {
        self.span
    }

    /// Returns the containing function for function-local observations.
    #[must_use]
    pub const fn owner(self) -> Option<FunctionId> {
        self.owner
    }

    /// Returns the semantic target.
    #[must_use]
    pub const fn resolution(self) -> NameResolution {
        self.resolution
    }
}

#[derive(Debug, Clone, Copy)]
struct CatalogEntry {
    definition: DefId,
    span: SourceSpan,
    visibility: Visibility,
}

#[derive(Clone, Default)]
struct Namespace {
    types: HashMap<String, CatalogEntry>,
    values: HashMap<String, CatalogEntry>,
}

struct ModuleCatalog {
    declared: HashMap<String, SourceSpan>,
    exports: HashMap<String, CatalogEntry>,
    bindings: Vec<BindingEvent>,
}

#[derive(Debug, Clone)]
struct BindingEvent {
    name: String,
    span: SourceSpan,
    entry: CatalogEntry,
}

/// Resolves declarations, named imports, and lexical source names without type checking.
#[must_use]
pub fn resolve_modules(programs: &[Program], links: &[ResolvedImport]) -> Resolution {
    let mut symbols = Vec::new();
    let mut names = Vec::new();
    let mut diagnostics = Vec::new();
    let catalogs = programs
        .iter()
        .map(|program| {
            (
                program.module,
                collect_module_catalog(program, &mut symbols, &mut names, &mut diagnostics),
            )
        })
        .collect::<HashMap<_, _>>();
    let environments =
        resolve_environments(programs, links, &catalogs, &mut names, &mut diagnostics);
    diagnostics.extend(resolve_cycle_diagnostics(
        graph_module_count(programs, links),
        links,
    ));
    resolve_data_declarations(programs, &environments, &mut names, &mut diagnostics);

    let mut scopes = Vec::new();
    let mut bindings = Vec::new();
    resolve_function_bodies(
        programs,
        &environments,
        &mut scopes,
        &mut bindings,
        &mut names,
        &mut diagnostics,
    );
    symbols.sort_by_key(|symbol| span_position(symbol.name_span));
    names.sort_by_key(|name| span_position(name.span));
    diagnostics.sort_by(|left, right| diagnostic_position(left).cmp(&diagnostic_position(right)));

    Resolution {
        symbols,
        scopes,
        bindings,
        names,
        diagnostics,
    }
}

fn graph_module_count(programs: &[Program], links: &[ResolvedImport]) -> usize {
    programs
        .iter()
        .map(|program| program.module.index())
        .chain(
            links
                .iter()
                .flat_map(|link| [link.importer().index(), link.target().index()]),
        )
        .max()
        .map_or(0, |index| index.saturating_add(1))
}

fn resolve_cycle_diagnostics(module_count: usize, links: &[ResolvedImport]) -> Vec<Diagnostic> {
    let mut visit_states = vec![VisitState::Unvisited; module_count];
    let mut parent_edges = vec![None; module_count];
    let mut back_edges = Vec::new();
    for index in 0..module_count {
        discover_cycles(
            ModuleId::new(index),
            links,
            &mut visit_states,
            &mut parent_edges,
            &mut back_edges,
        );
    }

    let components = strongly_connected_components(module_count, links);
    let mut component_by_module = vec![None; module_count];
    for (component_index, component) in components.iter().enumerate() {
        for module in component {
            if let Some(slot) = component_by_module.get_mut(module.index()) {
                *slot = Some(component_index);
            }
        }
    }

    let mut diagnostics = Vec::new();
    for (component_index, component) in components.iter().enumerate() {
        if !component_is_cyclic(component, links) {
            continue;
        }
        let witness = back_edges.iter().copied().find(|edge_index| {
            links.get(*edge_index).is_some_and(|edge| {
                component_by_module
                    .get(edge.importer().index())
                    .copied()
                    .flatten()
                    == Some(component_index)
                    && component_by_module
                        .get(edge.target().index())
                        .copied()
                        .flatten()
                        == Some(component_index)
            })
        });
        let Some(witness) = witness else {
            continue;
        };
        let Some(closing_edge) = links.get(witness).copied() else {
            continue;
        };
        let mut diagnostic =
            Diagnostic::error(MODULE_CYCLE, "cyclic module import").with_label(Label::primary(
                closing_edge.path_span(),
                "this import closes a module cycle",
            ));
        for edge_index in tree_path_edges(
            closing_edge.target(),
            closing_edge.importer(),
            links,
            &parent_edges,
        ) {
            if let Some(edge) = links.get(edge_index) {
                diagnostic = diagnostic.with_label(Label::secondary(
                    edge.path_span(),
                    "cycle continues through this import",
                ));
            }
        }
        diagnostics.push(diagnostic);
    }
    diagnostics
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unvisited,
    Active,
    Finished,
}

fn discover_cycles(
    module: ModuleId,
    links: &[ResolvedImport],
    states: &mut [VisitState],
    parent_edges: &mut [Option<usize>],
    back_edges: &mut Vec<usize>,
) {
    let Some(state) = states.get_mut(module.index()) else {
        return;
    };
    if *state != VisitState::Unvisited {
        return;
    }
    *state = VisitState::Active;

    for (edge_index, edge) in links.iter().copied().enumerate() {
        if edge.importer() != module {
            continue;
        }
        let target = edge.target();
        match states.get(target.index()).copied() {
            Some(VisitState::Unvisited) => {
                if let Some(parent) = parent_edges.get_mut(target.index()) {
                    *parent = Some(edge_index);
                }
                discover_cycles(target, links, states, parent_edges, back_edges);
            }
            Some(VisitState::Active) => back_edges.push(edge_index),
            Some(VisitState::Finished) | None => {}
        }
    }
    if let Some(state) = states.get_mut(module.index()) {
        *state = VisitState::Finished;
    }
}

fn tree_path_edges(
    ancestor: ModuleId,
    descendant: ModuleId,
    links: &[ResolvedImport],
    parent_edges: &[Option<usize>],
) -> Vec<usize> {
    let mut reversed = Vec::new();
    let mut current = descendant;
    for _ in 0..parent_edges.len() {
        if current == ancestor {
            reversed.reverse();
            return reversed;
        }
        let Some(parent_edge) = parent_edges.get(current.index()).copied().flatten() else {
            return Vec::new();
        };
        let Some(edge) = links.get(parent_edge) else {
            return Vec::new();
        };
        reversed.push(parent_edge);
        current = edge.importer();
    }
    Vec::new()
}

fn component_is_cyclic(component: &[ModuleId], links: &[ResolvedImport]) -> bool {
    component.len() > 1
        || component.first().is_some_and(|module| {
            links
                .iter()
                .any(|edge| edge.importer() == *module && edge.target() == *module)
        })
}

fn strongly_connected_components(
    module_count: usize,
    links: &[ResolvedImport],
) -> Vec<Vec<ModuleId>> {
    let mut tarjan = Tarjan::new(module_count);
    for index in 0..module_count {
        if tarjan.indices[index].is_none() {
            tarjan.visit(ModuleId::new(index), links);
        }
    }
    tarjan.components
}

struct Tarjan {
    next_index: usize,
    indices: Vec<Option<usize>>,
    lowlinks: Vec<usize>,
    stack: Vec<ModuleId>,
    on_stack: Vec<bool>,
    components: Vec<Vec<ModuleId>>,
}

impl Tarjan {
    fn new(module_count: usize) -> Self {
        Self {
            next_index: 0,
            indices: vec![None; module_count],
            lowlinks: vec![0; module_count],
            stack: Vec::new(),
            on_stack: vec![false; module_count],
            components: Vec::new(),
        }
    }

    fn visit(&mut self, module: ModuleId, links: &[ResolvedImport]) {
        let index = self.next_index;
        self.next_index += 1;
        self.indices[module.index()] = Some(index);
        self.lowlinks[module.index()] = index;
        self.stack.push(module);
        self.on_stack[module.index()] = true;

        for target in links
            .iter()
            .filter(|edge| edge.importer() == module)
            .map(|edge| edge.target())
        {
            if target.index() >= self.indices.len() {
                continue;
            }
            if self.indices[target.index()].is_none() {
                self.visit(target, links);
                self.lowlinks[module.index()] =
                    self.lowlinks[module.index()].min(self.lowlinks[target.index()]);
            } else if self.on_stack[target.index()] {
                if let Some(target_index) = self.indices[target.index()] {
                    self.lowlinks[module.index()] = self.lowlinks[module.index()].min(target_index);
                }
            }
        }

        if self.lowlinks[module.index()] == self.indices[module.index()].unwrap_or(usize::MAX) {
            let mut component = Vec::new();
            while let Some(current) = self.stack.pop() {
                self.on_stack[current.index()] = false;
                component.push(current);
                if current == module {
                    break;
                }
            }
            component.sort();
            self.components.push(component);
        }
    }
}

fn resolve_function_bodies(
    programs: &[Program],
    environments: &HashMap<ModuleId, Namespace>,
    scopes: &mut Vec<ResolverScope>,
    bindings: &mut Vec<ResolvedBinding>,
    names: &mut Vec<ResolvedName>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for program in programs {
        let Some(environment) = environments.get(&program.module) else {
            continue;
        };
        for (index, function) in program.functions.iter().enumerate() {
            let owner = FunctionId::in_module(program.module, index);
            let mut resolver = FunctionResolver {
                owner,
                programs,
                environment,
                scopes,
                bindings,
                names,
                diagnostics,
                stack: Vec::new(),
                next_scope: 0,
                next_local: 0,
                type_parameters: HashMap::new(),
            };
            resolver.resolve_type_parameters(&function.type_parameters);
            for parameter in &function.parameters {
                resolver.resolve_type(&parameter.ty);
            }
            resolver.resolve_type(&function.return_type);
            resolver.push_scope(ResolverScopeKind::Function, function.body.span);
            for parameter in &function.parameters {
                resolver.bind(&parameter.name, false);
            }
            resolver.resolve_block(&function.body, false);
            resolver.pop_scope();
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LocalEntry {
    local: LocalId,
    span: SourceSpan,
}

struct ScopeState {
    id: ResolverScopeId,
    names: HashMap<String, LocalEntry>,
}

struct FunctionResolver<'a> {
    owner: FunctionId,
    programs: &'a [Program],
    environment: &'a Namespace,
    scopes: &'a mut Vec<ResolverScope>,
    bindings: &'a mut Vec<ResolvedBinding>,
    names: &'a mut Vec<ResolvedName>,
    diagnostics: &'a mut Vec<Diagnostic>,
    stack: Vec<ScopeState>,
    next_scope: usize,
    next_local: usize,
    type_parameters: HashMap<String, (TypeParameterId, SourceSpan)>,
}

impl FunctionResolver<'_> {
    fn push_scope(&mut self, kind: ResolverScopeKind, span: SourceSpan) {
        let id = ResolverScopeId::new(self.owner, self.next_scope);
        self.next_scope += 1;
        self.scopes.push(ResolverScope {
            id,
            parent: self.stack.last().map(|scope| scope.id),
            kind,
            span,
        });
        self.stack.push(ScopeState {
            id,
            names: HashMap::new(),
        });
    }

    fn pop_scope(&mut self) {
        let _ = self.stack.pop();
    }

    fn resolve_block(&mut self, block: &Block, creates_scope: bool) {
        if creates_scope {
            self.push_scope(ResolverScopeKind::Block, block.span);
        }
        for statement in &block.statements {
            self.resolve_statement(statement);
        }
        if creates_scope {
            self.pop_scope();
        }
    }

    fn resolve_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Const(declaration) => self.resolve_const(declaration),
            Statement::Let(declaration) => self.resolve_let(declaration),
            Statement::Assignment(statement) => self.resolve_assignment(statement),
            Statement::If(statement) => self.resolve_if(statement),
            Statement::While(statement) => self.resolve_while(statement),
            Statement::ForOf(statement) => self.resolve_for(statement),
            Statement::Return(statement) => self.resolve_return(statement),
            Statement::Expression(statement) => self.resolve_expression(&statement.expression),
            Statement::Break(_) | Statement::Continue(_) => {}
        }
    }

    fn resolve_const(&mut self, declaration: &ConstDeclaration) {
        if let Some(annotation) = &declaration.annotation {
            self.resolve_type(annotation);
        }
        self.resolve_expression(&declaration.initializer);
        self.bind(&declaration.name, false);
    }

    fn resolve_let(&mut self, declaration: &LetDeclaration) {
        if let Some(annotation) = &declaration.annotation {
            self.resolve_type(annotation);
        }
        self.resolve_expression(&declaration.initializer);
        self.bind(&declaration.name, true);
    }

    fn resolve_assignment(&mut self, statement: &AssignmentStatement) {
        self.resolve_value_name(&statement.target, "value", "this scope");
        self.resolve_expression(&statement.value);
    }

    fn resolve_if(&mut self, statement: &IfStatement) {
        self.resolve_expression(&statement.condition);
        self.resolve_block(&statement.then_branch, true);
        if let Some(branch) = &statement.else_branch {
            self.resolve_block(branch, true);
        }
    }

    fn resolve_while(&mut self, statement: &WhileStatement) {
        self.resolve_expression(&statement.condition);
        self.resolve_block(&statement.body, true);
    }

    fn resolve_for(&mut self, statement: &ForOfStatement) {
        self.resolve_expression(&statement.iterable);
        self.push_scope(ResolverScopeKind::For, statement.body.span);
        self.bind(&statement.binding, false);
        self.resolve_block(&statement.body, false);
        self.pop_scope();
    }

    fn resolve_return(&mut self, statement: &ReturnStatement) {
        if let Some(value) = &statement.value {
            self.resolve_expression(value);
        }
    }

    fn resolve_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::Integer { .. } | Expression::Boolean { .. } | Expression::String { .. } => {
            }
            Expression::Array { elements, .. } => {
                for element in elements {
                    self.resolve_expression(element);
                }
            }
            Expression::Record { fields, .. } => {
                for field in fields {
                    self.resolve_expression(&field.value);
                }
            }
            Expression::Match {
                scrutinee, arms, ..
            } => {
                self.resolve_expression(scrutinee);
                for arm in arms {
                    self.push_scope(ResolverScopeKind::MatchArm, arm.span);
                    if let MatchPattern::Variant {
                        union,
                        variant,
                        bindings,
                        ..
                    } = &arm.pattern
                    {
                        self.resolve_variant(union, variant);
                        for binding in bindings {
                            self.bind(binding, false);
                        }
                    }
                    self.resolve_expression(&arm.value);
                    self.pop_scope();
                }
            }
            Expression::Index {
                collection, index, ..
            } => {
                self.resolve_expression(collection);
                self.resolve_expression(index);
            }
            Expression::Member { object, .. } => self.resolve_expression(object),
            Expression::Name(name) => self.resolve_value_name(name, "value", "this scope"),
            Expression::Arrow {
                parameters,
                return_type,
                body,
                span,
                ..
            } => {
                self.push_scope(ResolverScopeKind::Closure, *span);
                for parameter in parameters {
                    self.resolve_type(&parameter.ty);
                    self.bind(&parameter.name, false);
                }
                self.resolve_type(return_type);
                match body {
                    ArrowBody::Expression(expression) => self.resolve_expression(expression),
                    ArrowBody::Block(block) => self.resolve_block(block, false),
                }
                self.pop_scope();
            }
            Expression::Unary { expression, .. } | Expression::Parenthesized { expression, .. } => {
                self.resolve_expression(expression);
            }
            Expression::Binary { left, right, .. } => {
                self.resolve_expression(left);
                self.resolve_expression(right);
            }
            Expression::Call {
                callee, arguments, ..
            } => {
                match callee.as_ref() {
                    Expression::Name(name) => self.resolve_callable_name(name),
                    Expression::Member { object, member, .. }
                        if matches!(object.as_ref(), Expression::Name(_)) =>
                    {
                        let Expression::Name(qualifier) = object.as_ref() else {
                            return;
                        };
                        if !self.resolve_variant(qualifier, member) {
                            self.resolve_expression(object);
                        }
                    }
                    _ => self.resolve_expression(callee),
                }
                for argument in arguments {
                    self.resolve_expression(argument);
                }
            }
        }
    }

    fn resolve_callable_name(&mut self, name: &Name) {
        if let Some(local) = self.lookup_local(&name.text) {
            self.record_name(name.span, NameResolution::Local(local.local));
            return;
        }
        let builtin = match name.text.as_str() {
            "print" => Some(Builtin::Print),
            "toString" => Some(Builtin::ToString),
            "parseInt" => Some(Builtin::ParseInt),
            _ => None,
        };
        if let Some(builtin) = builtin {
            self.record_name(name.span, NameResolution::Builtin(builtin));
            return;
        }
        if let Some(entry) = self.environment.values.get(&name.text).copied() {
            self.record_name(name.span, definition_resolution(entry.definition));
            return;
        }
        self.undefined(name, "function", "this program");
    }

    fn resolve_value_name(&mut self, name: &Name, kind: &str, place: &str) {
        if let Some(local) = self.lookup_local(&name.text) {
            self.record_name(name.span, NameResolution::Local(local.local));
        } else if let Some(entry) = self.environment.values.get(&name.text).copied() {
            self.record_name(name.span, definition_resolution(entry.definition));
        } else {
            self.undefined(name, kind, place);
        }
    }

    fn resolve_type_name(&mut self, name: &Name) {
        if let Some((parameter, _)) = self.type_parameters.get(&name.text).copied() {
            self.record_name(name.span, NameResolution::TypeParameter(parameter));
        } else if let Some(entry) = self.environment.types.get(&name.text).copied() {
            self.record_name(name.span, definition_resolution(entry.definition));
        } else {
            self.undefined(name, "type", "this module");
        }
    }

    fn resolve_type_parameters(&mut self, parameters: &[TypeParameter]) {
        for (index, parameter) in parameters.iter().enumerate() {
            let id = TypeParameterId::new(TypeParameterOwner::Function(self.owner), index);
            self.record_name(parameter.name.span, NameResolution::TypeParameter(id));
            if let Some((_, previous)) = self.type_parameters.get(&parameter.name.text).copied() {
                self.diagnostics.push(
                    Diagnostic::error(
                        DUPLICATE_NAME,
                        format!("duplicate type parameter `{}`", parameter.name.text),
                    )
                    .with_label(Label::primary(
                        parameter.name.span,
                        "duplicate type parameter",
                    ))
                    .with_label(Label::secondary(previous, "first declared here")),
                );
            } else {
                self.type_parameters
                    .insert(parameter.name.text.clone(), (id, parameter.name.span));
            }
        }
    }

    fn resolve_type(&mut self, reference: &TypeReference) {
        match &reference.kind {
            TypeReferenceKind::Named { name, arguments } => {
                self.resolve_type_name(name);
                for argument in arguments {
                    self.resolve_type(argument);
                }
            }
            TypeReferenceKind::Array(element) => self.resolve_type(element),
            TypeReferenceKind::Function {
                parameters,
                return_type,
            } => {
                for parameter in parameters {
                    self.resolve_type(&parameter.ty);
                }
                self.resolve_type(return_type);
            }
            TypeReferenceKind::Int
            | TypeReferenceKind::Bool
            | TypeReferenceKind::String
            | TypeReferenceKind::Unit => {}
        }
    }

    fn resolve_variant(&mut self, qualifier: &Name, variant: &Name) -> bool {
        if self.lookup_local(&qualifier.text).is_some() {
            return false;
        }
        if let Some((parameter, _)) = self.type_parameters.get(&qualifier.text).copied() {
            self.record_name(qualifier.span, NameResolution::TypeParameter(parameter));
            return true;
        }
        let Some(entry) = self.environment.types.get(&qualifier.text).copied() else {
            return false;
        };
        self.record_name(qualifier.span, definition_resolution(entry.definition));
        let DefId::Union(union) = entry.definition else {
            return true;
        };
        if let Some((variant_id, _)) = find_variant(self.programs, union, &variant.text) {
            self.record_name(variant.span, NameResolution::Variant(variant_id));
        }
        true
    }

    fn bind(&mut self, name: &Name, mutable: bool) {
        let Some(scope) = self.stack.last() else {
            return;
        };
        if let Some(previous) = scope.names.get(&name.text).copied() {
            self.diagnostics.push(
                Diagnostic::error(DUPLICATE_NAME, format!("duplicate binding `{}`", name.text))
                    .with_label(Label::primary(name.span, "duplicate binding"))
                    .with_label(Label::secondary(previous.span, "first declared here")),
            );
            return;
        }

        let local = LocalId::new(self.next_local);
        self.next_local += 1;
        let scope_id = scope.id;
        if let Some(scope) = self.stack.last_mut() {
            scope.names.insert(
                name.text.clone(),
                LocalEntry {
                    local,
                    span: name.span,
                },
            );
        }
        self.bindings.push(ResolvedBinding {
            owner: self.owner,
            local,
            scope: scope_id,
            name_span: name.span,
            mutable,
        });
        self.record_name(name.span, NameResolution::Local(local));
    }

    fn lookup_local(&self, name: &str) -> Option<LocalEntry> {
        self.stack
            .iter()
            .rev()
            .find_map(|scope| scope.names.get(name).copied())
    }

    fn record_name(&mut self, span: SourceSpan, resolution: NameResolution) {
        self.names.push(ResolvedName {
            span,
            owner: Some(self.owner),
            resolution,
        });
    }

    fn undefined(&mut self, name: &Name, kind: &str, place: &str) {
        self.diagnostics.push(
            Diagnostic::error(UNDEFINED_NAME, format!("undefined {kind} `{}`", name.text))
                .with_label(Label::primary(name.span, format!("not found in {place}"))),
        );
    }
}

fn find_variant(
    programs: &[Program],
    union: UnionId,
    name: &str,
) -> Option<(VariantId, SourceSpan)> {
    let program = programs
        .iter()
        .find(|program| program.module == union.module())?;
    let declaration = program.unions.get(union.index())?;
    declaration
        .variants
        .iter()
        .enumerate()
        .find(|(_, variant)| variant.name.text == name)
        .map(|(index, variant)| (VariantId::new(union, index), variant.name.span))
}

fn resolve_data_declarations(
    programs: &[Program],
    environments: &HashMap<ModuleId, Namespace>,
    names: &mut Vec<ResolvedName>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for program in programs {
        let Some(environment) = environments.get(&program.module) else {
            continue;
        };
        for (record_index, record) in program.records.iter().enumerate() {
            let record_id = RecordId::in_module(program.module, record_index);
            let parameters = resolve_owned_type_parameters(
                TypeParameterOwner::Record(record_id),
                &record.type_parameters,
                names,
                diagnostics,
            );
            let mut declared_fields = HashMap::new();
            for (field_index, field) in record.fields.iter().enumerate() {
                names.push(ResolvedName {
                    span: field.name.span,
                    owner: None,
                    resolution: NameResolution::Field(FieldId::new(record_id, field_index)),
                });
                diagnose_nested_duplicate("field", &field.name, &mut declared_fields, diagnostics);
                resolve_owned_type(&field.ty, environment, &parameters, names, diagnostics);
            }
        }
        for (union_index, union) in program.unions.iter().enumerate() {
            let union_id = UnionId::in_module(program.module, union_index);
            let parameters = resolve_owned_type_parameters(
                TypeParameterOwner::Union(union_id),
                &union.type_parameters,
                names,
                diagnostics,
            );
            let mut declared_variants = HashMap::new();
            for (variant_index, variant) in union.variants.iter().enumerate() {
                let variant_id = VariantId::new(union_id, variant_index);
                names.push(ResolvedName {
                    span: variant.name.span,
                    owner: None,
                    resolution: NameResolution::Variant(variant_id),
                });
                diagnose_nested_duplicate(
                    "variant",
                    &variant.name,
                    &mut declared_variants,
                    diagnostics,
                );
                let mut declared_payloads = HashMap::new();
                for (payload_index, payload) in variant.payloads.iter().enumerate() {
                    names.push(ResolvedName {
                        span: payload.name.span,
                        owner: None,
                        resolution: NameResolution::Payload(PayloadId::new(
                            variant_id,
                            payload_index,
                        )),
                    });
                    diagnose_nested_duplicate(
                        "payload",
                        &payload.name,
                        &mut declared_payloads,
                        diagnostics,
                    );
                    resolve_owned_type(&payload.ty, environment, &parameters, names, diagnostics);
                }
            }
        }
    }
}

fn resolve_owned_type_parameters(
    owner: TypeParameterOwner,
    parameters: &[TypeParameter],
    names: &mut Vec<ResolvedName>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<String, (TypeParameterId, SourceSpan)> {
    let mut resolved = HashMap::new();
    for (index, parameter) in parameters.iter().enumerate() {
        let id = TypeParameterId::new(owner, index);
        names.push(ResolvedName {
            span: parameter.name.span,
            owner: None,
            resolution: NameResolution::TypeParameter(id),
        });
        if let Some((_, previous)) = resolved.get(&parameter.name.text).copied() {
            diagnostics.push(
                Diagnostic::error(
                    DUPLICATE_NAME,
                    format!("duplicate type parameter `{}`", parameter.name.text),
                )
                .with_label(Label::primary(
                    parameter.name.span,
                    "duplicate type parameter",
                ))
                .with_label(Label::secondary(previous, "first declared here")),
            );
        } else {
            resolved.insert(parameter.name.text.clone(), (id, parameter.name.span));
        }
    }
    resolved
}

fn resolve_owned_type(
    reference: &TypeReference,
    environment: &Namespace,
    parameters: &HashMap<String, (TypeParameterId, SourceSpan)>,
    names: &mut Vec<ResolvedName>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &reference.kind {
        TypeReferenceKind::Named { name, arguments } => {
            if let Some((parameter, _)) = parameters.get(&name.text).copied() {
                names.push(ResolvedName {
                    span: name.span,
                    owner: None,
                    resolution: NameResolution::TypeParameter(parameter),
                });
            } else if let Some(entry) = environment.types.get(&name.text).copied() {
                names.push(ResolvedName {
                    span: name.span,
                    owner: None,
                    resolution: definition_resolution(entry.definition),
                });
            } else {
                diagnostics.push(
                    Diagnostic::error(UNDEFINED_NAME, format!("undefined type `{}`", name.text))
                        .with_label(Label::primary(name.span, "not found in this module")),
                );
            }
            for argument in arguments {
                resolve_owned_type(argument, environment, parameters, names, diagnostics);
            }
        }
        TypeReferenceKind::Array(element) => {
            resolve_owned_type(element, environment, parameters, names, diagnostics);
        }
        TypeReferenceKind::Function {
            parameters: function_parameters,
            return_type,
        } => {
            for parameter in function_parameters {
                resolve_owned_type(&parameter.ty, environment, parameters, names, diagnostics);
            }
            resolve_owned_type(return_type, environment, parameters, names, diagnostics);
        }
        TypeReferenceKind::Int
        | TypeReferenceKind::Bool
        | TypeReferenceKind::String
        | TypeReferenceKind::Unit => {}
    }
}

fn diagnose_nested_duplicate(
    kind: &str,
    name: &Name,
    declared: &mut HashMap<String, SourceSpan>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(previous) = declared.get(&name.text).copied() {
        diagnostics.push(
            Diagnostic::error(DUPLICATE_NAME, format!("duplicate {kind} `{}`", name.text))
                .with_label(Label::primary(name.span, format!("duplicate {kind}")))
                .with_label(Label::secondary(previous, "first declared here")),
        );
    } else {
        declared.insert(name.text.clone(), name.span);
    }
}

fn collect_module_catalog(
    program: &Program,
    symbols: &mut Vec<ResolverSymbol>,
    names: &mut Vec<ResolvedName>,
    diagnostics: &mut Vec<Diagnostic>,
) -> ModuleCatalog {
    let mut declarations = program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            (
                function.name.text.clone(),
                CatalogEntry {
                    definition: DefId::Function(FunctionId::in_module(program.module, index)),
                    span: function.name.span,
                    visibility: function.visibility,
                },
            )
        })
        .chain(program.records.iter().enumerate().map(|(index, record)| {
            (
                record.name.text.clone(),
                CatalogEntry {
                    definition: DefId::Record(RecordId::in_module(program.module, index)),
                    span: record.name.span,
                    visibility: record.visibility,
                },
            )
        }))
        .chain(program.unions.iter().enumerate().map(|(index, union)| {
            (
                union.name.text.clone(),
                CatalogEntry {
                    definition: DefId::Union(UnionId::in_module(program.module, index)),
                    span: union.name.span,
                    visibility: union.visibility,
                },
            )
        }))
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(_, entry)| span_position(entry.span));

    let mut local = Namespace::default();
    let mut declared = HashMap::new();
    let mut bindings = Vec::with_capacity(declarations.len());
    for (name, entry) in &declarations {
        symbols.push(ResolverSymbol {
            definition: entry.definition,
            name_span: entry.span,
            visibility: entry.visibility,
        });
        names.push(ResolvedName {
            span: entry.span,
            owner: None,
            resolution: definition_resolution(entry.definition),
        });
        declared.entry(name.clone()).or_insert(entry.span);
        bindings.push(BindingEvent {
            name: name.clone(),
            span: entry.span,
            entry: *entry,
        });
        namespace_mut(&mut local, entry.definition)
            .entry(name.clone())
            .or_insert(*entry);
    }

    let mut exports = HashMap::<String, CatalogEntry>::new();
    for (name, entry) in declarations {
        if entry.visibility != Visibility::Exported
            || !namespace(&local, entry.definition)
                .get(&name)
                .is_some_and(|accepted| accepted.definition == entry.definition)
        {
            continue;
        }
        if let Some(previous) = exports.get(&name).copied() {
            diagnostics.push(
                Diagnostic::error(
                    EXPORT_COLLISION,
                    format!("export name `{name}` is ambiguous"),
                )
                .with_label(Label::primary(entry.span, "conflicting export"))
                .with_label(Label::secondary(previous.span, "first exported here")),
            );
        } else {
            exports.insert(name, entry);
        }
    }

    ModuleCatalog {
        declared,
        exports,
        bindings,
    }
}

fn resolve_environments(
    programs: &[Program],
    links: &[ResolvedImport],
    catalogs: &HashMap<ModuleId, ModuleCatalog>,
    names: &mut Vec<ResolvedName>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<ModuleId, Namespace> {
    let targets = links
        .iter()
        .map(|link| ((link.importer(), link.path_span()), link.target()))
        .collect::<HashMap<_, _>>();

    programs
        .iter()
        .filter_map(|program| {
            let catalog = catalogs.get(&program.module)?;
            let mut events = catalog.bindings.clone();

            for import in &program.imports {
                let Some(target) = targets.get(&(program.module, import.path_span)).copied() else {
                    continue;
                };
                let Some(target_catalog) = catalogs.get(&target) else {
                    continue;
                };
                for name in &import.names {
                    if let Some(export) = target_catalog.exports.get(&name.text).copied() {
                        names.push(ResolvedName {
                            span: name.span,
                            owner: None,
                            resolution: definition_resolution(export.definition),
                        });
                        events.push(BindingEvent {
                            name: name.text.clone(),
                            span: name.span,
                            entry: CatalogEntry {
                                span: name.span,
                                ..export
                            },
                        });
                    } else if let Some(declaration) =
                        target_catalog.declared.get(&name.text).copied()
                    {
                        diagnostics.push(
                            Diagnostic::error(
                                PRIVATE_EXPORT,
                                format!("`{}` is private in the imported module", name.text),
                            )
                            .with_label(Label::primary(name.span, "private import"))
                            .with_label(Label::secondary(declaration, "declared private here")),
                        );
                    } else {
                        diagnostics.push(
                            Diagnostic::error(
                                MISSING_EXPORT,
                                format!("imported module has no declaration named `{}`", name.text),
                            )
                            .with_label(Label::primary(name.span, "missing imported name")),
                        );
                    }
                }
            }

            events.sort_by_key(|event| span_position(event.span));
            let mut environment = Namespace::default();
            for event in events {
                insert_binding(&mut environment, event, diagnostics);
            }
            Some((program.module, environment))
        })
        .collect()
}

fn insert_binding(
    environment: &mut Namespace,
    event: BindingEvent,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let namespace = namespace_mut(environment, event.entry.definition);
    let previous = namespace.get(&event.name).copied();
    if event.name == "print" && matches!(event.entry.definition, DefId::Function(_))
        || previous.is_some()
    {
        let kind = if matches!(event.entry.definition, DefId::Function(_)) {
            "value"
        } else {
            "type"
        };
        let mut diagnostic = Diagnostic::error(
            DUPLICATE_NAME,
            format!("duplicate top-level {kind} `{}`", event.name),
        )
        .with_label(Label::primary(
            event.span,
            format!("duplicate {kind} binding"),
        ));
        if let Some(previous) = previous {
            diagnostic = diagnostic.with_label(Label::secondary(previous.span, "first bound here"));
        }
        diagnostics.push(diagnostic);
    } else {
        namespace.insert(event.name, event.entry);
    }
}

fn namespace(namespace: &Namespace, definition: DefId) -> &HashMap<String, CatalogEntry> {
    match definition {
        DefId::Function(_) => &namespace.values,
        DefId::Record(_) | DefId::Union(_) => &namespace.types,
    }
}

fn namespace_mut(
    namespace: &mut Namespace,
    definition: DefId,
) -> &mut HashMap<String, CatalogEntry> {
    match definition {
        DefId::Function(_) => &mut namespace.values,
        DefId::Record(_) | DefId::Union(_) => &mut namespace.types,
    }
}

const fn definition_resolution(definition: DefId) -> NameResolution {
    match definition {
        DefId::Function(function) => NameResolution::Function(function),
        DefId::Record(record) => NameResolution::Record(record),
        DefId::Union(union) => NameResolution::Union(union),
    }
}

fn span_position(span: SourceSpan) -> (u32, usize, usize) {
    (span.file().raw(), span.range().start(), span.range().end())
}

fn diagnostic_position(diagnostic: &Diagnostic) -> (u32, usize, usize, &str) {
    diagnostic
        .labels()
        .first()
        .map_or((u32::MAX, usize::MAX, usize::MAX, ""), |label| {
            let span = label.span();
            (
                span.file().raw(),
                span.range().start(),
                span.range().end(),
                diagnostic.code().as_str(),
            )
        })
}

const _: DiagnosticCode = UNDEFINED_NAME;
