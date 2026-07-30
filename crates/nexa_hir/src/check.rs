use std::collections::{HashMap, HashSet};

use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::SourceSpan;

use crate::{
    AssignmentStatement, BinaryOperator, Block, ConstDeclaration, Expression, FieldId, Function,
    FunctionId, IfStatement, LetDeclaration, MatchArm, MatchPattern, Name, PayloadId, Program,
    RecordFieldInitializer, RecordId, ReturnStatement, Statement, Type, TypeReference,
    TypeReferenceKind, UnaryOperator, UnionId, VariantId, WhileStatement,
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
    program: Program,
    expression_types: HashMap<SourceSpan, Type>,
    name_resolutions: HashMap<SourceSpan, NameResolution>,
    records: Vec<RecordFacts>,
    unions: Vec<UnionFacts>,
    variant_constructions: HashMap<SourceSpan, VariantConstructionFacts>,
    matches: HashMap<SourceSpan, MatchFacts>,
    functions: Vec<FunctionFacts>,
}

impl TypedProgram {
    /// Returns the validated underlying HIR program.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
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

    /// Returns resolved control-flow and binding facts for a match expression.
    #[must_use]
    pub fn match_facts(&self, span: SourceSpan) -> Option<&MatchFacts> {
        self.matches.get(&span)
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
    /// A compiler-provided operation.
    Builtin(Builtin),
}

/// Slot allocation facts for one validated function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFacts {
    id: FunctionId,
    parameters: Vec<LocalId>,
    parameter_types: Vec<Type>,
    return_type: Type,
    local_count: usize,
}

impl FunctionFacts {
    /// Returns this function's stable module-owned identifier.
    #[must_use]
    pub const fn id(&self) -> FunctionId {
        self.id
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

    /// Returns fields in declaration order.
    #[must_use]
    pub fn fields(&self) -> &[RecordFieldFacts] {
        &self.fields
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

    /// Returns variants in declaration order.
    #[must_use]
    pub fn variants(&self) -> &[VariantFacts] {
        &self.variants
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
}

/// Resolved constructor identity for one qualified variant call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariantConstructionFacts {
    union: UnionId,
    variant: VariantId,
}

impl VariantConstructionFacts {
    /// Returns the constructed union.
    #[must_use]
    pub const fn union(self) -> UnionId {
        self.union
    }

    /// Returns the selected variant.
    #[must_use]
    pub const fn variant(self) -> VariantId {
        self.variant
    }
}

/// Resolved variant dispatch and arm-binding facts for one match expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchFacts {
    union: UnionId,
    arms: Vec<MatchArmFacts>,
}

impl MatchFacts {
    /// Returns the matched nominal union.
    #[must_use]
    pub const fn union(&self) -> UnionId {
        self.union
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
}

#[derive(Default)]
struct FactBuilder {
    expression_types: HashMap<SourceSpan, Type>,
    name_resolutions: HashMap<SourceSpan, NameResolution>,
    records: Vec<RecordFacts>,
    unions: Vec<UnionFacts>,
    variant_constructions: HashMap<SourceSpan, VariantConstructionFacts>,
    matches: HashMap<SourceSpan, MatchFacts>,
    functions: Vec<FunctionFacts>,
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

    fn record_match(&mut self, span: SourceSpan, match_facts: MatchFacts) {
        let _ = self.matches.insert(span, match_facts);
    }
}

/// Resolves names and validates static semantics for a lowered program.
#[must_use]
pub fn type_check(program: &Program) -> Analysis {
    let mut diagnostics = Vec::new();
    let mut facts = FactBuilder::default();
    let type_symbols = collect_type_names(program, &mut diagnostics, &mut facts);
    let records = collect_record_facts(program, &type_symbols, &mut diagnostics, &mut facts);
    let unions = collect_union_facts(program, &type_symbols, &mut diagnostics, &mut facts);
    reject_recursive_records(&records, &mut diagnostics);
    let functions =
        collect_function_signatures(program, &type_symbols, &mut diagnostics, &mut facts);

    for (index, function) in program.functions.iter().enumerate() {
        check_function(
            function,
            FunctionId::in_module(program.module, index),
            &functions,
            &type_symbols,
            &records,
            &unions,
            &mut diagnostics,
            &mut facts,
        );
    }

    diagnostics.sort_by_key(diagnostic_position);
    facts.records = records;
    facts.unions = unions;
    let typed = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity() != nexa_diagnostics::Severity::Error)
        .then(|| TypedProgram {
            program: program.clone(),
            expression_types: facts.expression_types,
            name_resolutions: facts.name_resolutions,
            records: facts.records,
            unions: facts.unions,
            variant_constructions: facts.variant_constructions,
            matches: facts.matches,
            functions: facts.functions,
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

    const fn ty(self) -> Type {
        match self {
            Self::Record(record) => Type::Record(record),
            Self::Union(union) => Type::Union(union),
        }
    }

    const fn kind_name(self) -> &'static str {
        match self {
            Self::Record(_) => "record",
            Self::Union(_) => "union",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TypeEntry {
    symbol: TypeSymbol,
    name_span: SourceSpan,
}

type TypeCatalog = HashMap<String, TypeEntry>;

fn collect_type_names(
    program: &Program,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> TypeCatalog {
    let mut declarations = program
        .records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            (
                &record.name,
                TypeSymbol::Record(RecordId::in_module(program.module, index)),
            )
        })
        .chain(program.unions.iter().enumerate().map(|(index, union)| {
            (
                &union.name,
                TypeSymbol::Union(UnionId::in_module(program.module, index)),
            )
        }))
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(name, _)| (name.span.file().raw(), name.span.range().start()));

    let mut types = TypeCatalog::new();
    for (name, symbol) in declarations {
        facts.record_name(name.span, symbol.resolution());

        if let Some(previous) = types.get(&name.text).copied() {
            let description = if previous.symbol.kind_name() == symbol.kind_name() {
                symbol.kind_name()
            } else {
                "type"
            };
            diagnostics.push(
                Diagnostic::error(
                    DUPLICATE_NAME,
                    format!("duplicate {description} `{}`", name.text),
                )
                .with_label(Label::primary(
                    name.span,
                    format!("duplicate {description}"),
                ))
                .with_label(Label::secondary(previous.name_span, "first declared here")),
            );
        } else {
            types.insert(
                name.text.clone(),
                TypeEntry {
                    symbol,
                    name_span: name.span,
                },
            );
        }
    }

    types
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

                let Some(ty) = resolve_type(&field.ty, symbols, diagnostics, facts) else {
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

            RecordFacts {
                id: record_id,
                name: record.name.text.clone(),
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

                    let Some(ty) = resolve_type(&payload.ty, symbols, diagnostics, facts) else {
                        continue;
                    };
                    payloads.push(PayloadFacts {
                        id: payload_id,
                        name: payload.name.text.clone(),
                        ty,
                        span: payload.span,
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

            UnionFacts {
                id: union_id,
                name: union.name.text.clone(),
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
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> Option<Type> {
    let ty = resolve_type_kind(reference, types, diagnostics, facts)?;
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
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> Option<Type> {
    match &reference.kind {
        TypeReferenceKind::Int => Some(Type::Int),
        TypeReferenceKind::Bool => Some(Type::Bool),
        TypeReferenceKind::String => Some(Type::String),
        TypeReferenceKind::Unit => Some(Type::Unit),
        TypeReferenceKind::Named(name) => match types.get(&name.text).copied() {
            Some(entry) => {
                facts.record_name(name.span, entry.symbol.resolution());
                Some(entry.symbol.ty())
            }
            None => {
                diagnostics.push(
                    Diagnostic::error(UNDEFINED_NAME, format!("undefined type `{}`", name.text))
                        .with_label(Label::primary(name.span, "not found in this program")),
                );
                None
            }
        },
        TypeReferenceKind::Array(element) => resolve_type_kind(element, types, diagnostics, facts)
            .map(|element| Type::Array(Box::new(element))),
    }
}

fn reject_recursive_records(records: &[RecordFacts], diagnostics: &mut Vec<Diagnostic>) {
    for record in records {
        for field in &record.fields {
            if type_reaches_record(&field.ty, record.id, records, &mut HashSet::new()) {
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
                break;
            }
        }
    }
}

fn type_reaches_record(
    ty: &Type,
    target: RecordId,
    records: &[RecordFacts],
    visited: &mut HashSet<RecordId>,
) -> bool {
    match ty {
        Type::Record(record) if *record == target => true,
        Type::Record(record) => {
            if !visited.insert(*record) {
                return false;
            }
            records.get(record.index()).is_some_and(|record| {
                record
                    .fields
                    .iter()
                    .any(|field| type_reaches_record(&field.ty, target, records, visited))
            })
        }
        Type::Array(element) => type_reaches_record(element, target, records, visited),
        Type::Union(_) | Type::Int | Type::Bool | Type::String | Type::Unit => false,
    }
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    parameters: Vec<Option<Type>>,
    return_type: Option<Type>,
    declaration: Option<SourceSpan>,
    resolution: NameResolution,
}

struct FunctionCatalog {
    by_name: HashMap<String, FunctionSignature>,
    by_id: Vec<FunctionSignature>,
}

fn collect_function_signatures(
    program: &Program,
    types: &TypeCatalog,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> FunctionCatalog {
    let mut functions = HashMap::new();
    functions.insert(
        "print".to_owned(),
        FunctionSignature {
            parameters: vec![Some(Type::Int)],
            return_type: Some(Type::Unit),
            declaration: None,
            resolution: NameResolution::Builtin(Builtin::Print),
        },
    );
    let mut by_id = Vec::with_capacity(program.functions.len());

    for (index, function) in program.functions.iter().enumerate() {
        let function_id = FunctionId::in_module(program.module, index);
        facts.record_name(function.name.span, NameResolution::Function(function_id));
        let signature = FunctionSignature {
            parameters: function
                .parameters
                .iter()
                .map(|parameter| resolve_type(&parameter.ty, types, diagnostics, facts))
                .collect(),
            return_type: resolve_type(&function.return_type, types, diagnostics, facts),
            declaration: Some(function.name.span),
            resolution: NameResolution::Function(function_id),
        };

        if let Some(previous) = functions.get(&function.name.text) {
            let mut diagnostic = Diagnostic::error(
                DUPLICATE_NAME,
                format!("duplicate function `{}`", function.name.text),
            )
            .with_label(Label::primary(function.name.span, "duplicate function"));
            if let Some(previous) = previous.declaration {
                diagnostic =
                    diagnostic.with_label(Label::secondary(previous, "first declared here"));
            }
            diagnostics.push(diagnostic);
        } else {
            functions.insert(function.name.text.clone(), signature.clone());
        }
        by_id.push(signature);
    }

    FunctionCatalog {
        by_name: functions,
        by_id,
    }
}

fn check_function(
    function: &Function,
    function_id: FunctionId,
    functions: &FunctionCatalog,
    type_symbols: &TypeCatalog,
    records: &[RecordFacts],
    unions: &[UnionFacts],
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) {
    let signature = &functions.by_id[function_id.index()];
    let mut checker = FunctionChecker {
        function_id,
        functions: &functions.by_name,
        type_symbols,
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
        loop_depth: 0,
        parameters: Vec::new(),
        next_local: 0,
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
    if function.name.text == "main"
        && known_signature
        && (!valid_main_parameters || signature.return_type != Some(Type::Unit))
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
    records: &'a [RecordFacts],
    unions: &'a [UnionFacts],
    diagnostics: &'a mut Vec<Diagnostic>,
    facts: &'a mut FactBuilder,
    scopes: Vec<HashMap<String, Binding>>,
    return_type: Option<Type>,
    parameter_types: Vec<Type>,
    loop_depth: usize,
    parameters: Vec<LocalId>,
    next_local: usize,
}

#[derive(Debug, Clone)]
struct Binding {
    id: LocalId,
    ty: Option<Type>,
    span: SourceSpan,
    mutable: bool,
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
            resolve_type(annotation, self.type_symbols, self.diagnostics, self.facts)
        });
        let initializer_type =
            self.check_expression_with_expected(initializer, declared_type.as_ref());

        if let (Some(expected), Some(actual)) = (&declared_type, &initializer_type) {
            if expected != actual {
                self.type_mismatch(initializer.span(), expected, actual);
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
                    self.facts
                        .record_name(name.span, NameResolution::Local(binding.id));
                    binding.ty
                }
                None => {
                    self.error(
                        UNDEFINED_NAME,
                        name.span,
                        format!("undefined value `{}`", name.text),
                        "not found in this scope",
                    );
                    None
                }
            },
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
            } => self.check_call(callee, arguments, *span),
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
        let Some(Type::Record(record_id)) = expected else {
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
        let Some(record) = self.records.get(record_id.index()).cloned() else {
            self.error(
                TYPE_MISMATCH,
                span,
                "record type has no resolved layout",
                "invalid record type",
            );
            return None;
        };

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
            let actual = self.check_expression_with_expected(&initializer.value, Some(&field.ty));
            if let Some(actual) = actual {
                if actual != field.ty {
                    self.type_mismatch(initializer.value.span(), &field.ty, &actual);
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

        Some(Type::Record(*record_id))
    }

    fn check_match(
        &mut self,
        scrutinee: &Expression,
        arms: &[MatchArm],
        span: SourceSpan,
        expected: Option<&Type>,
    ) -> Option<Type> {
        let scrutinee_type = self.check_expression(scrutinee);
        let union_id = match scrutinee_type {
            Some(Type::Union(union)) => Some(union),
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
        let union = union_id.and_then(|union| self.unions.get(union.index()).cloned());
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
                                payload.map(|payload| payload.ty.clone()),
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
        let union = self.unions.get(union_id.index()).cloned()?;
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
        self.unions.get(union.index()).map_or_else(
            || format!("union#{}", union.index()),
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

        match self.check_expression(object) {
            Some(Type::Array(_)) if member.text == "length" => {
                self.facts
                    .record_name(member.span, NameResolution::Builtin(Builtin::ArrayLength));
                Some(Type::Int)
            }
            Some(Type::Record(record_id)) => {
                let Some(record) = self.records.get(record_id.index()) else {
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
                Some(field.ty)
            }
            Some(Type::Union(union_id)) => {
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
                if matches!(left_type, Type::Array(_) | Type::Record(_) | Type::Union(_))
                    && left_type == right_type
                {
                    let kind = match left_type {
                        Type::Array(_) => "array",
                        Type::Record(_) => "record",
                        Type::Union(_) => "tagged union",
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
    ) -> Option<Type> {
        if let Expression::Member { object, member, .. } = callee {
            if let Expression::Name(qualifier) = object.as_ref() {
                return self.check_variant_constructor(qualifier, member, arguments, span);
            }
        }

        let Expression::Name(name) = callee else {
            let _ = self.check_expression(callee);
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.error(
                TYPE_MISMATCH,
                callee.span(),
                "only named functions can be called",
                "not a callable function name",
            );
            return None;
        };

        if let Some(binding) = self.lookup_binding(name) {
            self.facts
                .record_name(name.span, NameResolution::Local(binding.id));
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            let binding_description = binding.ty.as_ref().map_or_else(
                || "value".to_owned(),
                |ty| format!("`{}` value", format_type(ty, self.records, self.unions)),
            );
            self.error(
                TYPE_MISMATCH,
                name.span,
                format!("cannot call a {binding_description}"),
                "not a function",
            );
            return None;
        }

        if name.text == "print" {
            self.facts
                .record_name(name.span, NameResolution::Builtin(Builtin::Print));
            let argument_types = arguments
                .iter()
                .map(|argument| self.check_expression(argument))
                .collect::<Vec<_>>();
            if arguments.len() != 1 {
                self.error(
                    CALL_ARITY,
                    name.span,
                    format!(
                        "function `print` expects 1 argument(s), found {}",
                        arguments.len()
                    ),
                    "incorrect argument count",
                );
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
            self.error(
                CALL_ARITY,
                name.span,
                format!(
                    "function `{}` expects {} argument(s), found {}",
                    name.text,
                    signature.parameters.len(),
                    arguments.len()
                ),
                "incorrect argument count",
            );
        }

        for (argument, expected) in arguments.iter().zip(&signature.parameters) {
            if let Some(expected) = expected {
                let actual = self.check_expression_with_expected(argument, Some(expected));
                if let Some(actual) = actual {
                    if &actual != expected {
                        self.type_mismatch(argument.span(), expected, &actual);
                    }
                }
            } else {
                let _ = self.check_expression(argument);
            }
        }
        for argument in arguments.iter().skip(signature.parameters.len()) {
            let _ = self.check_expression(argument);
        }

        signature.return_type
    }

    fn check_variant_constructor(
        &mut self,
        qualifier: &Name,
        variant: &Name,
        arguments: &[Expression],
        span: SourceSpan,
    ) -> Option<Type> {
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
        let Some(union) = self.unions.get(union_id.index()).cloned() else {
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

        for (argument, payload) in arguments.iter().zip(&variant_facts.payloads) {
            let actual = self.check_expression_with_expected(argument, Some(&payload.ty));
            if let Some(actual) = actual {
                if actual != payload.ty {
                    self.type_mismatch(argument.span(), &payload.ty, &actual);
                }
            }
        }
        for argument in arguments.iter().skip(variant_facts.payloads.len()) {
            let _ = self.check_expression(argument);
        }

        self.facts.record_variant_construction(
            span,
            VariantConstructionFacts {
                union: union_id,
                variant: variant_facts.id,
            },
        );
        Some(Type::Union(union_id))
    }

    fn bind(&mut self, name: &Name, ty: Option<Type>, mutable: bool) -> Option<LocalId> {
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

    fn type_mismatch(&mut self, span: SourceSpan, expected: &Type, actual: &Type) {
        self.error(
            TYPE_MISMATCH,
            span,
            format!(
                "expected `{}`, found `{}`",
                format_type(expected, self.records, self.unions),
                format_type(actual, self.records, self.unions)
            ),
            "type mismatch",
        );
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
            parameters: self.parameters,
            parameter_types: self.parameter_types,
            return_type: self.return_type.unwrap_or(Type::Unit),
            local_count: self.next_local,
        });
    }
}

fn contains_invalid_array_element(ty: &Type) -> bool {
    match ty {
        Type::Array(element) => {
            element.as_ref() == &Type::Unit || contains_invalid_array_element(element)
        }
        Type::Int | Type::Bool | Type::String | Type::Record(_) | Type::Union(_) | Type::Unit => {
            false
        }
    }
}

fn format_type(ty: &Type, records: &[RecordFacts], unions: &[UnionFacts]) -> String {
    match ty {
        Type::Int => "Int".to_owned(),
        Type::Bool => "Bool".to_owned(),
        Type::String => "String".to_owned(),
        Type::Array(element) => format!("{}[]", format_type(element, records, unions)),
        Type::Record(record) => records.get(record.index()).map_or_else(
            || format!("record#{}", record.index()),
            |record| record.name.clone(),
        ),
        Type::Union(union) => unions.get(union.index()).map_or_else(
            || format!("union#{}", union.index()),
            |union| union.name.clone(),
        ),
        Type::Unit => "Unit".to_owned(),
    }
}

fn diagnostic_position(diagnostic: &Diagnostic) -> (usize, usize) {
    diagnostic
        .labels()
        .first()
        .map_or((usize::MAX, usize::MAX), |label| {
            (label.span().range().start(), label.span().range().end())
        })
}
