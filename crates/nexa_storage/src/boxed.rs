use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, DerefMut};

use crate::{AllocationError, AllocationLease, AllocatorId, StorageAllocator, ValueLayout};

/// A uniquely owned, allocator-accounted heap value.
///
/// `StorageBox` is deliberately not `Clone` or `Copy`; moving it transfers the
/// only owner. The reference kernel charges logical bytes through
/// [`StorageAllocator`] while the standard-library `Box` owns the physical
/// value. Application-profile physical allocation failure retains the normal
/// abort behavior.
///
/// ```compile_fail
/// use nexa_storage::{StorageAllocator, StorageBox};
///
/// let allocator = StorageAllocator::with_capacity(64);
/// let boxed = StorageBox::try_new(allocator, 7_u64).unwrap();
/// let moved = boxed;
/// let _used_again = boxed;
/// let _ = moved;
/// ```
#[derive(Debug)]
pub struct StorageBox<T> {
    value: Box<T>,
    allocator: StorageAllocator,
    _lease: AllocationLease,
}

impl<T> StorageBox<T> {
    /// Allocates a uniquely owned value while preserving the input on logical
    /// allocation failure.
    ///
    /// # Errors
    ///
    /// Returns [`StorageBoxError`] when the allocator cannot reserve the
    /// logical layout for `T`.
    pub fn try_new(allocator: StorageAllocator, value: T) -> Result<Self, StorageBoxError<T>> {
        let lease = match allocator.try_reserve(ValueLayout::of::<T>(), 1) {
            Ok(lease) => lease,
            Err(error) => return Err(StorageBoxError { error, value }),
        };

        Ok(Self {
            value: Box::new(value),
            allocator,
            _lease: lease,
        })
    }

    /// Returns the allocator provenance retained by this owner.
    #[must_use]
    pub fn allocator_id(&self) -> AllocatorId {
        self.allocator.id()
    }

    /// Consumes the owner and returns its value.
    #[must_use]
    pub fn into_inner(self) -> T {
        let Self { value, .. } = self;
        *value
    }
}

impl<T> Deref for StorageBox<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for StorageBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

/// Failure while creating a [`StorageBox`].
#[derive(Debug)]
pub struct StorageBoxError<T> {
    error: AllocationError,
    value: T,
}

impl<T> StorageBoxError<T> {
    /// Returns the structured allocation failure.
    #[must_use]
    pub const fn error(&self) -> &AllocationError {
        &self.error
    }

    /// Returns ownership of the value that was never boxed.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T> Display for StorageBoxError<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.error, formatter)
    }
}

impl<T: Debug> std::error::Error for StorageBoxError<T> {}
