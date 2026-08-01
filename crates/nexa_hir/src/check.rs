use std::collections::{HashMap, HashSet};

use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::SourceSpan;

use crate::{
    ArrowBody, AssignmentStatement, BinaryOperator, Block, ClosureId, ConstDeclaration, DefId,
    Expression, FieldId, ForOfStatement, Function, FunctionId, IfStatement, LetDeclaration,
    MatchArm, MatchPattern, ModuleId, Name, Parameter, PayloadId, Program, RecordFieldInitializer,
    RecordId, ReturnStatement, Statement, Type, TypeParameter, TypeParameterId, TypeParameterOwner,
    TypeReference, TypeReferenceKind, UnaryOperator, UnionId, VariantId, Visibility,
    WhileStatement,
};

const UNDEFINED_NAME: DiagnosticCode = DiagnosticCode::new("E2001");
const DUPLICATE_NAME: DiagnosticCode = DiagnosticCode::new("E2002");
const CALL_ARITY: DiagnosticCode = DiagnosticCode::new("E2003");
const IMMUTABLE_ASSIGNMENT: DiagnosticCode = DiagnosticCode::new("E2004");
const UNKNOWN_MEMBER: DiagnosticCode = DiagnosticCode::new("E2005");
const MISSING_FIELD: DiagnosticCode = DiagnosticCode::new("E2006");
const TYPE_MISMATCH: DiagnosticCode = DiagnosticCode::new("E3001");
const NON_BOOLEAN_CONDITION: DiagnosticCode = DiagnosticCode::new("E3002");
const INVALID_RETURN: DiagnosticCode = DiagnosticCode::new("E3003");
const INVALID_LOOP_CONTROL: DiagnosticCode = DiagnosticCode::new("E3004");
const RECURSIVE_TYPE: DiagnosticCode = DiagnosticCode::new("E3005");
const NON_EXHAUSTIVE_MATCH: DiagnosticCode = DiagnosticCode::new("E3006");
const UNREACHABLE_ARM: DiagnosticCode = DiagnosticCode::new("E3007");
const UNCONSTRAINED_TYPE_PARAMETER: DiagnosticCode = DiagnosticCode::new("E3008");
const TYPE_INFERENCE: DiagnosticCode = DiagnosticCode::new("E3009");
const GENERIC_LIMIT: DiagnosticCode = DiagnosticCode::new("E3010");
const MUTABLE_CAPTURE: DiagnosticCode = DiagnosticCode::new("E3011");
const GENERIC_FUNCTION_VALUE: DiagnosticCode = DiagnosticCode::new("E3012");
const MISSING_EXPORT: DiagnosticCode = DiagnosticCode::new("E4003");
const PRIVATE_EXPORT: DiagnosticCode = DiagnosticCode::new("E4004");
const EXPORT_COLLISION: DiagnosticCode = DiagnosticCode::new("E4005");
const MAX_GENERIC_INSTANCES: usize = 256;

/// One source import whose target module was resolved by the compiler session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResolvedImport {
    importer: ModuleId,
    path_span: SourceSpan,
    target: ModuleId,
}

impl ResolvedImport {
    /// Creates a resolved edge for the import identified by its path span.
    #[must_use]
    pub const fn new(importer: ModuleId, path_span: SourceSpan, target: ModuleId) -> Self {
        Self {
            importer,
            path_span,
            target,
        }
    }

    /// Returns the module containing the import declaration.
    #[must_use]
    pub const fn importer(self) -> ModuleId {
        self.importer
    }

    /// Returns the exact path-literal span that identifies the import.
    #[must_use]
    pub const fn path_span(self) -> SourceSpan {
        self.path_span
    }

    /// Returns the module selected by source resolution.
    #[must_use]
    pub const fn target(self) -> ModuleId {
        self.target
    }
}

/// The semantic result of type checking a lowered program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    typed: Option<TypedProgram>,
    diagnostics: Vec<Diagnostic>,
}

impl Analysis {
    /// Returns the validated typed HIR when no error diagnostics were produced.
    #[must_use]
    pub fn typed(&self) -> Option<&TypedProgram> {
        self.typed.as_ref()
    }

    /// Returns all semantic diagnostics in source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns true when semantic analysis produced no error diagnostics.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.typed.is_some()
    }
}

/// A HIR program proven to satisfy the Language Core static type rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedProgram {
    modules: Vec<Program>,
    entry_index: usize,
    expression_types: HashMap<SourceSpan, Type>,
    name_resolutions: HashMap<SourceSpan, NameResolution>,
    records: Vec<RecordFacts>,
    unions: Vec<UnionFacts>,
    variant_constructions: HashMap<SourceSpan, VariantConstructionFacts>,
    calls: HashMap<SourceSpan, CallFacts>,
    matches: HashMap<SourceSpan, MatchFacts>,
    functions: Vec<FunctionFacts>,
    closures: Vec<ClosureFacts>,
}

impl TypedProgram {
    /// Returns the validated underlying HIR program.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.modules[self.entry_index]
    }

    /// Returns every checked module in compiler-session order.
    #[must_use]
    pub fn modules(&self) -> &[Program] {
        &self.modules
    }

    /// Returns the stable identity of the entry module.
    #[must_use]
    pub const fn entry(&self) -> ModuleId {
        ModuleId::ENTRY
    }

    /// Returns a checked module by its stable identity.
    #[must_use]
    pub fn module(&self, module: ModuleId) -> Option<&Program> {
        self.modules.iter().find(|program| program.module == module)
    }

    /// Returns a checked source function by its module-aware identity.
    #[must_use]
    pub fn function(&self, function: FunctionId) -> Option<&Function> {
        self.module(function.module())?
            .functions
            .get(function.index())
    }

    /// Returns the resolved type for a value expression at `span`.
    #[must_use]
    pub fn expression_type(&self, span: SourceSpan) -> Option<&Type> {
        self.expression_types.get(&span)
    }

    /// Returns the resolved declaration or builtin for a name token at `span`.
    #[must_use]
    pub fn name_resolution(&self, span: SourceSpan) -> Option<NameResolution> {
        self.name_resolutions.get(&span).copied()
    }

    /// Iterates over all resolved value-expression types.
    pub fn expression_types(&self) -> impl Iterator<Item = (SourceSpan, &Type)> {
        self.expression_types.iter().map(|(span, ty)| (*span, ty))
    }

    /// Iterates over all resolved source-name targets.
    pub fn name_resolutions(&self) -> impl Iterator<Item = (SourceSpan, NameResolution)> + '_ {
        self.name_resolutions
            .iter()
            .map(|(span, resolution)| (*span, *resolution))
    }

    /// Returns local-slot facts for a resolved source function.
    #[must_use]
    pub fn function_facts(&self, function: FunctionId) -> Option<&FunctionFacts> {
        self.functions.iter().find(|facts| facts.id == function)
    }

    /// Returns resolved function signatures and local-slot facts in stable source order.
    #[must_use]
    pub fn functions(&self) -> &[FunctionFacts] {
        &self.functions
    }

    /// Returns semantic facts for one stable arrow-function site.
    #[must_use]
    pub fn closure_facts(&self, closure: ClosureId) -> Option<&ClosureFacts> {
        self.closures.iter().find(|facts| facts.id == closure)
    }

    /// Returns all closure facts in stable owner and source order.
    #[must_use]
    pub fn closures(&self) -> &[ClosureFacts] {
        &self.closures
    }

    /// Returns resolved layout facts for a nominal record.
    #[must_use]
    pub fn record_facts(&self, record: RecordId) -> Option<&RecordFacts> {
        self.records.iter().find(|facts| facts.id == record)
    }

    /// Returns all nominal records in stable source order.
    #[must_use]
    pub fn records(&self) -> &[RecordFacts] {
        &self.records
    }

    /// Returns resolved declaration facts for a nominal tagged union.
    #[must_use]
    pub fn union_facts(&self, union: UnionId) -> Option<&UnionFacts> {
        self.unions.iter().find(|facts| facts.id == union)
    }

    /// Returns all nominal tagged unions in stable source order.
    #[must_use]
    pub fn unions(&self) -> &[UnionFacts] {
        &self.unions
    }

    /// Returns the resolved constructor selected by a call expression.
    #[must_use]
    pub fn variant_construction(&self, span: SourceSpan) -> Option<&VariantConstructionFacts> {
        self.variant_constructions.get(&span)
    }

    /// Iterates over all resolved tagged-union constructor calls.
    pub fn variant_constructions(
        &self,
    ) -> impl Iterator<Item = (SourceSpan, &VariantConstructionFacts)> {
        self.variant_constructions
            .iter()
            .map(|(span, facts)| (*span, facts))
    }

    /// Returns the resolved generic instantiation selected by a function call.
    #[must_use]
    pub fn call_facts(&self, span: SourceSpan) -> Option<&CallFacts> {
        self.calls.get(&span)
    }

    /// Iterates over all resolved direct function calls.
    pub fn calls(&self) -> impl Iterator<Item = (SourceSpan, &CallFacts)> {
        self.calls.iter().map(|(span, facts)| (*span, facts))
    }

    /// Returns resolved control-flow and binding facts for a match expression.
    #[must_use]
    pub fn match_facts(&self, span: SourceSpan) -> Option<&MatchFacts> {
        self.matches.get(&span)
    }

    /// Iterates over all resolved tagged-union match expressions.
    pub fn matches(&self) -> impl Iterator<Item = (SourceSpan, &MatchFacts)> {
        self.matches.iter().map(|(span, facts)| (*span, facts))
    }
}

/// A stable source-order identifier for a local slot within one function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(usize);

impl LocalId {
    /// Creates a local identifier from its zero-based slot index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based slot index within the containing function.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A compiler-provided operation resolved during semantic analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Builtin {
    /// The scalar `print` function.
    Print,
    /// The fixed array `length` member.
    ArrayLength,
    /// The Unicode-scalar string `length` member.
    StringLength,
    /// The immutable array `append` member call.
    ArrayAppend,
    /// The immutable array `concat` member call.
    ArrayConcat,
    /// The canonical integer-to-string conversion.
    ToString,
    /// The complete ASCII decimal string-to-integer conversion.
    ParseInt,
}

/// The semantic target of a source name token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameResolution {
    /// A function-local slot.
    Local(LocalId),
    /// A source function.
    Function(FunctionId),
    /// A nominal record declaration or named type reference.
    Record(RecordId),
    /// A declared, initialized, or projected record field.
    Field(FieldId),
    /// A nominal tagged union declaration or named type reference.
    Union(UnionId),
    /// A tagged union variant declaration, constructor, or pattern.
    Variant(VariantId),
    /// A named positional payload declaration.
    Payload(PayloadId),
    /// A generic type parameter declaration or reference.
    TypeParameter(TypeParameterId),
    /// A compiler-provided operation.
    Builtin(Builtin),
}

/// Slot allocation facts for one validated function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFacts {
    id: FunctionId,
    type_parameters: Vec<TypeParameterFacts>,
    parameters: Vec<LocalId>,
    parameter_types: Vec<Type>,
    return_type: Type,
    local_count: usize,
}

/// Resolved signature, local slots, and by-value captures for one arrow site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureFacts {
    id: ClosureId,
    parameters: Vec<LocalId>,
    parameter_types: Vec<Type>,
    return_type: Type,
    captures: Vec<LocalId>,
}

impl ClosureFacts {
    /// Returns the stable source-order closure identity.
    #[must_use]
    pub const fn id(&self) -> ClosureId {
        self.id
    }

    /// Returns closure parameter slots in declaration order.
    #[must_use]
    pub fn parameter_ids(&self) -> &[LocalId] {
        &self.parameters
    }

    /// Returns closure parameter types in declaration order.
    #[must_use]
    pub fn parameter_types(&self) -> &[Type] {
        &self.parameter_types
    }

    /// Returns the declared closure result type.
    #[must_use]
    pub const fn return_type(&self) -> &Type {
        &self.return_type
    }

    /// Returns captured owner-local slots in deterministic first-use order.
    #[must_use]
    pub fn captures(&self) -> &[LocalId] {
        &self.captures
    }
}

impl FunctionFacts {
    /// Returns this function's stable module-owned identifier.
    #[must_use]
    pub const fn id(&self) -> FunctionId {
        self.id
    }

    /// Returns generic parameters in declaration order.
    #[must_use]
    pub fn type_parameters(&self) -> &[TypeParameterFacts] {
        &self.type_parameters
    }

    /// Returns parameter slots in declaration order.
    #[must_use]
    pub fn parameter_ids(&self) -> &[LocalId] {
        &self.parameters
    }

    /// Returns resolved parameter types in declaration order.
    #[must_use]
    pub fn parameter_types(&self) -> &[Type] {
        &self.parameter_types
    }

    /// Returns the resolved function result type.
    #[must_use]
    pub const fn return_type(&self) -> &Type {
        &self.return_type
    }

    /// Returns the number of parameter and local slots used by the function.
    #[must_use]
    pub const fn local_count(&self) -> usize {
        self.local_count
    }
}

/// Resolved semantic facts for one nominal record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFacts {
    id: RecordId,
    name: String,
    type_parameters: Vec<TypeParameterFacts>,
    fields: Vec<RecordFieldFacts>,
    name_span: SourceSpan,
    span: SourceSpan,
}

impl RecordFacts {
    /// Returns this record's stable identifier.
    #[must_use]
    pub const fn id(&self) -> RecordId {
        self.id
    }

    /// Returns the declared record name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns generic parameters in declaration order.
    #[must_use]
    pub fn type_parameters(&self) -> &[TypeParameterFacts] {
        &self.type_parameters
    }

    /// Returns fields in declaration order.
    #[must_use]
    pub fn fields(&self) -> &[RecordFieldFacts] {
        &self.fields
    }

    /// Returns the declared record-name token range.
    #[must_use]
    pub const fn name_span(&self) -> SourceSpan {
        self.name_span
    }

    /// Returns the record declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// Resolved semantic facts for one immutable record field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFieldFacts {
    id: FieldId,
    name: String,
    ty: Type,
    span: SourceSpan,
    type_span: SourceSpan,
}

/// Resolved semantic facts for one nominal tagged union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionFacts {
    id: UnionId,
    name: String,
    type_parameters: Vec<TypeParameterFacts>,
    variants: Vec<VariantFacts>,
    name_span: SourceSpan,
    span: SourceSpan,
}

impl UnionFacts {
    /// Returns this union's stable identifier.
    #[must_use]
    pub const fn id(&self) -> UnionId {
        self.id
    }

    /// Returns the declared union name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns generic parameters in declaration order.
    #[must_use]
    pub fn type_parameters(&self) -> &[TypeParameterFacts] {
        &self.type_parameters
    }

    /// Returns variants in declaration order.
    #[must_use]
    pub fn variants(&self) -> &[VariantFacts] {
        &self.variants
    }

    /// Returns the declared union-name token range.
    #[must_use]
    pub const fn name_span(&self) -> SourceSpan {
        self.name_span
    }

    /// Returns the union declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// Resolved semantic facts for one tagged union variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantFacts {
    id: VariantId,
    name: String,
    payloads: Vec<PayloadFacts>,
    name_span: SourceSpan,
    span: SourceSpan,
}

impl VariantFacts {
    /// Returns this variant's owner-scoped identifier.
    #[must_use]
    pub const fn id(&self) -> VariantId {
        self.id
    }

    /// Returns the declared variant name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns positional payloads in declaration order.
    #[must_use]
    pub fn payloads(&self) -> &[PayloadFacts] {
        &self.payloads
    }

    /// Returns the declared variant-name token range.
    #[must_use]
    pub const fn name_span(&self) -> SourceSpan {
        self.name_span
    }

    /// Returns the variant declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// Resolved semantic facts for one named positional variant payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadFacts {
    id: PayloadId,
    name: String,
    ty: Type,
    span: SourceSpan,
    type_span: SourceSpan,
}

impl PayloadFacts {
    /// Returns this payload's owner-scoped positional identifier.
    #[must_use]
    pub const fn id(&self) -> PayloadId {
        self.id
    }

    /// Returns the payload declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the resolved payload type.
    #[must_use]
    pub const fn ty(&self) -> &Type {
        &self.ty
    }

    /// Returns the payload declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    /// Returns the payload type-syntax range.
    #[must_use]
    pub const fn type_span(&self) -> SourceSpan {
        self.type_span
    }
}

/// Resolved constructor identity for one qualified variant call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantConstructionFacts {
    union: UnionId,
    variant: VariantId,
    type_arguments: Box<[Type]>,
}

impl VariantConstructionFacts {
    /// Returns the constructed union.
    #[must_use]
    pub const fn union(&self) -> UnionId {
        self.union
    }

    /// Returns the selected variant.
    #[must_use]
    pub const fn variant(&self) -> VariantId {
        self.variant
    }

    /// Returns inferred arguments for the constructed union instance.
    #[must_use]
    pub fn type_arguments(&self) -> &[Type] {
        &self.type_arguments
    }
}

/// Resolved instantiation facts for one direct function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallFacts {
    function: FunctionId,
    type_arguments: Box<[Type]>,
}

impl CallFacts {
    /// Returns the source function definition selected by this call.
    #[must_use]
    pub const fn function(&self) -> FunctionId {
        self.function
    }

    /// Returns locally inferred type arguments in declaration order.
    #[must_use]
    pub fn type_arguments(&self) -> &[Type] {
        &self.type_arguments
    }
}

/// Resolved facts for one declared generic type parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameterFacts {
    id: TypeParameterId,
    name: String,
    span: SourceSpan,
}

impl TypeParameterFacts {
    /// Returns the owner-scoped parameter identity.
    #[must_use]
    pub const fn id(&self) -> TypeParameterId {
        self.id
    }

    /// Returns the source-declared parameter name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parameter declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// Resolved variant dispatch and arm-binding facts for one match expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchFacts {
    union: UnionId,
    type_arguments: Box<[Type]>,
    arms: Vec<MatchArmFacts>,
}

impl MatchFacts {
    /// Returns the matched nominal union.
    #[must_use]
    pub const fn union(&self) -> UnionId {
        self.union
    }

    /// Returns type arguments for the matched union instance.
    #[must_use]
    pub fn type_arguments(&self) -> &[Type] {
        &self.type_arguments
    }

    /// Returns arm facts in source order.
    #[must_use]
    pub fn arms(&self) -> &[MatchArmFacts] {
        &self.arms
    }
}

/// Resolved facts for one source match arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchArmFacts {
    /// A resolved variant arm with positional payload bindings.
    Variant {
        /// The selected variant.
        variant: VariantId,
        /// Payload-to-local bindings in declaration order.
        bindings: Vec<PayloadBindingFacts>,
    },
    /// A catch-all arm.
    Default,
}

/// One resolved pattern payload binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadBindingFacts {
    payload: PayloadId,
    local: LocalId,
}

impl PayloadBindingFacts {
    /// Returns the projected payload.
    #[must_use]
    pub const fn payload(self) -> PayloadId {
        self.payload
    }

    /// Returns the immutable local receiving the payload value.
    #[must_use]
    pub const fn local(self) -> LocalId {
        self.local
    }
}

impl RecordFieldFacts {
    /// Returns this field's stable identifier.
    #[must_use]
    pub const fn id(&self) -> FieldId {
        self.id
    }

    /// Returns the declared field name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the resolved field type.
    #[must_use]
    pub const fn ty(&self) -> &Type {
        &self.ty
    }

    /// Returns the field declaration's source range.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    /// Returns the field type-syntax range.
    #[must_use]
    pub const fn type_span(&self) -> SourceSpan {
        self.type_span
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GenericInstance {
    definition: DefId,
    arguments: Box<[Type]>,
}

#[derive(Clone)]
struct InstanceOccurrence {
    instance: GenericInstance,
    span: SourceSpan,
}

struct GenericCallEdge {
    caller: FunctionId,
    callee: FunctionId,
    arguments: Box<[Type]>,
    span: SourceSpan,
}

#[derive(Default)]
struct FactBuilder {
    expression_types: HashMap<SourceSpan, Type>,
    name_resolutions: HashMap<SourceSpan, NameResolution>,
    records: Vec<RecordFacts>,
    unions: Vec<UnionFacts>,
    variant_constructions: HashMap<SourceSpan, VariantConstructionFacts>,
    calls: HashMap<SourceSpan, CallFacts>,
    matches: HashMap<SourceSpan, MatchFacts>,
    functions: Vec<FunctionFacts>,
    closures: Vec<ClosureFacts>,
    generic_calls: Vec<GenericCallEdge>,
    instances: Vec<InstanceOccurrence>,
}

impl FactBuilder {
    fn record_expression(&mut self, span: SourceSpan, ty: Type) {
        let _ = self.expression_types.insert(span, ty);
    }

    fn record_name(&mut self, span: SourceSpan, resolution: NameResolution) {
        let _ = self.name_resolutions.insert(span, resolution);
    }

    fn record_variant_construction(
        &mut self,
        span: SourceSpan,
        construction: VariantConstructionFacts,
    ) {
        let _ = self.variant_constructions.insert(span, construction);
    }

    fn record_call(&mut self, span: SourceSpan, call: CallFacts) {
        let _ = self.calls.insert(span, call);
    }

    fn record_instance(&mut self, definition: DefId, arguments: &[Type], span: SourceSpan) {
        if arguments.is_empty() || arguments.iter().any(type_contains_parameter) {
            return;
        }
        self.instances.push(InstanceOccurrence {
            instance: GenericInstance {
                definition,
                arguments: arguments.to_vec().into_boxed_slice(),
            },
            span,
        });
    }

    fn record_match(&mut self, span: SourceSpan, match_facts: MatchFacts) {
        let _ = self.matches.insert(span, match_facts);
    }
}

/// Resolves names and validates static semantics for a lowered program.
#[must_use]
pub fn type_check(program: &Program) -> Analysis {
    type_check_modules(std::slice::from_ref(program), &[])
}

/// Resolves imports and validates static semantics for a complete module graph.
#[must_use]
pub fn type_check_modules(programs: &[Program], links: &[ResolvedImport]) -> Analysis {
    let mut diagnostics = Vec::new();
    let mut facts = FactBuilder::default();
    let local_modules = programs
        .iter()
        .map(|program| {
            (
                program.module,
                collect_local_module_catalog(program, &mut diagnostics, &mut facts),
            )
        })
        .collect::<HashMap<_, _>>();
    let environments = resolve_module_environments(
        programs,
        links,
        &local_modules,
        &mut diagnostics,
        &mut facts,
    );

    let mut records = Vec::new();
    for program in programs {
        let Some(environment) = environments.get(&program.module) else {
            continue;
        };
        records.extend(collect_record_facts(
            program,
            &environment.types,
            &mut diagnostics,
            &mut facts,
        ));
    }

    let mut unions = Vec::new();
    for program in programs {
        let Some(environment) = environments.get(&program.module) else {
            continue;
        };
        unions.extend(collect_union_facts(
            program,
            &environment.types,
            &mut diagnostics,
            &mut facts,
        ));
    }

    let unguarded_record_edges = reject_recursive_records(&records, &mut diagnostics);
    let functions =
        collect_function_signatures(programs, &environments, &mut diagnostics, &mut facts);

    for program in programs {
        let Some(environment) = environments.get(&program.module) else {
            continue;
        };
        for (index, function) in program.functions.iter().enumerate() {
            check_function(
                function,
                FunctionId::in_module(program.module, index),
                &functions,
                &environment.types,
                &records,
                &unions,
                &mut diagnostics,
                &mut facts,
            );
        }
    }

    reject_non_regular_generic_types(&records, &unions, &unguarded_record_edges, &mut diagnostics);
    reject_non_regular_generic_calls(&facts.generic_calls, &functions, &mut diagnostics);
    let instances =
        collect_closed_generic_instances(&facts.instances, programs, &records, &unions, &functions);
    reject_invalid_generic_instances(&instances, &records, &unions, &functions, &mut diagnostics);
    reject_generic_instance_budget(&instances, &records, &unions, &functions, &mut diagnostics);

    diagnostics.sort_by(|left, right| diagnostic_position(left).cmp(&diagnostic_position(right)));
    facts.records = records;
    facts.unions = unions;
    facts.closures.sort_by_key(|closure| {
        (
            closure.id.owner().module().index(),
            closure.id.owner().index(),
            closure.id.source_index(),
        )
    });
    let entry_index = programs
        .iter()
        .position(|program| program.module == ModuleId::ENTRY);
    let typed = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity() != nexa_diagnostics::Severity::Error)
        .then_some(entry_index)
        .flatten()
        .map(|entry_index| TypedProgram {
            modules: programs.to_vec(),
            entry_index,
            expression_types: facts.expression_types,
            name_resolutions: facts.name_resolutions,
            records: facts.records,
            unions: facts.unions,
            variant_constructions: facts.variant_constructions,
            calls: facts.calls,
            matches: facts.matches,
            functions: facts.functions,
            closures: facts.closures,
        });

    Analysis { typed, diagnostics }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeSymbol {
    Record(RecordId),
    Union(UnionId),
}

impl TypeSymbol {
    const fn resolution(self) -> NameResolution {
        match self {
            Self::Record(record) => NameResolution::Record(record),
            Self::Union(union) => NameResolution::Union(union),
        }
    }

    fn ty(self, arguments: Box<[Type]>) -> Type {
        match self {
            Self::Record(record) => Type::Record {
                definition: record,
                arguments,
            },
            Self::Union(union) => Type::Union {
                definition: union,
                arguments,
            },
        }
    }

    const fn definition(self) -> DefId {
        match self {
            Self::Record(record) => DefId::Record(record),
            Self::Union(union) => DefId::Union(union),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TypeEntry {
    symbol: TypeSymbol,
    name_span: SourceSpan,
    parameter_count: usize,
}

type TypeCatalog = HashMap<String, TypeEntry>;

#[derive(Debug, Clone, Copy)]
struct TypeParameterEntry {
    id: TypeParameterId,
    span: SourceSpan,
}

type TypeParameterCatalog = HashMap<String, TypeParameterEntry>;

#[derive(Debug, Clone, Copy)]
struct FunctionNameEntry {
    id: FunctionId,
    name_span: SourceSpan,
}

type FunctionNameCatalog = HashMap<String, FunctionNameEntry>;

#[derive(Debug, Clone, Copy)]
struct ExportEntry {
    definition: DefId,
    name_span: SourceSpan,
    parameter_count: usize,
}

struct LocalModuleCatalog {
    declared_names: HashMap<String, SourceSpan>,
    exports: HashMap<String, ExportEntry>,
}

struct ModuleEnvironment {
    types: TypeCatalog,
    functions: FunctionNameCatalog,
}

struct BindingEvent {
    name: String,
    span: SourceSpan,
    definition: DefId,
    parameter_count: usize,
}

fn collect_type_names(program: &Program, facts: &mut FactBuilder) -> TypeCatalog {
    let mut declarations = program
        .records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            (
                &record.name,
                TypeSymbol::Record(RecordId::in_module(program.module, index)),
                record.type_parameters.len(),
            )
        })
        .chain(program.unions.iter().enumerate().map(|(index, union)| {
            (
                &union.name,
                TypeSymbol::Union(UnionId::in_module(program.module, index)),
                union.type_parameters.len(),
            )
        }))
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(name, _, _)| (name.span.file().raw(), name.span.range().start()));

    let mut types = TypeCatalog::new();
    for (name, symbol, parameter_count) in declarations {
        facts.record_name(name.span, symbol.resolution());

        if !types.contains_key(&name.text) {
            types.insert(
                name.text.clone(),
                TypeEntry {
                    symbol,
                    name_span: name.span,
                    parameter_count,
                },
            );
        }
    }

    types
}

fn collect_local_module_catalog(
    program: &Program,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> LocalModuleCatalog {
    let types = collect_type_names(program, facts);
    let functions = collect_function_names(program, facts);
    let declared_names = collect_declared_names(program);
    let exports = collect_exports(program, &types, &functions, diagnostics);

    LocalModuleCatalog {
        declared_names,
        exports,
    }
}

fn collect_function_names(program: &Program, facts: &mut FactBuilder) -> FunctionNameCatalog {
    let mut functions = FunctionNameCatalog::new();

    for (index, function) in program.functions.iter().enumerate() {
        let id = FunctionId::in_module(program.module, index);
        facts.record_name(function.name.span, NameResolution::Function(id));
        if !functions.contains_key(&function.name.text) {
            functions.insert(
                function.name.text.clone(),
                FunctionNameEntry {
                    id,
                    name_span: function.name.span,
                },
            );
        }
    }

    functions
}

fn collect_declared_names(program: &Program) -> HashMap<String, SourceSpan> {
    let mut declarations = program
        .records
        .iter()
        .map(|record| &record.name)
        .chain(program.unions.iter().map(|union| &union.name))
        .chain(program.functions.iter().map(|function| &function.name))
        .collect::<Vec<_>>();
    declarations.sort_by_key(|name| span_position(name.span));

    let mut names = HashMap::new();
    for name in declarations {
        names.entry(name.text.clone()).or_insert(name.span);
    }
    names
}

fn collect_exports(
    program: &Program,
    types: &TypeCatalog,
    functions: &FunctionNameCatalog,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<String, ExportEntry> {
    let record_exports = program
        .records
        .iter()
        .enumerate()
        .filter(|(_, record)| record.visibility == Visibility::Exported)
        .filter_map(|(index, record)| {
            let definition = DefId::Record(RecordId::in_module(program.module, index));
            types
                .get(&record.name.text)
                .is_some_and(|entry| entry.symbol.definition() == definition)
                .then_some((&record.name, definition, record.type_parameters.len()))
        });
    let union_exports = program
        .unions
        .iter()
        .enumerate()
        .filter(|(_, union)| union.visibility == Visibility::Exported)
        .filter_map(|(index, union)| {
            let definition = DefId::Union(UnionId::in_module(program.module, index));
            types
                .get(&union.name.text)
                .is_some_and(|entry| entry.symbol.definition() == definition)
                .then_some((&union.name, definition, union.type_parameters.len()))
        });
    let function_exports = program
        .functions
        .iter()
        .enumerate()
        .filter(|(_, function)| function.visibility == Visibility::Exported)
        .filter_map(|(index, function)| {
            let definition = DefId::Function(FunctionId::in_module(program.module, index));
            functions
                .get(&function.name.text)
                .is_some_and(|entry| entry.id == FunctionId::in_module(program.module, index))
                .then_some((&function.name, definition, function.type_parameters.len()))
        });
    let mut declarations = record_exports
        .chain(union_exports)
        .chain(function_exports)
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(name, _, _)| span_position(name.span));

    let mut exports = HashMap::<String, ExportEntry>::new();
    for (name, definition, parameter_count) in declarations {
        if let Some(previous) = exports.get(&name.text).copied() {
            diagnostics.push(
                Diagnostic::error(
                    EXPORT_COLLISION,
                    format!("export name `{}` is ambiguous", name.text),
                )
                .with_label(Label::primary(name.span, "conflicting export"))
                .with_label(Label::secondary(previous.name_span, "first exported here")),
            );
        } else {
            exports.insert(
                name.text.clone(),
                ExportEntry {
                    definition,
                    name_span: name.span,
                    parameter_count,
                },
            );
        }
    }

    exports
}

fn resolve_module_environments(
    programs: &[Program],
    links: &[ResolvedImport],
    local_modules: &HashMap<ModuleId, LocalModuleCatalog>,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> HashMap<ModuleId, ModuleEnvironment> {
    let mut targets = HashMap::new();
    for link in links {
        targets
            .entry((link.importer, link.path_span))
            .or_insert(link.target);
    }

    programs
        .iter()
        .filter_map(|program| {
            let _ = local_modules.get(&program.module)?;
            let mut bindings = program
                .records
                .iter()
                .enumerate()
                .map(|(index, record)| BindingEvent {
                    name: record.name.text.clone(),
                    span: record.name.span,
                    definition: DefId::Record(RecordId::in_module(program.module, index)),
                    parameter_count: record.type_parameters.len(),
                })
                .chain(
                    program
                        .unions
                        .iter()
                        .enumerate()
                        .map(|(index, union)| BindingEvent {
                            name: union.name.text.clone(),
                            span: union.name.span,
                            definition: DefId::Union(UnionId::in_module(program.module, index)),
                            parameter_count: union.type_parameters.len(),
                        }),
                )
                .chain(
                    program
                        .functions
                        .iter()
                        .enumerate()
                        .map(|(index, function)| BindingEvent {
                            name: function.name.text.clone(),
                            span: function.name.span,
                            definition: DefId::Function(FunctionId::in_module(
                                program.module,
                                index,
                            )),
                            parameter_count: function.type_parameters.len(),
                        }),
                )
                .collect::<Vec<_>>();

            for import in &program.imports {
                let Some(target) = targets.get(&(program.module, import.path_span)).copied() else {
                    continue;
                };
                let Some(target_module) = local_modules.get(&target) else {
                    continue;
                };

                for name in &import.names {
                    if let Some(export) = target_module.exports.get(&name.text).copied() {
                        facts.record_name(name.span, definition_resolution(export.definition));
                        bindings.push(BindingEvent {
                            name: name.text.clone(),
                            span: name.span,
                            definition: export.definition,
                            parameter_count: export.parameter_count,
                        });
                    } else if let Some(declaration) =
                        target_module.declared_names.get(&name.text).copied()
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

            bindings.sort_by_key(|binding| span_position(binding.span));
            let mut environment = ModuleEnvironment {
                types: TypeCatalog::new(),
                functions: FunctionNameCatalog::new(),
            };
            for binding in bindings {
                insert_module_binding(&mut environment, binding, diagnostics);
            }

            Some((program.module, environment))
        })
        .collect()
}

fn insert_module_binding(
    environment: &mut ModuleEnvironment,
    binding: BindingEvent,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match binding.definition {
        DefId::Function(function) => {
            let previous = environment
                .functions
                .get(&binding.name)
                .map(|entry| entry.name_span);
            if binding.name == "print" || previous.is_some() {
                push_top_level_duplicate(
                    diagnostics,
                    &binding.name,
                    binding.span,
                    previous,
                    "value",
                );
            } else {
                environment.functions.insert(
                    binding.name,
                    FunctionNameEntry {
                        id: function,
                        name_span: binding.span,
                    },
                );
            }
        }
        DefId::Record(record) => insert_type_binding(
            environment,
            binding.name,
            binding.span,
            TypeSymbol::Record(record),
            binding.parameter_count,
            diagnostics,
        ),
        DefId::Union(union) => insert_type_binding(
            environment,
            binding.name,
            binding.span,
            TypeSymbol::Union(union),
            binding.parameter_count,
            diagnostics,
        ),
    }
}

fn insert_type_binding(
    environment: &mut ModuleEnvironment,
    name: String,
    span: SourceSpan,
    symbol: TypeSymbol,
    parameter_count: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(previous) = environment.types.get(&name).copied() {
        push_top_level_duplicate(diagnostics, &name, span, Some(previous.name_span), "type");
    } else {
        environment.types.insert(
            name,
            TypeEntry {
                symbol,
                name_span: span,
                parameter_count,
            },
        );
    }
}

fn push_top_level_duplicate(
    diagnostics: &mut Vec<Diagnostic>,
    name: &str,
    span: SourceSpan,
    previous: Option<SourceSpan>,
    namespace: &str,
) {
    let mut diagnostic = Diagnostic::error(
        DUPLICATE_NAME,
        format!("duplicate top-level {namespace} `{name}`"),
    )
    .with_label(Label::primary(
        span,
        format!("duplicate {namespace} binding"),
    ));
    if let Some(previous) = previous {
        diagnostic = diagnostic.with_label(Label::secondary(previous, "first bound here"));
    }
    diagnostics.push(diagnostic);
}

const fn definition_resolution(definition: DefId) -> NameResolution {
    match definition {
        DefId::Function(function) => NameResolution::Function(function),
        DefId::Record(record) => NameResolution::Record(record),
        DefId::Union(union) => NameResolution::Union(union),
    }
}

fn collect_type_parameters(
    parameters: &[TypeParameter],
    owner: TypeParameterOwner,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> (Vec<TypeParameterFacts>, TypeParameterCatalog) {
    let mut resolved = Vec::with_capacity(parameters.len());
    let mut catalog = TypeParameterCatalog::new();
    for (index, parameter) in parameters.iter().enumerate() {
        let id = TypeParameterId::new(owner, index);
        facts.record_name(parameter.name.span, NameResolution::TypeParameter(id));
        let parameter_facts = TypeParameterFacts {
            id,
            name: parameter.name.text.clone(),
            span: parameter.span,
        };
        if let Some(previous) = catalog.get(&parameter.name.text).copied() {
            diagnostics.push(
                Diagnostic::error(
                    DUPLICATE_NAME,
                    format!("duplicate type parameter `{}`", parameter.name.text),
                )
                .with_label(Label::primary(
                    parameter.name.span,
                    "duplicate type parameter",
                ))
                .with_label(Label::secondary(previous.span, "first declared here")),
            );
        } else {
            catalog.insert(
                parameter.name.text.clone(),
                TypeParameterEntry {
                    id,
                    span: parameter.span,
                },
            );
        }
        resolved.push(parameter_facts);
    }
    (resolved, catalog)
}

fn reject_unconstrained_parameters<'a>(
    parameters: &'a [TypeParameterFacts],
    catalog: &TypeParameterCatalog,
    types: impl Iterator<Item = &'a Type>,
    declaration_kind: &str,
    owner_span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let types = types.collect::<Vec<_>>();
    for parameter in parameters {
        let is_active = catalog
            .get(&parameter.name)
            .is_some_and(|entry| entry.id == parameter.id);
        if is_active
            && !types
                .iter()
                .any(|ty| type_contains_parameter_id(ty, parameter.id))
        {
            diagnostics.push(
                Diagnostic::error(
                    UNCONSTRAINED_TYPE_PARAMETER,
                    format!(
                        "type parameter `{}` is not constrained by any {declaration_kind} value",
                        parameter.name
                    ),
                )
                .with_label(Label::primary(
                    parameter.span,
                    "unconstrained type parameter",
                ))
                .with_label(Label::secondary(owner_span, "generic declaration here")),
            );
        }
    }
}

fn collect_record_facts(
    program: &Program,
    symbols: &TypeCatalog,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> Vec<RecordFacts> {
    program
        .records
        .iter()
        .enumerate()
        .map(|(record_index, record)| {
            let record_id = RecordId::in_module(program.module, record_index);
            let (type_parameters, parameter_catalog) = collect_type_parameters(
                &record.type_parameters,
                TypeParameterOwner::Record(record_id),
                diagnostics,
                facts,
            );
            let mut declared_fields = HashMap::<String, SourceSpan>::new();
            let mut fields = Vec::new();

            for (field_index, field) in record.fields.iter().enumerate() {
                let field_id = FieldId::new(record_id, field_index);
                facts.record_name(field.name.span, NameResolution::Field(field_id));
                if let Some(previous) = declared_fields.get(&field.name.text).copied() {
                    diagnostics.push(
                        Diagnostic::error(
                            DUPLICATE_NAME,
                            format!("duplicate field `{}`", field.name.text),
                        )
                        .with_label(Label::primary(field.name.span, "duplicate field"))
                        .with_label(Label::secondary(previous, "first declared here")),
                    );
                    continue;
                }
                declared_fields.insert(field.name.text.clone(), field.name.span);

                let Some(ty) =
                    resolve_type(&field.ty, symbols, &parameter_catalog, diagnostics, facts)
                else {
                    continue;
                };
                if ty == Type::Unit {
                    diagnostics.push(
                        Diagnostic::error(TYPE_MISMATCH, "record fields cannot have type `Unit`")
                            .with_label(Label::primary(field.ty.span, "invalid record field type")),
                    );
                }

                fields.push(RecordFieldFacts {
                    id: field_id,
                    name: field.name.text.clone(),
                    ty,
                    span: field.span,
                    type_span: field.ty.span,
                });
            }

            reject_unconstrained_parameters(
                &type_parameters,
                &parameter_catalog,
                fields.iter().map(|field| &field.ty),
                "record field",
                record.name.span,
                diagnostics,
            );

            RecordFacts {
                id: record_id,
                name: record.name.text.clone(),
                type_parameters,
                fields,
                name_span: record.name.span,
                span: record.span,
            }
        })
        .collect()
}

fn collect_union_facts(
    program: &Program,
    symbols: &TypeCatalog,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> Vec<UnionFacts> {
    program
        .unions
        .iter()
        .enumerate()
        .map(|(union_index, union)| {
            let union_id = UnionId::in_module(program.module, union_index);
            let (type_parameters, parameter_catalog) = collect_type_parameters(
                &union.type_parameters,
                TypeParameterOwner::Union(union_id),
                diagnostics,
                facts,
            );
            let mut declared_variants = HashMap::<String, SourceSpan>::new();
            let mut variants = Vec::new();

            for (variant_index, variant) in union.variants.iter().enumerate() {
                let variant_id = VariantId::new(union_id, variant_index);
                facts.record_name(variant.name.span, NameResolution::Variant(variant_id));
                if let Some(previous) = declared_variants.get(&variant.name.text).copied() {
                    diagnostics.push(
                        Diagnostic::error(
                            DUPLICATE_NAME,
                            format!("duplicate variant `{}`", variant.name.text),
                        )
                        .with_label(Label::primary(variant.name.span, "duplicate variant"))
                        .with_label(Label::secondary(previous, "first declared here")),
                    );
                    continue;
                }
                declared_variants.insert(variant.name.text.clone(), variant.name.span);

                let mut declared_payloads = HashMap::<String, SourceSpan>::new();
                let mut payloads = Vec::new();
                for (payload_index, payload) in variant.payloads.iter().enumerate() {
                    let payload_id = PayloadId::new(variant_id, payload_index);
                    facts.record_name(payload.name.span, NameResolution::Payload(payload_id));
                    if let Some(previous) = declared_payloads.get(&payload.name.text).copied() {
                        diagnostics.push(
                            Diagnostic::error(
                                DUPLICATE_NAME,
                                format!("duplicate payload `{}`", payload.name.text),
                            )
                            .with_label(Label::primary(payload.name.span, "duplicate payload"))
                            .with_label(Label::secondary(previous, "first declared here")),
                        );
                        continue;
                    }
                    declared_payloads.insert(payload.name.text.clone(), payload.name.span);

                    let Some(ty) =
                        resolve_type(&payload.ty, symbols, &parameter_catalog, diagnostics, facts)
                    else {
                        continue;
                    };
                    payloads.push(PayloadFacts {
                        id: payload_id,
                        name: payload.name.text.clone(),
                        ty,
                        span: payload.span,
                        type_span: payload.ty.span,
                    });
                }

                variants.push(VariantFacts {
                    id: variant_id,
                    name: variant.name.text.clone(),
                    payloads,
                    name_span: variant.name.span,
                    span: variant.span,
                });
            }

            reject_unconstrained_parameters(
                &type_parameters,
                &parameter_catalog,
                variants
                    .iter()
                    .flat_map(|variant| variant.payloads.iter())
                    .map(|payload| &payload.ty),
                "union payload",
                union.name.span,
                diagnostics,
            );

            UnionFacts {
                id: union_id,
                name: union.name.text.clone(),
                type_parameters,
                variants,
                name_span: union.name.span,
                span: union.span,
            }
        })
        .collect()
}

fn resolve_type(
    reference: &TypeReference,
    types: &TypeCatalog,
    parameters: &TypeParameterCatalog,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> Option<Type> {
    let ty = resolve_type_kind(reference, types, parameters, diagnostics, facts)?;
    if contains_invalid_array_element(&ty) {
        diagnostics.push(
            Diagnostic::error(TYPE_MISMATCH, "array elements cannot have type `Unit`")
                .with_label(Label::primary(reference.span, "invalid array element type")),
        );
    }
    Some(ty)
}

fn resolve_type_kind(
    reference: &TypeReference,
    types: &TypeCatalog,
    parameters: &TypeParameterCatalog,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> Option<Type> {
    match &reference.kind {
        TypeReferenceKind::Int => Some(Type::Int),
        TypeReferenceKind::Bool => Some(Type::Bool),
        TypeReferenceKind::String => Some(Type::String),
        TypeReferenceKind::Unit => Some(Type::Unit),
        TypeReferenceKind::Named { name, arguments } => {
            if let Some(parameter) = parameters.get(&name.text).copied() {
                facts.record_name(name.span, NameResolution::TypeParameter(parameter.id));
                if !arguments.is_empty() {
                    for argument in arguments {
                        let _ = resolve_type_kind(argument, types, parameters, diagnostics, facts);
                    }
                    diagnostics.push(
                        Diagnostic::error(
                            CALL_ARITY,
                            format!("type parameter `{}` does not accept arguments", name.text),
                        )
                        .with_label(Label::primary(reference.span, "unexpected type arguments"))
                        .with_label(Label::secondary(parameter.span, "parameter declared here")),
                    );
                    return None;
                }
                return Some(Type::Parameter(parameter.id));
            }

            let Some(entry) = types.get(&name.text).copied() else {
                for argument in arguments {
                    let _ = resolve_type_kind(argument, types, parameters, diagnostics, facts);
                }
                diagnostics.push(
                    Diagnostic::error(UNDEFINED_NAME, format!("undefined type `{}`", name.text))
                        .with_label(Label::primary(name.span, "not found in this program")),
                );
                return None;
            };
            facts.record_name(name.span, entry.symbol.resolution());
            let resolved_arguments = arguments
                .iter()
                .map(|argument| resolve_type_kind(argument, types, parameters, diagnostics, facts))
                .collect::<Option<Vec<_>>>();
            if entry.parameter_count != arguments.len() {
                diagnostics.push(
                    Diagnostic::error(
                        CALL_ARITY,
                        format!(
                            "type `{}` expects {} argument(s), found {}",
                            name.text,
                            entry.parameter_count,
                            arguments.len()
                        ),
                    )
                    .with_label(Label::primary(
                        reference.span,
                        "incorrect type argument count",
                    ))
                    .with_label(Label::secondary(entry.name_span, "type declared here")),
                );
                return None;
            }
            let resolved_arguments = resolved_arguments?.into_boxed_slice();
            facts.record_instance(
                entry.symbol.definition(),
                &resolved_arguments,
                reference.span,
            );
            Some(entry.symbol.ty(resolved_arguments))
        }
        TypeReferenceKind::Array(element) => {
            resolve_type_kind(element, types, parameters, diagnostics, facts)
                .map(|element| Type::Array(Box::new(element)))
        }
        TypeReferenceKind::Function {
            parameters: function_parameters,
            return_type,
        } => {
            let parameter_types = function_parameters
                .iter()
                .map(|parameter| {
                    resolve_type_kind(&parameter.ty, types, parameters, diagnostics, facts)
                })
                .collect::<Option<Vec<_>>>()?;
            let return_type =
                resolve_type_kind(return_type, types, parameters, diagnostics, facts)?;
            Some(Type::Function {
                parameters: parameter_types.into_boxed_slice(),
                return_type: Box::new(return_type),
            })
        }
    }
}

fn reject_recursive_records(
    records: &[RecordFacts],
    diagnostics: &mut Vec<Diagnostic>,
) -> HashSet<(DefId, SourceSpan)> {
    let mut rejected_edges = HashSet::new();
    for record in records {
        for field in &record.fields {
            if type_reaches_record(
                &field.ty,
                record.id,
                records,
                &HashMap::new(),
                &mut HashSet::new(),
            ) {
                diagnostics.push(
                    Diagnostic::error(
                        RECURSIVE_TYPE,
                        format!(
                            "record `{}` contains itself through field `{}`",
                            record.name, field.name
                        ),
                    )
                    .with_label(Label::primary(field.type_span, "recursive record field"))
                    .with_label(Label::secondary(record.name_span, "record declared here")),
                );
                rejected_edges.insert((DefId::Record(record.id), field.type_span));
                break;
            }
        }
    }
    rejected_edges
}

fn type_reaches_record(
    ty: &Type,
    target: RecordId,
    records: &[RecordFacts],
    substitutions: &HashMap<TypeParameterId, Type>,
    visited: &mut HashSet<GenericInstance>,
) -> bool {
    match ty {
        Type::Record { definition, .. } if *definition == target => true,
        Type::Record {
            definition,
            arguments,
        } => {
            let arguments = arguments
                .iter()
                .map(|argument| substitute_type(argument, substitutions))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            if !visited.insert(GenericInstance {
                definition: DefId::Record(*definition),
                arguments: arguments.clone(),
            }) {
                return false;
            }
            record_facts(records, *definition).is_some_and(|record| {
                let owner_substitutions =
                    substitutions_for_parameters(&record.type_parameters, &arguments);
                record.fields.iter().any(|field| {
                    type_reaches_record(&field.ty, target, records, &owner_substitutions, visited)
                })
            })
        }
        Type::Array(element) => {
            type_reaches_record(element, target, records, substitutions, visited)
        }
        Type::Function { .. } => false,
        Type::Parameter(parameter) => substitutions.get(parameter).is_some_and(|replacement| {
            replacement != ty
                && type_reaches_record(replacement, target, records, substitutions, visited)
        }),
        Type::Union { .. } | Type::Int | Type::Bool | Type::String | Type::Unit => false,
    }
}

struct NominalTypeEdge {
    source: DefId,
    target: DefId,
    arguments: Box<[Type]>,
    span: SourceSpan,
}

fn reject_non_regular_generic_types(
    records: &[RecordFacts],
    unions: &[UnionFacts],
    unguarded_record_edges: &HashSet<(DefId, SourceSpan)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut edges = Vec::new();
    for record in records {
        for field in &record.fields {
            collect_nominal_type_edges(
                DefId::Record(record.id),
                &field.ty,
                field.type_span,
                &mut edges,
            );
        }
    }
    for union in unions {
        for variant in &union.variants {
            for payload in &variant.payloads {
                collect_nominal_type_edges(
                    DefId::Union(union.id),
                    &payload.ty,
                    payload.type_span,
                    &mut edges,
                );
            }
        }
    }

    let mut graph = HashMap::<DefId, Vec<DefId>>::new();
    for edge in &edges {
        graph.entry(edge.source).or_default().push(edge.target);
    }

    for edge in edges {
        if unguarded_record_edges.contains(&(edge.source, edge.span)) {
            continue;
        }
        if !definition_reaches(edge.target, edge.source, &graph, &mut HashSet::new()) {
            continue;
        }
        let Some(parameters) = type_definition_parameters(edge.source, records, unions) else {
            continue;
        };
        let expected = parameters
            .iter()
            .map(|parameter| Type::Parameter(parameter.id))
            .collect::<Vec<_>>();
        if edge.arguments.as_ref() != expected.as_slice() {
            let mut diagnostic = Diagnostic::error(
                GENERIC_LIMIT,
                "recursive generic type changes its type arguments",
            )
            .with_label(Label::primary(
                edge.span,
                "non-regular recursive instantiation",
            ));
            if let Some(definition_span) = type_definition_name_span(edge.target, records, unions) {
                diagnostic = diagnostic
                    .with_label(Label::secondary(definition_span, "generic definition here"));
            }
            diagnostics.push(diagnostic);
        }
    }
}

fn type_definition_name_span(
    definition: DefId,
    records: &[RecordFacts],
    unions: &[UnionFacts],
) -> Option<SourceSpan> {
    match definition {
        DefId::Record(record) => record_facts(records, record).map(|facts| facts.name_span),
        DefId::Union(union) => union_facts(unions, union).map(|facts| facts.name_span),
        DefId::Function(_) => None,
    }
}

fn collect_nominal_type_edges(
    source: DefId,
    ty: &Type,
    span: SourceSpan,
    edges: &mut Vec<NominalTypeEdge>,
) {
    match ty {
        Type::Array(element) => collect_nominal_type_edges(source, element, span, edges),
        Type::Function {
            parameters,
            return_type,
        } => {
            for parameter in parameters {
                collect_nominal_type_edges(source, parameter, span, edges);
            }
            collect_nominal_type_edges(source, return_type, span, edges);
        }
        Type::Record {
            definition,
            arguments,
        } => {
            edges.push(NominalTypeEdge {
                source,
                target: DefId::Record(*definition),
                arguments: arguments.clone(),
                span,
            });
            for argument in arguments {
                collect_nominal_type_edges(source, argument, span, edges);
            }
        }
        Type::Union {
            definition,
            arguments,
        } => {
            edges.push(NominalTypeEdge {
                source,
                target: DefId::Union(*definition),
                arguments: arguments.clone(),
                span,
            });
            for argument in arguments {
                collect_nominal_type_edges(source, argument, span, edges);
            }
        }
        Type::Parameter(_) | Type::Int | Type::Bool | Type::String | Type::Unit => {}
    }
}

fn type_definition_parameters<'a>(
    definition: DefId,
    records: &'a [RecordFacts],
    unions: &'a [UnionFacts],
) -> Option<&'a [TypeParameterFacts]> {
    match definition {
        DefId::Record(record) => {
            record_facts(records, record).map(|facts| facts.type_parameters.as_slice())
        }
        DefId::Union(union) => {
            union_facts(unions, union).map(|facts| facts.type_parameters.as_slice())
        }
        DefId::Function(_) => None,
    }
}

fn definition_reaches(
    current: DefId,
    target: DefId,
    graph: &HashMap<DefId, Vec<DefId>>,
    visited: &mut HashSet<DefId>,
) -> bool {
    if current == target {
        return true;
    }
    if !visited.insert(current) {
        return false;
    }
    graph.get(&current).is_some_and(|dependencies| {
        dependencies
            .iter()
            .any(|dependency| definition_reaches(*dependency, target, graph, visited))
    })
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    type_parameters: Vec<TypeParameterFacts>,
    parameter_catalog: TypeParameterCatalog,
    parameters: Vec<Option<Type>>,
    parameter_type_spans: Vec<SourceSpan>,
    return_type: Option<Type>,
    return_type_span: SourceSpan,
    resolution: NameResolution,
    name_span: SourceSpan,
}

struct FunctionCatalog {
    by_module: HashMap<ModuleId, HashMap<String, FunctionSignature>>,
    by_id: HashMap<FunctionId, FunctionSignature>,
}

fn collect_function_signatures(
    programs: &[Program],
    environments: &HashMap<ModuleId, ModuleEnvironment>,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> FunctionCatalog {
    let mut by_id = HashMap::new();
    for program in programs {
        let Some(environment) = environments.get(&program.module) else {
            continue;
        };
        for (index, function) in program.functions.iter().enumerate() {
            let function_id = FunctionId::in_module(program.module, index);
            let (type_parameters, parameter_catalog) = collect_type_parameters(
                &function.type_parameters,
                TypeParameterOwner::Function(function_id),
                diagnostics,
                facts,
            );
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| {
                    resolve_type(
                        &parameter.ty,
                        &environment.types,
                        &parameter_catalog,
                        diagnostics,
                        facts,
                    )
                })
                .collect::<Vec<_>>();
            reject_unconstrained_parameters(
                &type_parameters,
                &parameter_catalog,
                parameters.iter().filter_map(Option::as_ref),
                "function parameter",
                function.name.span,
                diagnostics,
            );
            by_id.insert(
                function_id,
                FunctionSignature {
                    type_parameters,
                    parameter_catalog: parameter_catalog.clone(),
                    parameters,
                    parameter_type_spans: function
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty.span)
                        .collect(),
                    return_type: resolve_type(
                        &function.return_type,
                        &environment.types,
                        &parameter_catalog,
                        diagnostics,
                        facts,
                    ),
                    return_type_span: function.return_type.span,
                    resolution: NameResolution::Function(function_id),
                    name_span: function.name.span,
                },
            );
        }
    }

    let by_module = environments
        .iter()
        .map(|(module, environment)| {
            let functions = environment
                .functions
                .iter()
                .filter_map(|(name, entry)| {
                    by_id
                        .get(&entry.id)
                        .cloned()
                        .map(|signature| (name.clone(), signature))
                })
                .collect();
            (*module, functions)
        })
        .collect();

    FunctionCatalog { by_module, by_id }
}

fn reject_non_regular_generic_calls(
    edges: &[GenericCallEdge],
    catalog: &FunctionCatalog,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut graph = HashMap::<FunctionId, Vec<FunctionId>>::new();
    for edge in edges {
        graph.entry(edge.caller).or_default().push(edge.callee);
    }

    for edge in edges {
        if !function_reaches(edge.callee, edge.caller, &graph, &mut HashSet::new()) {
            continue;
        }
        let Some(signature) = catalog.by_id.get(&edge.caller) else {
            continue;
        };
        let expected = signature
            .type_parameters
            .iter()
            .map(|parameter| Type::Parameter(parameter.id))
            .collect::<Vec<_>>();
        if edge.arguments.as_ref() != expected.as_slice() {
            let mut diagnostic = Diagnostic::error(
                GENERIC_LIMIT,
                "recursive generic call changes its type arguments",
            )
            .with_label(Label::primary(
                edge.span,
                "non-regular recursive instantiation",
            ));
            if let Some(callee) = catalog.by_id.get(&edge.callee) {
                diagnostic = diagnostic.with_label(Label::secondary(
                    callee.name_span,
                    "generic definition here",
                ));
            }
            diagnostics.push(diagnostic);
        }
    }
}

fn function_reaches(
    current: FunctionId,
    target: FunctionId,
    graph: &HashMap<FunctionId, Vec<FunctionId>>,
    visited: &mut HashSet<FunctionId>,
) -> bool {
    if current == target {
        return true;
    }
    if !visited.insert(current) {
        return false;
    }
    graph.get(&current).is_some_and(|callees| {
        callees
            .iter()
            .any(|callee| function_reaches(*callee, target, graph, visited))
    })
}

fn collect_closed_generic_instances(
    roots: &[InstanceOccurrence],
    programs: &[Program],
    records: &[RecordFacts],
    unions: &[UnionFacts],
    functions: &FunctionCatalog,
) -> Vec<InstanceOccurrence> {
    let module_order = programs
        .iter()
        .enumerate()
        .map(|(index, program)| (program.span.file(), index))
        .collect::<HashMap<_, _>>();
    let mut ordered_roots = roots.iter().collect::<Vec<_>>();
    ordered_roots.sort_by_key(|occurrence| {
        let span = occurrence.span;
        (
            module_order
                .get(&span.file())
                .copied()
                .unwrap_or(usize::MAX),
            span.range().start(),
            span.range().end(),
            span.file().raw(),
        )
    });
    let mut collector = ClosedInstanceCollector {
        records,
        unions,
        functions,
        seen: HashSet::new(),
        instances: Vec::new(),
    };
    for occurrence in ordered_roots {
        collector.discover_instance(occurrence.instance.clone(), occurrence.span);
        if collector.instances.len() > MAX_GENERIC_INSTANCES {
            break;
        }
    }
    collector.instances
}

struct ClosedInstanceCollector<'a> {
    records: &'a [RecordFacts],
    unions: &'a [UnionFacts],
    functions: &'a FunctionCatalog,
    seen: HashSet<GenericInstance>,
    instances: Vec<InstanceOccurrence>,
}

impl ClosedInstanceCollector<'_> {
    fn discover_instance(&mut self, instance: GenericInstance, span: SourceSpan) {
        if self.instances.len() > MAX_GENERIC_INSTANCES
            || instance.arguments.is_empty()
            || instance.arguments.iter().any(type_contains_parameter)
            || !self.seen.insert(instance.clone())
        {
            return;
        }

        self.instances.push(InstanceOccurrence {
            instance: instance.clone(),
            span,
        });
        if self.instances.len() > MAX_GENERIC_INSTANCES {
            return;
        }

        for argument in &instance.arguments {
            self.discover_type(argument, span);
            if self.instances.len() > MAX_GENERIC_INSTANCES {
                return;
            }
        }

        for (ty, type_span) in self.instantiated_definition_types(&instance) {
            self.discover_type(&ty, type_span);
            if self.instances.len() > MAX_GENERIC_INSTANCES {
                return;
            }
        }
    }

    fn discover_type(&mut self, ty: &Type, span: SourceSpan) {
        match ty {
            Type::Array(element) => self.discover_type(element, span),
            Type::Function {
                parameters,
                return_type,
            } => {
                for parameter in parameters {
                    self.discover_type(parameter, span);
                }
                self.discover_type(return_type, span);
            }
            Type::Record {
                definition,
                arguments,
            } => self.discover_nominal(DefId::Record(*definition), arguments, span),
            Type::Union {
                definition,
                arguments,
            } => self.discover_nominal(DefId::Union(*definition), arguments, span),
            Type::Parameter(_) | Type::Int | Type::Bool | Type::String | Type::Unit => {}
        }
    }

    fn discover_nominal(&mut self, definition: DefId, arguments: &[Type], span: SourceSpan) {
        if !arguments.is_empty() && !arguments.iter().any(type_contains_parameter) {
            self.discover_instance(
                GenericInstance {
                    definition,
                    arguments: arguments.to_vec().into_boxed_slice(),
                },
                span,
            );
            return;
        }

        for argument in arguments {
            self.discover_type(argument, span);
            if self.instances.len() > MAX_GENERIC_INSTANCES {
                return;
            }
        }
    }

    fn instantiated_definition_types(&self, instance: &GenericInstance) -> Vec<(Type, SourceSpan)> {
        match instance.definition {
            DefId::Record(record_id) => record_facts(self.records, record_id)
                .map(|record| {
                    let substitutions =
                        substitutions_for_parameters(&record.type_parameters, &instance.arguments);
                    record
                        .fields
                        .iter()
                        .map(|field| (substitute_type(&field.ty, &substitutions), field.type_span))
                        .collect()
                })
                .unwrap_or_default(),
            DefId::Union(union_id) => union_facts(self.unions, union_id)
                .map(|union| {
                    let substitutions =
                        substitutions_for_parameters(&union.type_parameters, &instance.arguments);
                    union
                        .variants
                        .iter()
                        .flat_map(|variant| &variant.payloads)
                        .map(|payload| {
                            (
                                substitute_type(&payload.ty, &substitutions),
                                payload.type_span,
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            DefId::Function(function_id) => self
                .functions
                .by_id
                .get(&function_id)
                .map(|signature| {
                    let substitutions = substitutions_for_parameters(
                        &signature.type_parameters,
                        &instance.arguments,
                    );
                    let mut types = signature
                        .parameters
                        .iter()
                        .zip(&signature.parameter_type_spans)
                        .filter_map(|(ty, span)| {
                            ty.as_ref()
                                .map(|ty| (substitute_type(ty, &substitutions), *span))
                        })
                        .collect::<Vec<_>>();
                    if let Some(return_type) = &signature.return_type {
                        types.push((
                            substitute_type(return_type, &substitutions),
                            signature.return_type_span,
                        ));
                    }
                    types
                })
                .unwrap_or_default(),
        }
    }
}

fn reject_generic_instance_budget(
    occurrences: &[InstanceOccurrence],
    records: &[RecordFacts],
    unions: &[UnionFacts],
    functions: &FunctionCatalog,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(occurrence) = occurrences.get(MAX_GENERIC_INSTANCES) else {
        return;
    };
    let mut diagnostic = Diagnostic::error(
        GENERIC_LIMIT,
        format!("generic instance limit of {MAX_GENERIC_INSTANCES} exceeded"),
    )
    .with_label(Label::primary(
        occurrence.span,
        "too many distinct generic instances",
    ));
    if let Some(definition_span) =
        generic_definition_name_span(occurrence.instance.definition, records, unions, functions)
    {
        diagnostic =
            diagnostic.with_label(Label::secondary(definition_span, "generic definition here"));
    }
    diagnostics.push(diagnostic);
}

fn generic_definition_name_span(
    definition: DefId,
    records: &[RecordFacts],
    unions: &[UnionFacts],
    functions: &FunctionCatalog,
) -> Option<SourceSpan> {
    type_definition_name_span(definition, records, unions).or_else(|| match definition {
        DefId::Function(function) => functions.by_id.get(&function).map(|facts| facts.name_span),
        DefId::Record(_) | DefId::Union(_) => None,
    })
}

fn reject_invalid_generic_instances(
    occurrences: &[InstanceOccurrence],
    records: &[RecordFacts],
    unions: &[UnionFacts],
    functions: &FunctionCatalog,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for occurrence in occurrences {
        let invalid = match occurrence.instance.definition {
            DefId::Record(record_id) => record_facts(records, record_id).is_some_and(|record| {
                let substitutions = substitutions_for_parameters(
                    &record.type_parameters,
                    &occurrence.instance.arguments,
                );
                record.fields.iter().any(|field| {
                    let ty = substitute_type(&field.ty, &substitutions);
                    ty == Type::Unit || contains_invalid_array_element(&ty)
                })
            }),
            DefId::Union(union_id) => union_facts(unions, union_id).is_some_and(|union| {
                let substitutions = substitutions_for_parameters(
                    &union.type_parameters,
                    &occurrence.instance.arguments,
                );
                union.variants.iter().any(|variant| {
                    variant.payloads.iter().any(|payload| {
                        contains_invalid_array_element(&substitute_type(
                            &payload.ty,
                            &substitutions,
                        ))
                    })
                })
            }),
            DefId::Function(function_id) => {
                functions.by_id.get(&function_id).is_some_and(|signature| {
                    let substitutions = substitutions_for_parameters(
                        &signature.type_parameters,
                        &occurrence.instance.arguments,
                    );
                    signature
                        .parameters
                        .iter()
                        .filter_map(Option::as_ref)
                        .chain(signature.return_type.as_ref())
                        .any(|ty| {
                            contains_invalid_array_element(&substitute_type(ty, &substitutions))
                        })
                })
            }
        };
        if invalid {
            diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    "generic instantiation produces an invalid value type",
                )
                .with_label(Label::primary(
                    occurrence.span,
                    "invalid generic type arguments",
                )),
            );
        }
    }
}

fn check_function(
    function: &Function,
    function_id: FunctionId,
    catalog: &FunctionCatalog,
    type_symbols: &TypeCatalog,
    records: &[RecordFacts],
    unions: &[UnionFacts],
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) {
    let Some(signature) = catalog.by_id.get(&function_id) else {
        return;
    };
    let Some(functions) = catalog.by_module.get(&function_id.module()) else {
        return;
    };
    let mut checker = FunctionChecker {
        function_id,
        functions,
        type_symbols,
        type_parameters: signature.parameter_catalog.clone(),
        records,
        unions,
        diagnostics,
        facts,
        scopes: vec![HashMap::new()],
        return_type: signature.return_type.clone(),
        parameter_types: signature
            .parameters
            .iter()
            .filter_map(Clone::clone)
            .collect(),
        declared_type_parameters: signature.type_parameters.clone(),
        loop_depth: 0,
        parameters: Vec::new(),
        next_local: 0,
        closure_stack: Vec::new(),
    };

    for (parameter, ty) in function.parameters.iter().zip(&signature.parameters) {
        if let Some(local) = checker.bind(&parameter.name, ty.clone(), false) {
            checker.parameters.push(local);
        }
    }

    let always_returns = checker.check_block(&function.body, false);
    if signature
        .return_type
        .as_ref()
        .is_some_and(|ty| ty != &Type::Unit)
        && !always_returns
    {
        let return_type = format_type(
            signature.return_type.as_ref().unwrap_or(&Type::Unit),
            records,
            unions,
        );
        checker.error(
            INVALID_RETURN,
            function.return_type.span,
            format!(
                "function `{}` may not return `{}` on every path",
                function.name.text, return_type
            ),
            "return required on every path",
        );
    }

    let known_signature =
        signature.return_type.is_some() && signature.parameters.iter().all(Option::is_some);
    let valid_main_parameters = signature.parameters.is_empty()
        || matches!(signature.parameters.as_slice(), [Some(Type::Array(element))] if element.as_ref() == &Type::String);
    if function_id.module() == ModuleId::ENTRY
        && function.name.text == "main"
        && known_signature
        && (!function.type_parameters.is_empty()
            || !valid_main_parameters
            || signature.return_type != Some(Type::Unit))
    {
        checker.error(
            INVALID_RETURN,
            function.name.span,
            "`main` must take no parameters or one `String[]` parameter and return `Unit`",
            "invalid entry-point signature",
        );
    }

    checker.finish();
}

struct FunctionChecker<'a> {
    function_id: FunctionId,
    functions: &'a HashMap<String, FunctionSignature>,
    type_symbols: &'a TypeCatalog,
    type_parameters: TypeParameterCatalog,
    records: &'a [RecordFacts],
    unions: &'a [UnionFacts],
    diagnostics: &'a mut Vec<Diagnostic>,
    facts: &'a mut FactBuilder,
    scopes: Vec<HashMap<String, Binding>>,
    return_type: Option<Type>,
    parameter_types: Vec<Type>,
    declared_type_parameters: Vec<TypeParameterFacts>,
    loop_depth: usize,
    parameters: Vec<LocalId>,
    next_local: usize,
    closure_stack: Vec<ClosureCaptureState>,
}

#[derive(Debug, Clone)]
struct Binding {
    id: LocalId,
    ty: Option<Type>,
    span: SourceSpan,
    mutable: bool,
    closure_depth: usize,
}

struct ClosureCaptureState {
    id: ClosureId,
    captures: Vec<LocalId>,
    captured: HashSet<LocalId>,
    rejected_mutable: HashSet<LocalId>,
}

impl FunctionChecker<'_> {
    fn check_block(&mut self, block: &Block, creates_scope: bool) -> bool {
        if creates_scope {
            self.scopes.push(HashMap::new());
        }

        let mut always_returns = false;
        for statement in &block.statements {
            always_returns |= self.check_statement(statement);
        }

        if creates_scope {
            let _ = self.scopes.pop();
        }

        always_returns
    }

    fn check_statement(&mut self, statement: &Statement) -> bool {
        match statement {
            Statement::Const(declaration) => {
                self.check_const(declaration);
                false
            }
            Statement::Let(declaration) => {
                self.check_let(declaration);
                false
            }
            Statement::Assignment(statement) => {
                self.check_assignment(statement);
                false
            }
            Statement::If(statement) => self.check_if(statement),
            Statement::While(statement) => {
                self.check_while(statement);
                false
            }
            Statement::ForOf(statement) => {
                self.check_for_of(statement);
                false
            }
            Statement::Break(statement) => {
                self.check_loop_control(statement.span, "break");
                false
            }
            Statement::Continue(statement) => {
                self.check_loop_control(statement.span, "continue");
                false
            }
            Statement::Return(statement) => {
                self.check_return(statement);
                true
            }
            Statement::Expression(statement) => {
                let _ = self.check_expression(&statement.expression);
                false
            }
        }
    }

    fn check_const(&mut self, declaration: &ConstDeclaration) {
        self.check_binding(
            &declaration.name,
            declaration.annotation.as_ref(),
            &declaration.initializer,
            false,
        );
    }

    fn check_let(&mut self, declaration: &LetDeclaration) {
        self.check_binding(
            &declaration.name,
            declaration.annotation.as_ref(),
            &declaration.initializer,
            true,
        );
    }

    fn check_binding(
        &mut self,
        name: &Name,
        annotation: Option<&TypeReference>,
        initializer: &Expression,
        mutable: bool,
    ) {
        let declared_type = annotation.and_then(|annotation| {
            resolve_type(
                annotation,
                self.type_symbols,
                &self.type_parameters,
                self.diagnostics,
                self.facts,
            )
        });
        let initializer_type =
            self.check_expression_with_expected(initializer, declared_type.as_ref());

        if let (Some(expected), Some(actual)) = (&declared_type, &initializer_type) {
            if expected != actual {
                let mut diagnostic = type_mismatch_diagnostic(
                    initializer.span(),
                    expected,
                    actual,
                    self.records,
                    self.unions,
                );
                if let Some(annotation) = annotation {
                    diagnostic = diagnostic.with_label(Label::secondary(
                        annotation.span,
                        "expected type declared here",
                    ));
                }
                self.diagnostics.push(diagnostic);
            }
        }

        let _ = self.bind(name, declared_type.or(initializer_type), mutable);
    }

    fn check_assignment(&mut self, statement: &AssignmentStatement) {
        let Some(binding) = self.lookup_binding(&statement.target) else {
            let _ = self.check_expression(&statement.value);
            self.error(
                UNDEFINED_NAME,
                statement.target.span,
                format!("undefined value `{}`", statement.target.text),
                "not found in this scope",
            );
            return;
        };
        self.record_capture(&binding, statement.target.span);
        self.facts
            .record_name(statement.target.span, NameResolution::Local(binding.id));
        let value_type = self.check_expression_with_expected(&statement.value, binding.ty.as_ref());

        if !binding.mutable {
            self.diagnostics.push(
                Diagnostic::error(
                    IMMUTABLE_ASSIGNMENT,
                    format!(
                        "cannot assign to immutable binding `{}`",
                        statement.target.text
                    ),
                )
                .with_label(Label::primary(
                    statement.target.span,
                    "immutable assignment",
                ))
                .with_label(Label::secondary(binding.span, "binding declared here")),
            );
        }

        if let (Some(expected), Some(actual)) = (&binding.ty, &value_type) {
            if actual != expected {
                self.type_mismatch(statement.value.span(), expected, actual);
            }
        }
    }

    fn check_if(&mut self, statement: &IfStatement) -> bool {
        if let Some(actual) = self.check_expression(&statement.condition) {
            if actual != Type::Bool {
                self.error(
                    NON_BOOLEAN_CONDITION,
                    statement.condition.span(),
                    format!("if condition must have type `Bool`, found `{actual}`"),
                    "expected `Bool` condition",
                );
            }
        }

        let then_returns = self.check_block(&statement.then_branch, true);
        let else_returns = statement
            .else_branch
            .as_ref()
            .is_some_and(|branch| self.check_block(branch, true));

        then_returns && else_returns
    }

    fn check_while(&mut self, statement: &WhileStatement) {
        if let Some(actual) = self.check_expression(&statement.condition) {
            if actual != Type::Bool {
                self.error(
                    NON_BOOLEAN_CONDITION,
                    statement.condition.span(),
                    format!("while condition must have type `Bool`, found `{actual}`"),
                    "expected `Bool` condition",
                );
            }
        }

        self.loop_depth += 1;
        let _ = self.check_block(&statement.body, true);
        self.loop_depth -= 1;
    }

    fn check_for_of(&mut self, statement: &ForOfStatement) {
        let element_type = match self.check_expression(&statement.iterable) {
            Some(Type::Array(element)) => Some(*element),
            Some(actual) => {
                self.error(
                    TYPE_MISMATCH,
                    statement.iterable.span(),
                    format!(
                        "for...of requires an array, found `{}`",
                        format_type(&actual, self.records, self.unions)
                    ),
                    "expected an array value",
                );
                None
            }
            None => None,
        };

        self.scopes.push(HashMap::new());
        let _ = self.bind(&statement.binding, element_type, false);
        self.loop_depth += 1;
        let _ = self.check_block(&statement.body, false);
        self.loop_depth -= 1;
        let _ = self.scopes.pop();
    }

    fn check_arrow(
        &mut self,
        closure: ClosureId,
        parameters: &[Parameter],
        return_reference: &TypeReference,
        body: &ArrowBody,
        _span: SourceSpan,
    ) -> Option<Type> {
        let parameter_types = parameters
            .iter()
            .map(|parameter| {
                resolve_type(
                    &parameter.ty,
                    self.type_symbols,
                    &self.type_parameters,
                    self.diagnostics,
                    self.facts,
                )
            })
            .collect::<Vec<_>>();
        let return_type = resolve_type(
            return_reference,
            self.type_symbols,
            &self.type_parameters,
            self.diagnostics,
            self.facts,
        );

        self.closure_stack.push(ClosureCaptureState {
            id: closure,
            captures: Vec::new(),
            captured: HashSet::new(),
            rejected_mutable: HashSet::new(),
        });
        self.scopes.push(HashMap::new());
        let mut parameter_ids = Vec::with_capacity(parameters.len());
        for (parameter, ty) in parameters.iter().zip(&parameter_types) {
            if let Some(local) = self.bind(&parameter.name, ty.clone(), false) {
                parameter_ids.push(local);
            }
        }

        let outer_return_type = std::mem::replace(&mut self.return_type, return_type.clone());
        let outer_loop_depth = std::mem::replace(&mut self.loop_depth, 0);
        match body {
            ArrowBody::Expression(expression) => {
                let actual = self.check_expression_with_expected(expression, return_type.as_ref());
                if let (Some(expected), Some(actual)) = (&return_type, actual) {
                    if &actual != expected {
                        self.type_mismatch(expression.span(), expected, &actual);
                    }
                }
            }
            ArrowBody::Block(block) => {
                let always_returns = self.check_block(block, false);
                if return_type.as_ref().is_some_and(|ty| ty != &Type::Unit) && !always_returns {
                    self.error(
                        INVALID_RETURN,
                        return_reference.span,
                        format!(
                            "arrow may not return `{}` on every path",
                            return_type.as_ref().map_or_else(
                                || "value".to_owned(),
                                |ty| format_type(ty, self.records, self.unions)
                            )
                        ),
                        "return required on every path",
                    );
                }
            }
        }
        self.return_type = outer_return_type;
        self.loop_depth = outer_loop_depth;
        let _ = self.scopes.pop();
        let capture_state = self.closure_stack.pop();

        if let (Some(capture_state), Some(return_type)) = (capture_state, return_type.as_ref()) {
            if capture_state.id == closure
                && parameter_ids.len() == parameters.len()
                && parameter_types.iter().all(Option::is_some)
            {
                self.facts.closures.push(ClosureFacts {
                    id: closure,
                    parameters: parameter_ids,
                    parameter_types: parameter_types.iter().filter_map(Clone::clone).collect(),
                    return_type: return_type.clone(),
                    captures: capture_state.captures,
                });
            }
        }

        parameter_types
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .zip(return_type)
            .map(|(parameters, return_type)| Type::Function {
                parameters: parameters.into_boxed_slice(),
                return_type: Box::new(return_type),
            })
    }

    fn check_loop_control(&mut self, span: SourceSpan, keyword: &str) {
        if self.loop_depth == 0 {
            self.error(
                INVALID_LOOP_CONTROL,
                span,
                format!("`{keyword}` is only valid inside a loop"),
                "not inside a loop",
            );
        }
    }

    fn check_return(&mut self, statement: &ReturnStatement) {
        let Some(return_type) = self.return_type.clone() else {
            if let Some(value) = &statement.value {
                let _ = self.check_expression(value);
            }
            return;
        };
        match (&return_type, statement.value.as_ref()) {
            (Type::Unit, None) => {}
            (Type::Unit, Some(value)) => {
                let _ = self.check_expression(value);
                self.error(
                    INVALID_RETURN,
                    value.span(),
                    "`Unit` functions cannot return a value",
                    "unexpected return value",
                );
            }
            (expected, None) => self.error(
                INVALID_RETURN,
                statement.span,
                format!(
                    "expected a `{}` return value",
                    format_type(expected, self.records, self.unions)
                ),
                "missing return value",
            ),
            (expected, Some(value)) => {
                if let Some(actual) = self.check_expression_with_expected(value, Some(expected)) {
                    if &actual != expected {
                        self.error(
                            INVALID_RETURN,
                            value.span(),
                            format!(
                                "expected return type `{}`, found `{}`",
                                format_type(expected, self.records, self.unions),
                                format_type(&actual, self.records, self.unions)
                            ),
                            "invalid return value",
                        );
                    }
                }
            }
        }
    }

    fn check_expression(&mut self, expression: &Expression) -> Option<Type> {
        self.check_expression_with_expected(expression, None)
    }

    fn check_expression_with_expected(
        &mut self,
        expression: &Expression,
        expected: Option<&Type>,
    ) -> Option<Type> {
        let ty = match expression {
            Expression::Integer { .. } => Some(Type::Int),
            Expression::Boolean { .. } => Some(Type::Bool),
            Expression::String { .. } => Some(Type::String),
            Expression::Array { elements, span } => self.check_array(elements, *span, expected),
            Expression::Record { fields, span } => self.check_record(fields, *span, expected),
            Expression::Match {
                scrutinee,
                arms,
                span,
            } => self.check_match(scrutinee, arms, *span, expected),
            Expression::Index {
                collection,
                index,
                span,
            } => self.check_index(collection, index, *span),
            Expression::Member {
                object,
                member,
                span: _,
            } => self.check_member(object, member),
            Expression::Name(name) => match self.lookup_binding(name) {
                Some(binding) => {
                    self.record_capture(&binding, name.span);
                    self.facts
                        .record_name(name.span, NameResolution::Local(binding.id));
                    binding.ty
                }
                None => {
                    if let Some(signature) = self.functions.get(&name.text).cloned() {
                        self.facts.record_name(name.span, signature.resolution);
                        if !signature.type_parameters.is_empty() {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    GENERIC_FUNCTION_VALUE,
                                    format!(
                                        "generic function `{}` cannot be used as a value",
                                        name.text
                                    ),
                                )
                                .with_label(Label::primary(
                                    name.span,
                                    "polymorphic function value is not supported",
                                ))
                                .with_label(Label::secondary(
                                    signature.name_span,
                                    "generic function declared here",
                                )),
                            );
                            None
                        } else {
                            signature
                                .parameters
                                .into_iter()
                                .collect::<Option<Vec<_>>>()
                                .zip(signature.return_type)
                                .map(|(parameters, return_type)| Type::Function {
                                    parameters: parameters.into_boxed_slice(),
                                    return_type: Box::new(return_type),
                                })
                        }
                    } else {
                        self.error(
                            UNDEFINED_NAME,
                            name.span,
                            format!("undefined value `{}`", name.text),
                            "not found in this scope",
                        );
                        None
                    }
                }
            },
            Expression::Arrow {
                closure,
                parameters,
                return_type,
                body,
                span,
            } => self.check_arrow(*closure, parameters, return_type, body, *span),
            Expression::Unary {
                operator,
                expression,
                ..
            } => self.check_unary(*operator, expression),
            Expression::Binary {
                operator,
                left,
                right,
                ..
            } => self.check_binary(*operator, left, right),
            Expression::Call {
                callee,
                arguments,
                span,
            } => self.check_call(callee, arguments, *span, expected),
            Expression::Parenthesized { expression, .. } => {
                self.check_expression_with_expected(expression, expected)
            }
        };

        if let Some(ty) = &ty {
            self.facts.record_expression(expression.span(), ty.clone());
        }

        ty
    }

    fn check_array(
        &mut self,
        elements: &[Expression],
        span: SourceSpan,
        expected: Option<&Type>,
    ) -> Option<Type> {
        let contextual_element = match expected {
            Some(Type::Array(element)) => Some(element.as_ref()),
            _ => None,
        };

        if elements.is_empty() {
            let Some(element) = contextual_element else {
                self.error(
                    TYPE_MISMATCH,
                    span,
                    "cannot infer the element type of an empty array",
                    "add an exact array type context",
                );
                return None;
            };
            return (!contains_invalid_array_element(element))
                .then(|| Type::Array(Box::new(element.clone())));
        }

        let mut element_type = contextual_element.cloned();
        for element in elements {
            let actual = self.check_expression_with_expected(element, contextual_element);
            let Some(actual) = actual else {
                continue;
            };
            if actual == Type::Unit {
                self.error(
                    TYPE_MISMATCH,
                    element.span(),
                    "array elements cannot have type `Unit`",
                    "invalid array element",
                );
                continue;
            }

            if let Some(expected) = &element_type {
                if &actual != expected {
                    self.type_mismatch(element.span(), expected, &actual);
                }
            } else {
                element_type = Some(actual);
            }
        }

        element_type.map(|element| Type::Array(Box::new(element)))
    }

    fn check_record(
        &mut self,
        fields: &[RecordFieldInitializer],
        span: SourceSpan,
        expected: Option<&Type>,
    ) -> Option<Type> {
        let Some(Type::Record {
            definition: record_id,
            arguments,
        }) = expected
        else {
            for field in fields {
                let _ = self.check_expression(&field.value);
            }
            let message = expected.map_or_else(
                || "cannot infer the type of a record literal".to_owned(),
                |ty| {
                    format!(
                        "record literal requires a nominal record type, found `{}`",
                        format_type(ty, self.records, self.unions)
                    )
                },
            );
            self.error(
                TYPE_MISMATCH,
                span,
                message,
                "add an exact record type context",
            );
            return None;
        };
        let Some(record) = record_facts(self.records, *record_id).cloned() else {
            self.error(
                TYPE_MISMATCH,
                span,
                "record type has no resolved layout",
                "invalid record type",
            );
            return None;
        };
        let substitutions = substitutions_for_parameters(&record.type_parameters, arguments);

        let mut seen = HashMap::<String, SourceSpan>::new();
        let mut initialized = HashSet::new();
        for initializer in fields {
            let duplicate = if let Some(previous) = seen.get(&initializer.name.text).copied() {
                self.diagnostics.push(
                    Diagnostic::error(
                        DUPLICATE_NAME,
                        format!("duplicate field `{}`", initializer.name.text),
                    )
                    .with_label(Label::primary(
                        initializer.name.span,
                        "duplicate field initializer",
                    ))
                    .with_label(Label::secondary(previous, "first initialized here")),
                );
                true
            } else {
                seen.insert(initializer.name.text.clone(), initializer.name.span);
                false
            };
            let Some(field) = record
                .fields
                .iter()
                .find(|field| field.name == initializer.name.text)
            else {
                let _ = self.check_expression(&initializer.value);
                self.error(
                    UNKNOWN_MEMBER,
                    initializer.name.span,
                    format!(
                        "record `{}` has no field `{}`",
                        record.name, initializer.name.text
                    ),
                    "unknown record field",
                );
                continue;
            };

            self.facts
                .record_name(initializer.name.span, NameResolution::Field(field.id));
            let expected_field = substitute_type(&field.ty, &substitutions);
            let actual =
                self.check_expression_with_expected(&initializer.value, Some(&expected_field));
            if let Some(actual) = actual {
                if actual != expected_field {
                    self.type_mismatch(initializer.value.span(), &expected_field, &actual);
                }
            }
            if !duplicate {
                initialized.insert(field.id);
            }
        }

        for field in &record.fields {
            if !initialized.contains(&field.id) {
                self.diagnostics.push(
                    Diagnostic::error(
                        MISSING_FIELD,
                        format!(
                            "record literal for `{}` is missing field `{}`",
                            record.name, field.name
                        ),
                    )
                    .with_label(Label::primary(span, "missing record field"))
                    .with_label(Label::secondary(field.span, "field declared here")),
                );
            }
        }

        Some(Type::Record {
            definition: *record_id,
            arguments: arguments.clone(),
        })
    }

    fn check_match(
        &mut self,
        scrutinee: &Expression,
        arms: &[MatchArm],
        span: SourceSpan,
        expected: Option<&Type>,
    ) -> Option<Type> {
        let scrutinee_type = self.check_expression(scrutinee);
        let union_instance = match scrutinee_type {
            Some(Type::Union {
                definition,
                arguments,
            }) => Some((definition, arguments)),
            Some(actual) => {
                self.error(
                    TYPE_MISMATCH,
                    scrutinee.span(),
                    format!(
                        "match requires a tagged union, found `{}`",
                        format_type(&actual, self.records, self.unions)
                    ),
                    "expected a tagged union value",
                );
                None
            }
            None => None,
        };
        let union_id = union_instance.as_ref().map(|(definition, _)| *definition);
        let union = union_id.and_then(|union| union_facts(self.unions, union).cloned());
        let union_substitutions = union.as_ref().zip(union_instance.as_ref()).map_or_else(
            HashMap::new,
            |(union, (_, arguments))| {
                substitutions_for_parameters(&union.type_parameters, arguments)
            },
        );
        let mut covered = HashMap::<VariantId, SourceSpan>::new();
        let mut default_span = None;
        let mut complete_span = None;
        let mut coverage_reliable = union.is_some();
        let mut arm_facts = Vec::with_capacity(arms.len());
        let mut result_type = expected.cloned();

        for arm in arms {
            if let Some(previous) = default_span {
                self.diagnostics.push(
                    Diagnostic::error(UNREACHABLE_ARM, "match arm is unreachable")
                        .with_label(Label::primary(arm.span, "unreachable match arm"))
                        .with_label(Label::secondary(previous, "default arm appears here")),
                );
            }

            self.scopes.push(HashMap::new());
            let resolved = match &arm.pattern {
                MatchPattern::Variant {
                    union: qualifier,
                    variant,
                    bindings,
                    ..
                } => {
                    let variant_resolution = self.resolve_pattern_variant(qualifier, variant);
                    let mut payload_bindings = Vec::new();
                    if let Some((resolved_union, variant_facts)) = &variant_resolution {
                        if variant_facts.payloads.len() != bindings.len() {
                            coverage_reliable = false;
                            self.diagnostics.push(
                                Diagnostic::error(
                                    CALL_ARITY,
                                    format!(
                                        "variant `{}` expects {} payload binding(s), found {}",
                                        variant.text,
                                        variant_facts.payloads.len(),
                                        bindings.len()
                                    ),
                                )
                                .with_label(Label::primary(
                                    variant.span,
                                    "incorrect payload binding count",
                                ))
                                .with_label(Label::secondary(
                                    variant_facts.name_span,
                                    "variant declared here",
                                )),
                            );
                        }

                        for (index, binding) in bindings.iter().enumerate() {
                            let payload = variant_facts.payloads.get(index);
                            let local = self.bind(
                                binding,
                                payload.map(|payload| {
                                    if Some(*resolved_union) == union_id {
                                        substitute_type(&payload.ty, &union_substitutions)
                                    } else {
                                        payload.ty.clone()
                                    }
                                }),
                                false,
                            );
                            if let (Some(payload), Some(local)) = (payload, local) {
                                payload_bindings.push(PayloadBindingFacts {
                                    payload: payload.id,
                                    local,
                                });
                            }
                        }

                        if Some(*resolved_union) == union_id {
                            if let Some(previous) = covered.get(&variant_facts.id).copied() {
                                self.diagnostics.push(
                                    Diagnostic::error(
                                        DUPLICATE_NAME,
                                        format!("duplicate case for `{}`", variant.text),
                                    )
                                    .with_label(Label::primary(variant.span, "duplicate case"))
                                    .with_label(Label::secondary(previous, "first matched here")),
                                );
                            } else {
                                covered.insert(variant_facts.id, variant.span);
                                if union
                                    .as_ref()
                                    .is_some_and(|union| covered.len() == union.variants.len())
                                {
                                    complete_span = Some(arm.span);
                                }
                            }
                        } else if let Some(expected_union) = &union {
                            coverage_reliable = false;
                            self.diagnostics.push(
                                Diagnostic::error(
                                    TYPE_MISMATCH,
                                    format!(
                                        "case `{}` belongs to `{}` instead of `{}`",
                                        variant.text,
                                        self.union_name(*resolved_union),
                                        expected_union.name
                                    ),
                                )
                                .with_label(Label::primary(
                                    qualifier.span,
                                    "case uses a different union",
                                ))
                                .with_label(Label::secondary(
                                    expected_union.name_span,
                                    "matched union declared here",
                                )),
                            );
                        }

                        Some(MatchArmFacts::Variant {
                            variant: variant_facts.id,
                            bindings: payload_bindings,
                        })
                    } else {
                        coverage_reliable = false;
                        for binding in bindings {
                            let _ = self.bind(binding, None, false);
                        }
                        None
                    }
                }
                MatchPattern::Default { span: pattern_span } => {
                    if default_span.is_none() {
                        if let Some(previous) = complete_span {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    UNREACHABLE_ARM,
                                    "default arm is unreachable because all variants are covered",
                                )
                                .with_label(Label::primary(arm.span, "unreachable default arm"))
                                .with_label(Label::secondary(previous, "coverage completed here")),
                            );
                        }
                        default_span = Some(*pattern_span);
                    }
                    Some(MatchArmFacts::Default)
                }
            };

            let arm_expected = result_type.clone();
            let actual = self.check_expression_with_expected(&arm.value, arm_expected.as_ref());
            if let Some(actual) = actual {
                if let Some(required) = &result_type {
                    if &actual != required {
                        self.type_mismatch(arm.value.span(), required, &actual);
                    }
                } else {
                    result_type = Some(actual);
                }
            }
            let _ = self.scopes.pop();
            if let Some(resolved) = resolved {
                arm_facts.push(resolved);
            }
        }

        if let Some(union) = &union {
            if coverage_reliable && default_span.is_none() && covered.len() != union.variants.len()
            {
                let mut diagnostic = Diagnostic::error(
                    NON_EXHAUSTIVE_MATCH,
                    format!("match on `{}` is not exhaustive", union.name),
                )
                .with_label(Label::primary(span, "non-exhaustive match"));
                for variant in &union.variants {
                    if !covered.contains_key(&variant.id) {
                        diagnostic = diagnostic.with_label(Label::secondary(
                            variant.span,
                            format!("missing case `{}`", variant.name),
                        ));
                    }
                }
                self.diagnostics.push(diagnostic);
            }

            if arm_facts.len() == arms.len() {
                self.facts.record_match(
                    span,
                    MatchFacts {
                        union: union.id,
                        type_arguments: union_instance.as_ref().map_or_else(
                            || Vec::new().into_boxed_slice(),
                            |(_, arguments)| arguments.clone(),
                        ),
                        arms: arm_facts,
                    },
                );
            }
        }

        (!arms.is_empty()).then_some(result_type).flatten()
    }

    fn resolve_pattern_variant(
        &mut self,
        qualifier: &Name,
        variant: &Name,
    ) -> Option<(UnionId, VariantFacts)> {
        if let Some(parameter) = self.type_parameters.get(&qualifier.text).copied() {
            self.facts
                .record_name(qualifier.span, NameResolution::TypeParameter(parameter.id));
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "type parameter `{}` cannot qualify a match pattern",
                        qualifier.text
                    ),
                )
                .with_label(Label::primary(
                    qualifier.span,
                    "type parameter is not a tagged union declaration",
                ))
                .with_label(Label::secondary(parameter.span, "parameter declared here")),
            );
            return None;
        }

        let Some(entry) = self.type_symbols.get(&qualifier.text).copied() else {
            self.error(
                UNDEFINED_NAME,
                qualifier.span,
                format!("undefined union `{}`", qualifier.text),
                "not found in this program",
            );
            return None;
        };
        self.facts
            .record_name(qualifier.span, entry.symbol.resolution());
        let TypeSymbol::Union(union_id) = entry.symbol else {
            self.error(
                TYPE_MISMATCH,
                qualifier.span,
                format!("`{}` is a record, not a tagged union", qualifier.text),
                "expected a tagged union qualifier",
            );
            return None;
        };
        let union = union_facts(self.unions, union_id).cloned()?;
        let Some(variant_facts) = union
            .variants
            .iter()
            .find(|candidate| candidate.name == variant.text)
            .cloned()
        else {
            self.diagnostics.push(
                Diagnostic::error(
                    UNKNOWN_MEMBER,
                    format!("union `{}` has no variant `{}`", union.name, variant.text),
                )
                .with_label(Label::primary(variant.span, "unknown union variant"))
                .with_label(Label::secondary(union.name_span, "union declared here")),
            );
            return None;
        };
        self.facts
            .record_name(variant.span, NameResolution::Variant(variant_facts.id));

        Some((union_id, variant_facts))
    }

    fn union_name(&self, union: UnionId) -> String {
        union_facts(self.unions, union).map_or_else(
            || format!("union#{}:{}", union.module().index(), union.index()),
            |union| union.name.clone(),
        )
    }

    fn check_index(
        &mut self,
        collection: &Expression,
        index: &Expression,
        span: SourceSpan,
    ) -> Option<Type> {
        let collection_type = self.check_expression(collection);
        let index_type = self.check_expression(index);
        if let Some(index_type) = &index_type {
            if index_type != &Type::Int {
                self.type_mismatch(index.span(), &Type::Int, index_type);
            }
        }

        match collection_type {
            Some(Type::Array(element)) => (index_type == Some(Type::Int)).then_some(*element),
            Some(actual) => {
                self.error(
                    TYPE_MISMATCH,
                    span,
                    format!(
                        "cannot index a `{}` value",
                        format_type(&actual, self.records, self.unions)
                    ),
                    "expected an array value",
                );
                None
            }
            None => None,
        }
    }

    fn check_member(&mut self, object: &Expression, member: &Name) -> Option<Type> {
        if let Expression::Name(qualifier) = object {
            if self.lookup_binding(qualifier).is_none()
                && matches!(
                    self.type_symbols
                        .get(&qualifier.text)
                        .map(|entry| entry.symbol),
                    Some(TypeSymbol::Union(_))
                )
            {
                if let Some((_union, variant)) = self.resolve_pattern_variant(qualifier, member) {
                    self.error(
                        TYPE_MISMATCH,
                        member.span,
                        format!("variant constructor `{}` must be called", member.text),
                        "add constructor parentheses",
                    );
                    self.facts
                        .record_name(member.span, NameResolution::Variant(variant.id));
                    return None;
                }
            }
        }

        let object_type = self.check_expression(object);
        self.check_member_type(member, object_type)
    }

    fn check_member_type(&mut self, member: &Name, object_type: Option<Type>) -> Option<Type> {
        match object_type {
            Some(Type::Array(_)) if member.text == "length" => {
                self.facts
                    .record_name(member.span, NameResolution::Builtin(Builtin::ArrayLength));
                Some(Type::Int)
            }
            Some(Type::String) if member.text == "length" => {
                self.facts
                    .record_name(member.span, NameResolution::Builtin(Builtin::StringLength));
                Some(Type::Int)
            }
            Some(Type::Array(_)) if matches!(member.text.as_str(), "append" | "concat") => {
                self.error(
                    UNKNOWN_MEMBER,
                    member.span,
                    format!("array member `{}` must be called", member.text),
                    "array operations are not bound method values",
                );
                None
            }
            Some(Type::Record {
                definition: record_id,
                arguments,
            }) => {
                let Some(record) = record_facts(self.records, record_id) else {
                    self.error(
                        UNKNOWN_MEMBER,
                        member.span,
                        "record type has no resolved layout",
                        "invalid record type",
                    );
                    return None;
                };
                let Some(field) = record
                    .fields
                    .iter()
                    .find(|field| field.name == member.text)
                    .cloned()
                else {
                    let record_name = record.name.clone();
                    self.error(
                        UNKNOWN_MEMBER,
                        member.span,
                        format!("record `{record_name}` has no field `{}`", member.text),
                        "unknown record field",
                    );
                    return None;
                };
                self.facts
                    .record_name(member.span, NameResolution::Field(field.id));
                let substitutions =
                    substitutions_for_parameters(&record.type_parameters, &arguments);
                Some(substitute_type(&field.ty, &substitutions))
            }
            Some(Type::Union {
                definition: union_id,
                ..
            }) => {
                self.error(
                    TYPE_MISMATCH,
                    member.span,
                    format!(
                        "tagged union `{}` has no value member `{}`",
                        self.union_name(union_id),
                        member.text
                    ),
                    "variants are constructed through the union type",
                );
                None
            }
            Some(actual) => {
                self.error(
                    UNKNOWN_MEMBER,
                    member.span,
                    format!(
                        "type `{}` has no member `{}`",
                        format_type(&actual, self.records, self.unions),
                        member.text
                    ),
                    "unknown member",
                );
                None
            }
            None => None,
        }
    }

    fn check_unary(&mut self, operator: UnaryOperator, expression: &Expression) -> Option<Type> {
        let actual = self.check_expression(expression)?;
        let expected = match operator {
            UnaryOperator::Not => Type::Bool,
            UnaryOperator::Negate => Type::Int,
        };

        if actual != expected {
            self.type_mismatch(expression.span(), &expected, &actual);
            None
        } else {
            Some(expected)
        }
    }

    fn check_binary(
        &mut self,
        operator: BinaryOperator,
        left: &Expression,
        right: &Expression,
    ) -> Option<Type> {
        let left_type = self.check_expression(left);
        let right_type = self.check_expression(right);
        if matches!(
            operator,
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
        ) {
            return self.require_binary_bools(left, right, left_type, right_type);
        }

        let (Some(left_type), Some(right_type)) = (left_type, right_type) else {
            return None;
        };

        match operator {
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
                self.require_binary_bools(left, right, Some(left_type), Some(right_type))
            }
            BinaryOperator::Equal => {
                if matches!(
                    left_type,
                    Type::Array(_)
                        | Type::Function { .. }
                        | Type::Record { .. }
                        | Type::Union { .. }
                        | Type::Parameter(_)
                ) && left_type == right_type
                {
                    let kind = match left_type {
                        Type::Array(_) => "array",
                        Type::Function { .. } => "function",
                        Type::Record { .. } => "record",
                        Type::Union { .. } => "tagged union",
                        Type::Parameter(_) => "type parameter",
                        Type::Int | Type::Bool | Type::String | Type::Unit => "value",
                    };
                    self.error(
                        TYPE_MISMATCH,
                        right.span(),
                        format!("{kind} equality is not defined"),
                        format!("{kind}s cannot be compared with `===`"),
                    );
                    None
                } else if left_type != right_type {
                    self.error(
                        TYPE_MISMATCH,
                        right.span(),
                        format!(
                            "cannot compare `{}` with `{}` using `===`",
                            format_type(&left_type, self.records, self.unions),
                            format_type(&right_type, self.records, self.unions)
                        ),
                        "operands must have the same type",
                    );
                    None
                } else {
                    Some(Type::Bool)
                }
            }
            BinaryOperator::Less
            | BinaryOperator::LessEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual => {
                self.require_binary_ints(left, right, left_type, right_type, Type::Bool)
            }
            BinaryOperator::Add => {
                if left_type == Type::String && right_type == Type::String {
                    Some(Type::String)
                } else {
                    self.require_binary_ints(left, right, left_type, right_type, Type::Int)
                }
            }
            BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => {
                self.require_binary_ints(left, right, left_type, right_type, Type::Int)
            }
        }
    }

    fn require_binary_bools(
        &mut self,
        left: &Expression,
        right: &Expression,
        left_type: Option<Type>,
        right_type: Option<Type>,
    ) -> Option<Type> {
        if let Some(left_type) = &left_type {
            if left_type != &Type::Bool {
                self.type_mismatch(left.span(), &Type::Bool, left_type);
            }
        }
        if let Some(right_type) = &right_type {
            if right_type != &Type::Bool {
                self.type_mismatch(right.span(), &Type::Bool, right_type);
            }
        }

        (left_type == Some(Type::Bool) && right_type == Some(Type::Bool)).then_some(Type::Bool)
    }

    fn require_binary_ints(
        &mut self,
        left: &Expression,
        right: &Expression,
        left_type: Type,
        right_type: Type,
        result: Type,
    ) -> Option<Type> {
        if left_type != Type::Int {
            self.type_mismatch(left.span(), &Type::Int, &left_type);
        }
        if right_type != Type::Int {
            self.type_mismatch(right.span(), &Type::Int, &right_type);
        }

        (left_type == Type::Int && right_type == Type::Int).then_some(result)
    }

    fn check_call(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        span: SourceSpan,
        expected_result: Option<&Type>,
    ) -> Option<Type> {
        if let Expression::Member { object, member, .. } = callee {
            if let Expression::Name(qualifier) = object.as_ref() {
                let has_type_qualifier = self.type_symbols.contains_key(&qualifier.text);
                let supports_value_members =
                    self.lookup_binding(qualifier).is_some_and(|binding| {
                        matches!(binding.ty, Some(Type::Array(_) | Type::Record { .. }))
                    });
                if has_type_qualifier || !supports_value_members {
                    return self.check_variant_constructor(
                        qualifier,
                        member,
                        arguments,
                        span,
                        expected_result,
                    );
                }
            }

            return self.check_member_call(callee, object, member, arguments, span);
        }

        let Expression::Name(name) = callee else {
            return self.check_indirect_call(callee, arguments, span);
        };

        if self.lookup_binding(name).is_some() {
            return self.check_indirect_call(callee, arguments, span);
        }

        if name.text == "print" {
            self.facts
                .record_name(name.span, NameResolution::Builtin(Builtin::Print));
            let argument_types = arguments
                .iter()
                .map(|argument| self.check_expression(argument))
                .collect::<Vec<_>>();
            if arguments.len() != 1 {
                self.diagnostics.push(call_arity_diagnostic(
                    span,
                    format!(
                        "function `print` expects 1 argument(s), found {}",
                        arguments.len()
                    ),
                    None,
                ));
            }
            for (argument, actual) in arguments.iter().zip(argument_types) {
                if let Some(actual) = actual {
                    if !matches!(actual, Type::Int | Type::Bool | Type::String) {
                        self.error(
                            TYPE_MISMATCH,
                            argument.span(),
                            format!(
                                "`print` cannot display `{}`",
                                format_type(&actual, self.records, self.unions)
                            ),
                            "expected `Int`, `Bool`, or `String`",
                        );
                    }
                }
            }
            return Some(Type::Unit);
        }

        let scalar_builtin = match name.text.as_str() {
            "toString" => Some((Builtin::ToString, Type::Int, Type::String)),
            "parseInt" => Some((Builtin::ParseInt, Type::String, Type::Int)),
            _ => None,
        };
        if let Some((builtin, parameter, result)) = scalar_builtin {
            self.facts
                .record_name(name.span, NameResolution::Builtin(builtin));
            if arguments.len() != 1 {
                self.error(
                    CALL_ARITY,
                    span,
                    format!(
                        "function `{}` expects 1 argument(s), found {}",
                        name.text,
                        arguments.len()
                    ),
                    "incorrect argument count",
                );
            }
            for (index, argument) in arguments.iter().enumerate() {
                let expected = (index == 0).then_some(&parameter);
                let actual = self.check_expression_with_expected(argument, expected);
                if let (Some(expected), Some(actual)) = (expected, actual) {
                    if &actual != expected {
                        self.type_mismatch(argument.span(), expected, &actual);
                    }
                }
            }
            return Some(result);
        }

        let Some(signature) = self.functions.get(&name.text).cloned() else {
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.error(
                UNDEFINED_NAME,
                name.span,
                format!("undefined function `{}`", name.text),
                "not found in this program",
            );
            return None;
        };
        self.facts.record_name(name.span, signature.resolution);

        if signature.parameters.len() != arguments.len() {
            self.diagnostics.push(call_arity_diagnostic(
                span,
                format!(
                    "function `{}` expects {} argument(s), found {}",
                    name.text,
                    signature.parameters.len(),
                    arguments.len()
                ),
                Some(signature.name_span),
            ));
        }

        self.check_resolved_function_call(name, arguments, span, expected_result, &signature)
    }

    fn check_member_call(
        &mut self,
        callee: &Expression,
        object: &Expression,
        member: &Name,
        arguments: &[Expression],
        span: SourceSpan,
    ) -> Option<Type> {
        let object_type = self.check_expression(object);
        if let Some(Type::Array(element)) = object_type.as_ref() {
            let intrinsic = match member.text.as_str() {
                "append" => Some((Builtin::ArrayAppend, element.as_ref().clone())),
                "concat" => Some((
                    Builtin::ArrayConcat,
                    Type::Array(Box::new(element.as_ref().clone())),
                )),
                _ => None,
            };
            if let Some((builtin, parameter)) = intrinsic {
                self.facts
                    .record_name(member.span, NameResolution::Builtin(builtin));
                if arguments.len() != 1 {
                    self.error(
                        CALL_ARITY,
                        span,
                        format!(
                            "array `{}` expects 1 argument(s), found {}",
                            member.text,
                            arguments.len()
                        ),
                        "incorrect argument count",
                    );
                }
                for (index, argument) in arguments.iter().enumerate() {
                    let expected = (index == 0).then_some(&parameter);
                    let actual = self.check_expression_with_expected(argument, expected);
                    if let (Some(expected), Some(actual)) = (expected, actual) {
                        if &actual != expected {
                            self.type_mismatch(argument.span(), expected, &actual);
                        }
                    }
                }
                return Some(Type::Array(element.clone()));
            }
        }

        let callee_type = self.check_member_type(member, object_type);
        if let Some(ty) = &callee_type {
            self.facts.record_expression(callee.span(), ty.clone());
        }
        self.check_indirect_call_with_type(callee, arguments, span, callee_type)
    }

    fn check_indirect_call(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        span: SourceSpan,
    ) -> Option<Type> {
        let callee_type = self.check_expression(callee);
        self.check_indirect_call_with_type(callee, arguments, span, callee_type)
    }

    fn check_indirect_call_with_type(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        span: SourceSpan,
        callee_type: Option<Type>,
    ) -> Option<Type> {
        let Some(Type::Function {
            parameters,
            return_type,
        }) = callee_type
        else {
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            if let Some(actual) = callee_type {
                self.error(
                    TYPE_MISMATCH,
                    callee.span(),
                    format!(
                        "cannot call a `{}` value",
                        format_type(&actual, self.records, self.unions)
                    ),
                    "not a function",
                );
            }
            return None;
        };

        if parameters.len() != arguments.len() {
            self.error(
                CALL_ARITY,
                span,
                format!(
                    "function value expects {} argument(s), found {}",
                    parameters.len(),
                    arguments.len()
                ),
                "incorrect argument count",
            );
        }

        for (index, argument) in arguments.iter().enumerate() {
            let expected = parameters.get(index);
            let actual = self.check_expression_with_expected(argument, expected);
            if let (Some(expected), Some(actual)) = (expected, actual) {
                if &actual != expected {
                    self.type_mismatch(argument.span(), expected, &actual);
                }
            }
        }

        Some(*return_type)
    }

    fn check_resolved_function_call(
        &mut self,
        name: &Name,
        arguments: &[Expression],
        span: SourceSpan,
        expected_result: Option<&Type>,
        signature: &FunctionSignature,
    ) -> Option<Type> {
        let NameResolution::Function(function) = signature.resolution else {
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.error(
                TYPE_MISMATCH,
                name.span,
                "resolved callable is not a source function",
                "invalid function resolution",
            );
            return None;
        };
        let parameter_ids = signature
            .type_parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<HashSet<_>>();
        let mut substitutions = HashMap::new();
        let mut contextual_substitutions = HashMap::new();
        if let (Some(return_type), Some(expected_result)) =
            (signature.return_type.as_ref(), expected_result)
        {
            infer_missing_type_arguments(
                return_type,
                expected_result,
                &parameter_ids,
                &mut contextual_substitutions,
            );
        }
        let mut actual_types = vec![None; arguments.len()];
        let mut conflicting_arguments = HashSet::new();
        let mut first_constraint_spans = HashMap::new();
        let mut deferred_arguments = Vec::new();

        for (index, argument) in arguments.iter().enumerate() {
            let formal = signature.parameters.get(index).and_then(Option::as_ref);
            let mut contextual_mapping = contextual_substitutions.clone();
            contextual_mapping.extend(substitutions.clone());
            let contextual = formal
                .map(|formal| substitute_type(formal, &contextual_mapping))
                .filter(|formal| !formal_contains_any_parameter(formal, &parameter_ids));
            if formal.is_some()
                && contextual.is_none()
                && expression_requires_complete_context(argument)
            {
                deferred_arguments.push(index);
                continue;
            }
            let actual = self.check_expression_with_expected(argument, contextual.as_ref());
            if let (Some(formal), Some(actual)) = (formal, actual.as_ref()) {
                if let Err(UnificationError::Conflict(parameter)) = unify_type_arguments(
                    formal,
                    actual,
                    &parameter_ids,
                    &mut substitutions,
                    &mut first_constraint_spans,
                    argument.span(),
                ) {
                    conflicting_arguments.insert(index);
                    self.report_inference_conflict(
                        argument.span(),
                        format!("conflicting type constraints while calling `{}`", name.text),
                        parameter,
                        &signature.type_parameters,
                        &first_constraint_spans,
                    );
                }
            }
            actual_types[index] = actual;
        }

        if let (Some(return_type), Some(expected_result)) =
            (signature.return_type.as_ref(), expected_result)
        {
            infer_missing_type_arguments(
                return_type,
                expected_result,
                &parameter_ids,
                &mut substitutions,
            );
        }

        for index in deferred_arguments {
            let Some(argument) = arguments.get(index) else {
                continue;
            };
            let formal = signature.parameters.get(index).and_then(Option::as_ref);
            let contextual = formal
                .map(|formal| substitute_type(formal, &substitutions))
                .filter(|formal| !formal_contains_any_parameter(formal, &parameter_ids));
            let actual = self.check_expression_with_expected(argument, contextual.as_ref());
            if let (Some(formal), Some(actual)) = (formal, actual.as_ref()) {
                if let Err(UnificationError::Conflict(parameter)) = unify_type_arguments(
                    formal,
                    actual,
                    &parameter_ids,
                    &mut substitutions,
                    &mut first_constraint_spans,
                    argument.span(),
                ) {
                    conflicting_arguments.insert(index);
                    self.report_inference_conflict(
                        argument.span(),
                        format!("conflicting type constraints while calling `{}`", name.text),
                        parameter,
                        &signature.type_parameters,
                        &first_constraint_spans,
                    );
                }
            }
            actual_types[index] = actual;
        }

        let missing = signature
            .type_parameters
            .iter()
            .filter(|parameter| !substitutions.contains_key(&parameter.id))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            let missing_names = missing
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let mut diagnostic = Diagnostic::error(
                TYPE_INFERENCE,
                format!(
                    "cannot infer type argument(s) {} for `{}`",
                    missing_names, name.text
                ),
            )
            .with_label(Label::primary(span, "insufficient local type information"));
            for parameter in missing {
                diagnostic = diagnostic.with_label(Label::secondary(
                    parameter.span,
                    format!("type parameter `{}` declared here", parameter.name),
                ));
            }
            self.diagnostics.push(diagnostic);
        }

        for (index, ((argument, actual), formal)) in arguments
            .iter()
            .zip(&actual_types)
            .zip(&signature.parameters)
            .enumerate()
        {
            if conflicting_arguments.contains(&index) {
                continue;
            }
            if let (Some(actual), Some(formal)) = (actual, formal) {
                let expected = substitute_type(formal, &substitutions);
                if actual != &expected && !type_contains_parameter(&expected) {
                    self.type_mismatch(argument.span(), &expected, actual);
                }
            }
        }

        let type_arguments = signature
            .type_parameters
            .iter()
            .filter_map(|parameter| substitutions.get(&parameter.id).cloned())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        if type_arguments.len() == signature.type_parameters.len() {
            self.facts.record_call(
                span,
                CallFacts {
                    function,
                    type_arguments: type_arguments.clone(),
                },
            );
            self.facts.generic_calls.push(GenericCallEdge {
                caller: self.function_id,
                callee: function,
                arguments: type_arguments.clone(),
                span,
            });
            self.facts
                .record_instance(DefId::Function(function), &type_arguments, span);
        }

        signature
            .return_type
            .as_ref()
            .map(|return_type| substitute_type(return_type, &substitutions))
            .filter(|return_type| !formal_contains_any_parameter(return_type, &parameter_ids))
    }

    fn check_variant_constructor(
        &mut self,
        qualifier: &Name,
        variant: &Name,
        arguments: &[Expression],
        span: SourceSpan,
        expected_result: Option<&Type>,
    ) -> Option<Type> {
        if let Some(parameter) = self.type_parameters.get(&qualifier.text).copied() {
            self.facts
                .record_name(qualifier.span, NameResolution::TypeParameter(parameter.id));
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "type parameter `{}` has no variant constructors",
                        qualifier.text
                    ),
                )
                .with_label(Label::primary(
                    qualifier.span,
                    "type parameter is not a tagged union declaration",
                ))
                .with_label(Label::secondary(parameter.span, "parameter declared here")),
            );
            return None;
        }

        let Some(entry) = self.type_symbols.get(&qualifier.text).copied() else {
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.error(
                UNDEFINED_NAME,
                qualifier.span,
                format!("undefined union `{}`", qualifier.text),
                "not found in this program",
            );
            return None;
        };
        self.facts
            .record_name(qualifier.span, entry.symbol.resolution());
        let TypeSymbol::Union(union_id) = entry.symbol else {
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.error(
                TYPE_MISMATCH,
                qualifier.span,
                format!("`{}` is a record, not a tagged union", qualifier.text),
                "record types have no variant constructors",
            );
            return None;
        };
        let Some(union) = union_facts(self.unions, union_id).cloned() else {
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.error(
                TYPE_MISMATCH,
                qualifier.span,
                "union type has no resolved declaration facts",
                "invalid union type",
            );
            return None;
        };
        let Some(variant_facts) = union
            .variants
            .iter()
            .find(|candidate| candidate.name == variant.text)
            .cloned()
        else {
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.diagnostics.push(
                Diagnostic::error(
                    UNKNOWN_MEMBER,
                    format!("union `{}` has no variant `{}`", union.name, variant.text),
                )
                .with_label(Label::primary(variant.span, "unknown union variant"))
                .with_label(Label::secondary(union.name_span, "union declared here")),
            );
            return None;
        };
        self.facts
            .record_name(variant.span, NameResolution::Variant(variant_facts.id));

        if variant_facts.payloads.len() != arguments.len() {
            self.diagnostics.push(
                Diagnostic::error(
                    CALL_ARITY,
                    format!(
                        "variant `{}` expects {} payload value(s), found {}",
                        variant.text,
                        variant_facts.payloads.len(),
                        arguments.len()
                    ),
                )
                .with_label(Label::primary(
                    variant.span,
                    "incorrect constructor argument count",
                ))
                .with_label(Label::secondary(
                    variant_facts.name_span,
                    "variant declared here",
                )),
            );
        }

        let parameter_ids = union
            .type_parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<HashSet<_>>();
        let mut substitutions = HashMap::new();
        let mut contextual_substitutions = HashMap::new();
        if let Some(Type::Union {
            definition,
            arguments: expected_arguments,
        }) = expected_result
        {
            if *definition == union_id {
                for (parameter, argument) in
                    union.type_parameters.iter().zip(expected_arguments.iter())
                {
                    contextual_substitutions.insert(parameter.id, argument.clone());
                }
            }
        }
        let mut actual_types = vec![None; arguments.len()];
        let mut conflicting_arguments = HashSet::new();
        let mut first_constraint_spans = HashMap::new();
        let mut deferred_arguments = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let payload = variant_facts.payloads.get(index);
            let mut contextual_mapping = contextual_substitutions.clone();
            contextual_mapping.extend(substitutions.clone());
            let contextual = payload
                .map(|payload| substitute_type(&payload.ty, &contextual_mapping))
                .filter(|ty| !formal_contains_any_parameter(ty, &parameter_ids));
            if payload.is_some()
                && contextual.is_none()
                && expression_requires_complete_context(argument)
            {
                deferred_arguments.push(index);
                continue;
            }
            let actual = self.check_expression_with_expected(argument, contextual.as_ref());
            if let (Some(payload), Some(actual)) = (payload, actual.as_ref()) {
                if let Err(UnificationError::Conflict(parameter)) = unify_type_arguments(
                    &payload.ty,
                    actual,
                    &parameter_ids,
                    &mut substitutions,
                    &mut first_constraint_spans,
                    argument.span(),
                ) {
                    conflicting_arguments.insert(index);
                    self.report_inference_conflict(
                        argument.span(),
                        format!(
                            "conflicting type constraints for `{}.{}`",
                            qualifier.text, variant.text
                        ),
                        parameter,
                        &union.type_parameters,
                        &first_constraint_spans,
                    );
                }
            }
            actual_types[index] = actual;
        }

        if let Some(Type::Union {
            definition,
            arguments: expected_arguments,
        }) = expected_result
        {
            if *definition == union_id {
                for (parameter, argument) in
                    union.type_parameters.iter().zip(expected_arguments.iter())
                {
                    substitutions
                        .entry(parameter.id)
                        .or_insert_with(|| argument.clone());
                }
            }
        }

        for index in deferred_arguments {
            let Some(argument) = arguments.get(index) else {
                continue;
            };
            let payload = variant_facts.payloads.get(index);
            let contextual = payload
                .map(|payload| substitute_type(&payload.ty, &substitutions))
                .filter(|ty| !formal_contains_any_parameter(ty, &parameter_ids));
            let actual = self.check_expression_with_expected(argument, contextual.as_ref());
            if let (Some(payload), Some(actual)) = (payload, actual.as_ref()) {
                if let Err(UnificationError::Conflict(parameter)) = unify_type_arguments(
                    &payload.ty,
                    actual,
                    &parameter_ids,
                    &mut substitutions,
                    &mut first_constraint_spans,
                    argument.span(),
                ) {
                    conflicting_arguments.insert(index);
                    self.report_inference_conflict(
                        argument.span(),
                        format!(
                            "conflicting type constraints for `{}.{}`",
                            qualifier.text, variant.text
                        ),
                        parameter,
                        &union.type_parameters,
                        &first_constraint_spans,
                    );
                }
            }
            actual_types[index] = actual;
        }

        let missing = union
            .type_parameters
            .iter()
            .filter(|parameter| !substitutions.contains_key(&parameter.id))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            let missing_names = missing
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let mut diagnostic = Diagnostic::error(
                TYPE_INFERENCE,
                format!(
                    "cannot infer type argument(s) {} for `{}`",
                    missing_names, qualifier.text
                ),
            )
            .with_label(Label::primary(span, "insufficient local type information"));
            for parameter in missing {
                diagnostic = diagnostic.with_label(Label::secondary(
                    parameter.span,
                    format!("type parameter `{}` declared here", parameter.name),
                ));
            }
            self.diagnostics.push(diagnostic);
        }

        for (index, ((argument, actual), payload)) in arguments
            .iter()
            .zip(&actual_types)
            .zip(&variant_facts.payloads)
            .enumerate()
        {
            if conflicting_arguments.contains(&index) {
                continue;
            }
            if let Some(actual) = actual {
                let expected = substitute_type(&payload.ty, &substitutions);
                if actual != &expected && !type_contains_parameter(&expected) {
                    self.type_mismatch(argument.span(), &expected, actual);
                }
            }
        }

        let type_arguments = union
            .type_parameters
            .iter()
            .filter_map(|parameter| substitutions.get(&parameter.id).cloned())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        if type_arguments.len() != union.type_parameters.len() {
            return None;
        }
        self.facts.record_variant_construction(
            span,
            VariantConstructionFacts {
                union: union_id,
                variant: variant_facts.id,
                type_arguments: type_arguments.clone(),
            },
        );
        self.facts
            .record_instance(DefId::Union(union_id), &type_arguments, span);
        Some(Type::Union {
            definition: union_id,
            arguments: type_arguments,
        })
    }

    fn bind(&mut self, name: &Name, ty: Option<Type>, mutable: bool) -> Option<LocalId> {
        let closure_depth = self.closure_stack.len();
        let scope = self.scopes.last_mut()?;

        if let Some(previous) = scope.get(&name.text) {
            let diagnostic =
                Diagnostic::error(DUPLICATE_NAME, format!("duplicate binding `{}`", name.text))
                    .with_label(Label::primary(name.span, "duplicate binding"))
                    .with_label(Label::secondary(previous.span, "first declared here"));
            self.diagnostics.push(diagnostic);
            None
        } else {
            let id = LocalId::new(self.next_local);
            self.next_local += 1;
            scope.insert(
                name.text.clone(),
                Binding {
                    id,
                    ty,
                    span: name.span,
                    mutable,
                    closure_depth,
                },
            );
            self.facts.record_name(name.span, NameResolution::Local(id));
            Some(id)
        }
    }

    fn lookup_binding(&self, name: &Name) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&name.text).cloned())
    }

    fn record_capture(&mut self, binding: &Binding, reference: SourceSpan) {
        if binding.closure_depth >= self.closure_stack.len() {
            return;
        }

        let mut mutable_capture_count = 0;
        for closure in self.closure_stack.iter_mut().skip(binding.closure_depth) {
            if binding.mutable {
                if closure.rejected_mutable.insert(binding.id) {
                    mutable_capture_count += 1;
                }
            } else if closure.captured.insert(binding.id) {
                closure.captures.push(binding.id);
            }
        }

        for _ in 0..mutable_capture_count {
            self.diagnostics.push(
                Diagnostic::error(
                    MUTABLE_CAPTURE,
                    "closures cannot capture mutable `let` bindings",
                )
                .with_label(Label::primary(reference, "mutable capture required here"))
                .with_label(Label::secondary(
                    binding.span,
                    "mutable binding declared here",
                )),
            );
        }
    }

    fn type_mismatch(&mut self, span: SourceSpan, expected: &Type, actual: &Type) {
        self.diagnostics.push(type_mismatch_diagnostic(
            span,
            expected,
            actual,
            self.records,
            self.unions,
        ));
    }

    fn report_inference_conflict(
        &mut self,
        span: SourceSpan,
        message: impl Into<String>,
        parameter: TypeParameterId,
        parameters: &[TypeParameterFacts],
        first_constraint_spans: &HashMap<TypeParameterId, SourceSpan>,
    ) {
        let mut diagnostic = Diagnostic::error(TYPE_INFERENCE, message)
            .with_label(Label::primary(span, "conflicting type argument"));
        if let Some(parameter_facts) = parameters
            .iter()
            .find(|candidate| candidate.id == parameter)
        {
            diagnostic = diagnostic.with_label(Label::secondary(
                parameter_facts.span,
                format!("type parameter `{}` declared here", parameter_facts.name),
            ));
        }
        if let Some(first_constraint) = first_constraint_spans.get(&parameter).copied() {
            diagnostic = diagnostic.with_label(Label::secondary(
                first_constraint,
                "type parameter first constrained here",
            ));
        }
        self.diagnostics.push(diagnostic);
    }

    fn error(
        &mut self,
        code: DiagnosticCode,
        span: SourceSpan,
        message: impl Into<String>,
        label: impl Into<String>,
    ) {
        self.diagnostics
            .push(Diagnostic::error(code, message).with_label(Label::primary(span, label)));
    }

    fn finish(self) {
        self.facts.functions.push(FunctionFacts {
            id: self.function_id,
            type_parameters: self.declared_type_parameters,
            parameters: self.parameters,
            parameter_types: self.parameter_types,
            return_type: self.return_type.unwrap_or(Type::Unit),
            local_count: self.next_local,
        });
    }
}

fn type_mismatch_diagnostic(
    span: SourceSpan,
    expected: &Type,
    actual: &Type,
    records: &[RecordFacts],
    unions: &[UnionFacts],
) -> Diagnostic {
    Diagnostic::error(
        TYPE_MISMATCH,
        format!(
            "expected `{}`, found `{}`",
            format_type(expected, records, unions),
            format_type(actual, records, unions)
        ),
    )
    .with_label(Label::primary(span, "type mismatch"))
}

fn call_arity_diagnostic(
    span: SourceSpan,
    message: impl Into<String>,
    declaration: Option<SourceSpan>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(CALL_ARITY, message)
        .with_label(Label::primary(span, "incorrect argument count"));
    if let Some(declaration) = declaration {
        diagnostic = diagnostic.with_label(Label::secondary(declaration, "function declared here"));
    }
    diagnostic
}

fn expression_requires_complete_context(expression: &Expression) -> bool {
    match expression {
        Expression::Record { .. } => true,
        Expression::Array { elements, .. } => {
            elements.is_empty() || elements.iter().any(expression_requires_complete_context)
        }
        Expression::Match { arms, .. } => arms
            .iter()
            .any(|arm| expression_requires_complete_context(&arm.value)),
        Expression::Call { arguments, .. } => {
            arguments.is_empty() || arguments.iter().any(expression_requires_complete_context)
        }
        Expression::Parenthesized { expression, .. } => {
            expression_requires_complete_context(expression)
        }
        Expression::Arrow { .. } => false,
        Expression::Integer { .. }
        | Expression::Boolean { .. }
        | Expression::String { .. }
        | Expression::Index { .. }
        | Expression::Member { .. }
        | Expression::Name(_)
        | Expression::Unary { .. }
        | Expression::Binary { .. } => false,
    }
}

fn contains_invalid_array_element(ty: &Type) -> bool {
    match ty {
        Type::Array(element) => {
            element.as_ref() == &Type::Unit || contains_invalid_array_element(element)
        }
        Type::Function {
            parameters,
            return_type,
        } => {
            parameters.iter().any(contains_invalid_array_element)
                || contains_invalid_array_element(return_type)
        }
        Type::Int
        | Type::Bool
        | Type::String
        | Type::Parameter(_)
        | Type::Record { .. }
        | Type::Union { .. }
        | Type::Unit => false,
    }
}

fn type_contains_parameter(ty: &Type) -> bool {
    match ty {
        Type::Parameter(_) => true,
        Type::Array(element) => type_contains_parameter(element),
        Type::Function {
            parameters,
            return_type,
        } => parameters.iter().any(type_contains_parameter) || type_contains_parameter(return_type),
        Type::Record { arguments, .. } | Type::Union { arguments, .. } => {
            arguments.iter().any(type_contains_parameter)
        }
        Type::Int | Type::Bool | Type::String | Type::Unit => false,
    }
}

fn type_contains_parameter_id(ty: &Type, parameter: TypeParameterId) -> bool {
    match ty {
        Type::Parameter(candidate) => *candidate == parameter,
        Type::Array(element) => type_contains_parameter_id(element, parameter),
        Type::Function {
            parameters,
            return_type,
        } => {
            parameters
                .iter()
                .any(|ty| type_contains_parameter_id(ty, parameter))
                || type_contains_parameter_id(return_type, parameter)
        }
        Type::Record { arguments, .. } | Type::Union { arguments, .. } => arguments
            .iter()
            .any(|argument| type_contains_parameter_id(argument, parameter)),
        Type::Int | Type::Bool | Type::String | Type::Unit => false,
    }
}

fn formal_contains_any_parameter(ty: &Type, parameters: &HashSet<TypeParameterId>) -> bool {
    match ty {
        Type::Parameter(parameter) => parameters.contains(parameter),
        Type::Array(element) => formal_contains_any_parameter(element, parameters),
        Type::Function {
            parameters: function_parameters,
            return_type,
        } => {
            function_parameters
                .iter()
                .any(|ty| formal_contains_any_parameter(ty, parameters))
                || formal_contains_any_parameter(return_type, parameters)
        }
        Type::Record { arguments, .. } | Type::Union { arguments, .. } => arguments
            .iter()
            .any(|argument| formal_contains_any_parameter(argument, parameters)),
        Type::Int | Type::Bool | Type::String | Type::Unit => false,
    }
}

fn substitute_type(ty: &Type, substitutions: &HashMap<TypeParameterId, Type>) -> Type {
    match ty {
        Type::Parameter(parameter) => substitutions
            .get(parameter)
            .cloned()
            .unwrap_or(Type::Parameter(*parameter)),
        Type::Array(element) => Type::Array(Box::new(substitute_type(element, substitutions))),
        Type::Function {
            parameters,
            return_type,
        } => Type::Function {
            parameters: parameters
                .iter()
                .map(|parameter| substitute_type(parameter, substitutions))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            return_type: Box::new(substitute_type(return_type, substitutions)),
        },
        Type::Record {
            definition,
            arguments,
        } => Type::Record {
            definition: *definition,
            arguments: arguments
                .iter()
                .map(|argument| substitute_type(argument, substitutions))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
        Type::Union {
            definition,
            arguments,
        } => Type::Union {
            definition: *definition,
            arguments: arguments
                .iter()
                .map(|argument| substitute_type(argument, substitutions))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
        Type::Int => Type::Int,
        Type::Bool => Type::Bool,
        Type::String => Type::String,
        Type::Unit => Type::Unit,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnificationError {
    Conflict(TypeParameterId),
    Incompatible,
}

fn unify_type_arguments(
    template: &Type,
    actual: &Type,
    parameters: &HashSet<TypeParameterId>,
    substitutions: &mut HashMap<TypeParameterId, Type>,
    first_constraint_spans: &mut HashMap<TypeParameterId, SourceSpan>,
    constraint_span: SourceSpan,
) -> Result<(), UnificationError> {
    match (template, actual) {
        (Type::Parameter(parameter), actual) if parameters.contains(parameter) => {
            if let Some(previous) = substitutions.get(parameter) {
                if previous == actual {
                    first_constraint_spans
                        .entry(*parameter)
                        .or_insert(constraint_span);
                    Ok(())
                } else {
                    Err(UnificationError::Conflict(*parameter))
                }
            } else {
                substitutions.insert(*parameter, actual.clone());
                first_constraint_spans.insert(*parameter, constraint_span);
                Ok(())
            }
        }
        (Type::Array(template), Type::Array(actual)) => unify_type_arguments(
            template,
            actual,
            parameters,
            substitutions,
            first_constraint_spans,
            constraint_span,
        ),
        (
            Type::Function {
                parameters: template_parameters,
                return_type: template_return,
            },
            Type::Function {
                parameters: actual_parameters,
                return_type: actual_return,
            },
        ) if template_parameters.len() == actual_parameters.len() => {
            unify_type_argument_lists(
                template_parameters,
                actual_parameters,
                parameters,
                substitutions,
                first_constraint_spans,
                constraint_span,
            )?;
            unify_type_arguments(
                template_return,
                actual_return,
                parameters,
                substitutions,
                first_constraint_spans,
                constraint_span,
            )
        }
        (
            Type::Record {
                definition: template_definition,
                arguments: template_arguments,
            },
            Type::Record {
                definition: actual_definition,
                arguments: actual_arguments,
            },
        ) if template_definition == actual_definition
            && template_arguments.len() == actual_arguments.len() =>
        {
            unify_type_argument_lists(
                template_arguments,
                actual_arguments,
                parameters,
                substitutions,
                first_constraint_spans,
                constraint_span,
            )
        }
        (
            Type::Union {
                definition: template_definition,
                arguments: template_arguments,
            },
            Type::Union {
                definition: actual_definition,
                arguments: actual_arguments,
            },
        ) if template_definition == actual_definition
            && template_arguments.len() == actual_arguments.len() =>
        {
            unify_type_argument_lists(
                template_arguments,
                actual_arguments,
                parameters,
                substitutions,
                first_constraint_spans,
                constraint_span,
            )
        }
        _ if template == actual => Ok(()),
        _ => Err(UnificationError::Incompatible),
    }
}

fn unify_type_argument_lists(
    templates: &[Type],
    actuals: &[Type],
    parameters: &HashSet<TypeParameterId>,
    substitutions: &mut HashMap<TypeParameterId, Type>,
    first_constraint_spans: &mut HashMap<TypeParameterId, SourceSpan>,
    constraint_span: SourceSpan,
) -> Result<(), UnificationError> {
    for (template, actual) in templates.iter().zip(actuals) {
        unify_type_arguments(
            template,
            actual,
            parameters,
            substitutions,
            first_constraint_spans,
            constraint_span,
        )?;
    }
    Ok(())
}

fn infer_missing_type_arguments(
    template: &Type,
    actual: &Type,
    parameters: &HashSet<TypeParameterId>,
    substitutions: &mut HashMap<TypeParameterId, Type>,
) {
    match (template, actual) {
        (Type::Parameter(parameter), actual) if parameters.contains(parameter) => {
            substitutions
                .entry(*parameter)
                .or_insert_with(|| actual.clone());
        }
        (Type::Array(template), Type::Array(actual)) => {
            infer_missing_type_arguments(template, actual, parameters, substitutions);
        }
        (
            Type::Function {
                parameters: template_parameters,
                return_type: template_return,
            },
            Type::Function {
                parameters: actual_parameters,
                return_type: actual_return,
            },
        ) if template_parameters.len() == actual_parameters.len() => {
            for (template, actual) in template_parameters.iter().zip(actual_parameters) {
                infer_missing_type_arguments(template, actual, parameters, substitutions);
            }
            infer_missing_type_arguments(template_return, actual_return, parameters, substitutions);
        }
        (
            Type::Record {
                definition: template_definition,
                arguments: template_arguments,
            },
            Type::Record {
                definition: actual_definition,
                arguments: actual_arguments,
            },
        ) if template_definition == actual_definition => {
            for (template, actual) in template_arguments.iter().zip(actual_arguments) {
                infer_missing_type_arguments(template, actual, parameters, substitutions);
            }
        }
        (
            Type::Union {
                definition: template_definition,
                arguments: template_arguments,
            },
            Type::Union {
                definition: actual_definition,
                arguments: actual_arguments,
            },
        ) if template_definition == actual_definition => {
            for (template, actual) in template_arguments.iter().zip(actual_arguments) {
                infer_missing_type_arguments(template, actual, parameters, substitutions);
            }
        }
        _ => {}
    }
}

fn substitutions_for_parameters(
    parameters: &[TypeParameterFacts],
    arguments: &[Type],
) -> HashMap<TypeParameterId, Type> {
    parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.id, argument.clone()))
        .collect()
}

fn format_type(ty: &Type, records: &[RecordFacts], unions: &[UnionFacts]) -> String {
    match ty {
        Type::Int => "Int".to_owned(),
        Type::Bool => "Bool".to_owned(),
        Type::String => "String".to_owned(),
        Type::Array(element) => format!("{}[]", format_type(element, records, unions)),
        Type::Function {
            parameters,
            return_type,
        } => {
            let parameters = parameters
                .iter()
                .map(|parameter| format_type(parameter, records, unions))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "({parameters}) => {}",
                format_type(return_type, records, unions)
            )
        }
        Type::Parameter(parameter) => format!("parameter#{}", parameter.index()),
        Type::Record {
            definition,
            arguments,
        } => format_nominal_type(
            record_facts(records, *definition).map(|record| record.name.as_str()),
            &format!(
                "record#{}:{}",
                definition.module().index(),
                definition.index()
            ),
            arguments,
            records,
            unions,
        ),
        Type::Union {
            definition,
            arguments,
        } => format_nominal_type(
            union_facts(unions, *definition).map(|union| union.name.as_str()),
            &format!(
                "union#{}:{}",
                definition.module().index(),
                definition.index()
            ),
            arguments,
            records,
            unions,
        ),
        Type::Unit => "Unit".to_owned(),
    }
}

fn format_nominal_type(
    name: Option<&str>,
    fallback: &str,
    arguments: &[Type],
    records: &[RecordFacts],
    unions: &[UnionFacts],
) -> String {
    let name = name.unwrap_or(fallback);
    if arguments.is_empty() {
        return name.to_owned();
    }

    let arguments = arguments
        .iter()
        .map(|argument| format_type(argument, records, unions))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}

fn record_facts(records: &[RecordFacts], id: RecordId) -> Option<&RecordFacts> {
    records.iter().find(|record| record.id == id)
}

fn union_facts(unions: &[UnionFacts], id: UnionId) -> Option<&UnionFacts> {
    unions.iter().find(|union| union.id == id)
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
