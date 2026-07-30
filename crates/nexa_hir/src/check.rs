use std::collections::{HashMap, HashSet};

use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::SourceSpan;

use crate::{
    AssignmentStatement, BinaryOperator, Block, ConstDeclaration, Expression, FieldId, Function,
    IfStatement, LetDeclaration, Name, Program, RecordFieldInitializer, RecordId, ReturnStatement,
    Statement, Type, TypeReference, TypeReferenceKind, UnaryOperator, WhileStatement,
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
        self.functions.get(function.index())
    }

    /// Returns resolved layout facts for a nominal record.
    #[must_use]
    pub fn record_facts(&self, record: RecordId) -> Option<&RecordFacts> {
        self.records.get(record.index())
    }

    /// Returns all nominal records in stable source order.
    #[must_use]
    pub fn records(&self) -> &[RecordFacts] {
        &self.records
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

/// A stable source-order identifier for a function in one program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(usize);

impl FunctionId {
    /// Creates a function identifier from its zero-based source index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based source index within the program.
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
    /// A compiler-provided operation.
    Builtin(Builtin),
}

/// Slot allocation facts for one validated function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFacts {
    parameters: Vec<LocalId>,
    parameter_types: Vec<Type>,
    return_type: Type,
    local_count: usize,
}

impl FunctionFacts {
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
    functions: Vec<FunctionFacts>,
}

impl FactBuilder {
    fn record_expression(&mut self, span: SourceSpan, ty: Type) {
        let _ = self.expression_types.insert(span, ty);
    }

    fn record_name(&mut self, span: SourceSpan, resolution: NameResolution) {
        let _ = self.name_resolutions.insert(span, resolution);
    }
}

/// Resolves names and validates static semantics for a lowered program.
#[must_use]
pub fn type_check(program: &Program) -> Analysis {
    let mut diagnostics = Vec::new();
    let mut facts = FactBuilder::default();
    let record_symbols = collect_record_names(program, &mut diagnostics, &mut facts);
    let records = collect_record_facts(program, &record_symbols, &mut diagnostics, &mut facts);
    reject_recursive_records(&records, &mut diagnostics);
    let functions =
        collect_function_signatures(program, &record_symbols, &mut diagnostics, &mut facts);

    for (index, function) in program.functions.iter().enumerate() {
        check_function(
            function,
            index,
            &functions,
            &record_symbols,
            &records,
            &mut diagnostics,
            &mut facts,
        );
    }

    diagnostics.sort_by_key(diagnostic_position);
    facts.records = records;
    let typed = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity() != nexa_diagnostics::Severity::Error)
        .then(|| TypedProgram {
            program: program.clone(),
            expression_types: facts.expression_types,
            name_resolutions: facts.name_resolutions,
            records: facts.records,
            functions: facts.functions,
        });

    Analysis { typed, diagnostics }
}

fn collect_record_names(
    program: &Program,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> HashMap<String, RecordId> {
    let mut records = HashMap::<String, RecordId>::new();

    for (index, record) in program.records.iter().enumerate() {
        let id = RecordId::new(index);
        facts.record_name(record.name.span, NameResolution::Record(id));

        if let Some(previous) = records.get(&record.name.text).copied() {
            let previous_span = program.records[previous.index()].name.span;
            diagnostics.push(
                Diagnostic::error(
                    DUPLICATE_NAME,
                    format!("duplicate record `{}`", record.name.text),
                )
                .with_label(Label::primary(record.name.span, "duplicate record"))
                .with_label(Label::secondary(previous_span, "first declared here")),
            );
        } else {
            records.insert(record.name.text.clone(), id);
        }
    }

    records
}

fn collect_record_facts(
    program: &Program,
    symbols: &HashMap<String, RecordId>,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> Vec<RecordFacts> {
    program
        .records
        .iter()
        .enumerate()
        .map(|(record_index, record)| {
            let record_id = RecordId::new(record_index);
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

fn resolve_type(
    reference: &TypeReference,
    records: &HashMap<String, RecordId>,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> Option<Type> {
    let ty = resolve_type_kind(reference, records, diagnostics, facts)?;
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
    records: &HashMap<String, RecordId>,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> Option<Type> {
    match &reference.kind {
        TypeReferenceKind::Int => Some(Type::Int),
        TypeReferenceKind::Bool => Some(Type::Bool),
        TypeReferenceKind::String => Some(Type::String),
        TypeReferenceKind::Unit => Some(Type::Unit),
        TypeReferenceKind::Named(name) => match records.get(&name.text).copied() {
            Some(record) => {
                facts.record_name(name.span, NameResolution::Record(record));
                Some(Type::Record(record))
            }
            None => {
                diagnostics.push(
                    Diagnostic::error(UNDEFINED_NAME, format!("undefined type `{}`", name.text))
                        .with_label(Label::primary(name.span, "not found in this program")),
                );
                None
            }
        },
        TypeReferenceKind::Array(element) => {
            resolve_type_kind(element, records, diagnostics, facts)
                .map(|element| Type::Array(Box::new(element)))
        }
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
        Type::Int | Type::Bool | Type::String | Type::Unit => false,
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
    records: &HashMap<String, RecordId>,
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
        let function_id = FunctionId::new(index);
        facts.record_name(function.name.span, NameResolution::Function(function_id));
        let signature = FunctionSignature {
            parameters: function
                .parameters
                .iter()
                .map(|parameter| resolve_type(&parameter.ty, records, diagnostics, facts))
                .collect(),
            return_type: resolve_type(&function.return_type, records, diagnostics, facts),
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
    function_index: usize,
    functions: &FunctionCatalog,
    record_symbols: &HashMap<String, RecordId>,
    records: &[RecordFacts],
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) {
    let signature = &functions.by_id[function_index];
    let mut checker = FunctionChecker {
        functions: &functions.by_name,
        record_symbols,
        records,
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
    functions: &'a HashMap<String, FunctionSignature>,
    record_symbols: &'a HashMap<String, RecordId>,
    records: &'a [RecordFacts],
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
            resolve_type(
                annotation,
                self.record_symbols,
                self.diagnostics,
                self.facts,
            )
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
                    format_type(expected, self.records)
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
                                format_type(expected, self.records),
                                format_type(&actual, self.records)
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
                callee, arguments, ..
            } => self.check_call(callee, arguments),
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
                        format_type(ty, self.records)
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
                        format_type(&actual, self.records)
                    ),
                    "expected an array value",
                );
                None
            }
            None => None,
        }
    }

    fn check_member(&mut self, object: &Expression, member: &Name) -> Option<Type> {
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
            Some(actual) => {
                self.error(
                    UNKNOWN_MEMBER,
                    member.span,
                    format!(
                        "type `{}` has no member `{}`",
                        format_type(&actual, self.records),
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
                if matches!(left_type, Type::Array(_) | Type::Record(_)) && left_type == right_type
                {
                    let kind = match left_type {
                        Type::Array(_) => "array",
                        Type::Record(_) => "record",
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
                            format_type(&left_type, self.records),
                            format_type(&right_type, self.records)
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

    fn check_call(&mut self, callee: &Expression, arguments: &[Expression]) -> Option<Type> {
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
                |ty| format!("`{}` value", format_type(ty, self.records)),
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
                                format_type(&actual, self.records)
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
                format_type(expected, self.records),
                format_type(actual, self.records)
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
        Type::Int | Type::Bool | Type::String | Type::Record(_) | Type::Unit => false,
    }
}

fn format_type(ty: &Type, records: &[RecordFacts]) -> String {
    match ty {
        Type::Int => "Int".to_owned(),
        Type::Bool => "Bool".to_owned(),
        Type::String => "String".to_owned(),
        Type::Array(element) => format!("{}[]", format_type(element, records)),
        Type::Record(record) => records.get(record.index()).map_or_else(
            || format!("record#{}", record.index()),
            |record| record.name.clone(),
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
