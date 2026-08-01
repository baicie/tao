use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

use crate::{PushError, StorageAllocator, StorageVec};

#[derive(Debug)]
struct ArenaIdentity;

/// A generation-checked reference to one value owned by an arena.
#[derive(Debug, Clone)]
pub struct ArenaHandle<T> {
    identity: Rc<ArenaIdentity>,
    generation: u64,
    index: usize,
    marker: PhantomData<fn() -> T>,
}

/// An arena for values that require deterministic Drop processing.
#[derive(Debug)]
pub struct DropArena<T> {
    identity: Rc<ArenaIdentity>,
    generation: u64,
    values: StorageVec<T>,
}

impl<T> DropArena<T> {
    /// Creates an empty arena with a bounded number of registrations.
    #[must_use]
    pub fn new(allocator: StorageAllocator, max_values: usize) -> Self {
        Self {
            identity: Rc::new(ArenaIdentity),
            generation: 0,
            values: StorageVec::with_limit(allocator, max_values),
        }
    }

    /// Stores a value and returns a handle tied to this arena generation.
    ///
    /// # Errors
    ///
    /// Returns [`PushError`] with ownership of `value` if the arena bound or
    /// allocator budget rejects the registration.
    pub fn try_store(&mut self, value: T) -> Result<ArenaHandle<T>, PushError<T>> {
        let index = self.values.len();
        self.values.try_push(value)?;
        Ok(ArenaHandle {
            identity: Rc::clone(&self.identity),
            generation: self.generation,
            index,
            marker: PhantomData,
        })
    }

    /// Borrows a stored value after validating arena identity and generation.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaAccessError`] for a foreign, stale, or invalid handle.
    pub fn get(&self, handle: &ArenaHandle<T>) -> Result<&T, ArenaAccessError> {
        self.validate(handle)?;
        self.values
            .get(handle.index)
            .ok_or(ArenaAccessError::InvalidIndex)
    }

    /// Mutably borrows a stored value after validating its handle.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaAccessError`] for a foreign, stale, or invalid handle.
    pub fn get_mut(&mut self, handle: &ArenaHandle<T>) -> Result<&mut T, ArenaAccessError> {
        self.validate(handle)?;
        self.values
            .get_mut(handle.index)
            .ok_or(ArenaAccessError::InvalidIndex)
    }

    /// Drops registrations in reverse order and invalidates all prior handles.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaResetError::GenerationExhausted`] without modifying the
    /// arena if no fresh generation can be represented.
    pub fn reset(&mut self) -> Result<(), ArenaResetError> {
        let next = self
            .generation
            .checked_add(1)
            .ok_or(ArenaResetError::GenerationExhausted)?;
        self.values.clear();
        self.generation = next;
        Ok(())
    }

    fn validate(&self, handle: &ArenaHandle<T>) -> Result<(), ArenaAccessError> {
        if !Rc::ptr_eq(&self.identity, &handle.identity) {
            return Err(ArenaAccessError::WrongArena);
        }
        if self.generation != handle.generation {
            return Err(ArenaAccessError::StaleGeneration);
        }
        Ok(())
    }
}

/// An arena restricted to [`Copy`] values, which cannot hide Drop glue.
#[derive(Debug)]
pub struct PlainArena<T: Copy> {
    inner: DropArena<T>,
}

impl<T: Copy> PlainArena<T> {
    /// Creates an empty plain-data arena with a bounded value count.
    #[must_use]
    pub fn new(allocator: StorageAllocator, max_values: usize) -> Self {
        Self {
            inner: DropArena::new(allocator, max_values),
        }
    }

    /// Stores a Copy value in the current arena generation.
    ///
    /// # Errors
    ///
    /// Returns [`PushError`] if the arena bound or allocator budget is exhausted.
    pub fn try_store(&mut self, value: T) -> Result<ArenaHandle<T>, PushError<T>> {
        self.inner.try_store(value)
    }

    /// Borrows a stored Copy value after validating its handle.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaAccessError`] for a foreign, stale, or invalid handle.
    pub fn get(&self, handle: &ArenaHandle<T>) -> Result<&T, ArenaAccessError> {
        self.inner.get(handle)
    }

    /// Mutably borrows a stored Copy value after validating its handle.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaAccessError`] for a foreign, stale, or invalid handle.
    pub fn get_mut(&mut self, handle: &ArenaHandle<T>) -> Result<&mut T, ArenaAccessError> {
        self.inner.get_mut(handle)
    }

    /// Clears the arena and invalidates all handles from its previous generation.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaResetError::GenerationExhausted`] before changing state if
    /// the generation counter cannot advance.
    pub fn reset(&mut self) -> Result<(), ArenaResetError> {
        self.inner.reset()
    }
}

/// Reason an arena handle cannot be dereferenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ArenaAccessError {
    /// The handle was created by a different arena instance.
    #[error("arena handle belongs to a different arena")]
    WrongArena,
    /// The arena has been reset since the handle was created.
    #[error("arena handle belongs to a stale generation")]
    StaleGeneration,
    /// The handle index is not present in its claimed generation.
    #[error("arena handle index is not initialized")]
    InvalidIndex,
}

/// Failure while advancing an arena to a fresh generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ArenaResetError {
    /// The generation counter reached its representable maximum.
    #[error("arena generation counter is exhausted")]
    GenerationExhausted,
}
