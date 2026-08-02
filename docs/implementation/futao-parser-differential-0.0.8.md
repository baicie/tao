# Futao 0.0.8 Parser Differential Contract

## Status and Objective

Toolchain `0.0.8` delivers the second executable Futao-written compiler phase.
It consumes the verified Futao lexer from `0.0.7` and compares the parser
observables required by the self-hosting roadmap:

* the complete lossless CST event stream;
* ordered recovery regions represented by `Error` nodes; and
* parser-only diagnostic code, severity, label style, and UTF-8 byte range.

The Rust parser remains the reference implementation. The Futao parser must be
a real `futao-bootstrap-v1` implementation executed through the current Rust
front-end, MIR lowering, and reference interpreter. A Rust parser hidden behind
a Futao adapter does not satisfy this milestone.

## Assumptions and Boundary

This contract follows the accepted self-hosting plan and makes these assumptions
explicit:

* Language 1.0 syntax and recovery behavior remain frozen; `0.0.8` adds no
  language feature.
* The `0.0.7` numeric token kinds, UTF-8 ranges, trivia ordering, and lexical
  diagnostics are inputs, not decisions the parser may reinterpret.
* Parser diagnostics can be separated from lexical diagnostics without changing
  the combined ordering returned by the existing `parse_source` API.
* Bootstrap Profile v1 remains pure and immutable. The parser may use recursion,
  immutable arrays, records, tagged unions, and deterministic array order, but
  no denied syntax or Host capability.
* Differential mutation input is valid UTF-8 and at most 512 bytes. Checked-in
  conformance cases may be larger when they stay within the explicit execution
  budget.

This milestone does not migrate module loading, name resolution, type checking,
HIR/MIR/NIR lowering, or the compiler driver. It does not change the default
Rust compiler path.

## Rust Observable Contract

`nexa_parser::parse_source` remains the production parser entry point. The
returned `Parse` value additionally exposes parser-only diagnostics so the phase
adapter does not depend on presentation text to remove lexer diagnostics.
`Parse::diagnostics` continues to return the existing combined, source-ordered
lexer and parser diagnostics.

The Rust parser snapshot adapter accepts one stable `FileId` and one valid UTF-8
source string and returns schema 1:

```text
ParserSnapshot
  schemaVersion: 1
  cstEvents: ordered CstEvent values
  recoveryEvents: ordered RecoveryEvent values
  diagnostics: ordered parser-only Diagnostic values
```

The adapter must call the ordinary Rust lexer and parser. It may canonicalize
their public results, but it may not implement an alternate grammar or recovery
algorithm.

## Lossless CST Events

The CST is serialized as a balanced event stream. This avoids exposing Rowan
internals while preserving exact hierarchy, token splitting, empty nodes, and
UTF-8 offsets:

```text
CstEvent
  StartNode
    kindId: u16
    offset: u32
  Token
    kindId: u16
    start: u32
    end: u32
  FinishNode
    offset: u32
```

`StartNode` and `FinishNode` delimit every CST node, including the root and empty
nodes. `Token` events include trivia and any parser-level split of a lexer token,
such as `>=` becoming `>` plus `=` while closing a generic list. Token text is
not transmitted separately because it is recovered exactly from the shared
source and validated byte ranges.

A valid stream has exactly one `SourceFile` root, balanced node events, known
node/token kinds, monotonically increasing offsets, UTF-8 boundaries, and token
events that reproduce every source byte exactly once. A node opens and closes at
the current lossless cursor. A token must be nested below the root and cannot
cross a node boundary.

## Recovery Events

The stable recovery observable is the ordered range of every concrete
`SyntaxKind::Error` node in the lossless CST:

```text
RecoveryEvent
  start: u32
  end: u32
```

The parser currently performs no synthetic-token insertion. A missing token is
therefore observable through its parser diagnostic and any empty CST node, while
tokens skipped to a recovery boundary are observable through an `Error` node.
Internal loop iterations, lookahead, recovery-set storage, and helper-call order
are deliberately excluded so the parser can be refactored without changing the
contract.

The snapshot validator derives recovery ranges from `cstEvents` and requires an
exact match with `recoveryEvents`. Empty recovery regions are invalid.

## Parse Diagnostics

Only diagnostics emitted by parser grammar and recovery logic enter this phase
snapshot. Lexical diagnostics remain owned and compared by the `0.0.7` lexer
gate. Every parser diagnostic has this logical shape:

```text
ParserDiagnostic
  code: String
  severity: error | warning
  labelStyle: primary | secondary
  start: u32
  end: u32
```

Presentation text is excluded under ADR-000. Diagnostics are ordered by primary
label start and end exactly as the production parser returns them. A diagnostic
must have one in-bounds UTF-8 label for the requested `FileId`; unknown codes,
severities, styles, missing labels, and additional labels fail closed.

## Futao Parser Core

The pure Futao parser receives the original decoded source table and calls the
verified Futao lexer directly:

```text
ParserInput
  codePoints: Unicode scalar values in source order
  byteOffsets: UTF-8 start offset for every scalar plus final byte length
  byteLength: exact UTF-8 byte length

ParserResult
  cstEvents: CstEvent[]
  recoveryEvents: RecoveryEvent[]
  diagnostics: ParserDiagnostic[]
```

Parser state is explicit and immutable: token position, pending generic `=`, CST
events, diagnostics, and recovery output travel together. Node checkpoints are
event-array indices. Wrapping an already parsed left operand inserts a
`StartNode` at that checkpoint, preserving the Rust parser's precedence and
postfix tree shape without a mutable tree builder.

The parser implements only syntax decisions. It must not resolve names, validate
loop context, check types, infer generics, inspect modules, or lower IR.

## Host Bridge and Protocol

The Rust Host validates the original UTF-8 string and supplies the same scalar
and byte-offset encoding used by `0.0.7`. The pure parser profile entry imports
the real Futao lexer and parser and must pass `futao-bootstrap-v1`, including the
ambient-output rejection.

A separate application-profile driver serializes one result through the
reference interpreter's test output channel. Its line protocol is versioned,
count-delimited, and strict. It rejects unknown tags, kinds, severities, styles,
missing fields, extra fields, invalid counts, and trailing output. `print` is
confined to this Host/test driver and is not a compiler-core capability.

Both adapters validate their snapshots before comparison. Invalid snapshots are
adapter failures, not language differences.

## Differential Classification

Adapters compare the same schema 1 logical value. Differences are classified as:

* `cst` for hierarchy, node/token kind, split token, range, or ordering;
* `recovery` for an `Error`-node recovery range; and
* `diagnostics` for parser-only diagnostic structure or ordering.

Every difference begins unclassified and fails the gate. No suppression or
expected-divergence list exists. Reports retain the SHA-256 digest of both
complete canonical snapshots.

## Corpus and Fuzzing

The checked-in `.ft` corpus contains stable IDs in three categories:

* accepted cases covering empty input, declarations and modules, statements and
  control flow, types and generics, records and unions, expressions and
  precedence, closures, arrays, trivia, and UTF-8;
* rejected cases covering missing and mismatched delimiters, malformed lists,
  invalid top-level items, expression recovery, generic `>=` splitting, record
  and union recovery, match-arm recovery, and continued parsing after errors;
* parser fuzz seeds covering empty input, nested precedence/postfix forms,
  recovery boundaries, and generic close-token transitions.

Corpus discovery uses lexical path order and fails closed on duplicate IDs,
non-files, non-`.ft` entries, unreadable input, invalid UTF-8, empty categories,
or an empty total corpus.

The parser fuzz target compares both real adapters for valid UTF-8 inputs up to
512 bytes. It also retains the existing Rust lossless/determinism invariants.
Adapter errors, invalid snapshots, or any differential mismatch are crashes.
CI runs a bounded 256-iteration smoke; longer mutation campaigns remain a
maintainer task.

## Artifact Retention

On mismatch, `cargo xtask parser-differential` writes the original source and
both canonical snapshots below:

```text
target/parser-differential/<case-id>/
```

Artifacts are diagnostic output and are not committed. A successful run removes
no user data and reports the exact number of compared cases.

## Project Structure

```text
bootstrap/compiler/src/parser.ft
  pure parser and canonical event builder
bootstrap/compiler/src/parser_bridge.ft
  explicit source decoding for the private Host adapter
bootstrap/compiler/src/parser_profile.ft
  Bootstrap Profile validation entry
bootstrap/compiler/src/parser_driver.ft
  application-only line-protocol driver
bootstrap/compiler/tests/parser/{accepted,rejected}
  stable differential fixtures
crates/nexa_compiler/src/parser_differential.rs
  Rust/Futao adapters, schema validation, comparison, and reports
crates/nexa_compiler/tests/parser_differential.rs
  contract and real-adapter integration tests
xtask/src/parser_differential.rs
  checked-in corpus gate and mismatch retention
fuzz/fuzz_targets/parser.rs
  bounded real-adapter differential fuzz target
fuzz/corpus/parser
  stable parser mutation seeds
```

No new crate or dependency is required. Parser logic stays in the parser phase;
the compiler crate owns only differential orchestration.

## Code and Test Discipline

Implementation follows these constraints:

* write accepted and rejected tests before each behavior slice;
* keep Rust front-end crates under `#![forbid(unsafe_code)]`;
* use structured library errors and `anyhow` only in `xtask`;
* validate external protocol data once at the adapter boundary;
* keep source order deterministic and avoid unordered iteration;
* keep every commit independently buildable and verifiable; and
* make no unrelated parser refactor or language change.

Milestone-specific validation is:

```bash
cargo test --locked -p nexa_parser
cargo test --locked -p nexa_compiler --test parser_differential
cargo test --locked -p xtask --all-targets --all-features
cargo xtask lexer-differential
cargo xtask parser-differential
cargo +1.80.0 check --locked --manifest-path fuzz/Cargo.toml
```

Repository release validation remains:

```bash
make check
cargo xtask bootstrap-contract
cargo xtask bootstrap-contract --rebuild-stage0
cargo xtask security
cargo xtask release-check
pnpm --dir docs build
```

## Content Addressing

`bootstrap/compiler/bootstrap-compiler.json` adds the four parser source files,
the implemented `parser` phase, parser snapshot schema 1, and exact corpus
counts. The compiler source-tree digest continues to cover every sorted `.ft`
file below `bootstrap/compiler/src` with the existing length-delimited
`FUTAO-BOOTSTRAP-COMPILER` domain. The Stage 0 manifest repeats the digest and
fails when the manifest or any source byte drifts.

Corpus files remain versioned gate inputs outside the compiler-source digest.

## Default Path, Rollback, and Compatibility

The Rust parser remains the production/default parser in `0.0.8`. The Futao
parser is mandatory in local, CI, fuzz, and release differential gates, but it
cannot become the default compiler path before resolver, type checker, lowering,
driver, and ADR-011 fixed-point stages are complete.

Rollback is the squash revert of the `0.0.8` PR. Stage 0, the Rust parser, the
`0.0.7` lexer contract, and public source compatibility remain usable. Snapshot
schema 1 is internal and pre-`0.1.0`; an incompatible future change must increment
the schema and update fixtures atomically.

## Success Criteria

`0.0.8` succeeds only when:

1. parser-only diagnostics are exposed without changing existing combined
   diagnostic behavior;
2. both adapters emit valid, canonical schema 1 snapshots;
3. every accepted, rejected, and fuzz-seed case executes the real Rust and Futao
   lexer/parser chain with zero differences;
4. bounded mutation fuzzing compares the real adapters with no crash or mismatch;
5. Bootstrap Profile validation proves the parser core has no ambient Host
   capability;
6. compiler source and Stage 0 manifests bind the new source tree and corpus;
7. Rust 1.80, workspace, docs, security, clean Stage 0 rebuild, and release gates
   pass; and
8. the version advances to `0.0.8` while the default implementation remains
   `rust-reference`.

## Deferred Work

The following remain explicit non-goals:

* module graph, symbols, scope, and visibility (`0.0.9`);
* type inference, generic substitution, ownership, and match checking (`0.0.10`);
* HIR/MIR/NIR and compiler-driver equivalence (`0.0.11`);
* C1/C2/C3 construction and normalized fixed-point comparison (`0.0.12` onward);
* a public parser protocol, stable compiler plugin API, or general Host ABI; and
* switching the default compiler implementation before ADR-011 is proven.
