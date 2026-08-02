#![forbid(unsafe_code)]
//! Target-independent middle IR and its reference interpreter.
//!
//! MIR is lowered only from validated [`nexa_hir::TypedProgram`] values. Names
//! are resolved to slots and function identifiers before execution, so the
//! interpreter has no dependency on syntax trees or source text.

mod execute;
mod lower;
mod mir;

pub use execute::{
    run, run_with_args, run_with_args_and_step_limit, CallableValue, Execution, RuntimeError,
    RuntimeFailure, Value,
};
pub use lower::{lower, MirLoweringError};
pub use mir::{
    ArrayIntrinsic, BasicBlockId, Callee, LocalId, MirBasicBlock, MirClosure, MirExpression,
    MirFunction, MirPayload, MirProgram, MirRecord, MirRecordField, MirStatement, MirTerminator,
    MirUnion, MirVariant,
};
pub use nexa_hir::{ClosureId, FunctionId, ModuleId};
