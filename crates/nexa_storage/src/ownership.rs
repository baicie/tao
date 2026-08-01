use thiserror::Error;

/// Stable identifier for one ownership-analysis place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaceId(usize);

impl PlaceId {
    /// Returns the zero-based declaration index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Ownership behavior of values stored in a place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceKind {
    /// Consumption copies the value and leaves the source initialized.
    Copy,
    /// Consumption moves unique ownership from the source.
    Owned {
        /// Whether an initialized value needs deterministic cleanup.
        needs_drop: bool,
    },
}

impl PlaceKind {
    const fn needs_drop(self) -> bool {
        matches!(self, Self::Owned { needs_drop: true })
    }
}

/// Definite-initialization state of one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceState {
    /// No value has been initialized, or the prior value was dropped.
    Uninitialized,
    /// A value is initialized and available.
    Initialized,
    /// The value's ownership was moved elsewhere.
    Moved,
    /// Control-flow predecessors disagree about value availability.
    MaybeUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlaceRecord {
    kind: PlaceKind,
    state: PlaceState,
}

/// Result of consuming an initialized value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipEffect {
    /// A Copy value was duplicated without changing source state.
    Copied,
    /// An Owned value moved and made the source unavailable.
    Moved,
}

/// One deterministic scope-exit cleanup operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupAction {
    place: PlaceId,
    conditional: bool,
}

impl CleanupAction {
    /// Returns the place whose initialized value must be dropped.
    #[must_use]
    pub const fn place(self) -> PlaceId {
        self.place
    }

    /// Returns whether runtime availability must guard this cleanup.
    #[must_use]
    pub const fn is_conditional(self) -> bool {
        self.conditional
    }
}

/// Pure definite-initialization and Move state for one lexical scope.
#[derive(Debug, Clone, Default)]
pub struct OwnershipFrame {
    places: Vec<PlaceRecord>,
    initialization_order: Vec<PlaceId>,
}

impl OwnershipFrame {
    /// Creates an empty ownership frame.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            places: Vec::new(),
            initialization_order: Vec::new(),
        }
    }

    /// Declares an uninitialized place.
    ///
    /// # Errors
    ///
    /// Returns [`OwnershipErrorKind::PlaceLimitExceeded`] if another stable
    /// place index cannot be represented.
    pub fn declare(&mut self, kind: PlaceKind) -> Result<PlaceId, OwnershipError> {
        self.places
            .len()
            .checked_add(1)
            .ok_or_else(|| OwnershipError::new(OwnershipErrorKind::PlaceLimitExceeded, None))?;
        let place = PlaceId(self.places.len());
        self.places.push(PlaceRecord {
            kind,
            state: PlaceState::Uninitialized,
        });
        Ok(place)
    }

    /// Marks a previously unavailable place initialized.
    ///
    /// # Errors
    ///
    /// Rejects unknown, already initialized, or conditionally available places.
    pub fn initialize(&mut self, place: PlaceId) -> Result<(), OwnershipError> {
        let record = self.record_mut(place)?;
        match record.state {
            PlaceState::Uninitialized | PlaceState::Moved => {
                record.state = PlaceState::Initialized;
                self.initialization_order.push(place);
                Ok(())
            }
            PlaceState::Initialized => Err(OwnershipError::new(
                OwnershipErrorKind::AlreadyInitialized,
                Some(place),
            )),
            PlaceState::MaybeUnavailable => Err(OwnershipError::new(
                OwnershipErrorKind::PossiblyUnavailable,
                Some(place),
            )),
        }
    }

    /// Validates a non-consuming read.
    ///
    /// # Errors
    ///
    /// Returns a stable state-specific error when the place is unavailable.
    pub fn read(&self, place: PlaceId) -> Result<(), OwnershipError> {
        match self.record(place)?.state {
            PlaceState::Initialized => Ok(()),
            PlaceState::Uninitialized => Err(OwnershipError::new(
                OwnershipErrorKind::UseBeforeInitialization,
                Some(place),
            )),
            PlaceState::Moved => Err(OwnershipError::new(
                OwnershipErrorKind::UseAfterMove,
                Some(place),
            )),
            PlaceState::MaybeUnavailable => Err(OwnershipError::new(
                OwnershipErrorKind::PossiblyUnavailable,
                Some(place),
            )),
        }
    }

    /// Consumes a value according to its declared Copy or Owned behavior.
    ///
    /// # Errors
    ///
    /// Returns a stable state-specific error when the place is unavailable.
    pub fn consume(&mut self, place: PlaceId) -> Result<OwnershipEffect, OwnershipError> {
        self.read(place)?;
        let record = self.record_mut(place)?;
        match record.kind {
            PlaceKind::Copy => Ok(OwnershipEffect::Copied),
            PlaceKind::Owned { .. } => {
                record.state = PlaceState::Moved;
                Ok(OwnershipEffect::Moved)
            }
        }
    }

    /// Explicitly drops an initialized place and marks it uninitialized.
    ///
    /// # Errors
    ///
    /// Rejects unknown, moved, uninitialized, or conditionally available places.
    pub fn drop_place(&mut self, place: PlaceId) -> Result<Option<CleanupAction>, OwnershipError> {
        let record = self.record_mut(place)?;
        match record.state {
            PlaceState::Initialized => {
                record.state = PlaceState::Uninitialized;
                Ok(record.kind.needs_drop().then_some(CleanupAction {
                    place,
                    conditional: false,
                }))
            }
            PlaceState::Moved => Err(OwnershipError::new(
                OwnershipErrorKind::DropAfterMove,
                Some(place),
            )),
            PlaceState::Uninitialized => Err(OwnershipError::new(
                OwnershipErrorKind::DropBeforeInitialization,
                Some(place),
            )),
            PlaceState::MaybeUnavailable => Err(OwnershipError::new(
                OwnershipErrorKind::PossiblyUnavailable,
                Some(place),
            )),
        }
    }

    /// Returns the current state of a declared place.
    ///
    /// # Errors
    ///
    /// Returns [`OwnershipErrorKind::UnknownPlace`] for an out-of-range identifier.
    pub fn state(&self, place: PlaceId) -> Result<PlaceState, OwnershipError> {
        self.record(place).map(|record| record.state)
    }

    /// Joins two control-flow predecessor states conservatively.
    ///
    /// # Errors
    ///
    /// Returns [`OwnershipErrorKind::IncompatibleFrame`] if place declarations
    /// or initialization histories do not have the same structural shape.
    pub fn join(&self, other: &Self) -> Result<Self, OwnershipError> {
        if self.places.len() != other.places.len()
            || self.initialization_order != other.initialization_order
            || self
                .places
                .iter()
                .zip(&other.places)
                .any(|(left, right)| left.kind != right.kind)
        {
            return Err(OwnershipError::new(
                OwnershipErrorKind::IncompatibleFrame,
                None,
            ));
        }

        let places = self
            .places
            .iter()
            .zip(&other.places)
            .map(|(left, right)| PlaceRecord {
                kind: left.kind,
                state: join_state(left.state, right.state),
            })
            .collect();
        Ok(Self {
            places,
            initialization_order: self.initialization_order.clone(),
        })
    }

    /// Produces reverse-initialization cleanup and closes every live place.
    pub fn finish_scope(&mut self) -> Vec<CleanupAction> {
        let mut cleanup = Vec::new();
        for place in self.initialization_order.iter().rev().copied() {
            let Some(record) = self.places.get_mut(place.0) else {
                continue;
            };
            let conditional = record.state == PlaceState::MaybeUnavailable;
            if matches!(
                record.state,
                PlaceState::Initialized | PlaceState::MaybeUnavailable
            ) {
                if record.kind.needs_drop() {
                    cleanup.push(CleanupAction { place, conditional });
                }
                record.state = PlaceState::Uninitialized;
            }
        }
        cleanup
    }

    fn record(&self, place: PlaceId) -> Result<&PlaceRecord, OwnershipError> {
        self.places
            .get(place.0)
            .ok_or_else(|| OwnershipError::new(OwnershipErrorKind::UnknownPlace, Some(place)))
    }

    fn record_mut(&mut self, place: PlaceId) -> Result<&mut PlaceRecord, OwnershipError> {
        self.places
            .get_mut(place.0)
            .ok_or_else(|| OwnershipError::new(OwnershipErrorKind::UnknownPlace, Some(place)))
    }
}

fn join_state(left: PlaceState, right: PlaceState) -> PlaceState {
    if left == right {
        left
    } else if matches!(left, PlaceState::Uninitialized | PlaceState::Moved)
        && matches!(right, PlaceState::Uninitialized | PlaceState::Moved)
    {
        PlaceState::Uninitialized
    } else {
        PlaceState::MaybeUnavailable
    }
}

/// Stable failure category for ownership-frame operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipErrorKind {
    /// A place identifier is outside this frame's declaration range.
    UnknownPlace,
    /// Another stable place identifier cannot be represented.
    PlaceLimitExceeded,
    /// Initialization would overwrite a live value without Drop.
    AlreadyInitialized,
    /// A value was read or consumed before initialization.
    UseBeforeInitialization,
    /// A value was read or consumed after Move.
    UseAfterMove,
    /// Explicit Drop targeted a value that was already moved.
    DropAfterMove,
    /// Explicit Drop targeted a place without an initialized value.
    DropBeforeInitialization,
    /// Control flow cannot prove that the value remains available.
    PossiblyUnavailable,
    /// Control-flow frames have incompatible declaration or initialization shapes.
    IncompatibleFrame,
}

/// Structured ownership-frame failure with an optional affected place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("ownership operation failed for {place:?}: {kind:?}")]
pub struct OwnershipError {
    kind: OwnershipErrorKind,
    place: Option<PlaceId>,
}

impl OwnershipError {
    const fn new(kind: OwnershipErrorKind, place: Option<PlaceId>) -> Self {
        Self { kind, place }
    }

    /// Returns the stable failure category.
    #[must_use]
    pub const fn kind(&self) -> OwnershipErrorKind {
        self.kind
    }

    /// Returns the affected place, when the error is place-specific.
    #[must_use]
    pub const fn place(&self) -> Option<PlaceId> {
        self.place
    }
}
