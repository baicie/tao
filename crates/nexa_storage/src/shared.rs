use std::cell::{Ref, RefCell, RefMut};
use std::fmt::{Debug, Display, Formatter};
use std::rc::{Rc, Weak as RcWeak};

use thiserror::Error;

use crate::{AllocationError, AllocationLease, AllocatorId, StorageAllocator, ValueLayout};

#[derive(Debug)]
struct SharedState<T> {
    value: RefCell<Option<T>>,
    lease: RefCell<Option<AllocationLease>>,
    allocator_id: AllocatorId,
}

/// A shared, allocator-accounted owner with deterministic final Drop.
///
/// Cloning a [`Shared`] value increments only the logical strong-reference
/// count. The allocator lease is released when the last strong owner drops,
/// even if weak handles remain alive.
///
/// The reference kernel is intentionally single-threaded:
///
/// ```compile_fail
/// use nexa_storage::Shared;
///
/// fn requires_send<T: Send>() {}
/// requires_send::<Shared<u64>>();
/// ```
#[derive(Debug)]
pub struct Shared<T> {
    inner: Rc<SharedState<T>>,
}

impl<T> Shared<T> {
    /// Allocates a shared value while preserving the input on failure.
    ///
    /// # Errors
    ///
    /// Returns [`SharedError`] when the allocator cannot reserve the logical
    /// layout for `T`.
    pub fn try_new(allocator: StorageAllocator, value: T) -> Result<Self, SharedError<T>> {
        let lease = match allocator.try_reserve(ValueLayout::of::<T>(), 1) {
            Ok(lease) => lease,
            Err(error) => return Err(SharedError { error, value }),
        };
        Ok(Self {
            inner: Rc::new(SharedState {
                value: RefCell::new(Some(value)),
                lease: RefCell::new(Some(lease)),
                allocator_id: allocator.id(),
            }),
        })
    }

    /// Returns the allocator provenance retained by the shared value.
    #[must_use]
    pub fn allocator_id(&self) -> AllocatorId {
        self.inner.allocator_id
    }

    /// Returns the number of live strong owners.
    #[must_use]
    pub fn strong_count(&self) -> usize {
        Rc::strong_count(&self.inner)
    }

    /// Returns the number of live weak handles.
    #[must_use]
    pub fn weak_count(&self) -> usize {
        Rc::weak_count(&self.inner)
    }

    /// Returns whether this owner is the only live strong owner.
    #[must_use]
    pub fn is_unique(&self) -> bool {
        self.strong_count() == 1
    }

    /// Creates a weak handle that does not keep the value alive.
    #[must_use]
    pub fn downgrade(&self) -> Weak<T> {
        Weak {
            inner: Rc::downgrade(&self.inner),
            allocator_id: self.allocator_id(),
        }
    }

    /// Borrows the live value without exposing the control block.
    ///
    /// # Errors
    ///
    /// Returns [`SharedAccessError::BorrowConflict`] when another borrow is
    /// active, or [`SharedAccessError::Released`] if the value is no longer
    /// present in the control block.
    pub fn try_borrow(&self) -> Result<Ref<'_, T>, SharedAccessError> {
        let value = self
            .inner
            .value
            .try_borrow()
            .map_err(|_| SharedAccessError::BorrowConflict)?;
        Ref::filter_map(value, Option::as_ref).map_err(|_| SharedAccessError::Released)
    }

    /// Mutably borrows the live value without exposing the control block.
    ///
    /// # Errors
    ///
    /// Returns [`SharedAccessError::BorrowConflict`] when another borrow is
    /// active, or [`SharedAccessError::Released`] if the value is no longer
    /// present in the control block.
    pub fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, SharedAccessError> {
        let value = self
            .inner
            .value
            .try_borrow_mut()
            .map_err(|_| SharedAccessError::BorrowConflict)?;
        RefMut::filter_map(value, Option::as_mut).map_err(|_| SharedAccessError::Released)
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.inner) != 1 {
            return;
        }

        // The last owner is the only place that can release the value and its
        // logical allocation. `Option::take` also keeps Drop idempotent if a
        // future control-block cleanup path has already cleared one field.
        if let Some(value) = self.inner.value.borrow_mut().take() {
            drop(value);
        }
        let _ = self.inner.lease.borrow_mut().take();
    }
}

/// A non-owning handle that can upgrade while a [`Shared`] value is alive.
#[derive(Debug)]
pub struct Weak<T> {
    inner: RcWeak<SharedState<T>>,
    allocator_id: AllocatorId,
}

impl<T> Weak<T> {
    /// Returns the allocator provenance recorded when this handle was made.
    #[must_use]
    pub const fn allocator_id(&self) -> AllocatorId {
        self.allocator_id
    }

    /// Returns the number of live strong owners.
    #[must_use]
    pub fn strong_count(&self) -> usize {
        self.inner.strong_count()
    }

    /// Returns the number of live weak handles.
    #[must_use]
    pub fn weak_count(&self) -> usize {
        self.inner.weak_count()
    }

    /// Attempts to obtain a new strong owner without reviving a released value.
    #[must_use]
    pub fn upgrade(&self) -> Option<Shared<T>> {
        let inner = self.inner.upgrade()?;
        let is_live = inner.value.try_borrow().ok()?.is_some();
        is_live.then_some(Shared { inner })
    }
}

impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            allocator_id: self.allocator_id,
        }
    }
}

/// Failure while creating a shared value.
#[derive(Debug)]
pub struct SharedError<T> {
    error: AllocationError,
    value: T,
}

impl<T> SharedError<T> {
    /// Returns the structured allocation failure.
    #[must_use]
    pub const fn error(&self) -> &AllocationError {
        &self.error
    }

    /// Returns ownership of the value that was never shared.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T> Display for SharedError<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.error, formatter)
    }
}

impl<T: Debug> std::error::Error for SharedError<T> {}

/// Failure while borrowing a shared value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SharedAccessError {
    /// Another immutable or mutable borrow is still active.
    #[error("shared value is already borrowed")]
    BorrowConflict,
    /// The control block no longer contains its value.
    #[error("shared value has been released")]
    Released,
}
