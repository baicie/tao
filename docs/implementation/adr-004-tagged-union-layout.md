# ADR-004 Checked Tagged Union Target Layout Slice

## Status

Implemented as the next independently verifiable Phase M0 slice of ADR-004.
This slice extends the existing target-neutral NIR layout calculator without
claiming completion of the normalized layout table, private artifact schema
upgrade, Drop lowering, stable ABI, or cross-backend conformance gates.

## Decision

NIR represents a tagged union as an ordered, non-empty list of payload type
identities. The zero-based variant ordinal is its logical discriminant. A
payloadless source-language variant lowers to the existing `Unit` type, so the
NIR does not need a second nullary-variant representation.

For each explicit `TargetLayout`, the private bootstrap representation is:

```text
tag                 minimum 1/2/4/8-byte unsigned width for the largest ordinal
payload offset      tag size rounded up to the maximum payload alignment
payload storage     maximum payload size and alignment across every variant
total layout        payload end rounded up to max(tag align, payload align)
```

Every padding, offset, and size operation is checked. A tag is always present,
including for a one-variant or all-ZST union. This deliberately performs no
niche optimization and makes no claim that the representation is a stable C,
Host, Native, or Wasm ABI.

`TargetLayout::layout_of` continues to return the total `ValueLayout` for every
NIR type. A union-specific query also exposes the checked tag layout, payload
layout, payload offset, and total layout so future lowering can consume the
decision without recomputing it.

The verifier rejects empty unions with `InvalidType` (`E5120`) and continues to
reject missing payload identities with `UnknownType` (`E5101`). Layout rejects
by-value cycles with `RecursiveAggregate` (`E5503`), arithmetic overflow with
`SizeOverflow` (`E5504`), and an internally unrepresentable discriminant with
`InvalidTaggedUnion` (`E5505`).

## Artifact Compatibility

The closed `FUTAO-NIR` schema 1 type vocabulary predates tagged unions. This
slice intentionally does not rewrite that historical contract. Direct
`NirType` serde supports the new compiler-internal model, while
`CanonicalArtifact` rejects a verified module containing a tagged union with
`UnsupportedSchema` (`E5112`) before hashing or encoding. The loader applies
the same gate before content-hash validation.

Persisting tagged unions requires an explicit new private artifact schema,
fixture and bootstrap-manifest update in a later independently reviewed slice.

## Acceptance Tests

The implementation is accepted when tests prove:

- a union with scalar and pointer payloads has deterministic but target-specific
  32-bit and 64-bit offsets and total sizes;
- ZST, nested aggregate, and all-ZST payload sets remain representable;
- 256 variants use a one-byte tag and 257 variants widen to two bytes;
- union ownership is `Owned` when any possible payload is owned;
- direct `NirType` serde round trips preserve the ordered variants and unknown
  fields fail closed;
- private `CanonicalArtifact` schema 1 rejects modules containing tagged unions
  on both serialization and deserialization instead of silently widening its
  closed type vocabulary;
- empty unions and unknown payload type identities fail NIR verification;
- direct and indirect by-value recursion fails layout calculation;
- payload-end or final-alignment arithmetic overflow reports the stable layout
  overflow category.

## Delivery Plan

1. Add accepted and rejected NIR/verifier/layout/serde tests and record the red
   baseline before implementation.
2. Add the target-neutral `TaggedUnion` type shape and ownership/reference
   traversal.
3. Add checked tag selection plus detailed union-layout calculation while
   retaining the existing scalar, struct, array, and pointer behavior.
4. Update ADR-004 delivery status, documentation navigation, and changelogs.
5. Run focused tests, Rust 1.80 checks, Clippy, rustdoc, the documentation build,
   and `make check` before opening a pull request to `mvp`.

## Verification

```bash
cargo test --locked -p nexa_nir --all-targets --all-features
cargo +1.80 test --locked -p nexa_nir --all-targets --all-features
cargo clippy --locked -p nexa_nir --all-targets --all-features -- -D warnings
cargo doc --workspace --no-deps
pnpm --dir docs build
make check
```

The focused tests cover the complete acceptance matrix above while retaining
the existing scalar, pointer, struct, fixed-array, verifier, and canonical
schema 1 artifact regression suites.

## Deferred Work

- `LayoutId`, `TargetLayoutId`, canonical layout-table interning, and snapshots;
- a new private NIR artifact schema that can persist tagged-union modules;
- active-variant initialization state, Drop flags, cleanup blocks, and Drop glue;
- NIR construction/match operations and MIR-to-NIR union lowering;
- niche optimization or target/backend-specific representation tuning;
- Native/Wasm/reference Drop-trace conformance;
- stable external descriptors, `@repr(C)`, Host ABI wrappers, and ABI negotiation.
