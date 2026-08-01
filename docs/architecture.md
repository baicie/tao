# Architecture

Nexa is a small compiler workspace for Nexa Language 1.0. The workspace keeps
each crate aligned to a compiler responsibility rather than a generic
application layer.

The current implementation and compatibility contract remain defined by
Language 1.0. The [architecture decision log](adr/index.md) records the
bootstrap route and accepted Nexa 2.0 target architecture. Those decisions
constrain future work but do not imply that the 2.0 ownership, NIR, LLVM, or
stable ABI designs are already implemented.

The delivered 1.0 architecture preserves these phase boundaries. The compiler
driver owns a source session and module
graph, the semantic phase owns bounded generic instances and closure capture
facts, and each parser remains a pure one-file consumer.

Language Core v0.9 is a delivered stabilization milestone and does not add an
architectural layer. Its conformance, fuzzing, determinism, stress,
performance, documentation, and release tooling must call the same public
phase boundaries rather than duplicating parsing, resolution, MIR lowering, or
runtime dispatch.

Language Core v0.8 adds owner-scoped `ClosureId` identities and exact function
signatures to the existing module-owned function, record, union, field,
variant, payload, and generic identities. Typed HIR resolves imports,
visibility, inference, substitution, calls, captures, iteration, construction,
patterns, coverage, and intrinsic selection before MIR. Definition-level CFG
MIR validates those facts, erases type arguments, and dispatches source
functions and closure snapshots through closed IDs. The interpreter operates
only on resolved identities, layouts, capture slots, and intrinsic enums.

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
- String, array, record, union, function, closure, generic call, member, index,
  iteration, constructor, and match facts are resolved in typed HIR rather
  than rediscovered by MIR lowering.
- Generic instances are bounded and validated in typed HIR. CFG MIR retains
  one body or layout per source definition and erases generic type arguments.
- Array storage may be shared across immutable values, but storage identity is
  never exposed as language behavior.
- Closure environments snapshot checked capture slots by value. Runtime call
  dispatch uses `FunctionId`/`ClosureId` and never source names or `dyn Fn`.
- Language Core uses TypeScript-shaped syntax, not TypeScript or JavaScript
  compatibility. Do not add JavaScript runtime semantics solely for source
  compatibility.
- Future TypeScript interop may use OXC or SWC only behind an isolated adapter
  crate that lowers into Nexa HIR; their AST types must not enter core crates.
- Placeholder crates are avoided until a phase boundary has real behavior.

## Language 1.0 Architecture

```text
source files -> lossless CST -> module HIR -> typed HIR -> CFG MIR -> interpreter
CLI/provider -> SourceMap -> module graph -> resolved definitions
```

Definition, type, field, variant, function, and closure identities are
established before MIR.
The CLI or a future source provider reads files; the compiler session owns
registered sources and module ordering; parsers remain pure consumers of one
source text. This makes cross-file diagnostics possible without coupling syntax
or semantic crates to the host file system.

## Unsafe Code Policy

Front-end crates use `#![forbid(unsafe_code)]`. The workspace lint is `warn` so future low-level crates can isolate and document necessary `unsafe` without weakening the front end.
