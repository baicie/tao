# Futao 0.0.5 Typed NIR and Bootstrap Artifact

## Status and Boundary

Toolchain `0.0.5` delivers the ADR-003 N0/N1 boundary needed by the future
Futao compiler core: a target-neutral typed NIR, an independent verifier,
explicit target-layout calculation, deterministic private serialization, and a
machine-checked bootstrap artifact contract.

This milestone does not add a code-generation backend or freeze a public IR.
NIR remains compiler-private and may change incompatibly between `0.0.x`
releases. Stable distributed components remain the separate ADR-009 `.nexc`
lifecycle and never expose MIR or NIR for third-party execution.

| Area | `0.0.5` contract |
|---|---|
| N0 model | typed IDs, type table, functions, basic blocks, SSA values, operations, terminators, and source spans |
| N1 construction | `ModuleBuilder` creates only `UnverifiedModule`; canonical dense identities are checked before use |
| Verification | independent type, CFG, SSA, call, intrinsic, and ownership validation produces `VerifiedModule` |
| Layout | explicit 32/64-bit target description with checked scalar, pointer, aggregate, and fixed-array layout |
| Artifact | strict JSON schema 1, exact compiler compatibility, SHA-256 integrity, and canonical byte equality |
| Compiler bridge | the supported scalar, static-call, single-block MIR subset lowers to verified NIR; all later cases are `Deferred` |

## N0/N1 Model

`nexa_nir` has no dependency on LLVM, parser tokens, CST, or HIR. Its IDs are
scoped numeric identities, and user-observable operations retain stable source
ordinals and half-open byte spans. The model includes scalar and pointer-like
types, fixed logical aggregates, function signatures, block parameters, and
explicit `Return`, `Goto`, `Branch`, and `Unreachable` terminators.

The operation registry covers scalar constants, copy/move/drop, integer
addition, statically resolved calls, and typed runtime intrinsics. `PrintI64`
and `PrintI1` are registry identities with verifier-owned signatures; arbitrary
strings cannot become backend operations.

The builder checks local construction errors, but its output is deliberately
untrusted. Only `Verifier::verify` can create `VerifiedModule`, so a backend or
serializer cannot accidentally treat successful construction as proof of all
cross-reference and control-flow invariants.

## Independent Verifier

The verifier processes types, functions, blocks, instructions, and values in a
deterministic order. It rejects:

- duplicate, sparse, or otherwise invalid canonical identities;
- missing types, functions, blocks, terminators, or SSA values;
- use before definition and incompatible edge/block arguments;
- result-shape, call-signature, intrinsic-signature, branch, and return type
  mismatches;
- copy of owned values, move/drop of non-owned values, reuse after consumption,
  and owned values that reach a block exit without one move or drop;
- owned aggregates whose nested owned fields are not consumed correctly.

Stable `E5100` through `E5109` categories identify verifier failures. The
verifier does not trust the compiler lowering path or a deserialized artifact;
both must pass through the same boundary.

## Explicit Target Layout

`TargetLayout` receives pointer width, pointer alignment, aggregate alignment,
stack alignment, and endianness as explicit inputs. It never derives them from
the machine running the compiler. `TargetLayout` accepts 32-bit and
64-bit pointer widths and checks every alignment, padding, stride, and size
calculation.

Scalar, pointer/borrow/handle/function-reference, struct, and fixed-array
layouts are supported. Unknown types, invalid alignment, by-value recursive
aggregates, and arithmetic overflow fail with stable `E5500` through `E5504`
categories. The private artifact remains `target-neutral-v1`; a concrete
`TargetLayout` is selected only after NIR verification and is not inferred from
artifact producer Host state.

This is the NIR layout boundary, not completion of ADR-004 runtime storage,
allocator, union, exported ABI, Native/Wasm conformance, or backend work.

## Private Artifact Schema

`CanonicalArtifact` serializes only a `VerifiedModule`. Schema 1 contains:

```text
magic = FUTAO-NIR
nirSchemaVersion = 1
compilerVersion = exact consuming toolchain version
targetProfile = target-neutral-v1
featureFlags = sorted private capability atoms
contentHash = SHA-256 over schema, compatibility metadata, and module
module = verified target-neutral NIR
```

The loader treats bytes as untrusted. It rejects malformed JSON, unknown
fields, wrong magic or schema, incompatible compiler versions, malformed or
mismatched hashes, unsorted/duplicate metadata, invalid NIR, and any byte
sequence that is not the unique compact serializer output. A newline used by
the checked-in text fixture is removed by `xtask` before validation; the
artifact loader itself does not accept trailing whitespace.

`bootstrap/stage0/bootstrap-manifest.json` binds Stage output to the same
magic, schema, verifier crate/version, target profile, feature flag set,
canonical encoding, and hash algorithm. It continues to declare no public
extension and no signature envelope. Those belong to `.nexc` and ADR-009.

## Compiler N1 Bridge

The real compiler pipeline now attempts MIR-to-NIR lowering after MIR succeeds.
The `0.0.5` lowering slice accepts:

- `Bool`, `Int`, and `Unit` signatures and values;
- one MIR block ending in `Return` per function;
- integer/boolean constants and local reads;
- single-assignment MIR temporary slots converted to SSA values;
- statically resolved function calls;
- typed integer/boolean print intrinsics.

Unsupported language behavior never fabricates partial NIR. The canonical NIR
phase records `Deferred` with a stable reason and planned phase for strings,
arrays, records, unions, closures, function values, mutable/reassigned locals,
multiple blocks, arithmetic and other later operations, indirect calls, and
conversion intrinsics. A supported lowering error remains a real compiler
error rather than being mislabeled as deferred work.

## Validation

Run the milestone-specific gates with:

```bash
cargo test --locked -p nexa_nir --all-targets --all-features
cargo test --locked -p nexa_compiler --all-targets --all-features
cargo xtask nir-artifact
cargo xtask bootstrap-contract
```

The artifact gate loads one canonical accepted compiler artifact and requires
precise rejection categories for content mutation, an unknown JSON field, and
an unsupported schema. `release-check` also dumps the scalar compiler fixture
and requires all six canonical phases with NIR in the `produced` state.

Repository-wide `make check`, the Rust 1.80 workspace check, security tools,
and the documentation build remain release requirements.

## Deferred Work

The following are explicit non-goals for `0.0.5`:

- LLVM, Wasm, object generation, linking, or any executable NIR backend;
- a public NIR API, stable bytecode, `.nca`, or expansion of `.nexc`;
- Host ABI, stable C ABI, package/signature envelopes, or ABI negotiation;
- complete MIR CFG, ownership/drop lowering, strings, arrays, records, unions,
  closures, generic instances, async state machines, or error lowering;
- completion of the ADR-004 runtime memory-layout and cross-backend matrix;
- a Futao-written frontend or any C1/C2/C3 self-hosting stage.

The next milestone is `0.0.6`: freeze the Bootstrap Profile and Bootstrap
Stdlib contract used by the later Futao compiler stages.
