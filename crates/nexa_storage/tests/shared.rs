//! Shared ownership and weak-handle acceptance/rejection coverage.

use std::cell::RefCell;
use std::rc::Rc;

use nexa_storage::{AllocationErrorKind, AllocatorId, Shared, SharedAccessError, StorageAllocator};

#[derive(Debug)]
struct DropProbe {
    trace: Rc<RefCell<Vec<u32>>>,
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.trace.borrow_mut().push(1);
    }
}

#[test]
fn shared_clone_shares_one_allocation_and_last_owner_drops_once(
) -> Result<(), Box<dyn std::error::Error>> {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let allocator = StorageAllocator::new(AllocatorId::new(7), 128);
    let shared = Shared::try_new(
        allocator.clone(),
        DropProbe {
            trace: Rc::clone(&trace),
        },
    )?;
    let weak = shared.downgrade();
    let clone = shared.clone();

    assert_eq!(shared.strong_count(), 2);
    assert_eq!(shared.weak_count(), 1);
    assert_eq!(shared.allocator_id(), AllocatorId::new(7));
    assert_eq!(weak.allocator_id(), AllocatorId::new(7));
    assert_eq!(
        allocator.reserved_bytes(),
        std::mem::size_of::<DropProbe>() as u64
    );
    assert!(weak.upgrade().is_some());

    drop(shared);
    assert!(trace.borrow().is_empty());
    drop(clone);

    assert_eq!(&*trace.borrow(), &[1]);
    assert_eq!(allocator.reserved_bytes(), 0);
    assert!(weak.upgrade().is_none());
    Ok(())
}

#[test]
fn shared_oom_returns_the_unmoved_value_and_keeps_budget_unchanged(
) -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(0);
    let error = Shared::try_new(allocator.clone(), 42_u64)
        .err()
        .ok_or("expected shared allocation to fail")?;

    assert_eq!(error.error().kind(), AllocationErrorKind::OutOfMemory);
    assert_eq!(error.into_value(), 42);
    assert_eq!(allocator.reserved_bytes(), 0);
    Ok(())
}

#[test]
fn shared_borrow_conflicts_are_reported_without_releasing_the_value(
) -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(128);
    let shared = Shared::try_new(allocator, String::from("Futao"))?;
    let borrow = shared.try_borrow()?;

    assert_eq!(
        shared.try_borrow_mut().err(),
        Some(SharedAccessError::BorrowConflict)
    );
    assert_eq!(&*borrow, "Futao");
    Ok(())
}

#[test]
fn weak_upgrade_rejects_a_released_value() -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(128);
    let shared = Shared::try_new(allocator.clone(), 7_u64)?;
    let weak = shared.downgrade();

    drop(shared);

    assert_eq!(weak.strong_count(), 0);
    assert_eq!(allocator.reserved_bytes(), 0);
    assert!(weak.upgrade().is_none());
    Ok(())
}
