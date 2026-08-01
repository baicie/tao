//! Ownership state-machine acceptance/rejection coverage.

use nexa_storage::{OwnershipEffect, OwnershipErrorKind, OwnershipFrame, PlaceKind, PlaceState};

#[test]
fn consuming_copy_preserves_the_place_while_consuming_owned_moves_it(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut frame = OwnershipFrame::new();
    let copy = frame.declare(PlaceKind::Copy)?;
    let owned = frame.declare(PlaceKind::Owned { needs_drop: true })?;
    frame.initialize(copy)?;
    frame.initialize(owned)?;

    assert_eq!(frame.consume(copy)?, OwnershipEffect::Copied);
    assert_eq!(frame.consume(owned)?, OwnershipEffect::Moved);
    assert_eq!(frame.state(copy)?, PlaceState::Initialized);
    assert_eq!(frame.state(owned)?, PlaceState::Moved);
    Ok(())
}

#[test]
fn use_after_move_and_drop_after_move_have_distinct_fail_closed_errors(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut frame = OwnershipFrame::new();
    let owned = frame.declare(PlaceKind::Owned { needs_drop: true })?;
    frame.initialize(owned)?;
    frame.consume(owned)?;
    let use_error = frame
        .read(owned)
        .err()
        .ok_or("expected read after move to fail")?;
    let drop_error = frame
        .drop_place(owned)
        .err()
        .ok_or("expected drop after move to fail")?;

    assert_eq!(use_error.kind(), OwnershipErrorKind::UseAfterMove);
    assert_eq!(drop_error.kind(), OwnershipErrorKind::DropAfterMove);
    Ok(())
}

#[test]
fn scope_cleanup_uses_reverse_successful_initialization_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut frame = OwnershipFrame::new();
    let first = frame.declare(PlaceKind::Owned { needs_drop: true })?;
    let copy = frame.declare(PlaceKind::Copy)?;
    let last = frame.declare(PlaceKind::Owned { needs_drop: true })?;
    frame.initialize(first)?;
    frame.initialize(copy)?;
    frame.initialize(last)?;

    let cleanup = frame.finish_scope();
    assert_eq!(
        cleanup
            .iter()
            .map(|action| action.place())
            .collect::<Vec<_>>(),
        [last, first]
    );
    assert_eq!(frame.state(first)?, PlaceState::Uninitialized);
    assert_eq!(frame.state(last)?, PlaceState::Uninitialized);
    Ok(())
}

#[test]
fn control_flow_join_marks_a_conditionally_moved_place_as_unavailable(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut initialized = OwnershipFrame::new();
    let value = initialized.declare(PlaceKind::Owned { needs_drop: true })?;
    initialized.initialize(value)?;
    let mut moved = initialized.clone();
    moved.consume(value)?;

    let joined = initialized.join(&moved)?;
    let error = joined
        .read(value)
        .err()
        .ok_or("expected a conditionally moved place to fail closed")?;

    assert_eq!(joined.state(value)?, PlaceState::MaybeUnavailable);
    assert_eq!(error.kind(), OwnershipErrorKind::PossiblyUnavailable);
    Ok(())
}

#[test]
fn a_moved_mutable_place_can_be_reinitialized_and_dropped_once(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut frame = OwnershipFrame::new();
    let value = frame.declare(PlaceKind::Owned { needs_drop: true })?;
    frame.initialize(value)?;
    frame.consume(value)?;
    frame.initialize(value)?;

    assert_eq!(frame.finish_scope().len(), 1);
    assert_eq!(frame.state(value)?, PlaceState::Uninitialized);
    Ok(())
}

#[test]
fn moved_places_are_excluded_from_scope_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    let mut frame = OwnershipFrame::new();
    let retained = frame.declare(PlaceKind::Owned { needs_drop: true })?;
    let moved = frame.declare(PlaceKind::Owned { needs_drop: true })?;
    frame.initialize(retained)?;
    frame.initialize(moved)?;
    frame.consume(moved)?;

    assert_eq!(
        frame
            .finish_scope()
            .iter()
            .map(|action| action.place())
            .collect::<Vec<_>>(),
        [retained]
    );
    Ok(())
}

#[test]
fn join_rejects_frames_with_different_declaration_shapes() -> Result<(), Box<dyn std::error::Error>>
{
    let mut left = OwnershipFrame::new();
    left.declare(PlaceKind::Copy)?;
    let mut right = OwnershipFrame::new();
    right.declare(PlaceKind::Owned { needs_drop: false })?;

    let error = left
        .join(&right)
        .err()
        .ok_or("expected incompatible ownership frames to be rejected")?;
    assert_eq!(error.kind(), OwnershipErrorKind::IncompatibleFrame);
    Ok(())
}
