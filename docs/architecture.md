# Architecture

Nexa is a small compiler workspace for Language Core v0.6. The workspace keeps
each crate aligned to a compiler responsibility rather than a generic
application layer.

The active 1.0 target preserves these phase boundaries. v0.6 is the current
delivered architecture: the compiler driver owns a source session and module
graph while each parser remains a pure one-file consumer.

Language Core v0.6 extends function, record, union, field, variant, and payload
identities with module ownership. Typed HIR resolves imports, visibility,
construction, patterns, coverage, and payload bindings before MIR; the
interpreter operates only on resolved identities and layouts.

## Crate Responsibilities

| Crate | Responsibility |
|-------|----------------|
| `nexa_span` | `FileId`, `TextRange`, and source spans |
| `nexa_source` | source identities, provider boundary, ownership, and line/column lookup |
| `nexa_diagnostics` | structured errors, warnings, and source labels |
| `nexa_syntax` | tokens, `SyntaxKind`, and lossless CST support |
| `nexa_parser` | parser entry points and recovery diagnostics |
| `nexa_hir` | CST lowering, name resolution, typed expressions, and type checking |
| `nexa_mir` | CFG MIR lowering, immutable runtime values, and interpreter |
| `nexa_compiler` | source sessions, module graphs, checking, lowering, and execution orchestration |
| `nexac` | file-system source provider, command-line interface, and diagnostic rendering |

## Dependency Graph

```text
nexac -> nexa_source -> nexa_span
  |
  \-> nexa_compiler -> nexa_source
                     +-> nexa_parser -> nexa_syntax -> nexa_span
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
- The source provider resolves and loads opaque canonical keys; the compiler
  session owns reachability, source registration, graph order, and load
  failure caching.
- String, array, record, union, member, index, constructor, and match types are
  resolved in typed HIR rather than rediscovered by MIR lowering.
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
