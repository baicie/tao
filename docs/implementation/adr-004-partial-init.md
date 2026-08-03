# ADR-004 `PartialInit<T>` Slice

## Status

Implemented as a bounded, homogeneous runtime reference model for the
partial-initialization and deterministic-cleanup portion of ADR-004. This
slice does not mark the complete ADR-004 target-layout or compiler lowering
contract as finished.

## Contract

`PartialInit<T>` creates a fixed number of addressable slots and reserves the
logical value layout before any value is committed. Each slot can transition
from missing to initialized at most once. Initialization order is recorded by
successful commits, not inferred from slot indexes.

The implementation guarantees:

* duplicate and out-of-bounds writes reject without consuming the input value;
* an incomplete builder cannot be published as an aggregate;
* dropping a builder, dropping an incomplete-finish error, and explicit
  cancellation drop only committed values;
* committed values are dropped exactly once in reverse successful-
  initialization order;
* a complete `InitializedAggregate<T>` transfers ownership without an extra
  Drop, keeps slot-index lookup stable, and retains allocator provenance;
* zero-sized `T` values reserve zero logical backing bytes while every
  initialized logical slot still runs Drop once;
* slot metadata and value capacity are bounded before the first initialization,
  so a successful write cannot unexpectedly grow the builder.

This is a safe Rust model using standard-library containers and
`StorageAllocator` accounting. Values are homogeneous and stored in commit
order with a slot-to-value index map. That representation validates cleanup
semantics without claiming to be the final physical layout of heterogeneous
Record, Tuple, Class, or union values.

## Verification

```bash
cargo test --locked -p nexa_storage --test partial_init
cargo test --locked -p nexa_storage --doc
cargo clippy --locked -p nexa_storage --all-targets -- -D warnings
```

The focused suite covers arbitrary successful initialization order, early
return, natural Drop versus explicit cancellation, rejected writes and value
recovery, incomplete finish, exactly-once completion cleanup, ZST Drop, slot
metadata overflow, and allocator exhaustion.

## Deferred Work

The following remain outside this slice:

* heterogeneous aggregate physical layout, active tags, offsets, padding, and
  raw uninitialized storage;
* compiler MIR/NIR initialization state, Drop flags, cleanup blocks, Drop glue,
  and verifier diagnostics;
* Futao `?` lowering and ADR-006 cleanup-plan integration;
* Move source-flag lowering and compiler-generated panic cleanup;
* panic-isolation guarantees for a user Drop implementation;
* Native/Wasm/reference Drop-trace conformance;
* stable ABI descriptors, Host ABI, freestanding allocators, and custom raw
  allocation.
