//! Runtime partial-initialization acceptance and rejection coverage.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use nexa_storage::{
    AllocationErrorKind, PartialInit, PartialInitCreateErrorKind, PartialInitWriteErrorKind,
    StorageAllocator,
};

#[derive(Debug)]
struct DropTrace {
    label: &'static str,
    trace: Rc<RefCell<Vec<&'static str>>>,
}

impl DropTrace {
    fn new(label: &'static str, trace: &Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self {
            label,
            trace: Rc::clone(trace),
        }
    }
}

impl Drop for DropTrace {
    fn drop(&mut self) {
        self.trace.borrow_mut().push(self.label);
    }
}

#[test]
fn complete_aggregate_preserves_slot_access_and_reverse_initialization_drop(
) -> Result<(), Box<dyn std::error::Error>> {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let allocator = StorageAllocator::with_capacity(1024);
    let mut partial = PartialInit::try_new(allocator.clone(), 3)?;
    partial.try_initialize(2, DropTrace::new("third", &trace))?;
    partial.try_initialize(0, DropTrace::new("first", &trace))?;
    partial.try_initialize(1, DropTrace::new("second", &trace))?;

    assert_eq!(partial.initialized_len(), 3);
    assert!(partial.is_complete());
    let complete = partial.try_finish()?;
    assert_eq!(complete.len(), 3);
    assert_eq!(complete.get(0).map(|value| value.label), Some("first"));
    assert_eq!(complete.get(1).map(|value| value.label), Some("second"));
    assert_eq!(complete.get(2).map(|value| value.label), Some("third"));
    assert_eq!(
        complete.iter().map(|value| value.label).collect::<Vec<_>>(),
        ["first", "second", "third"]
    );
    assert!(trace.borrow().is_empty());

    drop(complete);
    assert_eq!(&*trace.borrow(), &["second", "first", "third"]);
    assert_eq!(allocator.reserved_bytes(), 0);
    Ok(())
}

#[test]
fn early_return_drops_only_successful_initializations_in_reverse_order(
) -> Result<(), Box<dyn std::error::Error>> {
    fn build(trace: &Rc<RefCell<Vec<&'static str>>>) -> Result<(), &'static str> {
        let allocator = StorageAllocator::with_capacity(1024);
        let mut partial = PartialInit::try_new(allocator, 4).map_err(|_| "allocation")?;
        partial
            .try_initialize(1, DropTrace::new("first", trace))
            .map_err(|_| "write")?;
        partial
            .try_initialize(3, DropTrace::new("second", trace))
            .map_err(|_| "write")?;
        Err("injected failure")
    }

    let trace = Rc::new(RefCell::new(Vec::new()));
    assert_eq!(build(&trace), Err("injected failure"));
    assert_eq!(&*trace.borrow(), &["second", "first"]);
    Ok(())
}

#[test]
fn natural_drop_and_explicit_cancel_have_the_same_cleanup_trace(
) -> Result<(), Box<dyn std::error::Error>> {
    fn partial(
        trace: &Rc<RefCell<Vec<&'static str>>>,
    ) -> Result<PartialInit<DropTrace>, Box<dyn std::error::Error>> {
        let mut partial = PartialInit::try_new(StorageAllocator::with_capacity(1024), 3)?;
        partial.try_initialize(2, DropTrace::new("first", trace))?;
        partial.try_initialize(0, DropTrace::new("second", trace))?;
        Ok(partial)
    }

    let natural_trace = Rc::new(RefCell::new(Vec::new()));
    drop(partial(&natural_trace)?);

    let cancel_trace = Rc::new(RefCell::new(Vec::new()));
    partial(&cancel_trace)?.cancel();

    assert_eq!(&*natural_trace.borrow(), &["second", "first"]);
    assert_eq!(&*cancel_trace.borrow(), &["second", "first"]);
    Ok(())
}

#[test]
fn rejected_writes_return_the_uncommitted_value_without_disturbing_state(
) -> Result<(), Box<dyn std::error::Error>> {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut partial = PartialInit::try_new(StorageAllocator::with_capacity(1024), 2)?;
    partial.try_initialize(0, DropTrace::new("committed", &trace))?;

    let duplicate = partial
        .try_initialize(0, DropTrace::new("duplicate", &trace))
        .err()
        .ok_or("a live slot must reject replacement")?;
    assert_eq!(
        duplicate.kind(),
        PartialInitWriteErrorKind::AlreadyInitialized
    );
    assert_eq!(duplicate.index(), 0);
    assert_eq!(duplicate.slot_count(), 2);
    assert_eq!(duplicate.into_value().label, "duplicate");

    let out_of_bounds = partial
        .try_initialize(2, DropTrace::new("outside", &trace))
        .err()
        .ok_or("an unknown slot must be rejected")?;
    assert_eq!(
        out_of_bounds.kind(),
        PartialInitWriteErrorKind::SlotOutOfBounds
    );
    assert_eq!(out_of_bounds.index(), 2);
    assert_eq!(out_of_bounds.slot_count(), 2);
    assert_eq!(out_of_bounds.into_value().label, "outside");
    assert_eq!(partial.initialized_len(), 1);

    drop(partial);
    assert_eq!(&*trace.borrow(), &["duplicate", "outside", "committed"]);
    Ok(())
}

#[test]
fn incomplete_finish_returns_the_live_partial_state_for_cleanup(
) -> Result<(), Box<dyn std::error::Error>> {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut partial = PartialInit::try_new(StorageAllocator::with_capacity(1024), 3)?;
    partial.try_initialize(1, DropTrace::new("only", &trace))?;

    let error = partial
        .try_finish()
        .err()
        .ok_or("an aggregate with missing slots must not be published")?;
    assert_eq!(error.initialized_len(), 1);
    assert_eq!(error.slot_count(), 3);
    assert_eq!(error.missing_len(), 2);
    assert!(trace.borrow().is_empty());

    drop(error);
    assert_eq!(&*trace.borrow(), &["only"]);
    Ok(())
}

#[test]
fn zero_sized_values_use_no_logical_bytes_but_each_value_is_dropped(
) -> Result<(), Box<dyn std::error::Error>> {
    static DROPS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug)]
    struct DropZst(PhantomData<&'static ()>);

    impl Drop for DropZst {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    DROPS.store(0, Ordering::SeqCst);
    let allocator = StorageAllocator::with_capacity(0);
    let mut partial = PartialInit::try_new(allocator.clone(), 4)?;
    for index in [3, 0, 2, 1] {
        partial.try_initialize(index, DropZst(PhantomData))?;
    }

    assert_eq!(allocator.reserved_bytes(), 0);
    drop(partial.try_finish()?);
    assert_eq!(DROPS.load(Ordering::SeqCst), 4);
    assert_eq!(allocator.reserved_bytes(), 0);
    Ok(())
}

#[test]
fn constructor_rejects_unrepresentable_slot_metadata_and_allocator_exhaustion(
) -> Result<(), Box<dyn std::error::Error>> {
    let overflow =
        match PartialInit::<u8>::try_new(StorageAllocator::with_capacity(u64::MAX), usize::MAX) {
            Ok(_) => return Err("slot metadata size must be checked before Host allocation".into()),
            Err(error) => error,
        };
    assert_eq!(
        overflow.kind(),
        PartialInitCreateErrorKind::SlotCountOverflow
    );

    let allocation = match PartialInit::<u64>::try_new(StorageAllocator::with_capacity(7), 1) {
        Ok(_) => return Err("logical value storage must honor the allocator budget".into()),
        Err(error) => error,
    };
    assert_eq!(allocation.kind(), PartialInitCreateErrorKind::Allocation);
    assert_eq!(
        allocation.allocation_error().map(|error| error.kind()),
        Some(AllocationErrorKind::OutOfMemory)
    );
    Ok(())
}
