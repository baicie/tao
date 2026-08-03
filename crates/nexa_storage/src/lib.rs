#![forbid(unsafe_code)]
//! Ownership and deterministic storage contracts for the Futao bootstrap toolchain.
//!
//! The crate is a safe Rust reference kernel. It does not define NIR, a public
//! ABI, or a target-independent physical object layout.
//!
//! Plain arenas accept only [`Copy`] values, so values with destructors cannot
//! silently bypass cleanup:
//!
//! ```compile_fail
//! use nexa_storage::{PlainArena, StorageAllocator};
//!
//! struct NeedsDrop;
//! impl Drop for NeedsDrop {
//!     fn drop(&mut self) {}
//! }
//!
//! let allocator = StorageAllocator::with_capacity(64);
//! let mut arena = PlainArena::<NeedsDrop>::new(allocator, 4);
//! let _ = arena.try_store(NeedsDrop);
//! ```
//!
//! Arena borrows cannot outlive their owner:
//!
//! ```compile_fail
//! use nexa_storage::{DropArena, StorageAllocator};
//!
//! let escaped = {
//!     let allocator = StorageAllocator::with_capacity(64);
//!     let mut arena = DropArena::new(allocator, 4);
//!     let handle = arena.try_store(String::from("temporary"))?;
//!     arena.get(&handle)?
//! };
//! println!("{escaped}");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod allocator;
mod arena;
mod boxed;
mod collections;
mod layout;
mod ownership;
mod shared;
mod storage;

pub use allocator::{
    AllocationError, AllocationErrorKind, AllocationLease, AllocatorId, StorageAllocator,
};
pub use arena::{ArenaAccessError, ArenaHandle, ArenaResetError, DropArena, PlainArena};
pub use boxed::{StorageBox, StorageBoxError};
pub use collections::{
    BitSet, BitSetError, DeterministicMap, DeterministicSet, IntMap, IntSet, InternError,
    InternTable, MapCapacityError, SetCapacityError, StringMap, StringSet, SymbolId,
};
pub use layout::{LayoutError, ValueLayout};
pub use ownership::{
    CleanupAction, OwnershipEffect, OwnershipError, OwnershipErrorKind, OwnershipFrame, PlaceId,
    PlaceKind, PlaceState,
};
pub use shared::{Shared, SharedAccessError, SharedError, Weak};
pub use storage::{
    PushError, StorageArray, StorageError, StorageString, StorageStringBuilder, StorageVec,
};
