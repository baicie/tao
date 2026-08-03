# Changelog

## Compiler Package 0.0.10 (Futao Type Checker Kernel)

- Added the first executable Futao type-checker expression kernel for primitive
  inference, operators, conditionals, and return checks.
- Froze `FUTAO-TYPECHECK-1` schema 1 with strict accepted/rejected fixtures and
  source-aware `E3001` through `E3003` diagnostics.
- Closed the v1 protocol boundary: node shapes and candidate node counts are
  validated, up to 2048 bounded diagnostics are accepted, diagnostic order is
  canonical, unknown output tags fail closed, and corpus fixtures carry
  semantic oracles covering all 12 node kinds.
- Hardened the corpus and protocol gates: fixture JSON rejects unknown fields
  and non-null kind-incompatible values before typed construction, while
  regression tests freeze the 1024-node and exact 2048-diagnostic bounds,
  schema/tags, spans, field completeness, and backward-only child references.
- Kept generic substitution, match coverage, ownership, and the Rust default
  path deferred to later `0.0.10` slices.

## Compiler Package 0.0.9 (Futao Resolver Differential)

- Added the third executable Futao-written compiler phase: deterministic module
  graph construction and name resolution under Bootstrap Profile v1.
- Froze resolver snapshot schema 1 for module graph, symbols, scopes, bindings,
  resolved names, and source-aware diagnostics, including deterministic cycle
  witnesses.
- Compared one accepted graph, one rejected graph, and four resolver fuzz seeds
  through real Rust and Futao adapters with no suppression or expected divergence.
- Bound resolver source, protocol, schema, corpus counts, and tree digest into
  the Stage 0 bootstrap contract; malformed output and mismatches fail closed.
- Hardened resolver fail-closed behavior: unknown internal enum values now emit
  invalid tags, and snapshot validation rejects child identities or declaration
  spans absent from the lowered source-declaration contract.
- Kept the Rust resolver as the default path until type checking, lowering, and
  the ADR-011 self-hosting gate are complete.

## Compiler Package 0.0.8 (Futao Parser Differential)

- Added the pure Futao parser as the second executable compiler phase, consuming
  the verified Futao lexer under Bootstrap Profile v1.
- Froze parser snapshot schema 1 for balanced lossless CST events, concrete
  recovery regions, and parser-only structured diagnostics.
- Compared 8 accepted cases, 8 rejected cases, and 5 fuzz seeds through both real
  adapters with no accepted differences or suppression list.
- Bound all nine Futao compiler source files, the lexer/parser phase boundary,
  both snapshot schemas, and exact corpus counts into Stage 0 provenance.
- Added chunked persistent accumulation and a 64-million-step per-source release
  gate that parses the complete sorted bootstrap compiler source graph.
- Kept the Rust parser as the production path until the remaining compiler
  phases and ADR-011 fixed-point gate are complete.

## Compiler Package 0.0.7 (Futao Lexer Differential)

- Added the first real Futao-written compiler phase under
  `futao-bootstrap-v1`, with immutable bounded traversal over explicit Unicode
  scalar and UTF-8 byte-offset input.
- Froze a lexer-only snapshot schema for numeric token kinds, byte ranges,
  trivia, and structured lexical diagnostics without fabricating later compiler
  phases.
- Added strict Rust/Futao adapters, snapshot validation, fail-closed reports,
  and retained mismatch artifacts.
- Added 11 accepted, rejected, and fuzz-seed cases plus a bounded differential
  fuzz target and local, MSRV, CI, and release gates.
- Kept the Rust lexer as the default path until the Futao parser and remaining
  ADR-011 compiler slices are verified.

## Compiler Package 0.0.6 (Bootstrap Profile and Stdlib)

- Froze the `.ft`-only `futao-bootstrap-v1` capability surface and added stable
  profile diagnostics for mutation, unbounded loop control, and ambient output.
- Added Bootstrap Stdlib `0.0.1` with pure compiler data structures and stable
  insertion-order behavior, plus content-addressed source and canonical build
  manifests.
- Added an 80-element runtime workload proving core stdlib operations do not
  exhaust the reference interpreter's 64-active-call limit.
- Bound compilation profiles into canonical dumps and the profile/stdlib
  digests into the Stage 0 manifest, with forward-only version fixtures.
- Added local, CI, release, and rebuilt `nexac 0.0.1` compatibility gates for
  the checked-in stdlib.

## Compiler Package 0.0.5 (Typed NIR and Bootstrap Artifact)

- Added target-neutral typed NIR construction and an independent verifier for
  type, CFG, SSA, static-call, intrinsic, and owned-value invariants.
- Added explicit checked 32/64-bit target layout and deterministic scalar
  MIR-to-NIR lowering with honest `Deferred` results for later phases.
- Added the strict private `FUTAO-NIR` schema, exact compiler compatibility,
  SHA-256 integrity, canonical bytes, rejected fixtures, and bootstrap/release
  gates without making NIR public or changing the `.nexc` lifecycle.

## Compiler Package 0.0.4 (Ownership and Storage Kernel)

- Added `nexa_storage`, a safe Rust reference kernel for validated Host layouts,
  logical allocator provenance, bounded `StorageVec`/`StorageArray`, UTF-8
  construction, deterministic collections, interned symbols, and arenas.
- Added a pure Place state machine for Copy, Move, reinitialization, conservative
  joins, and reverse successful-initialization cleanup.
- Added accepted, rejected, compile-fail, ZST, failed-growth, stale-handle, MSRV,
  release workload, and Miri gates without claiming the deferred full ADR-004
  target-layout, Box/Shared, ABI, or cross-backend work.

## Compiler Package 0.0.3 (Differential Foundation)

- Added `.ft` entry and import support without removing historical `.nexa`
  fixtures; mixed graphs are retained as the explicit migration contract.
- Added the explicit in-memory compiler core boundary and versioned canonical
  phase dumps for tokens, CST, diagnostic structure, typed HIR, and full MIR.
- Recorded NIR and the Futao compiler adapter as unavailable instead of
  creating placeholder implementations, and made unclassified differences
  fail closed in the differential harness.
- Added `nexac dump` for reproducible cross-process comparison and deterministic
  accepted/rejected corpus coverage.

## Compiler Package 0.0.2 (Bootstrap Contract)

- Accepted ADR-000 after aligning self-hosting with the private NIR and stable
  component boundaries in ADR-003 and ADR-009.
- Added a machine-checked Stage 0 source manifest, SHA-256 provenance
  verification, and a clean Rust 1.80 rebuild gate for `nexac 0.0.1`.
- Reserved public distribution for `.nexc`; internal NIR remains private and
  is compared through canonical C2/C3 outputs.

## Compiler Package 0.0.1 (Self-use)

- Packaged the complete Language 1.0 Reference Core as `nexac 0.0.1`.
- Documented local installation while keeping Rust crate APIs and distribution
  intentionally pre-stable.

## Nexa Language 1.0 Reference Core (Delivered 2026-08-01)

- Integrated the delivered v0.1 through v0.9 contracts as the Language 1.0
  source, semantic, diagnostic, CLI, runtime, and conformance baseline.
- Passed the complete Rust 1.80 and stable quality gates, 31-case versioned
  conformance suite, parser robustness/fuzz smoke, deterministic compiler
  stress tests, release-mode performance workload, canonical CLI smoke, and
  documentation build.
- Archived the exact scope, validation evidence, and deferred non-goals in the
  [delivery record](project/archive/language-1.0-delivery.md).

## Language Core v0.9 (Delivered)

- Added the v0.9 stabilization contract and validation gates without changing
  the v0.8 language surface.
- Added the versioned 1.0 conformance runner, parser fuzz and recovery,
  deterministic stress, performance-baseline, and release-validation gates.
- Published the compatibility policy, complete diagnostic catalog, language
  guide, CLI guide, and release-candidate documentation boundary.

Every v0.9 delivery gate passed. The milestone adds no source syntax or runtime
behavior beyond v0.8.

## Language Core v0.8 (Delivered)

- Added exact function types, module-aware named function values, indirect
  calls, typed arrow functions, and deterministic immutable closure captures.
- Added array-only `for...of` with CFG lowering, nearest-loop control, and
  per-iteration capture snapshots.
- Added immutable array `append`/`concat`, Unicode-scalar `String.length`, and
  explicit `toString`/`parseInt` conversion with fixed runtime failures.
- Added stable `ClosureId` identities, closed-enum MIR call dispatch, shared
  call-depth and step budgets, defensive runtime validation, and structured
  failures for malformed programmatic MIR.
- Added stable `E3011` and `E3012` diagnostics plus parser, semantic, MIR,
  interpreter, and multi-file CLI coverage.

The subsequent v0.9 stabilization and Language 1.0 integration milestones are
delivered.

## Language Core v0.7 (Delivered)

- Added bounded generic functions, nominal records, and tagged unions with
  owner-scoped type-parameter identities and complete named type arguments.
- Added deterministic local inference for calls and constructors, regular
  generic recursion, and a session-wide limit of 256 unique closed instances.
- Added ordinary source-defined `Option<T>` and `Result<T, E>` unions without
  an implicit prelude, exception handling, or propagation semantics.
- Added definition-level CFG MIR erasure with validation of resolved generic
  facts, including cross-module generic construction, calls, and matching.
- Added stable `E3008`, `E3009`, and `E3010` diagnostics, accepted/rejected
  conformance coverage, and executable single-file and multi-file examples.

This milestone remains a supported predecessor of v0.8.

## Language Core v0.6 (Delivered)

- Added explicit named imports and private-by-default exports for functions,
  nominal records, and tagged unions across reachable `.nexa` files.
- Added canonical source providers, deterministic DFS module graphs, cached
  loads, cycle detection, complete cross-file source maps, and `E4001` through
  `E4005` diagnostics.
- Added module-owned definition and layout identities through typed HIR, CFG
  MIR, and the interpreter, including resolved entry selection and diamond
  identity preservation.
- Added file-system and in-memory provider tests, multi-file CLI fixtures, and
  the executable `examples/modules/main.nexa` four-module example.

This milestone remains a supported predecessor of v0.7.

## Language Core v0.5 (Delivered)

- Added nominal tagged union declarations, qualified constructors, positional
  payloads, and exhaustive `match` expressions.
- Added stable `UnionId`, `VariantId`, and `PayloadId` identities through typed
  HIR and CFG MIR, including dense resolved tag dispatch.
- Added guarded recursive nominal types and a deterministic recursive-union
  runtime depth limit of 1024.
- Added `E3006` non-exhaustive-match and `E3007` unreachable-arm diagnostics,
  parser recovery, CLI fixtures, malformed-MIR checks, and the executable
  `examples/tagged_unions.nexa` example.

This milestone remains a supported predecessor of v0.6.

## Language Core v0.4 (Delivered)

- Added nominal immutable record declarations, contextually typed record
  literals, exact field validation, and immutable field access.
- Added stable `RecordId` and `FieldId` identities through typed HIR and CFG
  MIR, with resolved record layouts in the reference interpreter.
- Added diagnostics for unknown and missing fields, invalid record operations,
  and direct or indirect recursive record declarations.
- Added accepted and rejected parser, semantic, MIR, interpreter, compiler,
  and CLI coverage plus the `examples/named_records.nexa` example.

This milestone remains a supported predecessor of v0.5. Release notes will be
versioned further when Nexa has a stable release workflow.
