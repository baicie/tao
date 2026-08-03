# Futao 0.0.10 Type Checker Differential: Expression Kernel

## Status

Implemented and contract-closed as the first `0.0.10` slice on top of the
`0.0.9` resolver baseline. The Rust type checker remains the reference and
production implementation.

## Scope

This slice freezes a small semantic input contract rather than teaching the
type checker to read CST events. Host code supplies a resolved, source-spanned
expression table in canonical node order. Children must precede their parent,
node fields must match the declared kind, and at most 1024 nodes are accepted.
These rules keep the Futao implementation deterministic and independent of
parser tokens, syntax trees, filesystem state, and hash-map iteration.

Supported expression nodes are integer, boolean, string, and unit literals;
integer negation; boolean negation; integer addition; scalar equality;
boolean conjunction/disjunction; conditional expressions; and return checks.
The observable result contains one inferred type per node and source-aware
diagnostics `E3001` (type mismatch), `E3002` (non-boolean condition), and
`E3003` (invalid return type).

Generic substitution, function signatures, match exhaustiveness, ownership,
and mutable-capture checks remain later `0.0.10` slices. This boundary is
intentional: it proves the type-checker protocol and inference kernel without
coupling semantic checking to the parser or lowering implementation.

## Protocol

The application driver emits `FUTAO-TYPECHECK-1`, followed by `ok` and:

```text
schemaVersion
nodeCount
nodeTypeTag * nodeCount
diagnosticCount
diagnosticCode source start end * diagnosticCount
```

Type tags are `int`, `bool`, `string`, `unit`, or `unknown`. Every diagnostic
has one primary source span. The output node count must equal the input node
count; diagnostic counts are bounded to 2048 because one conditional can emit
both a condition and branch diagnostic. Diagnostics are ordered by
`(source, start, end, code)`. Malformed input, invalid node shapes, unknown
node/type/diagnostic tags, forward child references, out-of-range spans,
trailing fields, inverse diagnostic order, and malformed output fail closed in
the Rust adapter. Regression tests cover 1025 input nodes, the exact 2048
diagnostic boundary and its 2049 rejection, wrong schema versions, unknown
output tags, invalid diagnostic spans, and missing or trailing protocol fields.

## Acceptance and Rejection Corpus

The accepted fixture covers all 12 expression node kinds. The rejected fixture
covers invalid unary, equality, boolean, conditional, and return checks. Each
fixture carries an explicit semantic oracle for inferred types and diagnostics;
Rust and Futao adapters must also produce byte-for-byte equivalent canonical
snapshots for every fixture. Fixture JSON denies unknown fields at every level,
and the loader validates decoded optional values against the declared node kind
before constructing a typed node, so typed constructors cannot erase non-null
kind-incompatible values.

## Verification

```bash
cargo test --locked -p nexa_compiler --test typecheck_differential
cargo test --locked -p nexa_compiler typecheck_differential::tests
cargo test --locked -p xtask typecheck_differential::tests
cargo xtask typecheck-differential
```

`make check` and the release gate include this command once the slice is
merged. The Rust implementation remains the default compiler path.

## Deferred Work

The next slices extend the same schema with inferred generic arguments,
function/record/union signatures, match coverage, and ownership facts before
any default-path migration.
