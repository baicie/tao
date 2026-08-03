# ADR-004 `Shared<T>` and `Weak<T>` Slice

## Status

Implemented as the next independent ADR-004 runtime slice on top of the
`nexa_storage` reference kernel. This document records an implementation
boundary; it does not mark the complete ADR-004 target-layout and backend
contract as finished.

## Contract

`Shared<T>` uses a safe, single-threaded `Rc` control block. The control block
contains the live value, its allocator lease, and allocator identity. A
successful construction reserves `ValueLayout::of::<T>()` through the existing
`StorageAllocator`; a failed construction returns the original value through
`SharedError<T>`.

The implementation guarantees:

* cloning a `Shared<T>` changes only the strong-reference count and does not
  charge a second allocation;
* `Shared<T>` and `Weak<T>` retain the allocator identity without exposing
  control-block fields or physical addresses;
* the last strong owner drops `T` once and releases its logical allocation
  lease before weak handles become unusable;
* weak handles do not keep `T` alive, and `upgrade()` returns `None` after the
  last strong owner has gone away;
* immutable and mutable borrows use typed `SharedAccessError` failures rather
  than allowing a `RefCell` panic to cross the API boundary.

The reference kernel is intentionally single-threaded. `Shared<T>` and
`Weak<T>` do not implement `Send` or `Sync`; thread-safe counts and Host
resource handles remain separate design work under ADR-005 and ADR-007.

## Verification

Accepted and rejected behavior is covered by:

```bash
cargo test --locked -p nexa_storage --test shared
cargo test --locked -p nexa_storage --tests
```

The focused suite verifies shared accounting, last-owner Drop order, OOM
preservation of the input value, borrow-conflict rejection, and rejection of
weak upgrades after release. Existing ZST, overflow, arena, ownership, and
allocator provenance tests remain in the same crate suite.

## Deferred Work

This slice does not implement partial aggregate initialization,
panic-isolation cleanup traces, thread-safe sharing, Native/Wasm layout
conformance, or stable external descriptors. `StorageBox<T>` is documented in
the following [owned-box slice](adr-004-owned-box.md); the remaining items are
independent ADR-004/005/007 slices and must not be inferred from this API.
