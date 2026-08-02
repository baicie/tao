# Futao 0.0.7 Lexer Differential Contract

## Status and Boundary

Toolchain `0.0.7` delivers the first executable Futao-written compiler phase.
It freezes and compares only the lossless lexer observables required by the
self-hosting roadmap:

* numeric token kind;
* half-open UTF-8 byte range;
* trivia classification and ordering; and
* lexical diagnostic code, severity, label style, and byte range.

The Rust lexer remains the reference implementation. The Futao lexer is a real
`futao-bootstrap-v1` implementation executed through the current Rust
front-end, MIR lowering, and reference interpreter. It is not a Rust
reimplementation behind a Futao adapter.

This milestone does not migrate parsing, recovery, CST construction, name
resolution, type checking, lowering, or the compiler driver. The compiler-wide
Futao adapter therefore remains unavailable until its later vertical slices
exist. `0.0.7` adds a lexer-specific differential gate rather than fabricating
the missing phases.

## Rust Observable Contract

`nexa_parser::lex_source` is the frozen Rust phase entry point. It accepts a
stable `FileId` and one valid UTF-8 source string and returns:

```text
LexedSource
  tokens: ordered lossless Token values
  diagnostics: ordered lexical Diagnostic values
```

Every non-empty input byte belongs to exactly one token. Tokens are contiguous,
non-overlapping, ordered by source position, and end at the input byte length.
Every token boundary is a UTF-8 boundary. Empty input produces no token and no
diagnostic.

Token kinds use the frozen numeric `SyntaxKind` representation already present
in canonical dump schema 1. Whitespace and line comments are trivia; no other
kind is trivia. Token text remains lossless Rust output, but the differential
snapshot does not transmit it separately: the exact text is uniquely recovered
from the shared input and the validated byte range.

Every `Unknown` token produces exactly one lexical diagnostic:

```text
code:       E1001
severity:   error
labelStyle: primary
range:      the complete Unknown token range
```

Presentation text is excluded under ADR-000. Parser recovery diagnostics may
also use `E1001`, but they are not part of this phase result and cannot enter the
lexer snapshot.

## Source Semantics

Lexical classification follows the Rust reference over Unicode scalar values
with UTF-8 byte offsets:

* identifiers and decimal integers are ASCII-only;
* ASCII whitespace is `U+0009`, `U+000A`, `U+000C`, `U+000D`, or `U+0020`;
* line comments begin with `//` and stop before line feed;
* strings preserve their complete spelling and allow only `\"`, `\\`, `\n`,
  `\r`, and `\t` escapes;
* an invalid escape makes the complete terminated string one `Unknown` token;
* an unterminated string stops before CR/LF or at end of input;
* punctuation and paired operators use longest valid match except that `==`,
  `!=`, and `!==` remain one `Unknown` token as frozen by Language 1.0; and
* every otherwise unrecognized Unicode scalar is one `Unknown` token covering
  all of that scalar's UTF-8 bytes.

Keyword recognition is exact and ASCII case-sensitive. Contextual `of` remains
an identifier. No parser context may alter a token kind.

## Futao Core and Host Bridge

The pure Futao entry point receives an explicit decoded source table:

```text
LexerInput
  codePoints:  Unicode scalar values in source order
  byteOffsets: UTF-8 start offset for every scalar plus the final byte length
  byteLength:  exact UTF-8 byte length
```

This representation exists because Bootstrap Profile v1 has no string iterator
or byte-decoding intrinsic. The Rust Host validates the original UTF-8 string
and constructs the table deterministically; the lexer performs no filesystem,
environment, locale, clock, random, network, or process access. ADR-005 owns a
future general Host ABI, so this private differential bridge is not a stable
language API.

The Futao lexer uses immutable state and bounded divide-and-conquer traversal.
Leaf scans are bounded and the recursion depth grows logarithmically with input
size, keeping ordinary compiler sources below the reference interpreter's
active-call limit without weakening the Bootstrap Profile.

A separate application-profile driver serializes one result through the
reference interpreter's test output channel. Its line protocol is versioned,
length-delimited by record counts, parsed strictly, and rejects missing or extra
fields. `print` belongs only to this Host/test adapter; the lexer and input
decoder must independently pass `futao-bootstrap-v1` validation, including
`E6203` ambient-output rejection.

## Differential Snapshot

Both adapters produce the same schema 1 logical value:

```text
LexerSnapshot
  schemaVersion: 1
  tokens[]:
    kindId: u16
    start: u32
    end: u32
    trivia: bool
  diagnostics[]:
    code: String
    severity: error | warning
    labelStyle: primary | secondary
    start: u32
    end: u32
```

The harness validates each snapshot before comparison. Invalid coverage,
non-boundary offsets, unknown token kinds, inconsistent trivia, diagnostic
ordering, or a missing `Unknown` diagnostic is an adapter failure, not a
classifiable language difference.

Valid snapshots compare structurally and byte-for-byte after canonical JSON
serialization. A difference records its observable category and the SHA-256
digest of both complete snapshots. Every difference begins unclassified and
fails the gate. `0.0.7` is accepted only with zero differences; it defines no
suppression or expected-divergence list.

## Corpus and Artifact Retention

The gate uses stable case IDs and runs all of these inputs through both real
adapters:

* accepted fixtures covering every keyword, punctuation/operator, whitespace,
  comment, valid escape, ASCII identifier/integer boundary, and UTF-8 string;
* rejected fixtures covering unknown scalars, partial operators, invalid
  escapes, and CR/LF/EOF unterminated strings; and
* checked-in lexer fuzz seeds covering empty input, boundary transitions,
  Unicode byte widths, and inputs longer than one leaf scan.

Corpus discovery is lexical path order. Duplicate case IDs, unreadable files,
invalid UTF-8 fixtures, adapter failures, and an empty corpus fail closed.

On mismatch, `cargo xtask lexer-differential` writes the original source and
both canonical snapshots below `target/lexer-differential/<case-id>/`. These
artifacts are diagnostic output and are not committed. A successful run removes
no user data and reports the exact number of compared cases.

## Default-Path Boundary

The Rust lexer remains the production/default lexer in `0.0.7`. The Futao lexer
becomes mandatory in local, CI, and release differential gates, but it cannot
replace `nexa_parser::lex_source` until the parser and later compiler slices can
consume it without a Host serialization round trip.

This is a deliberate distinction between an executable self-hosted phase and a
default compiler path. `0.0.8` may build the Futao parser on this verified phase;
it must not bypass or reinterpret the lexer snapshot.

## Content Addressing

`bootstrap/compiler/bootstrap-compiler.json` pins the four Futao source files,
their roles, the implemented `lexer` phase, snapshot schema 1, the 11-case
corpus, and the Rust-reference default. The source tree uses length-delimited
portable paths and bytes under the `FUTAO-BOOTSTRAP-COMPILER` domain separator:

```text
sha256:565a901ae34c25251aad8db4761f4d8207561d28652f93c28841ea29deec5717
```

The Stage 0 manifest repeats this digest and fails when either manifest or any
source byte drifts. Test corpus files are versioned inputs to the differential
gate but are deliberately outside this compiler-source digest.

## Validation and Success Criteria

Milestone-specific validation is:

```bash
cargo test --locked -p nexa_parser
cargo test --locked -p nexa_compiler --test lexer_differential
cargo test --locked -p xtask --all-targets --all-features
cargo xtask lexer-differential
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

`0.0.7` succeeds only when the Futao core passes Bootstrap Profile v1, all
accepted/rejected/fuzz-seed cases execute both implementations, all snapshots
satisfy the structural invariants, no differences remain, and every repository
gate passes.

## Deferred Work

The following remain explicit non-goals:

* lossless CST, parser recovery events, and parse diagnostics (`0.0.8`);
* module graph, symbols, scope, and visibility (`0.0.9`);
* type inference, generic substitution, ownership, and match checking (`0.0.10`);
* HIR/MIR/NIR and compiler-driver equivalence (`0.0.11`);
* C1/C2/C3 construction and normalized fixed-point comparison (`0.0.12` onward);
* a public lexer protocol, stable compiler plugin API, or general Host ABI; and
* switching the default compiler implementation before ADR-011 is proven.
