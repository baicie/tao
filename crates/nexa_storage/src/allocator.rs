use std::cell::Cell;
use std::rc::Rc;

use thiserror::Error;

use crate::{LayoutError, ValueLayout};

/// Human-readable identity for one logical storage allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AllocatorId(u32);

impl AllocatorId {
    /// Creates an allocator identity for diagnostics and manifests.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw diagnostic identity.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug)]
struct AllocatorState {
    id: AllocatorId,
    max_bytes: u64,
    reserved_bytes: Cell<u64>,
}

/// A deterministic, fallible logical allocator for safe Host-backed storage.
///
/// The kernel deliberately leaves physical allocation to safe standard-library
/// containers. This type enforces portable byte accounting and provenance;
/// raw/custom allocator integration belongs behind the future NIR/backend
/// boundary.
#[derive(Debug, Clone)]
pub struct StorageAllocator {
    state: Rc<AllocatorState>,
}

impl StorageAllocator {
    /// Creates an allocator with a diagnostic identity and byte budget.
    #[must_use]
    pub fn new(id: AllocatorId, max_bytes: u64) -> Self {
        Self {
            state: Rc::new(AllocatorState {
                id,
                max_bytes,
                reserved_bytes: Cell::new(0),
            }),
        }
    }

    /// Creates an anonymous allocator with the provided byte budget.
    #[must_use]
    pub fn with_capacity(max_bytes: u64) -> Self {
        Self::new(AllocatorId::new(0), max_bytes)
    }

    /// Returns the diagnostic allocator identity.
    #[must_use]
    pub fn id(&self) -> AllocatorId {
        self.state.id
    }

    /// Returns the maximum logical backing-byte budget.
    #[must_use]
    pub fn max_bytes(&self) -> u64 {
        self.state.max_bytes
    }

    /// Returns logical bytes currently held by live allocation leases.
    #[must_use]
    pub fn reserved_bytes(&self) -> u64 {
        self.state.reserved_bytes.get()
    }

    /// Reserves storage for `count` values with the supplied layout.
    ///
    /// # Errors
    ///
    /// Returns [`AllocationError`] on size overflow or exhausted budget.
    pub fn try_reserve(
        &self,
        layout: ValueLayout,
        count: u64,
    ) -> Result<AllocationLease, AllocationError> {
        let bytes = layout
            .checked_array_bytes(count)
            .map_err(|error| self.layout_error(error))?;
        self.reserve_delta(bytes)?;
        Ok(AllocationLease {
            state: Rc::clone(&self.state),
            reserved_bytes: bytes,
        })
    }

    /// Resizes a live allocation without changing its allocator provenance.
    ///
    /// Failed growth leaves both the allocation and allocator accounting
    /// unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`AllocationError`] for layout overflow, exhausted budget, or
    /// an allocation owned by a different allocator instance.
    pub fn try_resize(
        &self,
        allocation: &mut AllocationLease,
        layout: ValueLayout,
        count: u64,
    ) -> Result<(), AllocationError> {
        let bytes = layout
            .checked_array_bytes(count)
            .map_err(|error| self.layout_error(error))?;
        self.commit_resize(allocation, bytes)
    }

    pub(crate) fn empty_lease(&self) -> AllocationLease {
        AllocationLease {
            state: Rc::clone(&self.state),
            reserved_bytes: 0,
        }
    }

    pub(crate) fn validate_resize(
        &self,
        allocation: &AllocationLease,
        bytes: u64,
    ) -> Result<(), AllocationError> {
        if !Rc::ptr_eq(&self.state, &allocation.state) {
            return Err(self.error(AllocationErrorKind::ProvenanceMismatch, bytes));
        }
        if bytes <= allocation.reserved_bytes {
            return Ok(());
        }
        let delta = bytes - allocation.reserved_bytes;
        let available = self
            .state
            .max_bytes
            .saturating_sub(self.state.reserved_bytes.get());
        if delta > available {
            return Err(self.error(AllocationErrorKind::OutOfMemory, bytes));
        }
        Ok(())
    }

    pub(crate) fn commit_resize(
        &self,
        allocation: &mut AllocationLease,
        bytes: u64,
    ) -> Result<(), AllocationError> {
        self.validate_resize(allocation, bytes)?;
        let used = self.state.reserved_bytes.get();
        let next = if bytes >= allocation.reserved_bytes {
            used + (bytes - allocation.reserved_bytes)
        } else {
            used - (allocation.reserved_bytes - bytes)
        };
        self.state.reserved_bytes.set(next);
        allocation.reserved_bytes = bytes;
        Ok(())
    }

    pub(crate) fn host_reserve_error(&self, requested_bytes: u64) -> AllocationError {
        self.error(AllocationErrorKind::OutOfMemory, requested_bytes)
    }

    fn reserve_delta(&self, bytes: u64) -> Result<(), AllocationError> {
        let used = self.state.reserved_bytes.get();
        let Some(next) = used.checked_add(bytes) else {
            return Err(self.error(AllocationErrorKind::SizeOverflow, bytes));
        };
        if next > self.state.max_bytes {
            return Err(self.error(AllocationErrorKind::OutOfMemory, bytes));
        }
        self.state.reserved_bytes.set(next);
        Ok(())
    }

    fn layout_error(&self, error: LayoutError) -> AllocationError {
        let kind = match error {
            LayoutError::InvalidAlignment { .. } => AllocationErrorKind::InvalidLayout,
            LayoutError::SizeOverflow | LayoutError::HostSizeOverflow { .. } => {
                AllocationErrorKind::SizeOverflow
            }
        };
        self.error(kind, 0)
    }

    fn error(&self, kind: AllocationErrorKind, requested_bytes: u64) -> AllocationError {
        AllocationError {
            kind,
            allocator: self.id(),
            requested_bytes,
            available_bytes: self
                .state
                .max_bytes
                .saturating_sub(self.state.reserved_bytes.get()),
        }
    }
}

/// Unique ownership of logical allocation budget and provenance.
#[derive(Debug)]
pub struct AllocationLease {
    state: Rc<AllocatorState>,
    reserved_bytes: u64,
}

impl AllocationLease {
    /// Returns the diagnostic identity of the owning allocator.
    #[must_use]
    pub fn allocator_id(&self) -> AllocatorId {
        self.state.id
    }

    /// Returns logical bytes charged to this allocation.
    #[must_use]
    pub const fn reserved_bytes(&self) -> u64 {
        self.reserved_bytes
    }
}

impl Drop for AllocationLease {
    fn drop(&mut self) {
        let used = self.state.reserved_bytes.get();
        debug_assert!(
            used >= self.reserved_bytes,
            "allocation lease exceeds allocator accounting"
        );
        self.state
            .reserved_bytes
            .set(used.saturating_sub(self.reserved_bytes));
    }
}

/// Stable category for a storage allocation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationErrorKind {
    /// Layout or capacity arithmetic overflowed.
    SizeOverflow,
    /// The Host-backed storage request or logical budget could not be satisfied.
    OutOfMemory,
    /// An allocator attempted to manipulate another allocator's lease.
    ProvenanceMismatch,
    /// A supplied layout did not satisfy kernel invariants.
    InvalidLayout,
}

/// Structured failure from the logical storage allocator.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "allocator {allocator:?} rejected {requested_bytes} bytes ({kind:?}); {available_bytes} bytes available"
)]
pub struct AllocationError {
    kind: AllocationErrorKind,
    allocator: AllocatorId,
    requested_bytes: u64,
    available_bytes: u64,
}

impl AllocationError {
    /// Returns the stable failure category.
    #[must_use]
    pub const fn kind(&self) -> AllocationErrorKind {
        self.kind
    }

    /// Returns the allocator's diagnostic identity.
    #[must_use]
    pub const fn allocator_id(&self) -> AllocatorId {
        self.allocator
    }

    /// Returns the requested final or incremental byte count.
    #[must_use]
    pub const fn requested_bytes(&self) -> u64 {
        self.requested_bytes
    }

    /// Returns unreserved logical bytes at the failure point.
    #[must_use]
    pub const fn available_bytes(&self) -> u64 {
        self.available_bytes
    }
}
