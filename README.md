# Nexa

Nexa is a Rust bootstrap compiler workspace for Language Core v0.6: a
TypeScript-shaped language with independently specified native semantics.

Language Core v0.6 is delivered. Development continues toward the
[Nexa Language 1.0 Reference Core](docs/spec/language-1.0.md): a statically
checked, multi-file command-line language executed by the CFG MIR reference
interpreter. The [1.0 roadmap](docs/project/roadmap/language-1.0.md) divides
that work into independently testable language milestones.

The current delivered specification is
[Language Core v0.6](docs/spec/language-core-v0.6.md), which adds explicit
multi-file modules, private-by-default exports, canonical source loading, and
module-aware identities. The active implementation milestone is v0.7: bounded
generics and ordinary `Option`/`Result` unions, tracked in the
[1.0 roadmap](docs/project/roadmap/language-1.0.md).

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
cargo run -p nexac -- check examples/modules/main.nexa
cargo run -p nexac -- run examples/modules/main.nexa
cargo run -p nexac -- parse examples/tagged_unions.nexa
```

[Language Core v0.6](docs/spec/language-core-v0.6.md) builds on immutable UTF-8
strings, homogeneous arrays, nominal records, tagged unions, and exhaustive
matching with deterministic multi-file modules. Nexa intentionally borrows
familiar TypeScript surface syntax without accepting TypeScript or JavaScript
compatibility as a goal. A future TypeScript interop layer, if needed, belongs
in an isolated adapter crate and must lower into Nexa HIR without leaking a
third-party AST or JavaScript runtime semantics into the core compiler.

The 1.0 target deliberately excludes native AOT, UI, package management,
exceptions, and full TypeScript compatibility. Statically typed function values
and lexical closures are part of the v0.8 milestone and must lower through HIR
and MIR without importing JavaScript runtime semantics.
