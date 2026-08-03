# ADR-004 `StorageBox<T>` Slice

## Status

Implemented as the allocator-accounted unique-owner slice following the
`Shared<T>`/`Weak<T>` control block. This document records the reference-kernel
boundary; it does not mark the complete ADR-004 target-layout and backend
contract as finished.

## Contract

`StorageBox<T>` combines a standard-library `Box<T>` physical owner with one
logical `AllocationLease`. The lease is acquired before `T` is moved, so a
logical budget failure returns the original value through `StorageBoxError<T>`.
Moving the box transfers its only owner; there is no `Clone` or `Copy`
implementation.

The implementation guarantees:

* `Deref` and `DerefMut` expose only the initialized `T` owned by the box;
* `into_inner()` consumes the owner and returns `T`, releasing the lease;
* dropping the box drops `T` exactly once before the lease is released;
* zero-sized values retain logical Drop behavior without consuming allocator
  bytes;
* allocator identity is retained for diagnostics and cannot be forged by a
  caller.

Physical allocation uses the safe standard-library `Box`. An Application
Profile physical OOM therefore retains its configured abort behavior; the
fallible API covers the explicit logical allocator budget, consistent with the
existing storage reference model.

## Verification

```bash
cargo test --locked -p nexa_storage --test boxed
cargo test --locked -p nexa_storage --doc
```

The focused suite covers mutation through a unique owner, Move and
`into_inner`, allocator provenance, exactly-once Drop, logical OOM rejection
with original-value recovery, and ZST Drop without allocation.

## Deferred Work

Partial aggregate initialization, panic-isolation cleanup traces,
Native/Wasm layout conformance, stable external descriptors, and
Freestanding/custom allocator integration remain separate ADR-004/005 slices.
