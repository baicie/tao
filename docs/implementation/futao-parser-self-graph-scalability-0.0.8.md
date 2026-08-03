# Futao Parser Self-Graph Scalability Prerequisite

Status: implemented on the `0.0.8` parser differential baseline.

## Context

ADR-000 Phase B5 requires the resolver to process the bootstrap compiler's
complete module graph. Before this prerequisite, the Futao parser matched the
Rust parser for the bounded 512-byte differential matrix but could not parse
its own `parser.ft`: immutable token and CST-event appends were quadratic, and
multiple recursive range runners retained more than the runtime limit of 64
active calls.

The parser baseline has nine sorted `.ft` sources, 122,588 source bytes, and a
100,770-byte `parser.ft` containing 228 top-level type/function items. The
resolver slice keeps this parser gate at the 0.0.8 source boundary; its own
resolver sources are currently parsed by the Rust Stage 0 reference compiler.
A
small-corpus differential alone is therefore insufficient evidence for Phase
B5.

## Decision

`sequence.ft` defines a persistent `Sequence<T>` with a 128-element tail and
completed chunks. Lexer tokens/diagnostics and parser CST events, recovery
events, and diagnostics append to this structure. Materialization concatenates
completed chunks with balanced traversal. Parser deferred-start ordering stays
unchanged, so precedence insertions retain the schema 1 event stream exactly.

Parser traversal keeps the Language 1.0 runtime limit of 64 active calls. The
source-item, expression-machine, pattern-binding, arrow-scan, and lookahead
range runners use fixed sequential leaf batches before or inside their balanced
fallbacks. Type parsing first executes a bounded 16-step local prefix. Trivia
first scans at most four nearby tokens. These batches execute the same state
transitions in the same order and do not enlarge parser step budgets.

Two distinct tool ceilings remain explicit:

* the 512-byte differential and mutation boundary uses 2,000,000 MIR basic-block
  steps per source;
* the self-graph gate uses 64,000,000 steps per source and the unchanged 64-call
  runtime limit.

Calibration on the largest source found that 36,000,000 steps fail and
40,000,000 pass. The frozen 64,000,000 ceiling leaves growth margin while being
eight times smaller than the temporary 512,000,000 diagnostic ceiling. The
Language 1.0 runtime default remains 100,000 steps; these are private tool
budgets only.

Runtime adapter failures retain the structured `RuntimeFailure` and resolve its
`SourceSpan` through the compiler `SourceMap`. A limit failure therefore names
the Futao source path, line, column, file ID, and byte range instead of losing
the failing call site.

## Self-Graph Gate

The integration gate compiles one real Futao parser adapter and parses every
source below `bootstrap/compiler/src` in manifest order:

```text
lexer.ft
lexer_bridge.ft
lexer_driver.ft
lexer_profile.ft
parser.ft
parser_bridge.ft
parser_driver.ft
parser_profile.ft
sequence.ft
```

Every source must finish within its resource ceiling, match the Rust parser's
complete schema 1 snapshot, and produce no parser diagnostic or recovery event.
The test asserts that its embedded identities exactly equal the manifest
`sourceFiles`; the manifest source list and length-delimited SHA-256 tree digest
therefore bind the same sorted graph. This is a parser scalability prerequisite
only: it does not claim that the `0.0.9` resolver exists yet.

## Preserved Observables

The change must retain all `0.0.7` and `0.0.8` contracts:

* token kind, UTF-8 range, trivia, and lexical diagnostics;
* complete balanced lossless CST events;
* deferred start-node ordering and generic `>` splitting;
* recovery regions and parser diagnostic ordering; and
* the 2,000,000-step adversarial 512-byte matrix.

No parser code performs name resolution or type checking, and the Rust parser
remains the default implementation.

## Rejected Alternatives

* Raising the runtime call-depth limit would change Language 1.0 behavior and
  hide the compiler's recursive composition problem.
* Raising only the step ceiling cannot fix call-depth exhaustion or quadratic
  immutable appends.
* Continuing direct immutable-array append makes full-source work scale
  quadratically.
* An AVL-style persistent buffer made append recursive and exceeded the same
  64-call limit on the existing 512-byte `max-record` case.
* Special-casing `parser.ft` would not prove the complete sorted source graph
  required by Phase B5.

## Verification

```bash
cargo test --locked -p nexa_compiler --test lexer_differential
cargo test --locked -p nexa_compiler --test parser_differential \
  -- --skip futao_parser_can_process_its_sorted_full_source_graph_for_the_resolver
cargo test --release --locked -p nexa_compiler \
  --test parser_differential \
  futao_parser_can_process_its_sorted_full_source_graph_for_the_resolver \
  -- --exact
cargo xtask lexer-differential
cargo xtask parser-differential
cargo xtask bootstrap-contract
make check
```

The next milestone may now build `0.0.9` Resolver differential tests on a parser
that can process the actual compiler source graph. Resolver semantics, symbol
identity, local scope, visibility, and cycle behavior remain separate work.
