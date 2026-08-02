# Changelog

All notable changes to this project will be documented in this file.

## 0.0.7 - 2026-08-02

- Add the first executable Futao-written compiler phase: a pure, bounded lexer
  compiled under `futao-bootstrap-v1` with explicit Unicode scalar and UTF-8
  byte-offset inputs.
- Freeze lexer snapshot schema 1 for token kinds, byte ranges, trivia, and
  structured lexical diagnostics, with strict Rust and Futao adapters.
- Gate 4 accepted cases, 3 rejected cases, and 4 fuzz seeds at zero observable
  differences while retaining canonical mismatch artifacts for diagnosis.
- Content-address the Futao compiler source tree in a strict compiler manifest
  and bind its digest, implemented phase, corpus, and Rust-reference default
  into the Stage 0 contract.
- Add the lexer differential command to local, CI, MSRV, fuzz, bootstrap, and
  release validation while keeping the Rust lexer as the default path.

## 0.0.6 - 2026-08-02

- Freeze `futao-bootstrap-v1` as a `.ft`-only pure compiler profile with stable
  `E6201` through `E6203` diagnostics for mutation, unbounded loop control, and
  ambient output.
- Add Bootstrap Stdlib `0.0.1` with pure compiler data types, arrays, text,
  deterministic persistent maps/sets/bit sets, and generation-checked arenas.
- Keep core stdlib operations below the former linear-recursion failure point,
  with an 80-element reference-interpreter regression workload.
- Bind profile, stdlib source tree, and canonical build digests into the Stage
  0 manifest and canonical compiler dumps, with forward upgrade and rollback
  rejection fixtures.
- Require the profile gate in local/CI checks and prove the complete stdlib is
  accepted by rebuilt `nexac 0.0.1` through a parser-guided `.ft` compatibility
  projection.

## 0.0.5 - 2026-08-02

- Add typed target-neutral NIR construction with explicit functions, basic
  blocks, SSA values, source spans, operations, terminators, and intrinsics.
- Add an independent verifier for type, CFG, SSA, call, intrinsic, and owned
  value invariants plus checked 32/64-bit target layout calculation.
- Add compiler lowering for the scalar static-call N1 subset, with explicit
  `Deferred` results for later MIR features instead of fabricated NIR.
- Add a strict private `FUTAO-NIR` schema with exact compiler compatibility,
  canonical JSON bytes, SHA-256 integrity, accepted/rejected fixtures, and
  bootstrap/release gates while keeping `.nexc` separate.

## 0.0.4 - 2026-08-02

- Add the safe `nexa_storage` reference kernel for checked layout arithmetic,
  unforgeable allocator provenance, bounded owned storage, UTF-8 construction,
  generation-checked arenas, and deterministic compiler collections.
- Add a pure Place initialization/Copy/Move/Drop state machine with reverse
  successful-initialization cleanup and conservative control-flow joins.
- Cover ZST Drop, failed-growth atomicity, stale and foreign arena handles,
  collection bounds, ownership rejection, compile-fail lifetimes, a 20,000-entry
  release workload, Rust 1.80, and Miri CI.
- Keep target layouts, NIR verification, Box/Shared, Host ABI, backend lowering,
  and the complete ADR-004 conformance matrix deferred to their planned phases.

## 0.0.3 - 2026-08-02

- Accept `.ft` source entries and imports while preserving the `.nexa`
  Language 1.0 compatibility corpus and mixed-extension migration graphs.
- Add a Host-independent `CompilerInput -> CompilerOutput` API with validated
  logical source identities and structured diagnostics, typed HIR, and MIR.
- Add versioned canonical token, CST, diagnostic, HIR, MIR, and explicit
  unavailable-NIR outputs plus an honest Rust/Futao differential adapter.
- Add `nexac dump` and determinism coverage across input order, Host paths,
  working directories, locale, timezone, and repeated compiler executions.

## 0.0.2 - 2026-08-01

- Accept the mixed self-hosting route with target-neutral internal NIR as the
  Stage boundary and `.nexc` as a separate stable component lifecycle.
- Pin the Rust `nexac 0.0.1` Stage 0 source commit, archive and lockfile
  SHA-256 digests, Rust 1.80 toolchain, and locked release build recipe.
- Add `cargo xtask bootstrap-contract` validation and a release-gated clean
  Stage 0 rebuild with accepted and rejected manifest coverage.

## 0.0.1 - 2026-08-01

- Package the complete Nexa Language 1.0 Reference Core as the self-use
  `nexac` CLI.
- Keep Rust crate APIs and distribution pre-stable while preserving the
  Language 1.0 source, diagnostic, and runtime compatibility baseline.
