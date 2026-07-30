# Architecture

Nexa is a small compiler workspace for Language Core v0.4. The workspace keeps
each crate aligned to a compiler responsibility rather than a generic
application layer.

The active 1.0 target preserves these phase boundaries while extending the
single-file driver into a source session and module graph. v0.4 is the current
delivered architecture; v0.5 adds tagged unions and exhaustive matching while
preserving the same phase boundaries.

Language Core v0.4 introduced module-ready `RecordId` and `FieldId` identities
without claiming a module loader. Typed HIR resolves record construction and
field access before MIR, and the interpreter operates only on resolved record
layouts.

## Crate Responsibilities

| Crate | Responsibility |
|-------|----------------|
| `nexa_span` | `FileId`, `TextRange`, and source spans |
| `nexa_source` | source file ownership and line/column lookup |
| `nexa_diagnostics` | structured errors, warnings, and source labels |
| `nexa_syntax` | tokens, `SyntaxKind`, and lossless CST support |
| `nexa_parser` | parser entry points and recovery diagnostics |
| `nexa_hir` | CST lowering, name resolution, typed expressions, and type checking |
| `nexa_mir` | CFG MIR lowering, immutable runtime values, and interpreter |
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
- String, array, record, member, and index types are resolved in typed HIR
  rather than rediscovered by MIR lowering.
- Array storage may be shared across immutable values, but storage identity is
  never exposed as language behavior.
- Language Core uses TypeScript-shaped syntax, not TypeScript or JavaScript
  compatibility. Do not add JavaScript runtime semantics solely for source
  compatibility.
- Future TypeScript interop may use OXC or SWC only behind an isolated adapter
  crate that lowers into Nexa HIR; their AST types must not enter core crates.
- Placeholder crates are avoided until a phase boundary has real behavior.

## 1.0 Architecture Target

```text
source files -> lossless CST -> module HIR -> typed HIR -> CFG MIR -> interpreter
CLI/provider -> SourceMap -> module graph -> resolved definitions
```

Definition, type, field, and variant identities are established before MIR.
The CLI or a future source provider reads files; the compiler session owns
registered sources and module ordering; parsers remain pure consumers of one
source text. This makes cross-file diagnostics possible without coupling syntax
or semantic crates to the host file system.

## Unsafe Code Policy

Front-end crates use `#![forbid(unsafe_code)]`. The workspace lint is `warn` so future low-level crates can isolate and document necessary `unsafe` without weakening the front end.
