# Nexa

Nexa is a Rust bootstrap compiler workspace for Language Core v0.1: a
TypeScript-shaped language with independently specified native semantics.

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
  \-> nexa_compiler -> nexa_parser -> nexa_syntax -> nexa_span
                     |                \-> nexa_diagnostics -> nexa_span
                     +-> nexa_hir -> nexa_syntax
                     |             \-> nexa_diagnostics -> nexa_span
                     \-> nexa_mir -> nexa_hir
```

## Commands

```bash
cargo xtask check
cargo run -p nexac -- check examples/language_core.nexa
cargo run -p nexac -- run examples/language_core.nexa
cargo run -p nexac -- parse examples/language_core.nexa
```

Language Core v0.1 parses a lossless CST, lowers it through HIR and MIR, checks
names and types, and interprets a single source file. It intentionally borrows
familiar TypeScript surface syntax without accepting TypeScript or JavaScript
compatibility as a goal. A future TypeScript interop layer, if needed, belongs
in an isolated adapter crate and must lower into Nexa HIR without leaking a
third-party AST or JavaScript runtime semantics into the core compiler.
