# Futao 0.0.11 Complete Compiler Core

## Status

Planned and in progress. This document is the implementation contract for the
`0.0.11` milestone in the [self-hosting plan](self-hosting-0.1.0.md). It does
not claim that the milestone, Stage 1, or the ADR-011 self-hosting gate is
already complete.

## Objective

Deliver a real compiler core written in Futao source that can compile the
versioned Language 1.0 `.nexa` conformance corpus and its one-to-one `.ft`
compatibility projection through canonical HIR and MIR, and every supported
Bootstrap Profile input through verified NIR, while matching the Rust Stage 0
reference exactly. Once every differential and regression gate
passes, the compiler driver exposes the Futao core as a real, explicitly
selectable candidate while CI exercises it on every required path. The release
default remains Rust until the `0.1.0` gate authorizes the switch.

This is a user-visible vertical milestone: accepted `.ft` programs continue to
run through the CLI, rejected programs retain stable diagnostics and spans,
and canonical dumps no longer report the Futao compiler adapter as unavailable.
It does not add source syntax, native code generation, Host ABI, async, UI, or
package-manager behavior.

## Assumptions

1. Language 1.0 remains the frozen source and observable-semantics baseline.
2. Rust remains the Stage 0 reference and the independent NIR verifier until
   ADR-011 proves the later C1/C2/C3 fixed point.
3. The candidate must execute checked-in `.ft` compiler source through the
   Bootstrap Profile. It must not call the Rust compiler and relabel its output.
4. Existing `nexa_*` crate names, `nexac`, and historical `.nexa` fixtures are
   retained during this milestone. ADR-010 establishes Futao as the language
   name; the current `baicie/tao` repository basename is not a second language
   or compiler implementation identity.
5. The milestone is delivered as a sequence of independently verifiable pull
   requests to `mvp`; the package version changes only in the closing PR.
6. `bootstrapCompiler.defaultImplementation` remains `rust-reference` in
   `0.0.11`; changing it belongs exclusively to the `0.1.0` release PR.
7. `v0.0.10` is an immutable published expression-kernel release. This
   milestone recovers its deferred semantic work as `0.0.11` prerequisites;
   it does not move, rewrite, or republish the `v0.0.10` tag.
8. The `0.0.11` implementation identity is `futao-bootstrap-candidate`.
   `futao-self-hosted` is reserved for a compiler produced by the later Stage 1
   gate and must not describe a Stage 0-executed candidate.

## Current Baseline

- Real Futao lexer, parser, and resolver adapters match their bounded corpora.
- Type-checker schema 1 covers 12 primitive expression-node kinds through two
  checked-in JSON cases; it does not cover declarations or full source graphs.
- The type-checker `.ft` files are not yet included in bootstrap compiler tree
  provenance because discovery is limited to the top-level `src/` directory.
- The complete Futao `CompilerAdapter` is explicitly unavailable.
- MIR-to-NIR accepts a scalar, single-block subset and reports honest
  `Deferred` states for aggregate types, full CFG, locals, and later operations.
- Rust is the release default and remains so throughout `0.0.11`.

## Architecture Contract

```text
explicit CompilerInput
  -> Futao lexer
  -> Futao parser
  -> Futao resolver
  -> Futao type checker
  -> Futao HIR lowering
  -> Futao HIR validation
  -> Futao Bootstrap Profile lint
  -> Futao MIR lowering
  -> Futao MIR validation
  -> Futao NIR builder
  -> independent Rust NIR verifier
  -> canonical CompilerOutput
```

Every arrow is a versioned, bounded protocol. Host Rust code may provide
explicit source bytes, execute verified MIR, validate protocol data, and invoke
the independent NIR verifier. It may not perform candidate semantic analysis
or lowering.

The existing phase-specific lexer, parser, resolver, and type-checker adapters
remain focused test interfaces. The complete compiler adapter composes the
same Futao modules through one checked-in `.ft` `compile(input)` entry and
returns the same six canonical phase artifacts as the Rust reference: tokens,
CST, diagnostics, HIR, MIR, and NIR. Rust may encode the explicit input, execute
the verified driver MIR, and validate its observation; it must not invoke and
compose candidate phases itself.

`conformance/1.0` remains the normative Language 1.0 corpus and continues to
use its historical `.nexa` identities. `0.0.11` adds a manifest-bound `.ft`
compatibility projection with a one-to-one case mapping and the same expected
language results. Installed-CLI `.ft` E2E cases are additional coverage, not a
substitute for either versioned corpus.

Before adding more compiler source, the bootstrap manifest must discover and
hash every declared compiler source root recursively. The current manifest
only binds top-level `bootstrap/compiler/src/*.ft`; the already checked-in
`bootstrap/compiler/typecheck/*.ft` files are outside that provenance. No new
semantic or lowering directory may land until this gap is closed with strict
manifest validation and drift tests.

Source hashing alone is not sufficient provenance. Each phase record in both
the bootstrap compiler manifest and Stage 0 manifest must bind the phase entry
point, observation and protocol schema versions, corpus category counts and
digest, fuzz seed count and digest, and applicable target-layout descriptor.
The gates calculate those values from checked-in inputs instead of trusting the
declared numbers. Every slice that adds source, changes a schema, or changes a
gate updates both manifests atomically.

The complete adapter also needs a validated canonical observation interface.
`CompilerOutput` is intentionally constructed by Rust internals today, and its
phase DTOs are serialization-only. A candidate must return an untrusted,
versioned observation that a strict Rust loader validates before differential
comparison; it must not be given a constructor that can claim Rust-only typed
state without proof.

### Type Checker Completion

The expression-only `FUTAO-TYPECHECK-1` schema remains a closed historical
contract. The complete semantic checker uses a new schema rather than silently
widening schema 1. Its input contains resolved declaration, type, scope, and
expression identities; its output contains canonical inferred types, generic
substitutions, match coverage, ownership facts, and structured diagnostics.

The contract must cover:

- primitive, array, function, parameter, Record, and Union types;
- local bindings, assignment, returns, branches, loops, and loop control;
- String and homogeneous Array literals, indexing, members, and intrinsics;
- function, Record, and Union signatures with closed generic substitution;
- direct and indirect calls, constructors, fields, indexing, and intrinsics;
- exhaustive and unreachable match analysis with payload bindings;
- the entry-module `main` signature and every checker-owned Language 1.0
  diagnostic: `E2003` through `E2006` and `E3001` through `E3012`, including
  labels, ordering, recovery, and suppression;
- immutable-owned value classification, assignment rules, capture snapshots,
  and mutable-capture rejection required by the frozen language baseline;
- accepted and rejected multi-module inputs with deterministic diagnostic order.

Bootstrap Profile v1 exposes no source-level ownership operations. Move/Drop,
use-after-move, and cleanup are therefore not new Language 1.0 type-checker
behavior. They are produced by MIR cleanup elaboration and must be explicit
before NIR verification as required by ADR-002 and ADR-003.
Parser-owned `E1001`, resolver-owned `E2001`/`E2002` and `E4002` through
`E4005`, and loader-owned `E4001` remain in their existing phases. A separate
Futao Bootstrap Profile pass owns `E6201` through `E6203`. The complete adapter
must preserve canonical phase ownership without duplicate diagnostics.

### HIR and MIR

Futao HIR uses the existing canonical identity and source-span vocabulary. It
must preserve module, definition, local, field, variant, and type-parameter
identities without depending on map iteration or Host paths.

Candidate HIR is untrusted protocol data. A strict structural validator checks
schemas, complete type and identity references, source-map bounds, canonical
ordering, closed generic instances, and phase invariants, then constructs an
opaque Host `VerifiedHirProgram`. The one-shot Futao driver applies the same
frozen rules with its own Futao validator before profile lint or MIR lowering;
the Host validates the returned observation independently and never feeds a
Rust-only typed value back between candidate phases.

Futao MIR lowering must reproduce the reference CFG contract, including block
order, local allocation, evaluation order, calls, branches, loops, match
switches, closures, Record/Union construction, and runtime intrinsics. Tests
compare the complete canonical MIR value, not only execution output.

Canonical MIR returned by either implementation passes through the same Host
DTO, strict decoder, and structural verifier before differential comparison,
reference execution, or Rust MIR-to-NIR lowering. That boundary validates
schema, bounds, identities, types, CFG edges, local uses, source spans, and
operation invariants. Adding `Deserialize` directly to the private `MirProgram`
model without this fail-closed boundary is insufficient. The schema types every
local, place, block parameter, ownership state, and initialization state. Only
the verifier constructs opaque Host `VerifiedMirProgram`; no Host consumer
accepts raw Rust-reference or candidate MIR. The one-shot Futao driver likewise
runs its Futao MIR validator before its own NIR builder, without a Rust semantic
callback.

### NIR

NIR is still private and target-neutral. The Futao builder may emit only the
vocabulary accepted by the independent Rust verifier. Missing NIR operations
or type forms are added in small schema-versioned slices with accepted and
rejected verifier tests. The closing corpus must not contain a `Deferred` NIR
artifact for any supported Bootstrap Profile program.

NIR expansion is reference-first. Each vocabulary slice extends the Rust model,
verifier, canonical logical DTO, layout evidence, and reference MIR-to-NIR
lowering before the Futao builder may emit it. The sequence is the in-memory
layout-table foundation, scalar/locals/CFG, String/Array and intrinsics,
Record/Union aggregates, then function values and closures. Schema 2 is frozen
only after that vocabulary and its layout/Drop-glue completeness gate close.
This keeps the independent verifier and reference oracle ahead of candidate
output without silently widening closed schema 1.

The existing artifact schema 1 remains the closed `target-neutral-v1` contract:
it verifies logical NIR before selecting a target and cannot carry aggregate
layout evidence. `0.0.11` introduces a separate schema 2 verification envelope.
The verifier first constructs target-neutral `VerifiedModule`, then verifies a
canonical `LayoutId`/`TargetLayoutId` table against that module digest and an
explicit target descriptor to construct `LayoutVerifiedModule`. Only the latter
may be serialized as schema 2 or sent toward execution. The envelope hashes the
logical module, target descriptor, layout table, and Drop-glue registry
separately so target-neutral and target-qualified evidence remain auditable.

Existing `Struct`, `FixedArray`, and `TaggedUnion` model types stay model-only or
rejected by closed artifact schema 1 until they are migrated into the
layout-bound schema 2 envelope. Every later materializable type and operation
adds its logical verifier rules, layout record, and Drop-glue declaration
atomically. `0.0.11` implements the records needed by the Bootstrap Profile and
compiler self-graph; it does not claim the remaining Application/Freestanding
allocator, ABI, or cross-backend ADR-004 gates.

The historical schema 1 reader and writer remain separate and unchanged; each
accepts only schema 1. A separate schema 2 reader and writer accepts only schema
2. No unified reader silently upgrades one version into the other, and callers
must select and retain the expected artifact version explicitly.

Target layout is explicit compiler input, never Host discovery. All `0.0.11`
differential, stdlib, self-graph, and reproducibility gates use the versioned
`bootstrap-layout-64le-v1` descriptor: 64-bit pointers, 8-byte pointer
alignment, a 16-byte aggregate-alignment ceiling, 16-byte stack alignment, and
little-endian byte order. The descriptor and `TargetLayoutId` are bound into the
candidate protocol; byte equality across Hosts always means equality for this
same descriptor. Logical NIR operations remain target-neutral; the private
layout table is a target-qualified verification record, not public ABI or
Host-derived state.

Language 1.0 MIR currently erases generic values at runtime, while ADR-003 NIR
requires deterministic monomorphized function instances. The canonical HIR
contract must therefore expose the complete closed-instance set before NIR
lowering. Likewise, cleanup and Drop elaboration must be represented before
NIR: the compiler self-graph contains owned String, Array, Record, Union, and
closure values, and whole-value consume verification alone is not a cleanup
plan.

## Project Structure

```text
bootstrap/compiler/src/             Futao lexer/parser/resolver and shared core
bootstrap/compiler/typecheck/       historical expression-kernel schema 1
bootstrap/compiler/semantic/        complete semantic checker and protocol
bootstrap/compiler/lowering/        canonical HIR, MIR, and NIR builders
bootstrap/compiler/driver/          one-shot Futao compiler driver core
crates/nexa_compiler/src/            Host shell, selection, adapters, validators
crates/nexa_compiler/tests/          focused and complete differential tests
bootstrap/compiler/tests/            accepted, rejected, and fuzz-seed corpus
crates/nexac/tests/e2e/              installed-CLI behavior over `.ft` programs
docs/implementation/                 milestone decisions and verification record
```

New directories are created only when their first executable slice lands; no
placeholder source or empty crate is permitted.

## Code Style

Futao compiler code stays inside `futao-bootstrap-v1`: immutable bindings,
bounded recursive traversal, stable insertion order, explicit inputs, and no
ambient Host I/O. Protocol tags are integers inside Futao and validated names
at the Rust adapter edge.

```text
function checkExpression(typeTag: Int, span: Span): CheckResult {
  if (typeTag === TYPE_INT) {
    return CheckResult.Accepted(TYPE_INT);
  }
  return CheckResult.Rejected(typeMismatch(span));
}
```

Rust library code remains `#![forbid(unsafe_code)]`, uses structured
`thiserror` errors, borrows input data where practical, documents every public
interface, and stays compatible with Rust 1.80 and edition 2021.

## Delivery Slices

Each item is one pull request unless its implementation must be split further
to remain independently reviewable.

### Phase 0: Close Provenance and Observation Interfaces

1. **Recursive compiler-source provenance**
   - Version the bootstrap compiler manifest so it declares every compiler
     source root and sorted `.ft` file, including the existing type checker.
   - Recursively reject undeclared files, symlinks, non-UTF-8 content,
     non-portable paths, duplicates, and tree-digest drift.
   - Add phase records for entry points, observation/protocol schemas, corpus
     category counts and digests, fuzz seed counts and digests, and target
     descriptors; bind the same values in the Stage 0 manifest.
   - Calculate every count and digest from checked-in files, and require each
     later schema/source/corpus slice to update both manifests atomically.
   - Verify with `cargo test --locked -p xtask bootstrap::tests` and
     `cargo xtask bootstrap-contract`.

2. **Canonical candidate observation**
   - Define a versioned untrusted observation for all six compiler phases,
     implementation identity, status, and structured diagnostics.
   - Rename the pre-Stage-1 Futao identity to `futao-bootstrap-candidate` and
     reject `futao-self-hosted` until the Stage 1 contract is proven.
   - Add a strict loader/validator with exact bounds, ordering, schema, phase,
     source-span, and produced/blocked/deferred state checks.
   - Prove malformed candidates fail closed before the full Futao adapter can
     implement `CompilerAdapter`.

### Phase A: Recover Deferred Semantics for `0.0.11`

The published `v0.0.10` tag remains the closed expression-kernel release. The
following work restores the roadmap's originally intended semantic coverage as
prerequisites of `0.0.11`; none of these slices is a replacement `0.0.10`.

3. **Semantic schema and type graph**
   - Freeze a strict schema 2 for canonical types, declarations, identities,
     spans, bounds, and deterministic ordering.
   - Add a versioned manifest that maps every normative Language 1.0 `.nexa`
     case to its checked-in `.ft` compatibility projection; neither corpus may
     be replaced by the smaller CLI E2E set.
   - Add malformed-input and malformed-output rejection tests before the Futao
     implementation.
   - Verify with `cargo test --locked -p nexa_compiler --test typecheck_differential`.

4. **Primitive, local, and control-flow semantics**
   - Implement primitive operators, local bindings, assignments, returns,
     branches, loops, and loop control against the frozen Rust reference.
   - Cover `E2004` and `E3001` through `E3004`, including invalid `main`
     signatures, exact labels, recovery, and deterministic order.

5. **Array, String, and intrinsic semantics**
   - Implement homogeneous/contextual Array rules, indexing, String behavior,
     builtins, and registered array intrinsics with exact arity and types.
   - Cover `E2003`, `E2005`, and related `E3001` failures for every operation.

6. **Signatures and generic substitution**
   - Implement function/Record/Union signatures, inference, explicit type
     arguments, arity checks, and non-regular/budget rejection.
   - Compare accepted and rejected Rust/Futao snapshots with no suppression.
   - Cover `E2003`, `E3008` through `E3010`, and `E3012`, including their
     derivative-diagnostic suppression rules.
   - Verify with `cargo xtask typecheck-differential`.

7. **Record, Union, and match semantics**
   - Implement constructors, fields, variant payloads, exhaustive coverage,
     duplicate/unreachable arms, and generic payload substitution.
   - Cover `E2003`, `E2005`, `E2006`, and `E3005` through `E3007`, including
     nested generic unions and multi-module declarations.
   - Verify focused tests plus the complete semantic corpus.

8. **Immutable ownership and closure facts**
   - Implement immutable-owned classification, assignment facts, function
     values, capture snapshots, and mutable-capture rejection.
   - Cover `E3011` with exact primary and secondary labels, suppression, and
     accepted/compile-fail cases; consume resolver facts without re-emitting
     resolver-owned diagnostics.
   - Start HIR work only when all recovered semantic observables match.

9. **Type-checker fuzz and CI gate**
   - Mutate checked-in UTF-8 sources plus strict source-graph and semantic-schema
     inputs into bounded valid, boundary-valid, and rejected cases. Run every
     case through independent complete Rust and Futao
     lexer/parser/resolver/checker chains and compare semantic observations.
   - Separately fuzz malformed candidate observations so decoding and validation
     fail closed without panic or unbounded work; observation-only fuzzing does
     not satisfy the semantic differential gate.
   - Persist minimized deterministic seeds and bind the generator/schema version,
     corpus counts, seed counts, and digests in both bootstrap manifests.
   - Add the target to the front-end fuzz matrix and required smoke checks so
     the matrix covers lexer, parser, resolver, and type checker.
   - Make the complete source-graph chain the mandatory Phase A and pre-HIR gate;
     synthetic semantic tables or phase-local DTO tests cannot close Phase A.

### Phase B: Complete `0.0.11` Lowering

10. **Canonical HIR schema and strict validator**
   - Freeze a bounded versioned DTO for modules, definitions, locals, fields,
     variants, types, spans, facts, and the complete closed-instance set.
   - Strictly decode candidate HIR and validate ordering, identity continuity,
     ownership, type/reference integrity, spans, closed instances, unknown
     fields, and exact protocol bounds before constructing opaque
     `VerifiedHirProgram`.
   - Add accepted, malformed, exact-bound, and one-over Rust tests first.

11. **Futao HIR builder**
   - Produce complete canonical typed HIR from the verified Futao semantic
     result without reusing Rust HIR construction.
   - Preserve the closed generic-instance set in stable declaration and
     type-argument order and match accepted/rejected multi-module snapshots.
   - Implement the same structural validator in Futao and fail closed before
     profile lint or MIR lowering; compare its status with the independent Rust
     validator over valid and mutated observations.

12. **Futao Bootstrap Profile pass**
   - Run the pure profile lint only when the checker and Futao HIR builder have
     produced canonical typed HIR; match Rust for `E6201` through `E6203`,
     labels, order, and recovery.
   - If an earlier phase fails, mark profile and later phases `Blocked` without
     emitting derivative profile diagnostics. Any profile diagnostic blocks MIR
     and NIR production.
   - Make profile-accepted and profile-rejected source graphs part of the
     differential gate; Rust must not add profile diagnostics to candidate HIR.

13. **Canonical MIR schema and reference model**
   - Freeze a new bounded canonical MIR schema with typed locals, places, block
     parameters, ownership/initialization state, operations, terminators, and
     source spans before candidate MIR exists.
   - Extend Rust reference lowering and canonical serialization first, with
     accepted/rejected schema and round-trip tests.

14. **Strict MIR decoder and structural verifier**
   - Encode Rust-reference and candidate canonical MIR through the same
     versioned DTO and structural verifier rather than trusting the private
     reference model. Reject unknown fields, non-canonical ordering, invalid
     source-map spans, oversized strings/collections, excessive nesting, and
     over-budget object graphs.
   - Verify typed places, block parameters, CFG edges, local definition/use,
     ownership and initialization states, spans, terminators, and operations
     before constructing opaque `VerifiedMirProgram`; the Rust interpreter and
     MIR-to-NIR oracle accept no raw MIR from either implementation.
   - Require every MIR vocabulary extension to land decoder and verifier rules
     before the Futao builder can emit that extension.
   - Implement the same structural gate in Futao before candidate NIR lowering;
     compare Futao and independent Rust validation status over mutation cases.

15. **Scalar and CFG MIR builder**
   - Lower functions, locals, expressions, calls, branches, loops, and returns
     with reference-identical block/local ordering.
   - Prove both snapshot equality and reference-interpreter output equality.

16. **Aggregate and closure MIR builder**
   - Lower arrays, Records, Unions, match switches, function values, closures,
     captures, and supported intrinsics.
   - Retain source spans and deterministic identities in every path.

17. **Deterministic monomorphization**
   - Consume the already frozen canonical HIR closed-instance set and assign a
     function identity per closed function plus concrete closed-type identities
     for generic Record/Union instances and closure environments. Slice 19 maps
     each `(TargetLayoutId, ClosedTypeId)` pair to its canonical `LayoutId`.
   - Order identities by declaration, type arguments, closure identity, and
     capture types so source insertion or map order cannot affect output.
   - Reject non-regular or over-budget expansion before lowering without
     changing the HIR schema established by slice 10.

18. **Cleanup and Drop elaboration**
   - Classify Copy and owned values, add initialization state and cleanup edges
     to MIR, and lower reverse-order cleanup across returns and branches.
   - Freeze a canonical `DropGlueId` registry with stable type association,
     signatures, and ordering before any layout record may reference Drop glue;
     later NIR vocabulary slices add each verified body atomically.
   - Cover aggregate fields, active Union variants, captured environments,
     partial initialization, and exactly-once cleanup with trace tests.

19. **Canonical NIR layout-table foundation**
   - Add `LayoutId`, `TargetLayoutId`, table ordering, and verifier rules for
     the scalar, pointer, handle, function-reference, and already modeled
     Struct/FixedArray/TaggedUnion types available at this slice; later type
     slices extend the table atomically.
   - Add `bootstrap-layout-64le-v1` as an explicit versioned compiler option and
     bind that exact descriptor into candidate input and comparison evidence.
   - Make the independent verifier reject missing, duplicate, inconsistent,
     recursive, overflowing, target-mismatched, or unresolved Drop-glue records.

20. **Scalar, local, and CFG NIR reference**
   - Extend the Rust NIR vocabulary, verifier, canonical logical DTO, and
     reference MIR-to-NIR lowering for locals, branches, block parameters, and
     scalar operations before candidate emission.
   - Add builder, verifier, DTO round-trip, schema 1 rejection, mutation,
     recursion, ownership, and overflow tests for each addition.

21. **String, Array, and intrinsic NIR reference**
   - Add independently verified operations and types for owned String,
     immutable Array construction, indexing, iteration, and copy-returning
     `append`/`concat` intrinsics.
   - Keep any lowering-internal aggregate fill unobservable; do not add a
     source-level mutable Array or growable collection operation.
   - Add String/Array logical DTO rules, layout records, and Drop glue in the
     same slice; require complete Rust reference lowering plus fail-closed
     schema 1 rejection tests.

22. **Record and Union NIR reference**
   - Add aggregate construction, projection, variant tags and payloads, switch
     lowering, layout references, and active-variant cleanup verification.
   - Add every instantiated aggregate layout and Drop glue in the same slice;
     prove complete Rust reference lowering before enabling candidate output.

23. **Function-value and closure NIR reference**
   - Add direct/indirect calls, function instances, environments, captures,
     and closure cleanup with deterministic identities.
   - Add closure-environment layouts and cleanup in the same slice and close all
     remaining Rust MIR-to-NIR `Deferred` states used by the compiler self-graph.

24. **Layout and Drop-glue completeness gate**
   - Walk every materializable closed type and require exactly one target layout
     record; require every owned layout to resolve to one verified
     `DropGlueId` body with the expected signature.
   - Add missing/extra/mismatched layout and glue mutation tests plus exact
     compiler self-graph counts before candidate NIR may be enabled.

25. **Private NIR artifact schema 2**
   - Freeze the now-complete logical NIR vocabulary and verified layout table as
     schema 2: target-neutral logical NIR plus a separately hashed
     target-qualified layout subdocument bound to the logical module digest.
   - Keep the historical schema 1 reader and writer unchanged and schema-1-only;
     add separate schema-2-only reader and writer paths with no implicit upgrade.
   - Pin schema, feature flags, `bootstrap-layout-64le-v1` descriptor/digest,
     canonical encoding, and content hash in both bootstrap manifests.
   - Define fixed-point normalization as canonical logical NIR, target ID, and
     layout records without producer-version or integrity-envelope fields.
   - Fail closed on a wrong schema, unknown target, unknown fields,
     cross-document digest mismatch, digest drift, or incomplete layout evidence.

26. **Futao NIR builder**
   - Lower complete Futao MIR to target-neutral NIR and pass the independent
     Rust logical and layout verifiers.
   - Require deterministic canonical bytes across source insertion order,
     working directory, locale, and supported Hosts when given the same
     explicit target descriptor.

### Phase C: Integrate and Release

27. **Compiler self-graph resource contract**
    - Freeze manifest ceilings and measurement rules for source count/UTF-8
      bytes, syntax and semantic nodes, types and closed instances, HIR nodes,
      MIR locals/blocks/operations, NIR types/functions/blocks/instructions and
      layouts, protocol bytes, call depth, and interpreter steps.
    - Add exact-bound and one-over rejection tests for every protocol dimension;
      record the complete compiler self-graph measurements below the ceilings.
    - Fail closed with structured resource errors before unbounded allocation,
      recursion, interpretation, or canonical serialization can begin.
    - Add `cargo xtask compiler-self-graph` and require it in CI, `make check`,
      and `release-check`.

28. **Futao compiler driver core**
    - Add one checked-in `.ft` `compile(input)` entry that invokes Futao lexer,
      parser, resolver, checker, HIR, profile, MIR, and NIR modules and returns
      one versioned candidate observation.
    - Invoke the driver once per compiler input. Rust validates the complete
      observation only after return and never injects `VerifiedHirProgram`,
      `VerifiedMirProgram`, or any phase result into the running candidate.
    - Prove the driver contains no Rust semantic callback, per-phase Rust
      orchestration, Host I/O, fallback, or fabricated phase state.

29. **Complete compiler adapter and corpus gate**
    - Replace the explicit `Unavailable` state with a real Futao adapter.
    - Compare all six phase statuses and all available canonical bytes for
      the normative `.nexa` corpus, its `.ft` projection, fuzz seeds, Bootstrap
      Stdlib, compiler self-graph, and CLI E2E inputs. Non-Bootstrap Language 1.0
      cases may match as NIR `Deferred`/`Blocked`; Bootstrap Profile, stdlib, and
      self-graph cases must be `Produced` and independently verified.
    - Retain mismatching snapshots and reject every unclassified difference.
    - Add `cargo xtask compiler-differential` with retained six-phase snapshots,
      full source-graph semantics, Bootstrap Stdlib, self-graph, and failure
      cases; wire it into CI, `make check`, and `release-check`.

30. **Explicit candidate driver path**
    - Expose Futao and Rust compiler implementations through explicit driver
      selection while keeping Rust as the release default.
    - An explicit Futao selection must either return validated candidate output
      or fail closed at selection, availability, preflight, execution, decoding,
      or verification; it must never fall back to Rust at any stage.
    - Make required CI and differential commands execute the Futao path. Add E2E
      proof that an omitted selector executes only `rust-reference`, while every
      injected explicit-candidate failure retains candidate identity and fails.

31. **`0.0.11` closure**
    - Update Cargo/package versions, bootstrap manifests, tree digests,
      changelog, guides, and release metadata.
    - Run the complete local, MSRV, documentation, security, bootstrap, E2E,
      and release gates before the closing squash merge.

## Testing Strategy

Every behavioral slice follows red/green/refactor and includes accepted and
rejected or compile-fail coverage. Small protocol validators use focused Rust
tests; phase adapters use differential integration tests; the complete driver
uses multi-module and installed-CLI E2E tests.

Required checkpoints include:

```bash
cargo test --locked -p nexa_compiler --all-targets --all-features
cargo test --locked -p nexac --all-targets --all-features
cargo +1.80.0 test --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo xtask typecheck-differential
cargo xtask compiler-differential
cargo xtask compiler-self-graph
cargo xtask fuzz-smoke
cargo xtask bootstrap-profile
cargo xtask bootstrap-contract
cargo xtask nir-artifact
cargo xtask release-check
pnpm --dir docs build
make check
```

Do not repeat an unchanged successful command. Run the focused gate after each
behavioral change and the broad gates once the slice is complete.

## Boundaries

Always:

- preserve stable source spans and deterministic identity/order contracts;
- validate every candidate protocol and NIR artifact as untrusted input;
- bind every executable `.ft` compiler source into checked provenance;
- update both bootstrap manifests atomically with each changed source, entry
  point, schema, corpus, seed set, target descriptor, count, and digest;
- add accepted and rejected tests before implementation behavior;
- branch from current `mvp`, target `mvp`, and squash merge each reviewed PR;
- delete merged topic branches locally and remotely.

Ask first:

- adding a third-party dependency or raising Rust 1.80;
- expanding Language 1.0 syntax or observable semantics;
- changing the public `.nexc` component lifecycle;
- removing the Rust reference or rollback implementation before ADR-011 is
  complete.

Never:

- implement the Futao adapter by calling the Rust compiler;
- let Parser perform type checking or let code generation consume CST/AST;
- infer input from Host paths, environment, time, randomness, or hash order;
- widen a closed schema without a version change and rejected old/new tests;
- claim `0.1.0`, Stage 1, native code generation, or full self-hosting here.

## Success Criteria

The milestone is complete only when all of the following are proven:

1. The complete Futao type checker matches Rust for the normative Language 1.0
   `.nexa` semantic corpus and its one-to-one `.ft` projection, including
   generics, match, immutable-owned facts, and capture diagnostics; Futao
   profile lint also matches `E6201` through `E6203`, emits no derivative
   diagnostics after an earlier failure, and blocks MIR/NIR when it rejects HIR.
2. A single real Futao `compile(input)` driver and compiler adapter are
   available; no candidate phase delegates semantics, lowering, profile lint,
   or phase orchestration to Rust.
3. Both bootstrap manifests bind every compiler source and phase entry point,
   schema versions, corpus/seed counts and digests, and the target descriptor;
   calculated drift, undeclared files, and mutated source trees are rejected.
4. Tokens, CST, diagnostics, HIR, and MIR match canonically for every Language
   1.0 gate case. All six phase statuses and available bytes match; non-Bootstrap
   NIR may be identically `Deferred`/`Blocked`, with no suppression list. Both
   reference and candidate canonical MIR pass the same Host structural verifier
   before comparison, interpretation, or Rust NIR lowering.
5. Every supported Bootstrap Profile input, the Bootstrap Stdlib, and the
   compiler self-graph produce schema 2 NIR whose logical module and complete
   layout/Drop-glue evidence are independently verified instead of `Deferred`
   or fabricated output.
6. Closed generic instances are canonical and monomorphized deterministically;
   cleanup/Drop traces are complete and exactly once on every tested CFG path.
7. CLI E2E proves accepted output, rejected diagnostics, runtime failures,
   multi-file source locations, explicit candidate/reference selection, and
   fail-closed candidate errors at every selection stage; an omitted selector
   executes only Rust and the release manifest records `rust-reference`.
8. The self-graph stays within manifest-bound resource ceilings and every
   protocol dimension has exact-bound and one-over fail-closed tests.
9. Repeated and reordered logical NIR is byte-for-byte target-neutral, and its
   schema 2 execution envelope is deterministic for the same explicit target
   descriptor; Rust 1.80, stable Rust, documentation, security, bootstrap, and
   release checks pass.
10. `cargo xtask compiler-differential` compares full source graphs and all six
    phases, while `cargo xtask compiler-self-graph` enforces complete resource
    and layout evidence; both retain failures and are required by CI,
    `make check`, and release.
11. The candidate identifies as `futao-bootstrap-candidate`; version `0.0.11`
   and all bootstrap/source digests describe the exact merged
   sources; every delivery PR is squash merged to `mvp` and its topic branch is
   removed.

Only after these criteria pass may the implementation status change to
Implemented. Stage 1 remains the separate `0.0.12` milestone.
