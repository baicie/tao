//! Canonical DTOs for typed HIR and MIR differential output.

use nexa_hir::{
    ArrowBody, BinaryOperator, Block, Builtin, CallFacts, ClosureFacts, ClosureId, Expression,
    FieldId, Function, FunctionFacts, FunctionId, ImportDeclaration, LocalId as HirLocalId,
    MatchArm, MatchArmFacts, MatchFacts, MatchPattern, ModuleId, Name, NameResolution, Parameter,
    PayloadBindingFacts, PayloadFacts, PayloadId, Program, RecordDeclaration, RecordFacts,
    RecordFieldDeclaration, RecordFieldFacts, RecordFieldInitializer, RecordId, Statement, Type,
    TypeParameter, TypeParameterFacts, TypeParameterId, TypeParameterOwner, TypeReference,
    TypeReferenceKind, TypedProgram, UnaryOperator, UnionDeclaration, UnionFacts, UnionId,
    UnionVariantDeclaration, VariantConstructionFacts, VariantFacts, VariantId,
    VariantPayloadDeclaration, Visibility,
};
use nexa_mir::{
    ArrayIntrinsic, BasicBlockId, Callee, LocalId as MirLocalId, MirBasicBlock, MirClosure,
    MirExpression, MirFunction, MirPayload, MirProgram, MirRecord, MirRecordField, MirStatement,
    MirTerminator, MirUnion, MirVariant,
};
use nexa_span::SourceSpan;
use serde::Serialize;
use serde_json::Value;

use crate::core::{span_position, CompileError};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalSpan {
    source: u32,
    start: u32,
    end: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalFunctionId {
    module: u32,
    index: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalClosureId {
    owner: CanonicalFunctionId,
    source_index: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRecordId {
    module: u32,
    index: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalFieldId {
    record: CanonicalRecordId,
    index: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalUnionId {
    module: u32,
    index: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalVariantId {
    union: CanonicalUnionId,
    index: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalPayloadId {
    variant: CanonicalVariantId,
    index: u32,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalTypeParameterOwner {
    Function { id: CanonicalFunctionId },
    Record { id: CanonicalRecordId },
    Union { id: CanonicalUnionId },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalTypeParameterId {
    owner: CanonicalTypeParameterOwner,
    index: u32,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalType {
    Int,
    Bool,
    String,
    Array {
        element: Box<CanonicalType>,
    },
    Function {
        parameters: Vec<CanonicalType>,
        return_type: Box<CanonicalType>,
    },
    Parameter {
        id: CanonicalTypeParameterId,
    },
    Record {
        definition: CanonicalRecordId,
        arguments: Vec<CanonicalType>,
    },
    Union {
        definition: CanonicalUnionId,
        arguments: Vec<CanonicalType>,
    },
    Unit,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalName {
    text: String,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum CanonicalVisibility {
    Private,
    Exported,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalTypeReference {
    Int {
        span: CanonicalSpan,
    },
    Bool {
        span: CanonicalSpan,
    },
    String {
        span: CanonicalSpan,
    },
    Unit {
        span: CanonicalSpan,
    },
    Named {
        name: CanonicalName,
        arguments: Vec<CanonicalTypeReference>,
        span: CanonicalSpan,
    },
    Array {
        element: Box<CanonicalTypeReference>,
        span: CanonicalSpan,
    },
    Function {
        parameters: Vec<CanonicalParameter>,
        return_type: Box<CanonicalTypeReference>,
        span: CanonicalSpan,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalParameter {
    name: CanonicalName,
    ty: CanonicalTypeReference,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalTypeParameter {
    name: CanonicalName,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalImport {
    names: Vec<CanonicalName>,
    path: String,
    path_span: CanonicalSpan,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRecordDeclaration {
    name: CanonicalName,
    type_parameters: Vec<CanonicalTypeParameter>,
    visibility: CanonicalVisibility,
    fields: Vec<CanonicalRecordFieldDeclaration>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRecordFieldDeclaration {
    name: CanonicalName,
    ty: CanonicalTypeReference,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalUnionDeclaration {
    name: CanonicalName,
    type_parameters: Vec<CanonicalTypeParameter>,
    visibility: CanonicalVisibility,
    variants: Vec<CanonicalUnionVariantDeclaration>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalUnionVariantDeclaration {
    name: CanonicalName,
    payloads: Vec<CanonicalVariantPayloadDeclaration>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalVariantPayloadDeclaration {
    name: CanonicalName,
    ty: CanonicalTypeReference,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalFunction {
    name: CanonicalName,
    type_parameters: Vec<CanonicalTypeParameter>,
    visibility: CanonicalVisibility,
    parameters: Vec<CanonicalParameter>,
    return_type: CanonicalTypeReference,
    body: CanonicalBlock,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalBlock {
    statements: Vec<CanonicalStatement>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalStatement {
    Const {
        name: CanonicalName,
        annotation: Option<CanonicalTypeReference>,
        initializer: CanonicalExpression,
        span: CanonicalSpan,
    },
    Let {
        name: CanonicalName,
        annotation: Option<CanonicalTypeReference>,
        initializer: CanonicalExpression,
        span: CanonicalSpan,
    },
    Assignment {
        target: CanonicalName,
        value: CanonicalExpression,
        span: CanonicalSpan,
    },
    If {
        condition: CanonicalExpression,
        then_branch: CanonicalBlock,
        else_branch: Option<CanonicalBlock>,
        span: CanonicalSpan,
    },
    While {
        condition: CanonicalExpression,
        body: CanonicalBlock,
        span: CanonicalSpan,
    },
    ForOf {
        binding: CanonicalName,
        iterable: CanonicalExpression,
        body: CanonicalBlock,
        span: CanonicalSpan,
    },
    Break {
        span: CanonicalSpan,
    },
    Continue {
        span: CanonicalSpan,
    },
    Return {
        value: Option<CanonicalExpression>,
        span: CanonicalSpan,
    },
    Expression {
        expression: CanonicalExpression,
        span: CanonicalSpan,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalExpression {
    Integer {
        value: i64,
        span: CanonicalSpan,
    },
    Boolean {
        value: bool,
        span: CanonicalSpan,
    },
    String {
        value: String,
        span: CanonicalSpan,
    },
    Array {
        elements: Vec<CanonicalExpression>,
        span: CanonicalSpan,
    },
    Record {
        fields: Vec<CanonicalRecordFieldInitializer>,
        span: CanonicalSpan,
    },
    Match {
        scrutinee: Box<CanonicalExpression>,
        arms: Vec<CanonicalMatchArm>,
        span: CanonicalSpan,
    },
    Index {
        collection: Box<CanonicalExpression>,
        index: Box<CanonicalExpression>,
        span: CanonicalSpan,
    },
    Member {
        object: Box<CanonicalExpression>,
        member: CanonicalName,
        span: CanonicalSpan,
    },
    Name {
        name: CanonicalName,
    },
    Arrow {
        closure: CanonicalClosureId,
        parameters: Vec<CanonicalParameter>,
        return_type: CanonicalTypeReference,
        body: CanonicalArrowBody,
        span: CanonicalSpan,
    },
    Unary {
        operator: CanonicalUnaryOperator,
        expression: Box<CanonicalExpression>,
        span: CanonicalSpan,
    },
    Binary {
        operator: CanonicalBinaryOperator,
        left: Box<CanonicalExpression>,
        right: Box<CanonicalExpression>,
        span: CanonicalSpan,
    },
    Call {
        callee: Box<CanonicalExpression>,
        arguments: Vec<CanonicalExpression>,
        span: CanonicalSpan,
    },
    Parenthesized {
        expression: Box<CanonicalExpression>,
        span: CanonicalSpan,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRecordFieldInitializer {
    name: CanonicalName,
    value: CanonicalExpression,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMatchArm {
    pattern: CanonicalMatchPattern,
    value: CanonicalExpression,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalMatchPattern {
    Variant {
        union: CanonicalName,
        variant: CanonicalName,
        bindings: Vec<CanonicalName>,
        span: CanonicalSpan,
    },
    Default {
        span: CanonicalSpan,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalArrowBody {
    Expression {
        expression: Box<CanonicalExpression>,
    },
    Block {
        block: CanonicalBlock,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum CanonicalUnaryOperator {
    Not,
    Negate,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum CanonicalBinaryOperator {
    LogicalAnd,
    LogicalOr,
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalModule {
    id: u32,
    imports: Vec<CanonicalImport>,
    records: Vec<CanonicalRecordDeclaration>,
    unions: Vec<CanonicalUnionDeclaration>,
    functions: Vec<CanonicalFunction>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalHir {
    entry_module: u32,
    modules: Vec<CanonicalModule>,
    facts: CanonicalHirFacts,
}

pub(crate) fn hir_value(program: &TypedProgram) -> Result<Value, CompileError> {
    let value = CanonicalHir {
        entry_module: module_id(program.entry())?,
        modules: program
            .modules()
            .iter()
            .map(canonical_module)
            .collect::<Result<Vec<_>, _>>()?,
        facts: canonical_hir_facts(program)?,
    };
    Ok(serde_json::to_value(value)?)
}

fn canonical_module(program: &Program) -> Result<CanonicalModule, CompileError> {
    Ok(CanonicalModule {
        id: module_id(program.module)?,
        imports: program
            .imports
            .iter()
            .map(canonical_import)
            .collect::<Result<Vec<_>, _>>()?,
        records: program
            .records
            .iter()
            .map(canonical_record_declaration)
            .collect::<Result<Vec<_>, _>>()?,
        unions: program
            .unions
            .iter()
            .map(canonical_union_declaration)
            .collect::<Result<Vec<_>, _>>()?,
        functions: program
            .functions
            .iter()
            .map(canonical_function)
            .collect::<Result<Vec<_>, _>>()?,
        span: canonical_span(program.span)?,
    })
}

fn canonical_import(import: &ImportDeclaration) -> Result<CanonicalImport, CompileError> {
    Ok(CanonicalImport {
        names: import
            .names
            .iter()
            .map(canonical_name)
            .collect::<Result<Vec<_>, _>>()?,
        path: import.path.clone(),
        path_span: canonical_span(import.path_span)?,
        span: canonical_span(import.span)?,
    })
}

fn canonical_record_declaration(
    record: &RecordDeclaration,
) -> Result<CanonicalRecordDeclaration, CompileError> {
    Ok(CanonicalRecordDeclaration {
        name: canonical_name(&record.name)?,
        type_parameters: record
            .type_parameters
            .iter()
            .map(canonical_type_parameter)
            .collect::<Result<Vec<_>, _>>()?,
        visibility: canonical_visibility(record.visibility),
        fields: record
            .fields
            .iter()
            .map(canonical_record_field_declaration)
            .collect::<Result<Vec<_>, _>>()?,
        span: canonical_span(record.span)?,
    })
}

fn canonical_record_field_declaration(
    field: &RecordFieldDeclaration,
) -> Result<CanonicalRecordFieldDeclaration, CompileError> {
    Ok(CanonicalRecordFieldDeclaration {
        name: canonical_name(&field.name)?,
        ty: canonical_type_reference(&field.ty)?,
        span: canonical_span(field.span)?,
    })
}

fn canonical_union_declaration(
    union: &UnionDeclaration,
) -> Result<CanonicalUnionDeclaration, CompileError> {
    Ok(CanonicalUnionDeclaration {
        name: canonical_name(&union.name)?,
        type_parameters: union
            .type_parameters
            .iter()
            .map(canonical_type_parameter)
            .collect::<Result<Vec<_>, _>>()?,
        visibility: canonical_visibility(union.visibility),
        variants: union
            .variants
            .iter()
            .map(canonical_union_variant_declaration)
            .collect::<Result<Vec<_>, _>>()?,
        span: canonical_span(union.span)?,
    })
}

fn canonical_union_variant_declaration(
    variant: &UnionVariantDeclaration,
) -> Result<CanonicalUnionVariantDeclaration, CompileError> {
    Ok(CanonicalUnionVariantDeclaration {
        name: canonical_name(&variant.name)?,
        payloads: variant
            .payloads
            .iter()
            .map(canonical_variant_payload_declaration)
            .collect::<Result<Vec<_>, _>>()?,
        span: canonical_span(variant.span)?,
    })
}

fn canonical_variant_payload_declaration(
    payload: &VariantPayloadDeclaration,
) -> Result<CanonicalVariantPayloadDeclaration, CompileError> {
    Ok(CanonicalVariantPayloadDeclaration {
        name: canonical_name(&payload.name)?,
        ty: canonical_type_reference(&payload.ty)?,
        span: canonical_span(payload.span)?,
    })
}

fn canonical_function(function: &Function) -> Result<CanonicalFunction, CompileError> {
    Ok(CanonicalFunction {
        name: canonical_name(&function.name)?,
        type_parameters: function
            .type_parameters
            .iter()
            .map(canonical_type_parameter)
            .collect::<Result<Vec<_>, _>>()?,
        visibility: canonical_visibility(function.visibility),
        parameters: function
            .parameters
            .iter()
            .map(canonical_parameter)
            .collect::<Result<Vec<_>, _>>()?,
        return_type: canonical_type_reference(&function.return_type)?,
        body: canonical_block(&function.body)?,
        span: canonical_span(function.span)?,
    })
}

fn canonical_block(block: &Block) -> Result<CanonicalBlock, CompileError> {
    Ok(CanonicalBlock {
        statements: block
            .statements
            .iter()
            .map(canonical_statement)
            .collect::<Result<Vec<_>, _>>()?,
        span: canonical_span(block.span)?,
    })
}

fn canonical_statement(statement: &Statement) -> Result<CanonicalStatement, CompileError> {
    Ok(match statement {
        Statement::Const(declaration) => CanonicalStatement::Const {
            name: canonical_name(&declaration.name)?,
            annotation: declaration
                .annotation
                .as_ref()
                .map(canonical_type_reference)
                .transpose()?,
            initializer: canonical_expression(&declaration.initializer)?,
            span: canonical_span(declaration.span)?,
        },
        Statement::Let(declaration) => CanonicalStatement::Let {
            name: canonical_name(&declaration.name)?,
            annotation: declaration
                .annotation
                .as_ref()
                .map(canonical_type_reference)
                .transpose()?,
            initializer: canonical_expression(&declaration.initializer)?,
            span: canonical_span(declaration.span)?,
        },
        Statement::Assignment(assignment) => CanonicalStatement::Assignment {
            target: canonical_name(&assignment.target)?,
            value: canonical_expression(&assignment.value)?,
            span: canonical_span(assignment.span)?,
        },
        Statement::If(statement) => CanonicalStatement::If {
            condition: canonical_expression(&statement.condition)?,
            then_branch: canonical_block(&statement.then_branch)?,
            else_branch: statement
                .else_branch
                .as_ref()
                .map(canonical_block)
                .transpose()?,
            span: canonical_span(statement.span)?,
        },
        Statement::While(statement) => CanonicalStatement::While {
            condition: canonical_expression(&statement.condition)?,
            body: canonical_block(&statement.body)?,
            span: canonical_span(statement.span)?,
        },
        Statement::ForOf(statement) => CanonicalStatement::ForOf {
            binding: canonical_name(&statement.binding)?,
            iterable: canonical_expression(&statement.iterable)?,
            body: canonical_block(&statement.body)?,
            span: canonical_span(statement.span)?,
        },
        Statement::Break(statement) => CanonicalStatement::Break {
            span: canonical_span(statement.span)?,
        },
        Statement::Continue(statement) => CanonicalStatement::Continue {
            span: canonical_span(statement.span)?,
        },
        Statement::Return(statement) => CanonicalStatement::Return {
            value: statement
                .value
                .as_ref()
                .map(canonical_expression)
                .transpose()?,
            span: canonical_span(statement.span)?,
        },
        Statement::Expression(statement) => CanonicalStatement::Expression {
            expression: canonical_expression(&statement.expression)?,
            span: canonical_span(statement.span)?,
        },
    })
}

fn canonical_expression(expression: &Expression) -> Result<CanonicalExpression, CompileError> {
    Ok(match expression {
        Expression::Integer { value, span } => CanonicalExpression::Integer {
            value: *value,
            span: canonical_span(*span)?,
        },
        Expression::Boolean { value, span } => CanonicalExpression::Boolean {
            value: *value,
            span: canonical_span(*span)?,
        },
        Expression::String { value, span } => CanonicalExpression::String {
            value: value.clone(),
            span: canonical_span(*span)?,
        },
        Expression::Array { elements, span } => CanonicalExpression::Array {
            elements: elements
                .iter()
                .map(canonical_expression)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        Expression::Record { fields, span } => CanonicalExpression::Record {
            fields: fields
                .iter()
                .map(canonical_record_field_initializer)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        Expression::Match {
            scrutinee,
            arms,
            span,
        } => CanonicalExpression::Match {
            scrutinee: Box::new(canonical_expression(scrutinee)?),
            arms: arms
                .iter()
                .map(canonical_match_arm)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        Expression::Index {
            collection,
            index,
            span,
        } => CanonicalExpression::Index {
            collection: Box::new(canonical_expression(collection)?),
            index: Box::new(canonical_expression(index)?),
            span: canonical_span(*span)?,
        },
        Expression::Member {
            object,
            member,
            span,
        } => CanonicalExpression::Member {
            object: Box::new(canonical_expression(object)?),
            member: canonical_name(member)?,
            span: canonical_span(*span)?,
        },
        Expression::Name(name) => CanonicalExpression::Name {
            name: canonical_name(name)?,
        },
        Expression::Arrow {
            closure,
            parameters,
            return_type,
            body,
            span,
        } => CanonicalExpression::Arrow {
            closure: closure_id(*closure)?,
            parameters: parameters
                .iter()
                .map(canonical_parameter)
                .collect::<Result<Vec<_>, _>>()?,
            return_type: canonical_type_reference(return_type)?,
            body: canonical_arrow_body(body)?,
            span: canonical_span(*span)?,
        },
        Expression::Unary {
            operator,
            expression,
            span,
        } => CanonicalExpression::Unary {
            operator: canonical_unary_operator(*operator),
            expression: Box::new(canonical_expression(expression)?),
            span: canonical_span(*span)?,
        },
        Expression::Binary {
            operator,
            left,
            right,
            span,
        } => CanonicalExpression::Binary {
            operator: canonical_binary_operator(*operator),
            left: Box::new(canonical_expression(left)?),
            right: Box::new(canonical_expression(right)?),
            span: canonical_span(*span)?,
        },
        Expression::Call {
            callee,
            arguments,
            span,
        } => CanonicalExpression::Call {
            callee: Box::new(canonical_expression(callee)?),
            arguments: arguments
                .iter()
                .map(canonical_expression)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        Expression::Parenthesized { expression, span } => CanonicalExpression::Parenthesized {
            expression: Box::new(canonical_expression(expression)?),
            span: canonical_span(*span)?,
        },
    })
}

fn canonical_record_field_initializer(
    field: &RecordFieldInitializer,
) -> Result<CanonicalRecordFieldInitializer, CompileError> {
    Ok(CanonicalRecordFieldInitializer {
        name: canonical_name(&field.name)?,
        value: canonical_expression(&field.value)?,
        span: canonical_span(field.span)?,
    })
}

fn canonical_match_arm(arm: &MatchArm) -> Result<CanonicalMatchArm, CompileError> {
    Ok(CanonicalMatchArm {
        pattern: canonical_match_pattern(&arm.pattern)?,
        value: canonical_expression(&arm.value)?,
        span: canonical_span(arm.span)?,
    })
}

fn canonical_match_pattern(pattern: &MatchPattern) -> Result<CanonicalMatchPattern, CompileError> {
    Ok(match pattern {
        MatchPattern::Variant {
            union,
            variant,
            bindings,
            span,
        } => CanonicalMatchPattern::Variant {
            union: canonical_name(union)?,
            variant: canonical_name(variant)?,
            bindings: bindings
                .iter()
                .map(canonical_name)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        MatchPattern::Default { span } => CanonicalMatchPattern::Default {
            span: canonical_span(*span)?,
        },
    })
}

fn canonical_arrow_body(body: &ArrowBody) -> Result<CanonicalArrowBody, CompileError> {
    Ok(match body {
        ArrowBody::Expression(expression) => CanonicalArrowBody::Expression {
            expression: Box::new(canonical_expression(expression)?),
        },
        ArrowBody::Block(block) => CanonicalArrowBody::Block {
            block: canonical_block(block)?,
        },
    })
}

fn canonical_type_reference(
    reference: &TypeReference,
) -> Result<CanonicalTypeReference, CompileError> {
    let span = canonical_span(reference.span)?;
    Ok(match &reference.kind {
        TypeReferenceKind::Int => CanonicalTypeReference::Int { span },
        TypeReferenceKind::Bool => CanonicalTypeReference::Bool { span },
        TypeReferenceKind::String => CanonicalTypeReference::String { span },
        TypeReferenceKind::Unit => CanonicalTypeReference::Unit { span },
        TypeReferenceKind::Named { name, arguments } => CanonicalTypeReference::Named {
            name: canonical_name(name)?,
            arguments: arguments
                .iter()
                .map(canonical_type_reference)
                .collect::<Result<Vec<_>, _>>()?,
            span,
        },
        TypeReferenceKind::Array(element) => CanonicalTypeReference::Array {
            element: Box::new(canonical_type_reference(element)?),
            span,
        },
        TypeReferenceKind::Function {
            parameters,
            return_type,
        } => CanonicalTypeReference::Function {
            parameters: parameters
                .iter()
                .map(canonical_parameter)
                .collect::<Result<Vec<_>, _>>()?,
            return_type: Box::new(canonical_type_reference(return_type)?),
            span,
        },
    })
}

fn canonical_parameter(parameter: &Parameter) -> Result<CanonicalParameter, CompileError> {
    Ok(CanonicalParameter {
        name: canonical_name(&parameter.name)?,
        ty: canonical_type_reference(&parameter.ty)?,
        span: canonical_span(parameter.span)?,
    })
}

fn canonical_type_parameter(
    parameter: &TypeParameter,
) -> Result<CanonicalTypeParameter, CompileError> {
    Ok(CanonicalTypeParameter {
        name: canonical_name(&parameter.name)?,
        span: canonical_span(parameter.span)?,
    })
}

fn canonical_name(name: &Name) -> Result<CanonicalName, CompileError> {
    Ok(CanonicalName {
        text: name.text.clone(),
        span: canonical_span(name.span)?,
    })
}

const fn canonical_visibility(visibility: Visibility) -> CanonicalVisibility {
    match visibility {
        Visibility::Private => CanonicalVisibility::Private,
        Visibility::Exported => CanonicalVisibility::Exported,
    }
}

const fn canonical_unary_operator(operator: UnaryOperator) -> CanonicalUnaryOperator {
    match operator {
        UnaryOperator::Not => CanonicalUnaryOperator::Not,
        UnaryOperator::Negate => CanonicalUnaryOperator::Negate,
    }
}

const fn canonical_binary_operator(operator: BinaryOperator) -> CanonicalBinaryOperator {
    match operator {
        BinaryOperator::LogicalAnd => CanonicalBinaryOperator::LogicalAnd,
        BinaryOperator::LogicalOr => CanonicalBinaryOperator::LogicalOr,
        BinaryOperator::Add => CanonicalBinaryOperator::Add,
        BinaryOperator::Subtract => CanonicalBinaryOperator::Subtract,
        BinaryOperator::Multiply => CanonicalBinaryOperator::Multiply,
        BinaryOperator::Divide => CanonicalBinaryOperator::Divide,
        BinaryOperator::Equal => CanonicalBinaryOperator::Equal,
        BinaryOperator::Less => CanonicalBinaryOperator::Less,
        BinaryOperator::LessEqual => CanonicalBinaryOperator::LessEqual,
        BinaryOperator::Greater => CanonicalBinaryOperator::Greater,
        BinaryOperator::GreaterEqual => CanonicalBinaryOperator::GreaterEqual,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalHirFacts {
    expression_types: Vec<CanonicalExpressionTypeFact>,
    name_resolutions: Vec<CanonicalNameResolutionFact>,
    functions: Vec<CanonicalFunctionFacts>,
    closures: Vec<CanonicalClosureFacts>,
    records: Vec<CanonicalRecordFacts>,
    unions: Vec<CanonicalUnionFacts>,
    variant_constructions: Vec<CanonicalVariantConstructionFact>,
    calls: Vec<CanonicalCallFact>,
    matches: Vec<CanonicalMatchFact>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalExpressionTypeFact {
    span: CanonicalSpan,
    ty: CanonicalType,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalNameResolutionFact {
    span: CanonicalSpan,
    resolution: CanonicalNameResolution,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalNameResolution {
    Local { id: u32 },
    Function { id: CanonicalFunctionId },
    Record { id: CanonicalRecordId },
    Field { id: CanonicalFieldId },
    Union { id: CanonicalUnionId },
    Variant { id: CanonicalVariantId },
    Payload { id: CanonicalPayloadId },
    TypeParameter { id: CanonicalTypeParameterId },
    Builtin { builtin: CanonicalBuiltin },
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum CanonicalBuiltin {
    Print,
    ArrayLength,
    StringLength,
    ArrayAppend,
    ArrayConcat,
    ToString,
    ParseInt,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalFunctionFacts {
    id: CanonicalFunctionId,
    type_parameters: Vec<CanonicalTypeParameterFacts>,
    parameters: Vec<u32>,
    parameter_types: Vec<CanonicalType>,
    return_type: CanonicalType,
    local_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalClosureFacts {
    id: CanonicalClosureId,
    parameters: Vec<u32>,
    parameter_types: Vec<CanonicalType>,
    return_type: CanonicalType,
    captures: Vec<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalTypeParameterFacts {
    id: CanonicalTypeParameterId,
    name: String,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRecordFacts {
    id: CanonicalRecordId,
    name: String,
    type_parameters: Vec<CanonicalTypeParameterFacts>,
    fields: Vec<CanonicalRecordFieldFacts>,
    name_span: CanonicalSpan,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRecordFieldFacts {
    id: CanonicalFieldId,
    name: String,
    ty: CanonicalType,
    span: CanonicalSpan,
    type_span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalUnionFacts {
    id: CanonicalUnionId,
    name: String,
    type_parameters: Vec<CanonicalTypeParameterFacts>,
    variants: Vec<CanonicalVariantFacts>,
    name_span: CanonicalSpan,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalVariantFacts {
    id: CanonicalVariantId,
    name: String,
    payloads: Vec<CanonicalPayloadFacts>,
    name_span: CanonicalSpan,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalPayloadFacts {
    id: CanonicalPayloadId,
    name: String,
    ty: CanonicalType,
    span: CanonicalSpan,
    type_span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalVariantConstructionFact {
    span: CanonicalSpan,
    union: CanonicalUnionId,
    variant: CanonicalVariantId,
    type_arguments: Vec<CanonicalType>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalCallFact {
    span: CanonicalSpan,
    function: CanonicalFunctionId,
    type_arguments: Vec<CanonicalType>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMatchFact {
    span: CanonicalSpan,
    union: CanonicalUnionId,
    type_arguments: Vec<CanonicalType>,
    arms: Vec<CanonicalMatchArmFacts>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalMatchArmFacts {
    Variant {
        variant: CanonicalVariantId,
        bindings: Vec<CanonicalPayloadBindingFacts>,
    },
    Default,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalPayloadBindingFacts {
    payload: CanonicalPayloadId,
    local: u32,
}

fn canonical_hir_facts(program: &TypedProgram) -> Result<CanonicalHirFacts, CompileError> {
    let mut expression_types = program.expression_types().collect::<Vec<_>>();
    expression_types.sort_by_key(|(span, _)| span_position(*span));
    let expression_types = expression_types
        .into_iter()
        .map(|(span, ty)| {
            Ok(CanonicalExpressionTypeFact {
                span: canonical_span(span)?,
                ty: canonical_type(ty)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;

    let mut name_resolutions = program.name_resolutions().collect::<Vec<_>>();
    name_resolutions.sort_by_key(|(span, _)| span_position(*span));
    let name_resolutions = name_resolutions
        .into_iter()
        .map(|(span, resolution)| {
            Ok(CanonicalNameResolutionFact {
                span: canonical_span(span)?,
                resolution: canonical_name_resolution(resolution)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;

    let mut variant_constructions = program.variant_constructions().collect::<Vec<_>>();
    variant_constructions.sort_by_key(|(span, _)| span_position(*span));
    let variant_constructions = variant_constructions
        .into_iter()
        .map(|(span, facts)| canonical_variant_construction_fact(span, facts))
        .collect::<Result<Vec<_>, _>>()?;

    let mut calls = program.calls().collect::<Vec<_>>();
    calls.sort_by_key(|(span, _)| span_position(*span));
    let calls = calls
        .into_iter()
        .map(|(span, facts)| canonical_call_fact(span, facts))
        .collect::<Result<Vec<_>, _>>()?;

    let mut matches = program.matches().collect::<Vec<_>>();
    matches.sort_by_key(|(span, _)| span_position(*span));
    let matches = matches
        .into_iter()
        .map(|(span, facts)| canonical_match_fact(span, facts))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CanonicalHirFacts {
        expression_types,
        name_resolutions,
        functions: program
            .functions()
            .iter()
            .map(canonical_function_facts)
            .collect::<Result<Vec<_>, _>>()?,
        closures: program
            .closures()
            .iter()
            .map(canonical_closure_facts)
            .collect::<Result<Vec<_>, _>>()?,
        records: program
            .records()
            .iter()
            .map(canonical_record_facts)
            .collect::<Result<Vec<_>, _>>()?,
        unions: program
            .unions()
            .iter()
            .map(canonical_union_facts)
            .collect::<Result<Vec<_>, _>>()?,
        variant_constructions,
        calls,
        matches,
    })
}

fn canonical_name_resolution(
    resolution: NameResolution,
) -> Result<CanonicalNameResolution, CompileError> {
    Ok(match resolution {
        NameResolution::Local(id) => CanonicalNameResolution::Local {
            id: hir_local_id(id)?,
        },
        NameResolution::Function(id) => CanonicalNameResolution::Function {
            id: function_id(id)?,
        },
        NameResolution::Record(id) => CanonicalNameResolution::Record { id: record_id(id)? },
        NameResolution::Field(id) => CanonicalNameResolution::Field { id: field_id(id)? },
        NameResolution::Union(id) => CanonicalNameResolution::Union { id: union_id(id)? },
        NameResolution::Variant(id) => CanonicalNameResolution::Variant {
            id: variant_id(id)?,
        },
        NameResolution::Payload(id) => CanonicalNameResolution::Payload {
            id: payload_id(id)?,
        },
        NameResolution::TypeParameter(id) => CanonicalNameResolution::TypeParameter {
            id: type_parameter_id(id)?,
        },
        NameResolution::Builtin(builtin) => CanonicalNameResolution::Builtin {
            builtin: canonical_builtin(builtin),
        },
    })
}

fn canonical_function_facts(facts: &FunctionFacts) -> Result<CanonicalFunctionFacts, CompileError> {
    Ok(CanonicalFunctionFacts {
        id: function_id(facts.id())?,
        type_parameters: facts
            .type_parameters()
            .iter()
            .map(canonical_type_parameter_facts)
            .collect::<Result<Vec<_>, _>>()?,
        parameters: facts
            .parameter_ids()
            .iter()
            .copied()
            .map(hir_local_id)
            .collect::<Result<Vec<_>, _>>()?,
        parameter_types: canonical_types(facts.parameter_types())?,
        return_type: canonical_type(facts.return_type())?,
        local_count: canonical_u32(facts.local_count())?,
    })
}

fn canonical_closure_facts(facts: &ClosureFacts) -> Result<CanonicalClosureFacts, CompileError> {
    Ok(CanonicalClosureFacts {
        id: closure_id(facts.id())?,
        parameters: facts
            .parameter_ids()
            .iter()
            .copied()
            .map(hir_local_id)
            .collect::<Result<Vec<_>, _>>()?,
        parameter_types: canonical_types(facts.parameter_types())?,
        return_type: canonical_type(facts.return_type())?,
        captures: facts
            .captures()
            .iter()
            .copied()
            .map(hir_local_id)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn canonical_type_parameter_facts(
    facts: &TypeParameterFacts,
) -> Result<CanonicalTypeParameterFacts, CompileError> {
    Ok(CanonicalTypeParameterFacts {
        id: type_parameter_id(facts.id())?,
        name: facts.name().to_owned(),
        span: canonical_span(facts.span())?,
    })
}

fn canonical_record_facts(facts: &RecordFacts) -> Result<CanonicalRecordFacts, CompileError> {
    Ok(CanonicalRecordFacts {
        id: record_id(facts.id())?,
        name: facts.name().to_owned(),
        type_parameters: facts
            .type_parameters()
            .iter()
            .map(canonical_type_parameter_facts)
            .collect::<Result<Vec<_>, _>>()?,
        fields: facts
            .fields()
            .iter()
            .map(canonical_record_field_facts)
            .collect::<Result<Vec<_>, _>>()?,
        name_span: canonical_span(facts.name_span())?,
        span: canonical_span(facts.span())?,
    })
}

fn canonical_record_field_facts(
    facts: &RecordFieldFacts,
) -> Result<CanonicalRecordFieldFacts, CompileError> {
    Ok(CanonicalRecordFieldFacts {
        id: field_id(facts.id())?,
        name: facts.name().to_owned(),
        ty: canonical_type(facts.ty())?,
        span: canonical_span(facts.span())?,
        type_span: canonical_span(facts.type_span())?,
    })
}

fn canonical_union_facts(facts: &UnionFacts) -> Result<CanonicalUnionFacts, CompileError> {
    Ok(CanonicalUnionFacts {
        id: union_id(facts.id())?,
        name: facts.name().to_owned(),
        type_parameters: facts
            .type_parameters()
            .iter()
            .map(canonical_type_parameter_facts)
            .collect::<Result<Vec<_>, _>>()?,
        variants: facts
            .variants()
            .iter()
            .map(canonical_variant_facts)
            .collect::<Result<Vec<_>, _>>()?,
        name_span: canonical_span(facts.name_span())?,
        span: canonical_span(facts.span())?,
    })
}

fn canonical_variant_facts(facts: &VariantFacts) -> Result<CanonicalVariantFacts, CompileError> {
    Ok(CanonicalVariantFacts {
        id: variant_id(facts.id())?,
        name: facts.name().to_owned(),
        payloads: facts
            .payloads()
            .iter()
            .map(canonical_payload_facts)
            .collect::<Result<Vec<_>, _>>()?,
        name_span: canonical_span(facts.name_span())?,
        span: canonical_span(facts.span())?,
    })
}

fn canonical_payload_facts(facts: &PayloadFacts) -> Result<CanonicalPayloadFacts, CompileError> {
    Ok(CanonicalPayloadFacts {
        id: payload_id(facts.id())?,
        name: facts.name().to_owned(),
        ty: canonical_type(facts.ty())?,
        span: canonical_span(facts.span())?,
        type_span: canonical_span(facts.type_span())?,
    })
}

fn canonical_variant_construction_fact(
    span: SourceSpan,
    facts: &VariantConstructionFacts,
) -> Result<CanonicalVariantConstructionFact, CompileError> {
    Ok(CanonicalVariantConstructionFact {
        span: canonical_span(span)?,
        union: union_id(facts.union())?,
        variant: variant_id(facts.variant())?,
        type_arguments: canonical_types(facts.type_arguments())?,
    })
}

fn canonical_call_fact(
    span: SourceSpan,
    facts: &CallFacts,
) -> Result<CanonicalCallFact, CompileError> {
    Ok(CanonicalCallFact {
        span: canonical_span(span)?,
        function: function_id(facts.function())?,
        type_arguments: canonical_types(facts.type_arguments())?,
    })
}

fn canonical_match_fact(
    span: SourceSpan,
    facts: &MatchFacts,
) -> Result<CanonicalMatchFact, CompileError> {
    Ok(CanonicalMatchFact {
        span: canonical_span(span)?,
        union: union_id(facts.union())?,
        type_arguments: canonical_types(facts.type_arguments())?,
        arms: facts
            .arms()
            .iter()
            .map(canonical_match_arm_facts)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn canonical_match_arm_facts(
    facts: &MatchArmFacts,
) -> Result<CanonicalMatchArmFacts, CompileError> {
    Ok(match facts {
        MatchArmFacts::Variant { variant, bindings } => CanonicalMatchArmFacts::Variant {
            variant: variant_id(*variant)?,
            bindings: bindings
                .iter()
                .map(canonical_payload_binding_facts)
                .collect::<Result<Vec<_>, _>>()?,
        },
        MatchArmFacts::Default => CanonicalMatchArmFacts::Default,
    })
}

fn canonical_payload_binding_facts(
    facts: &PayloadBindingFacts,
) -> Result<CanonicalPayloadBindingFacts, CompileError> {
    Ok(CanonicalPayloadBindingFacts {
        payload: payload_id(facts.payload())?,
        local: hir_local_id(facts.local())?,
    })
}

const fn canonical_builtin(builtin: Builtin) -> CanonicalBuiltin {
    match builtin {
        Builtin::Print => CanonicalBuiltin::Print,
        Builtin::ArrayLength => CanonicalBuiltin::ArrayLength,
        Builtin::StringLength => CanonicalBuiltin::StringLength,
        Builtin::ArrayAppend => CanonicalBuiltin::ArrayAppend,
        Builtin::ArrayConcat => CanonicalBuiltin::ArrayConcat,
        Builtin::ToString => CanonicalBuiltin::ToString,
        Builtin::ParseInt => CanonicalBuiltin::ParseInt,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMir {
    modules: Vec<u32>,
    entry_module: u32,
    entry_function: Option<CanonicalFunctionId>,
    records: Vec<CanonicalMirRecord>,
    unions: Vec<CanonicalMirUnion>,
    functions: Vec<CanonicalMirFunction>,
    closures: Vec<CanonicalMirClosure>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMirRecord {
    id: CanonicalRecordId,
    name: String,
    fields: Vec<CanonicalMirRecordField>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMirRecordField {
    id: CanonicalFieldId,
    name: String,
    ty: CanonicalType,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMirUnion {
    id: CanonicalUnionId,
    name: String,
    variants: Vec<CanonicalMirVariant>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMirVariant {
    id: CanonicalVariantId,
    name: String,
    payloads: Vec<CanonicalMirPayload>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMirPayload {
    id: CanonicalPayloadId,
    name: String,
    ty: CanonicalType,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMirFunction {
    id: CanonicalFunctionId,
    name: String,
    parameters: Vec<u32>,
    parameter_types: Vec<CanonicalType>,
    local_count: u32,
    return_type: CanonicalType,
    entry_block: u32,
    blocks: Vec<CanonicalMirBasicBlock>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMirClosure {
    id: CanonicalClosureId,
    captures: Vec<u32>,
    parameters: Vec<u32>,
    parameter_types: Vec<CanonicalType>,
    local_count: u32,
    return_type: CanonicalType,
    entry_block: u32,
    blocks: Vec<CanonicalMirBasicBlock>,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalMirBasicBlock {
    id: u32,
    statements: Vec<CanonicalMirStatement>,
    terminator: CanonicalMirTerminator,
    span: CanonicalSpan,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalMirStatement {
    Store {
        local: u32,
        value: CanonicalMirExpression,
        span: CanonicalSpan,
    },
    Expression {
        expression: CanonicalMirExpression,
        span: CanonicalSpan,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalMirTerminator {
    Goto {
        target: u32,
        span: CanonicalSpan,
    },
    Branch {
        condition: CanonicalMirExpression,
        then_target: u32,
        else_target: u32,
        span: CanonicalSpan,
    },
    SwitchVariant {
        scrutinee: u32,
        union: CanonicalUnionId,
        targets: Vec<u32>,
        span: CanonicalSpan,
    },
    Return {
        value: Option<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalMirExpression {
    Integer {
        value: i64,
        span: CanonicalSpan,
    },
    Boolean {
        value: bool,
        span: CanonicalSpan,
    },
    String {
        value: String,
        span: CanonicalSpan,
    },
    Function {
        function: CanonicalFunctionId,
        span: CanonicalSpan,
    },
    Closure {
        closure: CanonicalClosureId,
        captures: Vec<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    Array {
        elements: Vec<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    Record {
        record: CanonicalRecordId,
        fields: Vec<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    Variant {
        union: CanonicalUnionId,
        variant: CanonicalVariantId,
        payloads: Vec<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    Index {
        target: Box<CanonicalMirExpression>,
        index: Box<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    Length {
        target: Box<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    ArrayIntrinsic {
        operation: CanonicalArrayIntrinsic,
        element_type: CanonicalType,
        target: Box<CanonicalMirExpression>,
        argument: Box<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    Field {
        target: Box<CanonicalMirExpression>,
        record: CanonicalRecordId,
        field: CanonicalFieldId,
        span: CanonicalSpan,
    },
    VariantPayload {
        source: u32,
        union: CanonicalUnionId,
        variant: CanonicalVariantId,
        payload: CanonicalPayloadId,
        span: CanonicalSpan,
    },
    Local {
        local: u32,
        span: CanonicalSpan,
    },
    Unary {
        operator: CanonicalUnaryOperator,
        expression: Box<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    Binary {
        operator: CanonicalBinaryOperator,
        left: Box<CanonicalMirExpression>,
        right: Box<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    Call {
        callee: CanonicalCallee,
        arguments: Vec<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
    IndirectCall {
        callee: Box<CanonicalMirExpression>,
        arguments: Vec<CanonicalMirExpression>,
        span: CanonicalSpan,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum CanonicalArrayIntrinsic {
    Append,
    Concat,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CanonicalCallee {
    Function { id: CanonicalFunctionId },
    Print,
    ToString,
    ParseInt,
}

pub(crate) fn mir_value(program: &MirProgram) -> Result<Value, CompileError> {
    let value = CanonicalMir {
        modules: program
            .modules()
            .iter()
            .copied()
            .map(module_id)
            .collect::<Result<Vec<_>, _>>()?,
        entry_module: module_id(program.entry_module())?,
        entry_function: program.entry_function().map(function_id).transpose()?,
        records: program
            .records()
            .iter()
            .map(canonical_mir_record)
            .collect::<Result<Vec<_>, _>>()?,
        unions: program
            .unions()
            .iter()
            .map(canonical_mir_union)
            .collect::<Result<Vec<_>, _>>()?,
        functions: program
            .functions()
            .iter()
            .map(canonical_mir_function)
            .collect::<Result<Vec<_>, _>>()?,
        closures: program
            .closures()
            .iter()
            .map(canonical_mir_closure)
            .collect::<Result<Vec<_>, _>>()?,
        span: canonical_span(program.span())?,
    };
    Ok(serde_json::to_value(value)?)
}

fn canonical_mir_record(record: &MirRecord) -> Result<CanonicalMirRecord, CompileError> {
    Ok(CanonicalMirRecord {
        id: record_id(record.id())?,
        name: record.name().to_owned(),
        fields: record
            .fields()
            .iter()
            .map(canonical_mir_record_field)
            .collect::<Result<Vec<_>, _>>()?,
        span: canonical_span(record.span())?,
    })
}

fn canonical_mir_record_field(
    field: &MirRecordField,
) -> Result<CanonicalMirRecordField, CompileError> {
    Ok(CanonicalMirRecordField {
        id: field_id(field.id())?,
        name: field.name().to_owned(),
        ty: canonical_type(field.ty())?,
        span: canonical_span(field.span())?,
    })
}

fn canonical_mir_union(union: &MirUnion) -> Result<CanonicalMirUnion, CompileError> {
    Ok(CanonicalMirUnion {
        id: union_id(union.id())?,
        name: union.name().to_owned(),
        variants: union
            .variants()
            .iter()
            .map(canonical_mir_variant)
            .collect::<Result<Vec<_>, _>>()?,
        span: canonical_span(union.span())?,
    })
}

fn canonical_mir_variant(variant: &MirVariant) -> Result<CanonicalMirVariant, CompileError> {
    Ok(CanonicalMirVariant {
        id: variant_id(variant.id())?,
        name: variant.name().to_owned(),
        payloads: variant
            .payloads()
            .iter()
            .map(canonical_mir_payload)
            .collect::<Result<Vec<_>, _>>()?,
        span: canonical_span(variant.span())?,
    })
}

fn canonical_mir_payload(payload: &MirPayload) -> Result<CanonicalMirPayload, CompileError> {
    Ok(CanonicalMirPayload {
        id: payload_id(payload.id())?,
        name: payload.name().to_owned(),
        ty: canonical_type(payload.ty())?,
        span: canonical_span(payload.span())?,
    })
}

fn canonical_mir_function(function: &MirFunction) -> Result<CanonicalMirFunction, CompileError> {
    Ok(CanonicalMirFunction {
        id: function_id(function.id())?,
        name: function.name().to_owned(),
        parameters: function
            .parameter_ids()
            .iter()
            .copied()
            .map(mir_local_id)
            .collect::<Result<Vec<_>, _>>()?,
        parameter_types: canonical_types(function.parameter_types())?,
        local_count: canonical_u32(function.local_count())?,
        return_type: canonical_type(function.return_type())?,
        entry_block: basic_block_id(function.entry_block())?,
        blocks: canonical_mir_blocks(function.blocks())?,
        span: canonical_span(function.span())?,
    })
}

fn canonical_mir_closure(closure: &MirClosure) -> Result<CanonicalMirClosure, CompileError> {
    Ok(CanonicalMirClosure {
        id: closure_id(closure.id())?,
        captures: closure
            .captures()
            .iter()
            .copied()
            .map(mir_local_id)
            .collect::<Result<Vec<_>, _>>()?,
        parameters: closure
            .parameter_ids()
            .iter()
            .copied()
            .map(mir_local_id)
            .collect::<Result<Vec<_>, _>>()?,
        parameter_types: canonical_types(closure.parameter_types())?,
        local_count: canonical_u32(closure.local_count())?,
        return_type: canonical_type(closure.return_type())?,
        entry_block: basic_block_id(closure.entry_block())?,
        blocks: canonical_mir_blocks(closure.blocks())?,
        span: canonical_span(closure.span())?,
    })
}

fn canonical_mir_blocks(
    blocks: &[MirBasicBlock],
) -> Result<Vec<CanonicalMirBasicBlock>, CompileError> {
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            Ok(CanonicalMirBasicBlock {
                id: canonical_u32(index)?,
                statements: block
                    .statements()
                    .iter()
                    .map(canonical_mir_statement)
                    .collect::<Result<Vec<_>, _>>()?,
                terminator: canonical_mir_terminator(block.terminator())?,
                span: canonical_span(block.span())?,
            })
        })
        .collect()
}

fn canonical_mir_statement(
    statement: &MirStatement,
) -> Result<CanonicalMirStatement, CompileError> {
    Ok(match statement {
        MirStatement::Store { local, value, span } => CanonicalMirStatement::Store {
            local: mir_local_id(*local)?,
            value: canonical_mir_expression(value)?,
            span: canonical_span(*span)?,
        },
        MirStatement::Expression { expression, span } => CanonicalMirStatement::Expression {
            expression: canonical_mir_expression(expression)?,
            span: canonical_span(*span)?,
        },
    })
}

fn canonical_mir_terminator(
    terminator: &MirTerminator,
) -> Result<CanonicalMirTerminator, CompileError> {
    Ok(match terminator {
        MirTerminator::Goto { target, span } => CanonicalMirTerminator::Goto {
            target: basic_block_id(*target)?,
            span: canonical_span(*span)?,
        },
        MirTerminator::Branch {
            condition,
            then_target,
            else_target,
            span,
        } => CanonicalMirTerminator::Branch {
            condition: canonical_mir_expression(condition)?,
            then_target: basic_block_id(*then_target)?,
            else_target: basic_block_id(*else_target)?,
            span: canonical_span(*span)?,
        },
        MirTerminator::SwitchVariant {
            scrutinee,
            union,
            targets,
            span,
        } => CanonicalMirTerminator::SwitchVariant {
            scrutinee: mir_local_id(*scrutinee)?,
            union: union_id(*union)?,
            targets: targets
                .iter()
                .copied()
                .map(basic_block_id)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        MirTerminator::Return { value, span } => CanonicalMirTerminator::Return {
            value: value.as_ref().map(canonical_mir_expression).transpose()?,
            span: canonical_span(*span)?,
        },
    })
}

fn canonical_mir_expression(
    expression: &MirExpression,
) -> Result<CanonicalMirExpression, CompileError> {
    Ok(match expression {
        MirExpression::Integer { value, span } => CanonicalMirExpression::Integer {
            value: *value,
            span: canonical_span(*span)?,
        },
        MirExpression::Boolean { value, span } => CanonicalMirExpression::Boolean {
            value: *value,
            span: canonical_span(*span)?,
        },
        MirExpression::String { value, span } => CanonicalMirExpression::String {
            value: value.clone(),
            span: canonical_span(*span)?,
        },
        MirExpression::Function { function, span } => CanonicalMirExpression::Function {
            function: function_id(*function)?,
            span: canonical_span(*span)?,
        },
        MirExpression::Closure {
            closure,
            captures,
            span,
        } => CanonicalMirExpression::Closure {
            closure: closure_id(*closure)?,
            captures: captures
                .iter()
                .map(canonical_mir_expression)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        MirExpression::Array { elements, span } => CanonicalMirExpression::Array {
            elements: elements
                .iter()
                .map(canonical_mir_expression)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        MirExpression::Record {
            record,
            fields,
            span,
        } => CanonicalMirExpression::Record {
            record: record_id(*record)?,
            fields: fields
                .iter()
                .map(canonical_mir_expression)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        MirExpression::Variant {
            union,
            variant,
            payloads,
            span,
        } => CanonicalMirExpression::Variant {
            union: union_id(*union)?,
            variant: variant_id(*variant)?,
            payloads: payloads
                .iter()
                .map(canonical_mir_expression)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        MirExpression::Index {
            target,
            index,
            span,
        } => CanonicalMirExpression::Index {
            target: Box::new(canonical_mir_expression(target)?),
            index: Box::new(canonical_mir_expression(index)?),
            span: canonical_span(*span)?,
        },
        MirExpression::Length { target, span } => CanonicalMirExpression::Length {
            target: Box::new(canonical_mir_expression(target)?),
            span: canonical_span(*span)?,
        },
        MirExpression::ArrayIntrinsic {
            operation,
            element_type,
            target,
            argument,
            span,
        } => CanonicalMirExpression::ArrayIntrinsic {
            operation: canonical_array_intrinsic(*operation),
            element_type: canonical_type(element_type)?,
            target: Box::new(canonical_mir_expression(target)?),
            argument: Box::new(canonical_mir_expression(argument)?),
            span: canonical_span(*span)?,
        },
        MirExpression::Field {
            target,
            record,
            field,
            span,
        } => CanonicalMirExpression::Field {
            target: Box::new(canonical_mir_expression(target)?),
            record: record_id(*record)?,
            field: field_id(*field)?,
            span: canonical_span(*span)?,
        },
        MirExpression::VariantPayload {
            source,
            union,
            variant,
            payload,
            span,
        } => CanonicalMirExpression::VariantPayload {
            source: mir_local_id(*source)?,
            union: union_id(*union)?,
            variant: variant_id(*variant)?,
            payload: payload_id(*payload)?,
            span: canonical_span(*span)?,
        },
        MirExpression::Local { local, span } => CanonicalMirExpression::Local {
            local: mir_local_id(*local)?,
            span: canonical_span(*span)?,
        },
        MirExpression::Unary {
            operator,
            expression,
            span,
        } => CanonicalMirExpression::Unary {
            operator: canonical_unary_operator(*operator),
            expression: Box::new(canonical_mir_expression(expression)?),
            span: canonical_span(*span)?,
        },
        MirExpression::Binary {
            operator,
            left,
            right,
            span,
        } => CanonicalMirExpression::Binary {
            operator: canonical_binary_operator(*operator),
            left: Box::new(canonical_mir_expression(left)?),
            right: Box::new(canonical_mir_expression(right)?),
            span: canonical_span(*span)?,
        },
        MirExpression::Call {
            callee,
            arguments,
            span,
        } => CanonicalMirExpression::Call {
            callee: canonical_callee(*callee)?,
            arguments: arguments
                .iter()
                .map(canonical_mir_expression)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
        MirExpression::IndirectCall {
            callee,
            arguments,
            span,
        } => CanonicalMirExpression::IndirectCall {
            callee: Box::new(canonical_mir_expression(callee)?),
            arguments: arguments
                .iter()
                .map(canonical_mir_expression)
                .collect::<Result<Vec<_>, _>>()?,
            span: canonical_span(*span)?,
        },
    })
}

const fn canonical_array_intrinsic(operation: ArrayIntrinsic) -> CanonicalArrayIntrinsic {
    match operation {
        ArrayIntrinsic::Append => CanonicalArrayIntrinsic::Append,
        ArrayIntrinsic::Concat => CanonicalArrayIntrinsic::Concat,
    }
}

fn canonical_callee(callee: Callee) -> Result<CanonicalCallee, CompileError> {
    Ok(match callee {
        Callee::Function(id) => CanonicalCallee::Function {
            id: function_id(id)?,
        },
        Callee::Print => CanonicalCallee::Print,
        Callee::ToString => CanonicalCallee::ToString,
        Callee::ParseInt => CanonicalCallee::ParseInt,
    })
}

fn canonical_type(ty: &Type) -> Result<CanonicalType, CompileError> {
    Ok(match ty {
        Type::Int => CanonicalType::Int,
        Type::Bool => CanonicalType::Bool,
        Type::String => CanonicalType::String,
        Type::Array(element) => CanonicalType::Array {
            element: Box::new(canonical_type(element)?),
        },
        Type::Function {
            parameters,
            return_type,
        } => CanonicalType::Function {
            parameters: canonical_types(parameters)?,
            return_type: Box::new(canonical_type(return_type)?),
        },
        Type::Parameter(id) => CanonicalType::Parameter {
            id: type_parameter_id(*id)?,
        },
        Type::Record {
            definition,
            arguments,
        } => CanonicalType::Record {
            definition: record_id(*definition)?,
            arguments: canonical_types(arguments)?,
        },
        Type::Union {
            definition,
            arguments,
        } => CanonicalType::Union {
            definition: union_id(*definition)?,
            arguments: canonical_types(arguments)?,
        },
        Type::Unit => CanonicalType::Unit,
    })
}

fn canonical_types(types: &[Type]) -> Result<Vec<CanonicalType>, CompileError> {
    types.iter().map(canonical_type).collect()
}

fn canonical_span(span: SourceSpan) -> Result<CanonicalSpan, CompileError> {
    Ok(CanonicalSpan {
        source: span.file().raw(),
        start: canonical_u32(span.range().start())?,
        end: canonical_u32(span.range().end())?,
    })
}

fn module_id(id: ModuleId) -> Result<u32, CompileError> {
    canonical_u32(id.index())
}

fn function_id(id: FunctionId) -> Result<CanonicalFunctionId, CompileError> {
    Ok(CanonicalFunctionId {
        module: module_id(id.module())?,
        index: canonical_u32(id.index())?,
    })
}

fn closure_id(id: ClosureId) -> Result<CanonicalClosureId, CompileError> {
    Ok(CanonicalClosureId {
        owner: function_id(id.owner())?,
        source_index: canonical_u32(id.source_index())?,
    })
}

fn record_id(id: RecordId) -> Result<CanonicalRecordId, CompileError> {
    Ok(CanonicalRecordId {
        module: module_id(id.module())?,
        index: canonical_u32(id.index())?,
    })
}

fn field_id(id: FieldId) -> Result<CanonicalFieldId, CompileError> {
    Ok(CanonicalFieldId {
        record: record_id(id.record())?,
        index: canonical_u32(id.index())?,
    })
}

fn union_id(id: UnionId) -> Result<CanonicalUnionId, CompileError> {
    Ok(CanonicalUnionId {
        module: module_id(id.module())?,
        index: canonical_u32(id.index())?,
    })
}

fn variant_id(id: VariantId) -> Result<CanonicalVariantId, CompileError> {
    Ok(CanonicalVariantId {
        union: union_id(id.union())?,
        index: canonical_u32(id.index())?,
    })
}

fn payload_id(id: PayloadId) -> Result<CanonicalPayloadId, CompileError> {
    Ok(CanonicalPayloadId {
        variant: variant_id(id.variant())?,
        index: canonical_u32(id.index())?,
    })
}

fn type_parameter_id(id: TypeParameterId) -> Result<CanonicalTypeParameterId, CompileError> {
    Ok(CanonicalTypeParameterId {
        owner: match id.owner() {
            TypeParameterOwner::Function(owner) => CanonicalTypeParameterOwner::Function {
                id: function_id(owner)?,
            },
            TypeParameterOwner::Record(owner) => CanonicalTypeParameterOwner::Record {
                id: record_id(owner)?,
            },
            TypeParameterOwner::Union(owner) => CanonicalTypeParameterOwner::Union {
                id: union_id(owner)?,
            },
        },
        index: canonical_u32(id.index())?,
    })
}

fn hir_local_id(id: HirLocalId) -> Result<u32, CompileError> {
    canonical_u32(id.index())
}

fn mir_local_id(id: MirLocalId) -> Result<u32, CompileError> {
    canonical_u32(id.index())
}

fn basic_block_id(id: BasicBlockId) -> Result<u32, CompileError> {
    canonical_u32(id.index())
}

fn canonical_u32(value: usize) -> Result<u32, CompileError> {
    u32::try_from(value).map_err(|_| CompileError::CanonicalOffsetOverflow)
}
