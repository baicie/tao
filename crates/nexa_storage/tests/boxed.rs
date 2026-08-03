//! Unique ownership and allocator-accounted box acceptance/rejection coverage.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use nexa_storage::{AllocationErrorKind, AllocatorId, StorageAllocator, StorageBox};

#[derive(Debug)]
struct DropProbe {
    trace: Rc<RefCell<Vec<u32>>>,
}

static ZST_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
struct ZstDropProbe;

impl Drop for ZstDropProbe {
    fn drop(&mut self) {
        ZST_DROP_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.trace.borrow_mut().push(1);
    }
}

#[test]
fn storage_box_moves_and_mutates_one_value_with_allocator_provenance(
) -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::new(AllocatorId::new(12), 128);
    let mut boxed = StorageBox::try_new(allocator.clone(), String::from("Futao"))?;

    assert_eq!(boxed.allocator_id(), AllocatorId::new(12));
    assert_eq!(&*boxed, "Futao");
    boxed.push_str(" language");
    let moved = boxed;

    assert_eq!(&*moved, "Futao language");
    let value = moved.into_inner();
    assert_eq!(value, "Futao language");
    assert_eq!(allocator.reserved_bytes(), 0);
    Ok(())
}

#[test]
fn storage_box_drops_its_value_once_and_releases_its_lease(
) -> Result<(), Box<dyn std::error::Error>> {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let allocator = StorageAllocator::with_capacity(128);
    let boxed = StorageBox::try_new(
        allocator.clone(),
        DropProbe {
            trace: Rc::clone(&trace),
        },
    )?;

    assert_eq!(
        allocator.reserved_bytes(),
        std::mem::size_of::<DropProbe>() as u64
    );
    drop(boxed);

    assert_eq!(&*trace.borrow(), &[1]);
    assert_eq!(allocator.reserved_bytes(), 0);
    Ok(())
}

#[test]
fn storage_box_oom_returns_the_unmoved_value_and_keeps_budget_unchanged(
) -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(0);
    let error = StorageBox::try_new(allocator.clone(), 42_u64)
        .err()
        .ok_or("expected box allocation to fail")?;

    assert_eq!(error.error().kind(), AllocationErrorKind::OutOfMemory);
    assert_eq!(error.into_value(), 42);
    assert_eq!(allocator.reserved_bytes(), 0);
    Ok(())
}

#[test]
fn storage_box_zst_drops_logically_without_reserving_bytes(
) -> Result<(), Box<dyn std::error::Error>> {
    ZST_DROP_COUNT.store(0, Ordering::SeqCst);
    let allocator = StorageAllocator::with_capacity(0);
    let boxed = StorageBox::try_new(allocator.clone(), ZstDropProbe)?;

    assert_eq!(allocator.reserved_bytes(), 0);
    drop(boxed);
    assert_eq!(ZST_DROP_COUNT.load(Ordering::SeqCst), 1);
    Ok(())
}
