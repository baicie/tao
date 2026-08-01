# Nexa

Nexa is a Rust bootstrap compiler workspace for the delivered
[Nexa Language 1.0 Reference Core](docs/spec/language-1.0.md): a
TypeScript-shaped language with independently specified native semantics.

Language 1.0 is a statically checked, deterministic, multi-file command-line
language executed by the CFG MIR reference interpreter. Its versioned
milestones and final integration gates are recorded in the
[1.0 roadmap](docs/project/roadmap/language-1.0.md) and
[delivery archive](docs/project/archive/language-1.0-delivery.md).

[Language Core v0.8](docs/spec/language-core-v0.8.md) completed the practical
language surface with exact function types, named function values, lexical
closures, array `for...of`, immutable `append`/`concat`, Unicode-scalar string
length, and explicit integer and string conversion. The delivered
[v0.9 stabilization contract](docs/spec/language-core-v0.9.md) freezes and
hardens that surface without adding syntax.

## Layout

```txt
crates/
  nexa_span/         # FileId, TextRange, and source spans
  nexa_source/       # Source file ownership and line/column lookup
  nexa_diagnostics/  # Structured errors, warnings, and source labels
  nexa_syntax/       # Tokens, SyntaxKind, and lossless CST support
  nexa_parser/       # Parser entry points and recovery diagnostics
  nexa_hir/          # HIR lowering, name resolution, and type checking
  nexa_mir/          # MIR lowering and interpreter
  nexa_compiler/     # Compiler driver
  nexac/              # CLI entry point
xtask/               # Repository automation commands
docs/spec/           # Language notes and accepted design decisions
crates/nexac/tests/  # CLI integration tests
```

## Dependency Direction

```txt
nexac -> nexa_source -> nexa_span
  |
  \-> nexa_compiler -> nexa_source
                     +-> nexa_parser -> nexa_syntax -> nexa_span
                     |                \-> nexa_diagnostics -> nexa_span
                     +-> nexa_hir -> nexa_syntax
                     |             \-> nexa_diagnostics -> nexa_span
                     \-> nexa_mir -> nexa_hir
```

## Commands

```bash
cargo xtask check
cargo run -p nexac -- check examples/practical-core/main.nexa
cargo run -p nexac -- run examples/practical-core/main.nexa -- 20
cargo run -p nexac -- parse examples/practical-core/main.nexa
```

[Language Core v0.8](docs/spec/language-core-v0.8.md) builds on immutable UTF-8
strings, homogeneous arrays, nominal records, tagged unions, exhaustive
matching, deterministic multi-file modules, and bounded generics. Function
values and closures lower through typed HIR and CFG MIR with stable identities
and immutable capture snapshots rather than JavaScript runtime objects.
Nexa intentionally borrows familiar TypeScript surface syntax without
accepting TypeScript or JavaScript compatibility as a goal. A future TypeScript
interop layer, if needed, belongs in an isolated adapter crate and must lower
into Nexa HIR without leaking a third-party AST or JavaScript runtime semantics
into the core compiler.

Language 1.0 deliberately excludes native AOT, UI, package management,
exceptions, and full TypeScript compatibility. v0.9 adds conformance,
determinism, recovery, fuzz-smoke, stress, and release-baseline coverage without
expanding the delivered language syntax.
