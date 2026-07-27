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
    BinaryOperator, Block, ConstDeclaration, Expression, ExpressionStatement, Function,
    IfStatement, Name, Parameter, Program, ReturnStatement, Statement, Type, TypeReference,
    UnaryOperator,
};
pub use lower::{lower, LoweringError};
