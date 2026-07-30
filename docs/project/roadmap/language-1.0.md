# Nexa Language 1.0 Roadmap

## Goal

Deliver the [Nexa Language 1.0 Reference Core](../../spec/language-1.0.md): a
statically checked, multi-file command-line language executed by the CFG MIR
reference interpreter.

Language Core v0.6 is fully delivered. It establishes deterministic multi-file
modules, private-by-default exports, source sessions, and module-owned resolved
identities across CST, typed HIR, CFG MIR, and the interpreter. v0.7, bounded
generics and ordinary `Option`/`Result` error values, is the active
implementation milestone.

## Delivery Milestones

### v0.4: Nominal Immutable Records (Delivered)

- Add named record declarations, record literals, and field access.
- Reject missing, duplicate, unknown, and mistyped fields.
- Keep fields immutable and records free of prototype or structural-object
  semantics.
- Introduce stable definition, type, and field identities that can later be
  extended with module identity.
- Lower fields to MIR by resolved IDs rather than source strings.

### v0.5: Tagged Unions And Matching (Delivered)

- Add nominal tagged unions whose variants may carry typed payloads.
- Add exhaustive `match` expressions with stable pattern spans.
- Diagnose duplicate, unreachable, and missing cases.
- Define and test the accepted recursive-type shapes.
- Evaluate only the selected match arm.

### v0.6: Multi-File Modules (Delivered)

- Add explicit named imports and exports using relative source paths.
- Make the compiler session own the source map, module graph, and stable
  module-aware definition identities.
- Diagnose missing modules, private symbols, duplicate exports, import cycles,
  and cross-file duplicate definitions.
- Sort and render diagnostics by file and source position.
- Keep file-system access in the CLI/provider boundary rather than the parser.

### v0.7: Bounded Generics And Error Values (Active)

- Add declared type parameters to functions, records, and tagged unions.
- Infer type arguments only from local call or construction context.
- Reject unconstrained parameters and unbounded recursive instantiation.
- Define `Option<T>` and `Result<T, E>` using ordinary union declarations.
- Keep recoverable errors explicit; do not add exceptions or implicit
  propagation.

### v0.8: Function Values And Practical Core

- Add statically typed function types and indirect calls.
- Add arrow-function expressions and immutable lexical captures.
- Define closure identity, capture order, recursion restrictions, and runtime
  limits without introducing JavaScript `this` or prototype semantics.
- Add array `for...of` iteration with deterministic source-order behavior.
- Add immutable array append/concatenation operations.
- Define Unicode-scalar string length semantics.
- Add explicit `toString` and `parseInt` conversions.
- Deliver a realistic multi-file CLI example using every earlier milestone.

### v0.9: Stabilization

- Freeze the keyword set, grammar, evaluation order, and required diagnostic
  codes without adding new language syntax.
- Build a versioned accepted/rejected conformance corpus.
- Add parser fuzzing, larger programs, recovery stress tests, and performance
  baselines.
- Complete the language guide and compatibility policy.

### 1.0: Integration Release

- Run one multi-file program through parse, check, typed HIR, CFG MIR, and the
  reference interpreter.
- Cover records, unions, exhaustive matching, generics, `Result`, imports,
  closures, loops, immutable data, and CLI arguments in that program.
- Publish the complete reference specification and conformance suite.
- Pass Rust 1.80 checks, all workspace tests, rustdoc, documentation build, and
  release validation.

## Slice Rules

Each implementation milestone is a vertical slice through syntax, parser
recovery, HIR lowering, semantic checking, MIR, interpreter, compiler driver,
CLI fixtures, documentation, and examples. A feature is not delivered when it
only parses.

Every new accepted behavior needs an executable test. Every new rejection path
needs a stable diagnostic-code and source-span test. MIR must consume resolved
typed HIR facts; it may not perform source-level lookup. New crates are added
only when a phase boundary has real behavior.

Each milestone is committed independently on the 1.0 integration branch after
`cargo xtask check` and its milestone-specific manual examples pass.

## Dependency Route

```text
source files -> lossless CST -> module HIR -> typed HIR -> CFG MIR -> interpreter
                    |               |
                    +-> syntax      +-> resolved DefId / TypeId / FieldId
CLI/provider -> SourceMap -> module graph
```

v0.5 establishes module-ready identities without prematurely implementing a
loader. v0.6 extends those identities with module ownership and changes the
compiler driver from a single `(FileId, &str)` entry point to a source-session
entry point.

## Deferred Beyond 1.0

`Float`, exceptions, async/concurrency, mutable heap collections, native code
generation, UI, FFI, package management, LSP, and TypeScript ecosystem
compatibility remain post-1.0 work.
