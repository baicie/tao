//! Allocation and layout acceptance/rejection coverage.

use nexa_storage::{AllocationErrorKind, AllocatorId, LayoutError, StorageAllocator, ValueLayout};

#[test]
fn value_layout_rejects_non_power_of_two_alignment() {
    assert_eq!(
        ValueLayout::new(8, 3, false),
        Err(LayoutError::InvalidAlignment { align: 3 })
    );
}

#[test]
fn value_layout_checks_array_stride_overflow() -> Result<(), Box<dyn std::error::Error>> {
    let layout = ValueLayout::new(8, 8, false)?;

    assert_eq!(
        layout.checked_array_bytes(u64::MAX),
        Err(LayoutError::SizeOverflow)
    );
    Ok(())
}

#[test]
fn zero_sized_layout_keeps_logical_count_without_allocating_bytes(
) -> Result<(), Box<dyn std::error::Error>> {
    let layout = ValueLayout::new(0, 8, true)?;

    assert!(layout.is_zero_sized());
    assert!(layout.needs_drop());
    assert_eq!(layout.checked_array_bytes(u64::MAX)?, 0);
    Ok(())
}

#[test]
fn allocation_resize_preserves_provenance_and_releases_budget(
) -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::new(AllocatorId::new(7), 64);
    let layout = ValueLayout::new(8, 8, false)?;
    let mut allocation = allocator.try_reserve(layout, 2)?;

    allocator.try_resize(&mut allocation, layout, 4)?;
    assert_eq!(allocation.allocator_id(), AllocatorId::new(7));
    assert_eq!(allocation.reserved_bytes(), 32);
    assert_eq!(allocator.reserved_bytes(), 32);

    drop(allocation);
    assert_eq!(allocator.reserved_bytes(), 0);
    Ok(())
}

#[test]
fn allocation_shrink_releases_only_the_removed_budget() -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::with_capacity(64);
    let layout = ValueLayout::new(8, 8, false)?;
    let mut allocation = allocator.try_reserve(layout, 4)?;

    allocator.try_resize(&mut allocation, layout, 1)?;
    assert_eq!(allocation.reserved_bytes(), 8);
    assert_eq!(allocator.reserved_bytes(), 8);
    Ok(())
}

#[test]
fn failed_growth_leaves_the_original_allocation_valid() -> Result<(), Box<dyn std::error::Error>> {
    let allocator = StorageAllocator::new(AllocatorId::new(1), 32);
    let layout = ValueLayout::new(8, 8, false)?;
    let mut allocation = allocator.try_reserve(layout, 2)?;
    let error = allocator
        .try_resize(&mut allocation, layout, 8)
        .err()
        .ok_or("expected growth to exceed the allocator budget")?;

    assert_eq!(error.kind(), AllocationErrorKind::OutOfMemory);
    assert_eq!(allocation.reserved_bytes(), 16);
    assert_eq!(allocator.reserved_bytes(), 16);
    Ok(())
}

#[test]
fn a_different_allocator_cannot_resize_an_allocation() -> Result<(), Box<dyn std::error::Error>> {
    let owner = StorageAllocator::new(AllocatorId::new(9), 64);
    let other = StorageAllocator::new(AllocatorId::new(9), 64);
    let layout = ValueLayout::new(8, 8, false)?;
    let mut allocation = owner.try_reserve(layout, 1)?;
    let error = other
        .try_resize(&mut allocation, layout, 2)
        .err()
        .ok_or("expected allocator provenance mismatch")?;

    assert_eq!(error.kind(), AllocationErrorKind::ProvenanceMismatch);
    assert_eq!(owner.reserved_bytes(), 8);
    assert_eq!(other.reserved_bytes(), 0);
    Ok(())
}
