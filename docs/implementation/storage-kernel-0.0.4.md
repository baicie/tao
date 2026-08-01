# Futao 0.0.4 Ownership and Storage Kernel

## Status and Boundary

Toolchain `0.0.4` delivers the minimal ADR-002/ADR-004 reference contract needed
by the future Futao compiler core. The implementation lives in `nexa_storage`,
uses Rust 1.80 safe code, and has no dependency on parser, HIR, MIR, NIR, LLVM,
Wasm, or Host ABI crates.

This milestone does not mark ADR-004 fully implemented. It freezes observable
bootstrap behavior while leaving target layout verification and backend/runtime
integration to their scheduled phases.

| Kernel area | `0.0.4` contract |
|---|---|
| Layout | checked Host-reference size, alignment, stride, and array-byte arithmetic |
| Allocation | logical byte budgets and allocator-instance provenance through unforgeable leases |
| Owned storage | bounded `StorageVec`, zero-copy `freeze`, immutable `StorageArray`, UTF-8 builder/string |
| Arena | reverse cleanup, `Copy`-only plain arena, instance and generation checked handles |
| Collections | bounded key-ordered map/set, sparse bit set, first-intern-order symbol IDs |
| Ownership | Place initialization, Copy/Move, reinitialization, conservative joins, cleanup actions |

## Safe Host Reference Model

`StorageAllocator` does not replace Rust's global allocator and does not expose
raw pointers. Standard-library containers own physical allocations; a private
`Rc` identity makes logical allocator provenance unforgeable outside the crate.
Every live lease charges checked `u64` bytes, resize commits only after both the
new budget and Host reserve succeed, and dropping the backing storage precedes
releasing its logical lease.

This separation is deliberate:

- `StorageVec` and `StorageStringBuilder` are explicit fallible storage paths.
  Capacity, arithmetic, logical budget, and Host reserve failures leave the old
  value valid; failed element insertion returns the unconsumed value.
- `DeterministicMap`, `DeterministicSet`, and `InternTable` use safe
  `BTreeMap`/`BTreeSet` storage. Their explicit bounds are recoverable; physical
  allocation follows the ADR-004 Application Profile Host OOM/abort policy.
- Freestanding allocation, custom allocators, target pointers, and physical
  object layout are not represented by this Host reference model.

Zero-sized values consume no logical bytes but retain normal length, iteration,
ownership, and per-element Drop behavior. Empty values require no allocation.

## Ownership and Cleanup

`OwnershipFrame` is a pure state machine, not a parser or type checker. A Place
is declared as Copy or Owned and transitions among `Uninitialized`,
`Initialized`, `Moved`, and `MaybeUnavailable`.

- consuming Copy leaves the source initialized;
- consuming Owned marks the source moved without running Drop;
- a moved Place may be initialized again;
- reads and Drop fail closed for unavailable states;
- scope cleanup walks successful initialization events in reverse and excludes
  values that were moved or explicitly dropped;
- a join with differing availability becomes `MaybeUnavailable`, producing a
  conditional cleanup action when Drop may still be required;
- frames with different declaration shapes or initialization histories do not
  join, so this bootstrap model cannot invent a cleanup order. Frames are
  structurally identified in `0.0.4`; lexical scope IDs arrive with later IR.

Later MIR/NIR lowering will attach source spans, stable diagnostics, runtime Drop
flags, and path-specific cleanup blocks. Those are not hidden in this crate.

## Storage and Arena Invariants

`StorageVec<T>` stores only its initialized prefix and explicitly drops remaining
elements in reverse index order. `freeze` consumes the mutable owner and moves
the backing storage and lease into `StorageArray<T>` without changing accounting.
UTF-8 builders count bytes, never implicit terminators, and cloning creates an
independent logical allocation.

`DropArena<T>` registers initialized values and clears them in reverse order on
reset or arena Drop. `PlainArena<T: Copy>` rejects Drop-bearing types at compile
time. Handles contain private arena identity, generation, and index; foreign,
stale, and invalid handles fail before access. Returned borrows remain tied to
the arena borrow and cannot escape its lifetime.

## Determinism and Workload

Map and set iteration is ascending by key/value, bit-set iteration is ascending
over allocated words, and symbol IDs follow first successful intern order.
Replacing an existing map key or reinterning text does not consume distinct-entry
capacity. The release workload inserts 20,000 reverse-ordered entries and checks
ordering, bounds, and symbol resolution, preventing the earlier sorted-vector
quadratic insertion design from returning.

Run the milestone gate with:

```bash
cargo xtask storage-kernel
cargo +nightly miri test --locked -p nexa_storage --tests
```

The first command runs all storage tests with Rust 1.80 and then executes the
release workload. CI installs Miri and runs the safe kernel tests separately.
The repository-wide `make check` and `cargo xtask release-check` also cover the
crate; release-check includes the workload.

## Acceptance Evidence

Accepted and rejected coverage proves:

- alignment, stride overflow, ZST accounting, and allocator provenance;
- allocation-growth failure atomicity and budget release;
- reverse Vec/Arena Drop, zero-copy freeze, UTF-8 preservation, and independent clone;
- stale and foreign arena handles plus compile-fail Arena lifetime and PlainArena Drop bounds;
- stable collection order, owned-value return at capacity, bit bounds, and intern reuse;
- Copy versus Move, use/drop after Move, reinitialization, conditional joins, and cleanup exclusion.

No test asserts a private field order, growth factor, standard-library node
layout, raw address, or allocator implementation detail.

## Deferred Work

The following remain explicit non-goals for `0.0.4`:

- target data layouts, `LayoutId`, aggregate/union layout, and NIR verification;
- `Box`, `Shared`, `Weak`, partial aggregate initialization, and panic isolation;
- Native/Wasm Drop-trace conformance and stable external descriptors;
- custom/Freestanding allocators, unsafe code, raw pointers, or public ABI;
- source-language `take`, `mut`, Borrow checking, or new Language 2.0 syntax;
- Host ABI, errors, async, UI/Wasm Host, packages, or signing.

The next milestone is `0.0.5`: target-neutral NIR builder, independent verifier,
canonical serialization, and the internal bootstrap artifact boundary.
