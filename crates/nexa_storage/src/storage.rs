use std::fmt::{Debug, Display, Formatter};
use std::mem::{self, ManuallyDrop};

use thiserror::Error;

use crate::{AllocationError, AllocationLease, AllocatorId, StorageAllocator, ValueLayout};

/// A uniquely owned, bounded sequence with deterministic reverse Drop.
#[derive(Debug)]
pub struct StorageVec<T> {
    values: Vec<ManuallyDrop<T>>,
    allocator: StorageAllocator,
    lease: Option<AllocationLease>,
    accounted_capacity: usize,
    max_len: usize,
}

impl<T> StorageVec<T> {
    /// Creates an empty sequence bounded only by representable Host capacity.
    #[must_use]
    pub fn new(allocator: StorageAllocator) -> Self {
        Self::with_limit(allocator, usize::MAX)
    }

    /// Creates an empty sequence with an explicit logical element limit.
    #[must_use]
    pub fn with_limit(allocator: StorageAllocator, max_len: usize) -> Self {
        let lease = allocator.empty_lease();
        Self {
            values: Vec::new(),
            allocator,
            lease: Some(lease),
            accounted_capacity: 0,
            max_len,
        }
    }

    /// Returns the number of initialized logical elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns true when no logical element is initialized.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the allocator provenance retained by this sequence.
    #[must_use]
    pub fn allocator_id(&self) -> AllocatorId {
        self.allocator.id()
    }

    /// Returns one initialized element by stable index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.values.get(index).map(|value| &**value)
    }

    /// Returns one initialized element mutably by stable index.
    #[must_use]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.values.get_mut(index).map(|value| &mut **value)
    }

    /// Iterates initialized elements in ascending index order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        self.values.iter().map(|value| &**value)
    }

    /// Appends a value, moving it only after capacity has been secured.
    ///
    /// # Errors
    ///
    /// Returns [`PushError`] with the original value when the element limit,
    /// logical allocator budget, arithmetic checks, or Host reserve fails.
    pub fn try_push(&mut self, value: T) -> Result<(), PushError<T>> {
        if let Err(error) = self.ensure_capacity(1) {
            return Err(PushError { error, value });
        }
        self.values.push(ManuallyDrop::new(value));
        Ok(())
    }

    /// Removes and returns the last initialized element.
    pub fn pop(&mut self) -> Option<T> {
        self.values.pop().map(ManuallyDrop::into_inner)
    }

    /// Drops every initialized element in reverse index order.
    pub fn clear(&mut self) {
        while let Some(value) = self.pop() {
            drop(value);
        }
    }

    /// Consumes the mutable sequence and transfers its storage to an immutable array.
    #[must_use]
    pub fn freeze(mut self) -> StorageArray<T> {
        StorageArray {
            values: mem::take(&mut self.values),
            allocator: self.allocator.clone(),
            _lease: self.lease.take(),
        }
    }

    fn ensure_capacity(&mut self, additional: usize) -> Result<(), StorageError> {
        let required = self
            .len()
            .checked_add(additional)
            .ok_or(StorageError::LengthOverflow)?;
        if required > self.max_len {
            return Err(StorageError::CapacityExceeded {
                limit: self.max_len,
            });
        }
        if required <= self.accounted_capacity {
            return Ok(());
        }

        let doubled = self.accounted_capacity.saturating_mul(2).max(1);
        let next_capacity = doubled.max(required).min(self.max_len);
        let count = u64::try_from(next_capacity).map_err(|_| StorageError::LengthOverflow)?;
        let bytes = ValueLayout::of::<T>()
            .checked_array_bytes(count)
            .map_err(|_| StorageError::LengthOverflow)?;
        let lease = self.lease.as_ref().ok_or(StorageError::MissingLease)?;
        self.allocator.validate_resize(lease, bytes)?;
        self.values
            .try_reserve_exact(next_capacity.saturating_sub(self.values.len()))
            .map_err(|_| self.allocator.host_reserve_error(bytes))?;
        let lease = self.lease.as_mut().ok_or(StorageError::MissingLease)?;
        self.allocator.commit_resize(lease, bytes)?;
        self.accounted_capacity = next_capacity;
        Ok(())
    }
}

impl<T> Drop for StorageVec<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

/// An immutable owned sequence produced by [`StorageVec::freeze`].
#[derive(Debug)]
pub struct StorageArray<T> {
    values: Vec<ManuallyDrop<T>>,
    allocator: StorageAllocator,
    _lease: Option<AllocationLease>,
}

impl<T> StorageArray<T> {
    /// Returns the number of initialized logical elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns true when no logical element is present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the allocator provenance transferred from the mutable sequence.
    #[must_use]
    pub fn allocator_id(&self) -> AllocatorId {
        self.allocator.id()
    }

    /// Returns one element by stable index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.values.get(index).map(|value| &**value)
    }

    /// Iterates elements in ascending index order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        self.values.iter().map(|value| &**value)
    }
}

impl<T> Drop for StorageArray<T> {
    fn drop(&mut self) {
        while let Some(value) = self.values.pop() {
            drop(ManuallyDrop::into_inner(value));
        }
    }
}

/// A fallible UTF-8 builder with explicit allocator provenance and byte limit.
#[derive(Debug)]
pub struct StorageStringBuilder {
    value: String,
    allocator: StorageAllocator,
    lease: Option<AllocationLease>,
    accounted_capacity: usize,
    max_bytes: usize,
}

impl StorageStringBuilder {
    /// Creates a builder bounded only by representable Host capacity.
    #[must_use]
    pub fn new(allocator: StorageAllocator) -> Self {
        Self::with_limit(allocator, usize::MAX)
    }

    /// Creates a builder with an explicit UTF-8 byte limit.
    #[must_use]
    pub fn with_limit(allocator: StorageAllocator, max_bytes: usize) -> Self {
        let lease = allocator.empty_lease();
        Self {
            value: String::new(),
            allocator,
            lease: Some(lease),
            accounted_capacity: 0,
            max_bytes,
        }
    }

    /// Returns the current UTF-8 byte length.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.value.len()
    }

    /// Appends valid UTF-8 after securing logical and Host capacity.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the byte limit, allocator budget, size
    /// arithmetic, or Host reserve rejects the append.
    pub fn try_push_str(&mut self, value: &str) -> Result<(), StorageError> {
        let required = self
            .value
            .len()
            .checked_add(value.len())
            .ok_or(StorageError::LengthOverflow)?;
        if required > self.max_bytes {
            return Err(StorageError::CapacityExceeded {
                limit: self.max_bytes,
            });
        }
        if required > self.accounted_capacity {
            let doubled = self.accounted_capacity.saturating_mul(2).max(1);
            let next_capacity = doubled.max(required).min(self.max_bytes);
            let bytes = u64::try_from(next_capacity).map_err(|_| StorageError::LengthOverflow)?;
            let lease = self.lease.as_ref().ok_or(StorageError::MissingLease)?;
            self.allocator.validate_resize(lease, bytes)?;
            self.value
                .try_reserve_exact(next_capacity.saturating_sub(self.value.len()))
                .map_err(|_| self.allocator.host_reserve_error(bytes))?;
            let lease = self.lease.as_mut().ok_or(StorageError::MissingLease)?;
            self.allocator.commit_resize(lease, bytes)?;
            self.accounted_capacity = next_capacity;
        }
        self.value.push_str(value);
        Ok(())
    }

    /// Finishes the builder without copying its UTF-8 bytes.
    #[must_use]
    pub fn finish(mut self) -> StorageString {
        StorageString {
            value: mem::take(&mut self.value),
            allocator: self.allocator.clone(),
            _lease: self.lease.take(),
        }
    }
}

/// An immutable owned UTF-8 value with allocator provenance.
#[derive(Debug)]
pub struct StorageString {
    value: String,
    allocator: StorageAllocator,
    _lease: Option<AllocationLease>,
}

impl StorageString {
    /// Returns the complete UTF-8 text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Returns the UTF-8 byte length, excluding any implicit terminator.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.value.len()
    }

    /// Returns allocator provenance for this owned string.
    #[must_use]
    pub fn allocator_id(&self) -> AllocatorId {
        self.allocator.id()
    }

    /// Explicitly clones this logical value into an independent allocation.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the target allocator or Host cannot reserve
    /// the complete UTF-8 byte sequence.
    pub fn try_clone_in(&self, allocator: StorageAllocator) -> Result<Self, StorageError> {
        let mut builder = StorageStringBuilder::with_limit(allocator, self.value.len());
        builder.try_push_str(&self.value)?;
        Ok(builder.finish())
    }
}

/// Failure category shared by bounded sequence and string operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StorageError {
    /// Logical or Host allocation failed.
    #[error(transparent)]
    Allocation(#[from] AllocationError),
    /// A configured element or byte bound was exceeded.
    #[error("storage capacity limit {limit} exceeded")]
    CapacityExceeded {
        /// Configured logical limit.
        limit: usize,
    },
    /// Length or capacity arithmetic overflowed.
    #[error("storage length calculation overflowed")]
    LengthOverflow,
    /// A private allocation lease invariant was violated.
    #[error("storage allocation lease is missing")]
    MissingLease,
}

/// Failed append that returns ownership of the uncommitted value.
#[derive(Debug)]
pub struct PushError<T> {
    error: StorageError,
    value: T,
}

impl<T> PushError<T> {
    /// Returns the structured failure without consuming the rejected value.
    #[must_use]
    pub const fn error(&self) -> &StorageError {
        &self.error
    }

    /// Returns ownership of the value that was never appended.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T> Display for PushError<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.error, formatter)
    }
}

impl<T: Debug> std::error::Error for PushError<T> {}
