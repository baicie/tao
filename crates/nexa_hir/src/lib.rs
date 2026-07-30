#![forbid(unsafe_code)]
//! High-level intermediate representation and semantic checking for Nexa.
//!
//! This crate lowers a syntax-valid lossless CST into HIR, then validates names,
//! calls, types, and function returns. It deliberately does not parse source or
//! execute programs.

mod check;
mod hir;
mod lower;

pub use check::{
    type_check, type_check_modules, Analysis, Builtin, FunctionFacts, LocalId, MatchArmFacts,
    MatchFacts, NameResolution, PayloadBindingFacts, PayloadFacts, RecordFacts, RecordFieldFacts,
    ResolvedImport, TypedProgram, UnionFacts, VariantConstructionFacts, VariantFacts,
};
pub use hir::{
    AssignmentStatement, BinaryOperator, Block, BreakStatement, ConstDeclaration,
    ContinueStatement, DefId, Expression, ExpressionStatement, FieldId, Function, FunctionId,
    IfStatement, ImportDeclaration, LetDeclaration, MatchArm, MatchPattern, ModuleId, Name,
    Parameter, PayloadId, Program, RecordDeclaration, RecordFieldDeclaration,
    RecordFieldInitializer, RecordId, ReturnStatement, Statement, Type, TypeReference,
    TypeReferenceKind, UnaryOperator, UnionDeclaration, UnionId, UnionVariantDeclaration,
    VariantId, VariantPayloadDeclaration, Visibility, WhileStatement,
};
pub use lower::{lower, lower_module, LoweringError};
