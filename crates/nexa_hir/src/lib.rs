#![forbid(unsafe_code)]
//! High-level intermediate representation and semantic checking for Nexa.
//!
//! This crate lowers a syntax-valid lossless CST into HIR, then validates names,
//! calls, types, and function returns. It deliberately does not parse source or
//! execute programs.

mod check;
mod hir;
mod lower;

pub use check::{type_check, Analysis, TypedProgram};
pub use hir::{
    AssignmentStatement, BinaryOperator, Block, BreakStatement, ConstDeclaration,
    ContinueStatement, Expression, ExpressionStatement, Function, IfStatement, LetDeclaration,
    Name, Parameter, Program, ReturnStatement, Statement, Type, TypeReference, UnaryOperator,
    WhileStatement,
};
pub use lower::{lower, LoweringError};
