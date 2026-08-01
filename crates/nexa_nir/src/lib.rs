#![forbid(unsafe_code)]
//! Typed, verifiable target-neutral IR for the Futao bootstrap toolchain.
//!
//! Builders produce [`UnverifiedModule`] values. Consumers must pass those
//! values through [`Verifier`] before code generation or serialization.

mod artifact;
mod builder;
mod layout;
mod model;
mod verify;

pub use artifact::{
    ArtifactCompatibility, ArtifactError, ArtifactErrorCode, ArtifactMetadata, CanonicalArtifact,
    NIR_ARTIFACT_MAGIC, NIR_SCHEMA_VERSION,
};
pub use builder::{BuilderError, ModuleBuilder, MAX_NIR_IDENTIFIER_BYTES};
pub use layout::{Endianness, LayoutError, LayoutErrorCode, TargetLayout, ValueLayout};
pub use model::{
    BlockId, FunctionId, InstructionId, IntrinsicId, NirSpan, NirType, Operation, Terminator,
    TypeId, TypedValue, UnverifiedModule, ValueId, ValueOwnership,
};
pub use verify::{VerificationCode, VerificationError, VerifiedModule, Verifier};
