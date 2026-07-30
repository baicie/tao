# Nexa Language 1.0 Reference Core

## Status

This document defines the target contract for Nexa Language 1.0. The current
delivered implementation is [Language Core v0.5](language-core-v0.5.md); the
features below become language guarantees only when their milestone is marked
delivered in the [1.0 roadmap](../project/roadmap/language-1.0.md).

## Product Contract

Nexa Language 1.0 Reference Core is a TypeScript-shaped, statically typed,
deterministic language for multi-file command-line programs. Source is parsed
into a lossless CST, lowered to typed HIR and CFG MIR, and executed by the
reference interpreter.

Familiar syntax does not imply TypeScript or JavaScript compatibility. Nexa
defines its own value representations, type rules, evaluation order, module
system, error model, and runtime limits.

## Required Language Surface

The 1.0 reference core includes:

- `Int`, `Bool`, `String`, `Unit`, and immutable homogeneous arrays;
- nominal immutable record types with checked construction and field access;
- nominal tagged unions with payloads and exhaustive pattern matching;
- top-level functions, local `const` and `let` bindings, assignment, branches,
  loops, direct calls, and recursion;
- bounded generic functions and generic data types with explicit declarations
  and local type-argument inference;
- statically typed function values, arrow-function expressions, and lexical
  closures with explicit parameter and result types;
- `Option<T>` and `Result<T, E>` expressed through ordinary tagged unions, so
  recoverable errors are values rather than exceptions;
- explicit relative-path imports and exports across multiple source files;
- deterministic left-to-right evaluation and stable diagnostics for every
  source file; and
- `nexac parse`, `nexac check`, and `nexac run`, backed by a versioned
  accepted/rejected conformance suite.

## Semantic Invariants

- There is no implicit `any`, nullability, truthiness, numeric/string coercion,
  or implicit error propagation.
- Records and tagged unions are nominal. Equal field shapes do not make two
  separately declared types interchangeable.
- Heap-backed values exposed by 1.0 are immutable and have no observable
  allocation identity.
- Expressions evaluate left to right exactly once. A runtime failure preserves
  output already emitted and stops later evaluation.
- Name, type, field, variant, and module references are resolved before MIR.
  MIR and the interpreter never inspect CST tokens or repeat source-level name
  lookup.
- Runtime call-depth and execution-step limits are deterministic parts of the
  reference interpreter contract.
- Every diagnostic has a stable code and source span. Cross-file diagnostics
  identify the correct file and may contain primary and secondary labels.

## Module And Entry-Point Contract

A program consists of one entry module and its explicit relative imports. A
runnable program exports or declares exactly one valid `main` entry point using
one of the signatures already defined by v0.3:

```nexa
function main(): Unit {}
function main(args: String[]): Unit {}
```

The CLI owns file-system access and supplies source files to a compiler
session. Parsers do not read files, and module loading does not make host paths
observable to running Nexa code.

## Compatibility Policy

Programs accepted by v0.3 retain their behavior except when they used a word
that a later milestone explicitly reserves as an identifier. Every milestone
specification must list its newly reserved words and retain regression tests
for all other v0.3 behavior.

Before 1.0, syntax and diagnostics may change only through a versioned
milestone specification. At 1.0, the grammar, evaluation rules, required
diagnostic codes, and conformance fixtures become the compatibility baseline.

## 1.0 Acceptance Program

The release is complete when one multi-file example uses records, tagged
unions, exhaustive matching, generics, `Result`, imports, a lexical closure,
loops, immutable collections, and command-line arguments in one checked and
interpreted flow.
The same release must provide rejected fixtures for visibility, exhaustiveness,
generic constraints, module cycles, type mismatches, and deterministic runtime
failures.

The workspace must pass formatting, Clippy with warnings denied, all-target
tests on Rust 1.80, rustdoc, documentation-site build, parser fuzz smoke tests,
and the versioned conformance suite.

## Non-Goals

Nexa Language 1.0 does not include:

- TypeScript source compatibility, npm compatibility, or JavaScript runtime
  semantics such as `any`, `null`, `undefined`, coercion, or prototypes;
- classes, interfaces, traits, inheritance, or operator overloading;
- exceptions, `throw`/`catch`, implicit error propagation, async execution,
  threads, or concurrency;
- `Float`, mutable heap objects, or growable mutable collections;
- native AOT code generation, LLVM, WebAssembly, UI, FFI, or platform APIs;
- a package manager, remote dependencies, LSP, formatter, or complete standard
  library; or
- OXC or SWC types in the core parser, HIR, MIR, or runtime.

These are possible post-1.0 extensions. None may weaken the phase boundaries
or independent semantics established by the reference core.
