# Architecture

Nexa is a small compiler workspace for Language Core v0.2. The workspace keeps
each crate aligned to a compiler responsibility rather than a generic
application layer.

## Crate Responsibilities

| Crate | Responsibility |
|-------|----------------|
| `nexa_span` | `FileId`, `TextRange`, and source spans |
| `nexa_source` | source file ownership and line/column lookup |
| `nexa_diagnostics` | structured errors, warnings, and source labels |
| `nexa_syntax` | tokens, `SyntaxKind`, and lossless CST support |
| `nexa_parser` | parser entry points and recovery diagnostics |
| `nexa_hir` | CST lowering, name resolution, and type checking |
| `nexa_mir` | middle IR lowering and interpreter |
| `nexa_compiler` | compiler-driver entry points for checking and running |
| `nexac` | command-line interface and diagnostic rendering |

## Dependency Graph

```text
nexac -> nexa_source -> nexa_span
  |
  \-> nexa_compiler -> nexa_parser -> nexa_syntax -> nexa_span
                     |                \-> nexa_diagnostics -> nexa_span
                     +-> nexa_hir -> nexa_syntax
                     |             \-> nexa_diagnostics -> nexa_span
                     \-> nexa_mir -> nexa_hir
```

Rules:

- Parser code does not perform type checking.
- Diagnostics are expressed in terms of stable source spans.
- The compiler driver orchestrates parser, HIR, and MIR; the interpreter only
  consumes MIR.
- Language Core uses TypeScript-shaped syntax, not TypeScript or JavaScript
  compatibility. Do not add JavaScript runtime semantics solely for source
  compatibility.
- Future TypeScript interop may use OXC or SWC only behind an isolated adapter
  crate that lowers into Nexa HIR; their AST types must not enter core crates.
- Placeholder crates are avoided until a phase boundary has real behavior.

## Unsafe Code Policy

Front-end crates use `#![forbid(unsafe_code)]`. The workspace lint is `warn` so future low-level crates can isolate and document necessary `unsafe` without weakening the front end.
