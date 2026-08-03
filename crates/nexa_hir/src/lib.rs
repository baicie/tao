#![forbid(unsafe_code)]
//! High-level intermediate representation and semantic checking for Nexa.
//!
//! This crate lowers a syntax-valid lossless CST into HIR, then validates names,
//! calls, types, and function returns. It deliberately does not parse source or
//! execute programs.

mod check;
mod hir;
mod lower;
mod resolve;

pub use check::{
    type_check, type_check_modules, Analysis, Builtin, CallFacts, ClosureFacts, FunctionFacts,
    LocalId, MatchArmFacts, MatchFacts, NameResolution, PayloadBindingFacts, PayloadFacts,
    RecordFacts, RecordFieldFacts, ResolvedImport, TypeParameterFacts, TypedProgram, UnionFacts,
    VariantConstructionFacts, VariantFacts,
};
pub use hir::{
    ArrowBody, AssignmentStatement, BinaryOperator, Block, BreakStatement, ClosureId,
    ConstDeclaration, ContinueStatement, DefId, Expression, ExpressionStatement, FieldId,
    ForOfStatement, Function, FunctionId, IfStatement, ImportDeclaration, LetDeclaration, MatchArm,
    MatchPattern, ModuleId, Name, Parameter, PayloadId, Program, RecordDeclaration,
    RecordFieldDeclaration, RecordFieldInitializer, RecordId, ReturnStatement, Statement, Type,
    TypeParameter, TypeParameterId, TypeParameterOwner, TypeReference, TypeReferenceKind,
    UnaryOperator, UnionDeclaration, UnionId, UnionVariantDeclaration, VariantId,
    VariantPayloadDeclaration, Visibility, WhileStatement,
};
pub use lower::{lower, lower_module, LoweringError};
pub use resolve::{
    resolve_modules, Resolution, ResolvedBinding, ResolvedName, ResolverScope, ResolverScopeId,
    ResolverScopeKind, ResolverSymbol,
};
