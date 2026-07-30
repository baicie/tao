# Nexa

Nexa is a Rust bootstrap compiler workspace for Language Core v0.7: a
TypeScript-shaped language with independently specified native semantics.

Language Core v0.7 is delivered. Development continues toward the
[Nexa Language 1.0 Reference Core](docs/spec/language-1.0.md): a statically
checked, multi-file command-line language executed by the CFG MIR reference
interpreter. The [1.0 roadmap](docs/project/roadmap/language-1.0.md) divides
that work into independently testable language milestones.

The current delivered specification is
[Language Core v0.7](docs/spec/language-core-v0.7.md), which adds bounded
generic functions and nominal data, local type-argument inference, ordinary
source-defined `Option`/`Result` unions, and deterministic generic-instance
limits. The active implementation milestone is v0.8: function values,
closures, iteration, and practical immutable-data operations, tracked in the
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
cargo run -p nexac -- check examples/generic-modules/main.nexa
cargo run -p nexac -- run examples/generic-modules/main.nexa
cargo run -p nexac -- parse examples/generics.nexa
```

[Language Core v0.7](docs/spec/language-core-v0.7.md) builds on immutable UTF-8
strings, homogeneous arrays, nominal records, tagged unions, exhaustive
matching, and deterministic multi-file modules. It adds bounded generic
functions, records, and unions while keeping type arguments out of the runtime.
Nexa intentionally borrows familiar TypeScript surface syntax without
accepting TypeScript or JavaScript compatibility as a goal. A future TypeScript
interop layer, if needed, belongs in an isolated adapter crate and must lower
into Nexa HIR without leaking a third-party AST or JavaScript runtime semantics
into the core compiler.

The 1.0 target deliberately excludes native AOT, UI, package management,
exceptions, and full TypeScript compatibility. Statically typed function values
and lexical closures are the active v0.8 milestone and must lower through HIR
and MIR without importing JavaScript runtime semantics.
