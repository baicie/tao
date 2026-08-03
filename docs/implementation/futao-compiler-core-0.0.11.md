# Futao 0.0.11 Complete Compiler Core

## Status

Planned and in progress. This document is the implementation contract for the
`0.0.11` milestone in the [self-hosting plan](self-hosting-0.1.0.md). It does
not claim that the milestone, Stage 1, or the ADR-011 self-hosting gate is
already complete.

## Objective

Deliver a real compiler core written in Futao source that can compile the
versioned Language 1.0 `.ft` corpus through canonical HIR and MIR, and every
supported Bootstrap Profile input through verified NIR, while matching the
Rust Stage 0 reference exactly. Once every differential and regression gate
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
   retained during this milestone. The repository and new source use the Tao
   repository/Futao language naming established by ADR-010.
5. The milestone is delivered as a sequence of independently verifiable pull
   requests to `mvp`; the package version changes only in the closing PR.
6. `bootstrapCompiler.defaultImplementation` remains `rust-reference` in
   `0.0.11`; changing it belongs exclusively to the `0.1.0` release PR.

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
  -> Futao MIR lowering
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
same Futao modules and returns the same six canonical phase artifacts as the
Rust reference: tokens, CST, diagnostics, HIR, MIR, and NIR.

Before adding more compiler source, the bootstrap manifest must discover and
hash every declared compiler source root recursively. The current manifest
only binds top-level `bootstrap/compiler/src/*.ft`; the already checked-in
`bootstrap/compiler/typecheck/*.ft` files are outside that provenance. No new
semantic or lowering directory may land until this gap is closed with strict
manifest validation and drift tests.

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
- the entry-module `main` signature and the complete frozen `E2003` through
  `E2006` and `E3001` through `E3004` diagnostic families;
- immutable-owned value classification, assignment rules, capture snapshots,
  and mutable-capture rejection required by the frozen language baseline;
- accepted and rejected multi-module inputs with deterministic diagnostic order.

Bootstrap Profile v1 exposes no source-level ownership operations. Move/Drop,
use-after-move, and cleanup lowering therefore remain NIR/ADR-002 work and must
not be introduced as new Language 1.0 type-checker behavior in this milestone.

### HIR and MIR

Futao HIR uses the existing canonical identity and source-span vocabulary. It
must preserve module, definition, local, field, variant, and type-parameter
identities without depending on map iteration or Host paths.

Futao MIR lowering must reproduce the reference CFG contract, including block
order, local allocation, evaluation order, calls, branches, loops, match
switches, closures, Record/Union construction, and runtime intrinsics. Tests
compare the complete canonical MIR value, not only execution output.

Canonical MIR produced by the candidate is untrusted protocol data. The Rust
reference interpreter may execute it only after a strict decoder and structural
verifier validate schema, bounds, identities, types, CFG edges, local uses,
source spans, and operation invariants. Adding `Deserialize` directly to the
private `MirProgram` model without that fail-closed boundary is insufficient.

### NIR

NIR is still private and target-neutral. The Futao builder may emit only the
vocabulary accepted by the independent Rust verifier. Missing NIR operations
or type forms are added in small schema-versioned slices with accepted and
rejected verifier tests. The closing corpus must not contain a `Deferred` NIR
artifact for any supported Bootstrap Profile program.

NIR expansion is reference-first. Each vocabulary slice extends the Rust
model, verifier, artifact codec, and reference MIR-to-NIR lowering before the
Futao builder may emit it. The sequence is scalar/locals/CFG, canonical layout
records, String/Array and intrinsics, Record/Union aggregates, then function
values and closures. This keeps the independent verifier and reference oracle
ahead of candidate output.

Before any materializable aggregate enters NIR, the verifier must consume the
canonical `LayoutId`/`TargetLayoutId` table required by ADR-004. `0.0.11`
implements the records needed by the Bootstrap Profile and compiler self-graph;
it does not claim the remaining Application/Freestanding allocator, ABI, or
cross-backend ADR-004 gates.

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
crates/nexa_compiler/src/            Host adapters, validators, and driver core
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
function checkExpression(node: SemanticNode, facts: SemanticFacts): CheckResult {
  const input = facts.typeAt(node.left);
  return input === TypeTag.Int()
    ? CheckResult.Accepted(TypeTag.Int())
    : CheckResult.Rejected(typeMismatch(node.span));
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
   - Verify with `cargo test --locked -p xtask bootstrap::tests` and
     `cargo xtask bootstrap-contract`.

2. **Canonical candidate observation**
   - Define a versioned untrusted observation for all six compiler phases,
     implementation identity, status, and structured diagnostics.
   - Add a strict loader/validator with exact bounds, ordering, schema, phase,
     source-span, and produced/blocked/deferred state checks.
   - Prove malformed candidates fail closed before the full Futao adapter can
     implement `CompilerAdapter`.

### Phase A: Close `0.0.10` Semantics

3. **Semantic schema and type graph**
   - Freeze a strict schema 2 for canonical types, declarations, identities,
     spans, bounds, and deterministic ordering.
   - Add malformed-input and malformed-output rejection tests before the Futao
     implementation.
   - Verify with `cargo test --locked -p nexa_compiler --test typecheck_differential`.

4. **Primitive, local, and control-flow semantics**
   - Implement primitive operators, local bindings, assignments, returns,
     branches, loops, and loop control against the frozen Rust reference.
   - Cover `E2004` and `E3001` through `E3004`, including invalid `main`
     signatures, exact spans, recovery, and deterministic diagnostic order.

5. **Array, String, and intrinsic semantics**
   - Implement homogeneous/contextual Array rules, indexing, String behavior,
     builtins, and registered array intrinsics with exact arity and types.
   - Cover `E2003`, `E2005`, and related `E3001` failures for every operation.

6. **Signatures and generic substitution**
   - Implement function/Record/Union signatures, inference, explicit type
     arguments, arity checks, and non-regular/budget rejection.
   - Compare accepted and rejected Rust/Futao snapshots with no suppression.
   - Verify with `cargo xtask typecheck-differential`.

7. **Record, Union, and match semantics**
   - Implement constructors, fields, variant payloads, exhaustive coverage,
     duplicate/unreachable arms, and generic payload substitution.
   - Cover `E2003`, `E2005`, `E2006`, nested generic unions, and multi-module
     declarations.
   - Verify focused tests plus the complete semantic corpus.

8. **Immutable ownership and closure facts**
   - Implement immutable-owned classification, assignment facts, function
     values, capture snapshots, and mutable-capture rejection.
   - Add accepted and compile-fail cases with exact diagnostic spans.
   - Close `0.0.10` only when all semantic observables match.

9. **Type-checker fuzz and CI gate**
   - Add a dedicated bounded Type Checker fuzz target and deterministic seed
     corpus; malformed candidate observations must fail closed without panic.
   - Add the target to the front-end fuzz matrix and required smoke checks so
     the matrix covers lexer, parser, resolver, and type checker.

### Phase B: Complete `0.0.11` Lowering

10. **Canonical HIR builder**
   - Produce complete canonical typed HIR from the verified Futao semantic
     result without reusing Rust HIR construction.
   - Compare accepted and rejected multi-module corpus snapshots.

11. **Strict MIR decoder and structural verifier**
   - Decode candidate canonical MIR through a versioned DTO rather than the
     private reference model, rejecting unknown fields and oversized inputs.
   - Verify identities, types, block parameters, CFG edges, local definition
     and use, spans, terminators, operations, and initialization before any
     candidate MIR reaches the reference interpreter.

12. **Scalar and CFG MIR builder**
   - Lower functions, locals, expressions, calls, branches, loops, and returns
     with reference-identical block/local ordering.
   - Prove both snapshot equality and reference-interpreter output equality.

13. **Aggregate and closure MIR builder**
   - Lower arrays, Records, Unions, match switches, function values, closures,
     captures, and supported intrinsics.
   - Retain source spans and deterministic identities in every path.

14. **Closed generic instances and monomorphization**
   - Add the checker-computed closed generic-instance set to the canonical HIR
     observation with stable declaration and type-argument ordering.
   - Deterministically assign one NIR function-instance identity per closed
     instance and reject non-regular or over-budget expansion before lowering.

15. **Cleanup and Drop elaboration**
   - Classify Copy and owned values, add initialization state and cleanup edges
     to MIR, and lower reverse-order cleanup across returns and branches.
   - Cover aggregate fields, active Union variants, captured environments,
     partial initialization, and exactly-once cleanup with trace tests.

16. **Scalar, local, and CFG NIR reference**
   - Extend the Rust NIR vocabulary, verifier, artifact codec, and reference
     MIR-to-NIR lowering for locals, branches, block parameters, and scalar
     operations before candidate emission.
   - Add builder, verifier, artifact round-trip, mutation, recursion, ownership,
     and overflow tests for each schema-versioned addition.

17. **Canonical NIR layout table**
   - Add canonical `LayoutId` and `TargetLayoutId` records for every
     materializable Bootstrap Profile type, including size, alignment, value
     category, fields, and Drop glue identity.
   - Make the independent verifier reject missing, duplicate, inconsistent,
     recursive, overflowing, or target-mismatched layout records.

18. **String, Array, and intrinsic NIR reference**
   - Add independently verified target-neutral operations and types for owned
     String, Array, indexing, mutation, iteration, and runtime intrinsics.
   - Require complete Rust reference lowering and fail-closed artifact tests.

19. **Record and Union NIR reference**
   - Add aggregate construction, projection, variant tags and payloads, switch
     lowering, layout references, and active-variant cleanup verification.
   - Prove complete Rust reference lowering before enabling candidate output.

20. **Function-value and closure NIR reference**
   - Add direct/indirect calls, function instances, environments, captures,
     and closure cleanup with deterministic identities.
   - Close all remaining Rust MIR-to-NIR `Deferred` states used by the compiler
     self-graph before the Futao builder depends on the vocabulary.

21. **Futao NIR builder**
   - Lower complete Futao MIR to target-neutral NIR and pass the independent
     Rust verifier.
   - Require deterministic canonical bytes across source insertion order,
     working directory, locale, and supported Hosts.

### Phase C: Integrate and Release

22. **Complete compiler adapter and corpus gate**
    - Replace the explicit `Unavailable` state with a real Futao adapter.
    - Compare all six canonical artifacts for accepted, rejected, fuzz-seed,
      Bootstrap Stdlib, compiler self-graph, and CLI E2E inputs.
    - Retain mismatching snapshots and reject every unclassified difference.

23. **Explicit candidate driver path**
    - Expose Futao and Rust compiler implementations through explicit driver
      selection while keeping Rust as the release default.
    - Make required CI and differential commands execute the Futao path; never
      silently fall back after candidate execution starts.

24. **`0.0.11` closure**
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
- add accepted and rejected tests before implementation behavior;
- branch from current `mvp`, target `mvp`, and squash merge each reviewed PR;
- delete merged topic branches locally and remotely.

Ask first:

- adding a third-party dependency or raising Rust 1.80;
- expanding Language 1.0 syntax or observable semantics;
- changing the public `.nexc` component lifecycle;
- removing the Rust reference/fallback before ADR-011 is complete.

Never:

- implement the Futao adapter by calling the Rust compiler;
- let Parser perform type checking or let code generation consume CST/AST;
- infer input from Host paths, environment, time, randomness, or hash order;
- widen a closed schema without a version change and rejected old/new tests;
- claim `0.1.0`, Stage 1, native code generation, or full self-hosting here.

## Success Criteria

The milestone is complete only when all of the following are proven:

1. The complete Futao type checker matches Rust for the full Language 1.0
   semantic corpus, including generics, match, immutable-owned facts, and
   capture diagnostics.
2. A real Futao compiler adapter is available and no candidate phase delegates
   semantic work or lowering to the Rust implementation.
3. The bootstrap manifest binds every compiler source, including semantic and
   lowering modules, and rejects undeclared or mutated source trees.
4. Tokens, CST, diagnostics, HIR, and MIR match canonically for every Language
   1.0 gate case; NIR matches for every reference-supported case, with no
   suppression list at either boundary.
5. Every supported Bootstrap Profile input, the Bootstrap Stdlib, and the
   compiler self-graph produce verified NIR with canonical layout identities
   instead of `Deferred` or fabricated output.
6. Closed generic instances are canonical and monomorphized deterministically;
   cleanup/Drop traces are complete and exactly once on every tested CFG path.
7. CLI E2E proves accepted output, rejected diagnostics, runtime failures,
   multi-file source locations, and explicit candidate/reference selection;
   the release manifest still records `rust-reference` as the default.
8. Repeated and reordered inputs are byte-for-byte deterministic; Rust 1.80,
   stable Rust, documentation, security, bootstrap, and release checks pass.
9. Version `0.0.11` and all bootstrap/source digests describe the exact merged
   sources; every delivery PR is squash merged to `mvp` and its topic branch is
   removed.

Only after these criteria pass may the implementation status change to
Implemented. Stage 1 remains the separate `0.0.12` milestone.
