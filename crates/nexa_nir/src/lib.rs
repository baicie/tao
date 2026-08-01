#![forbid(unsafe_code)]
//! Typed, verifiable target-neutral IR for the Futao bootstrap toolchain.
//!
//! Builders produce [`UnverifiedModule`] values. Consumers must pass those
//! values through [`Verifier`] before code generation or serialization.

mod builder;
mod model;
mod verify;

pub use builder::{BuilderError, ModuleBuilder};
pub use model::{
    BlockId, FunctionId, InstructionId, NirSpan, NirType, Operation, Terminator, TypeId,
    TypedValue, UnverifiedModule, ValueId, ValueOwnership,
};
pub use verify::{VerificationCode, VerificationError, VerifiedModule, Verifier};
