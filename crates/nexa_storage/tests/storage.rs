//! Owned storage and arena acceptance/rejection coverage.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use nexa_storage::{
    ArenaAccessError, DropArena, PlainArena, StorageAllocator, StorageStringBuilder, StorageVec,
};

#[derive(Debug)]
struct DropProbe {
    id: u32,
    trace: Rc<RefCell<Vec<u32>>>,
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.trace.borrow_mut().push(self.id);
    }
}

static ZST_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
struct ZstDropProbe;

impl Drop for ZstDropProbe {
    fn drop(&mut self) {
        ZST_DROP_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn storage_vec_drops_remaining_elements_in_reverse_index_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let allocator = StorageAllocator::with_capacity(1024);
    let mut values = StorageVec::with_limit(allocator, 3);
    for id in 1..=3 {
        values.try_push(DropProbe {
            id,
            trace: Rc::clone(&trace),
        })?;
    }

    drop(values);
    assert_eq!(&*trace.borrow(), &[3, 2, 1]);
    Ok(())
}

#[test]
fn storage_vec_pop_transfers_ownership_before_remaining_values_drop(
) -> Result<(), Box<dyn std::error::Error>> {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let allocator = StorageAllocator::with_capacity(1024);
    let mut values = StorageVec::with_limit(allocator, 2);
    for id in 1..=2 {
        values.try_push(DropProbe {
            id,
            trace: Rc::clone(&trace),
        })?;
    }

    let popped = values.pop().ok_or("expected the last value")?;
    assert!(trace.borrow().is_empty());
    drop(popped);
    assert_eq!(&*trace.borrow(), &[2]);
    drop(values);
    assert_eq!(&*trace.borrow(), &[2, 1]);
    Ok(())
}

#[test]
fn storage_vec_capacity_failure_returns_the_unmoved_value() -> Result<(), Box<dyn std::error::Error>>
{
    let allocator = StorageAllocator::with_capacity(64);
    let mut values = StorageVec::with_limit(allocator, 1);
    values.try_push(10_u64)?;
    let error = values
        .try_push(20)
        .err()
        .ok_or("expected the element limit to reject the second value")?;

    assert_eq!(error.into_value(), 20);
    assert_eq!(values.get(0), Some(&10));
    Ok(())
}

#[test]
fn allocator_growth_failure_preserves_the_vector_and_rejected_value(
) -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(8);
    let mut values = StorageVec::with_limit(allocator.clone(), 2);
    values.try_push(10_u64)?;
    let error = values
        .try_push(20)
        .err()
        .ok_or("expected allocator budget to reject vector growth")?;

    assert_eq!(error.into_value(), 20);
    assert_eq!(values.len(), 1);
    assert_eq!(values.get(0), Some(&10));
    assert_eq!(allocator.reserved_bytes(), 8);
    Ok(())
}

#[test]
fn zero_sized_values_count_logically_and_still_run_drop() -> Result<(), Box<dyn std::error::Error>>
{
    ZST_DROP_COUNT.store(0, Ordering::SeqCst);
    let allocator = StorageAllocator::with_capacity(0);
    let mut values = StorageVec::with_limit(allocator.clone(), 3);
    values.try_push(ZstDropProbe)?;
    values.try_push(ZstDropProbe)?;
    values.try_push(ZstDropProbe)?;

    assert_eq!(values.len(), 3);
    assert_eq!(allocator.reserved_bytes(), 0);
    drop(values);
    assert_eq!(ZST_DROP_COUNT.load(Ordering::SeqCst), 3);
    Ok(())
}

#[test]
fn freeze_moves_storage_into_an_immutable_array_without_changing_budget(
) -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(128);
    let mut values = StorageVec::new(allocator.clone());
    values.try_push(String::from("first"))?;
    values.try_push(String::from("second"))?;
    let reserved = allocator.reserved_bytes();

    let values = values.freeze();
    assert_eq!(
        values.iter().map(String::as_str).collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(allocator.reserved_bytes(), reserved);

    drop(values);
    assert_eq!(allocator.reserved_bytes(), 0);
    Ok(())
}

#[test]
fn string_builder_keeps_utf8_and_clone_uses_independent_storage(
) -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(256);
    let mut builder = StorageStringBuilder::with_limit(allocator.clone(), 64);
    builder.try_push_str("Fu")?;
    builder.try_push_str("tao ")?;
    builder.try_push_str("语言")?;
    let value = builder.finish();
    let clone = value.try_clone_in(allocator.clone())?;

    assert_eq!(value.as_str(), "Futao 语言");
    assert_eq!(value.byte_len(), "Futao 语言".len());
    assert_eq!(clone.as_str(), value.as_str());
    assert!(allocator.reserved_bytes() >= value.byte_len() as u64 * 2);
    Ok(())
}

#[test]
fn drop_arena_reset_drops_values_in_reverse_registration_order_and_stales_handles(
) -> Result<(), Box<dyn std::error::Error>> {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let allocator = StorageAllocator::with_capacity(1024);
    let mut arena = DropArena::new(allocator, 3);
    let first = arena.try_store(DropProbe {
        id: 1,
        trace: Rc::clone(&trace),
    })?;
    let _second = arena.try_store(DropProbe {
        id: 2,
        trace: Rc::clone(&trace),
    })?;

    arena.reset()?;
    assert_eq!(&*trace.borrow(), &[2, 1]);
    assert!(matches!(
        arena.get(&first),
        Err(ArenaAccessError::StaleGeneration)
    ));
    Ok(())
}

#[test]
fn plain_arena_accepts_copy_values_and_rejects_stale_handles(
) -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(128);
    let mut arena = PlainArena::new(allocator, 2);
    let handle = arena.try_store(42_u64)?;

    assert_eq!(arena.get(&handle)?, &42);
    arena.reset()?;
    assert_eq!(arena.get(&handle), Err(ArenaAccessError::StaleGeneration));
    Ok(())
}

#[test]
fn arena_rejects_a_handle_from_another_instance() -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(128);
    let mut first = PlainArena::new(allocator.clone(), 1);
    let second = PlainArena::new(allocator, 1);
    let handle = first.try_store(42_u64)?;

    assert_eq!(second.get(&handle), Err(ArenaAccessError::WrongArena));
    Ok(())
}
