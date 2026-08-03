use std::fmt::{Debug, Display, Formatter};
use std::mem::{self, ManuallyDrop};

use thiserror::Error;

use crate::{AllocationError, StorageAllocator, ValueLayout};

/// A bounded runtime reference model for partially initialized homogeneous slots.
///
/// Values are committed in successful-initialization order. Slot lookup remains
/// index-based, while cleanup pops committed values in reverse order. The model
/// is deliberately homogeneous and does not claim to be the final physical
/// layout for heterogeneous Futao records.
#[derive(Debug)]
pub struct PartialInit<T> {
    slots: Vec<Option<usize>>,
    values: Vec<ManuallyDrop<T>>,
    allocator: StorageAllocator,
    lease: Option<crate::AllocationLease>,
}

impl<T> PartialInit<T> {
    /// Creates a fixed-slot builder and reserves the value layout up front.
    ///
    /// The slot metadata is Host-backed bookkeeping; the allocator lease tracks
    /// the logical bytes occupied by `T` values. A zero-sized `T` therefore
    /// reserves zero logical bytes while every initialized slot still runs Drop.
    ///
    /// # Errors
    ///
    /// Returns a structured error when slot metadata overflows or the logical
    /// allocator/Host cannot reserve the requested value capacity.
    pub fn try_new(
        allocator: StorageAllocator,
        slot_count: usize,
    ) -> Result<Self, PartialInitCreateError> {
        let metadata_bytes = slot_count
            .checked_mul(mem::size_of::<Option<usize>>())
            .ok_or_else(PartialInitCreateError::slot_count_overflow)?;
        if slot_count > isize::MAX as usize || metadata_bytes > isize::MAX as usize {
            return Err(PartialInitCreateError::slot_count_overflow());
        }
        let count =
            u64::try_from(slot_count).map_err(|_| PartialInitCreateError::slot_count_overflow())?;
        let lease = allocator
            .try_reserve(ValueLayout::of::<T>(), count)
            .map_err(PartialInitCreateError::allocation)?;

        let mut slots = Vec::new();
        slots.try_reserve_exact(slot_count).map_err(|_| {
            PartialInitCreateError::allocation(allocator.host_reserve_error(lease.reserved_bytes()))
        })?;
        slots.resize(slot_count, None);

        let mut values = Vec::new();
        values.try_reserve_exact(slot_count).map_err(|_| {
            PartialInitCreateError::allocation(allocator.host_reserve_error(lease.reserved_bytes()))
        })?;

        Ok(Self {
            slots,
            values,
            allocator,
            lease: Some(lease),
        })
    }

    /// Returns the fixed number of addressable slots.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Returns the number of successfully committed values.
    #[must_use]
    pub fn initialized_len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether every slot has been initialized exactly once.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.initialized_len() == self.slot_count()
    }

    /// Returns allocator provenance retained by the builder.
    #[must_use]
    pub fn allocator_id(&self) -> crate::AllocatorId {
        self.allocator.id()
    }

    /// Returns an initialized value by stable slot index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        let position = self.slots.get(index)?.as_ref().copied()?;
        self.values.get(position).map(|value| &**value)
    }

    /// Returns an initialized value mutably by stable slot index.
    #[must_use]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let position = self.slots.get(index)?.as_ref().copied()?;
        self.values.get_mut(position).map(|value| &mut **value)
    }

    /// Commits one value into an uninitialized slot.
    ///
    /// The value is returned unchanged on every rejection. Since all capacity
    /// is reserved by [`Self::try_new`], a successful commit cannot fail after
    /// ownership has been transferred.
    ///
    /// # Errors
    ///
    /// Returns [`PartialInitWriteError`] for an unknown or already initialized
    /// slot, retaining the rejected value for the caller.
    pub fn try_initialize(
        &mut self,
        index: usize,
        value: T,
    ) -> Result<(), PartialInitWriteError<T>> {
        let Some(slot) = self.slots.get_mut(index) else {
            return Err(PartialInitWriteError::new(
                PartialInitWriteErrorKind::SlotOutOfBounds,
                index,
                self.slot_count(),
                value,
            ));
        };
        if slot.is_some() {
            return Err(PartialInitWriteError::new(
                PartialInitWriteErrorKind::AlreadyInitialized,
                index,
                self.slot_count(),
                value,
            ));
        }

        let position = self.values.len();
        self.values.push(ManuallyDrop::new(value));
        *slot = Some(position);
        Ok(())
    }

    /// Publishes the complete aggregate by transferring all committed values.
    ///
    /// # Errors
    ///
    /// Returns the still-live builder when one or more slots remain missing.
    /// Dropping the error performs the same reverse successful-initialization
    /// cleanup as dropping the builder directly.
    pub fn try_finish(mut self) -> Result<InitializedAggregate<T>, PartialInitFinishError<T>> {
        if !self.is_complete() {
            let initialized = self.initialized_len();
            let slot_count = self.slot_count();
            return Err(PartialInitFinishError {
                partial: self,
                initialized,
                slot_count,
            });
        }

        Ok(InitializedAggregate {
            slots: mem::take(&mut self.slots),
            values: mem::take(&mut self.values),
            allocator: self.allocator.clone(),
            _lease: self.lease.take(),
        })
    }

    /// Explicitly cancels the builder and runs deterministic cleanup now.
    pub fn cancel(mut self) {
        self.clear_values();
    }

    fn clear_values(&mut self) {
        drop_values(&mut self.values);
    }
}

impl<T> Drop for PartialInit<T> {
    fn drop(&mut self) {
        self.clear_values();
    }
}

/// A complete aggregate produced by [`PartialInit::try_finish`].
#[derive(Debug)]
pub struct InitializedAggregate<T> {
    slots: Vec<Option<usize>>,
    values: Vec<ManuallyDrop<T>>,
    allocator: StorageAllocator,
    _lease: Option<crate::AllocationLease>,
}

impl<T> InitializedAggregate<T> {
    /// Returns the number of published slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Returns whether the aggregate has no slots.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Returns allocator provenance transferred from the builder.
    #[must_use]
    pub fn allocator_id(&self) -> crate::AllocatorId {
        self.allocator.id()
    }

    /// Returns a published value by stable slot index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        let position = self.slots.get(index)?.as_ref().copied()?;
        self.values.get(position).map(|value| &**value)
    }

    /// Iterates published values in ascending slot-index order.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.slots
            .iter()
            .filter_map(|position| position.map(|position| &*self.values[position]))
    }
}

impl<T> Drop for InitializedAggregate<T> {
    fn drop(&mut self) {
        drop_values(&mut self.values);
    }
}

fn drop_values<T>(values: &mut Vec<ManuallyDrop<T>>) {
    while let Some(value) = values.pop() {
        drop(ManuallyDrop::into_inner(value));
    }
}

/// Stable constructor failure category for [`PartialInit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialInitCreateErrorKind {
    /// Slot metadata or its Host representation cannot be bounded.
    SlotCountOverflow,
    /// The logical allocator or Host rejected the value backing storage.
    Allocation,
}

/// Structured failure while creating a [`PartialInit`].
#[derive(Debug, Error)]
#[error("partial initialization construction failed: {kind:?}")]
pub struct PartialInitCreateError {
    kind: PartialInitCreateErrorKind,
    allocation: Option<AllocationError>,
}

impl PartialInitCreateError {
    fn slot_count_overflow() -> Self {
        Self {
            kind: PartialInitCreateErrorKind::SlotCountOverflow,
            allocation: None,
        }
    }

    fn allocation(error: AllocationError) -> Self {
        Self {
            kind: PartialInitCreateErrorKind::Allocation,
            allocation: Some(error),
        }
    }

    /// Returns the stable failure category.
    #[must_use]
    pub const fn kind(&self) -> PartialInitCreateErrorKind {
        self.kind
    }

    /// Returns the underlying allocator failure, when allocation was rejected.
    #[must_use]
    pub const fn allocation_error(&self) -> Option<&AllocationError> {
        self.allocation.as_ref()
    }
}

/// Stable rejection category for one initialization write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialInitWriteErrorKind {
    /// The requested slot is outside the fixed slot range.
    SlotOutOfBounds,
    /// The requested slot already contains a committed value.
    AlreadyInitialized,
}

/// A rejected initialization that retains ownership of the input value.
#[derive(Debug)]
pub struct PartialInitWriteError<T> {
    kind: PartialInitWriteErrorKind,
    index: usize,
    slot_count: usize,
    value: T,
}

impl<T> PartialInitWriteError<T> {
    const fn new(
        kind: PartialInitWriteErrorKind,
        index: usize,
        slot_count: usize,
        value: T,
    ) -> Self {
        Self {
            kind,
            index,
            slot_count,
            value,
        }
    }

    /// Returns the stable rejection category.
    #[must_use]
    pub const fn kind(&self) -> PartialInitWriteErrorKind {
        self.kind
    }

    /// Returns the rejected slot index.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the fixed slot count used for bounds checking.
    #[must_use]
    pub const fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// Returns ownership of the value that was not committed.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T: Debug> Display for PartialInitWriteError<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "partial initialization write for slot {} of {} failed: {:?}",
            self.index, self.slot_count, self.kind
        )
    }
}

impl<T: Debug> std::error::Error for PartialInitWriteError<T> {}

/// Failure returned when a partial builder is finished before all slots exist.
#[derive(Debug)]
pub struct PartialInitFinishError<T> {
    partial: PartialInit<T>,
    initialized: usize,
    slot_count: usize,
}

impl<T> PartialInitFinishError<T> {
    /// Returns the number of committed values at the failed finish.
    #[must_use]
    pub const fn initialized_len(&self) -> usize {
        self.initialized
    }

    /// Returns the fixed slot count at the failed finish.
    #[must_use]
    pub const fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// Returns the number of missing slots.
    #[must_use]
    pub const fn missing_len(&self) -> usize {
        self.slot_count - self.initialized
    }

    /// Returns ownership of the live builder for recovery or inspection.
    #[must_use]
    pub fn into_partial(self) -> PartialInit<T> {
        self.partial
    }
}

impl<T: Debug> Display for PartialInitFinishError<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "partial initialization is incomplete: {} of {} slots initialized",
            self.initialized, self.slot_count
        )
    }
}

impl<T: Debug> std::error::Error for PartialInitFinishError<T> {}
