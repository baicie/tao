# Language Core v0.8 Roadmap

## Status

Language Core v0.8 is delivered. Its normative behavior is frozen by the
[v0.8 specification](../../spec/language-core-v0.8.md), and the compiler
satisfies every exit criterion and verification gate below. v0.9 stabilization
is now the active milestone in the [1.0 roadmap](language-1.0.md).

## Goal

Deliver one complete vertical slice for monomorphic function values and the
practical language core: exact function types, non-generic named function
values, indirect calls, typed arrows, deterministic immutable captures,
array-only `for...of`, immutable append/concat, Unicode-scalar string length,
explicit integer/string conversion, stable HIR/MIR callable identities, and a
realistic multi-file CLI program combining every earlier milestone.

v0.8 reserves only `for`; `of` remains contextual. All v0.7 behavior outside
the newly reserved spelling must remain unchanged.

## Frozen Boundaries

- Function types use `(value: Int) => Int`; parameter names are non-semantic
  and signatures are exact and invariant.
- Arrows use `(value: Int): Int => expression` or a block body, with explicit
  parameter and result types.
- Every arrow site has `ClosureId { owner: FunctionId, source_index }`.
- Captures are immutable by-value snapshots ordered by first lexical use.
- Capturing an enclosing `let` uses `E3011`; no shared mutable closure cell is
  introduced.
- Local and imported non-generic source functions are first-class values.
- A generic function in value position uses `E3012`; direct generic calls keep
  v0.7 inference.
- `for (const value of values)` evaluates one array once and creates a fresh
  immutable binding for each source-order element.
- `append` and `concat` return new arrays and are intrinsic member-call forms,
  not extractable bound methods.
- `String.length` counts Unicode scalar values.
- `toString(Int)` and `parseInt(String)` are explicit direct builtins with the
  exact conversion and runtime-error contract in the specification.
- Typed HIR resolves every callable/member/iteration fact. CFG MIR dispatches
  through closed enums and stable IDs, never `dyn Fn` or source names.
- `this`, prototypes, bound methods, coercion, an iterator protocol, and
  exceptions remain outside the milestone.

## Delivery Slices

### 1. Specification And Syntax Model

Suggested branch: `codex/v0.8-spec-syntax`.

- Keep this roadmap and the v0.8 specification aligned before implementation.
- Reserve `for` and retain contextual lexing/parsing for `of`.
- Add CST kinds for function types, function-type parameters, arrow
  expressions, arrow parameters/bodies, and `for...of` statements.
- Extend type parsing without changing generic `>` splitting or expression
  comparison precedence.
- Disambiguate arrow blocks, record literals, parenthesized expressions,
  function types, and match-arm `=>` tokens.
- Recover from missing parentheses, colons, types, arrows, `of`, expressions,
  and blocks without swallowing later statements or declarations.

Exit criteria:

- Lossless CST reconstructs every accepted form byte for byte.
- Accepted parser tests cover nested function types, arrays of functions,
  expression/block arrows, trivia, and contextual `of`.
- Rejected parser tests cover every delimiter and separator rule with `E1001`,
  exact spans, forward progress, and later-declaration recovery.
- Existing comparison, generic, match-arrow, and record-literal parser suites
  remain green.

### 2. Function Types And Named Function Values

Suggested branch: `codex/v0.8-function-values`.

- Extend source and resolved types with exact ordered function signatures.
- Resolve function types recursively inside arrays, generic applications,
  records, unions, and function signatures.
- Treat non-generic module-local and imported function names as expressions
  carrying their stable `FunctionId` and signature.
- Distinguish direct generic calls, direct monomorphic calls, and indirect
  calls in typed facts.
- Type-check callee-before-arguments evaluation, exact arity, exact parameter
  types, returns, storage, and unsupported function operations.
- Emit `E3012` for every generic function value while preserving direct local
  inference and suppressing derivative mismatches.

Exit criteria:

- Source/imported functions can be passed, returned, stored in immutable data,
  placed in arrays, and called indirectly.
- Generic inference structurally traverses function types.
- Local/imported generic function-value rejection asserts `E3012` primary and
  secondary spans.
- Function equality, printing, non-callable calls, arity, and signature
  mismatches retain stable diagnostics.

### 3. Arrow HIR And Capture Analysis

Suggested branch: `codex/v0.8-closures`.

- Lower expression- and block-bodied arrows with explicit signatures.
- Assign preorder `ClosureId` values scoped by the containing source
  `FunctionId`, including nested arrows.
- Allocate arrow parameters and locals in callable-local scopes.
- Resolve free references and build deterministic first-use capture vectors.
- Forward distant captures through intermediate closures.
- Allow enclosing parameters and `const`; emit one `E3011` per arrow/captured
  outer `let` pair with the first causal reference and declaration labels.
- Reject local self-reference with ordinary `E2001`; do not add implicit fixed
  points or recursive environment cells.

Exit criteria:

- Focused HIR tests assert stable IDs, nested ordering, signatures, capture
  order, deduplication, forwarding, source spans, and module ownership.
- Expression bodies and block return analysis match source functions.
- Closures inside generic bodies substitute symbolic signature/capture types
  without making the generic source function a first-class value.
- Direct and nested mutable captures have exact `E3011` tests.

### 4. Callable MIR And Interpreter

Suggested branch: `codex/v0.8-closure-mir`.

- Lower one callable CFG per source function and one per `ClosureId`.
- Add stable closed-enum representations for source functions, closures,
  builtins, direct calls, and indirect calls.
- Materialize closures by copying typed capture slots in checked order.
- Evaluate indirect callees before arguments, then invoke without name or type
  lookup.
- Share the global 64-active-call limit and 100000-step budget across direct
  functions, closures, and nested calls.
- Preserve output before indirect-call runtime failure.
- Defensively validate callable identities, signature metadata, capture count,
  capture slots, and runtime value kinds.

Exit criteria:

- One closure site produces independent runtime capture snapshots.
- Returned and nested closures execute after their creating frames return.
- Imported named function values dispatch by exporting `FunctionId`.
- Exactly 64 active mixed calls succeed and the 65th fails at the exact call
  span.
- Forged MIR cases return structured errors without panic, source lookup,
  `dyn Fn`, or runtime type inference.

### 5. Array For-Of

Suggested branch: `codex/v0.8-for-of`.

- Lower and check an array-only iterable expression and inferred immutable loop
  binder.
- Lower iteration to explicit CFG state over one evaluated array value and a
  monotonically increasing internal index.
- Reuse nearest-loop `break`/`continue` facts for mixed nested `while` and
  `for...of` loops.
- Ensure `continue` advances, `break` exits, return propagates, empty arrays
  skip, and closures capture per-iteration values.
- Keep iteration under the existing global step budget.

Exit criteria:

- Executable tests cover empty/non-empty arrays, nested loops, nearest control,
  early return, one-time iterable effects, and source-order values.
- Per-iteration closure snapshots remain distinct.
- Non-array iteration and loop-binding assignment have exact `E3001`/`E2004`
  spans.
- No runtime iterator object, callback lowering, protocol lookup, or source
  member name enters MIR.

### 6. Practical Immutable Data And Conversions

Suggested branch: `codex/v0.8-practical-data`.

- Resolve array `.append(value)` and `.concat(array)` only as immediate
  intrinsic member calls.
- Return new immutable arrays while preserving base-then-argument,
  left-to-right, exactly-once evaluation.
- Reject method extraction with `E2005`, bad arity with `E2003`, and exact type
  mismatches with `E3001`.
- Extend `.length` to `String` and count Unicode scalar values without
  normalization or grapheme segmentation.
- Add stable builtin IDs and typed direct-call facts for `toString` and
  `parseInt`.
- Implement canonical signed decimal formatting and complete `-?[0-9]+`
  parsing with separate fixed malformed/overflow runtime messages.
- Preserve output and exact call spans on conversion failures.

Exit criteria:

- Append/concat tests prove unchanged inputs, element order, contextual empty
  arrays, exact generic element types, and evaluation order.
- String tests distinguish bytes, Unicode scalars, and combining sequences.
- Conversion tests cover zero, signs, leading zeroes, both `Int` bounds,
  malformed classes, and overflow in both directions.
- MIR uses intrinsic/builtin enums rather than bound closures or string names.

### 7. Multi-File CLI Integration

Suggested branch: `codex/v0.8-cli-example`.

- Add a realistic multi-file example based on the accepted program in the
  specification.
- Exercise imports/exports, nominal records, tagged unions, exhaustive match,
  bounded generics, ordinary `Result`, named function values, one capturing
  arrow, arrays, `for...of`, append/concat, conversions, and CLI arguments.
- Add accepted CLI fixtures for check/run and rejected fixtures for `E3011`,
  `E3012`, function mismatch, loop misuse, intrinsic misuse, malformed parse,
  overflow, and prior-output retention.
- Assert rendered paths and line/column locations for local and cross-module
  labels.

Exit criteria:

- `nexac check` accepts the example and `nexac run ... -- 20` prints exactly
  `42` and `5` on separate lines.
- Local and imported generic function values render `E3012` with both files
  when applicable.
- Every new runtime failure preserves earlier standard output and renders its
  exact source location.
- All v0.7 accepted/rejected fixtures retain their behavior except explicit
  `for`-identifier compatibility fixtures.

### 8. Stabilization And Delivery

Suggested branch: `codex/v0.8-integration`.

- Run the complete accepted/rejected matrix and independently review parser,
  HIR, MIR, runtime, module, and diagnostic boundaries.
- Review public Rust APIs and rustdoc for callable IDs, signatures, captures,
  intrinsic IDs, and structured failure types.
- Update README, guide, architecture, changelog, navigation, spec index, and
  project index only after executable behavior is complete.
- Mark v0.8 delivered and v0.9 active only after every gate below passes.

## Required Test Matrix

Accepted coverage:

- nested and array function types with unchanged generic/comparison parsing;
- local/imported named function values in every value position;
- expression/block arrows, nested captures, return, and generic substitution;
- deterministic IDs, capture order, forwarding, deduplication, and snapshots;
- empty/non-empty/nested `for...of`, nearest control, return, and step limit;
- immutable append/concat and per-iteration captured elements;
- Unicode scalar length and all explicit-conversion boundaries;
- mixed direct/indirect call-depth accounting and prior-output retention;
- valid and malformed programmatic callable MIR; and
- the complete multi-file CLI example.

Rejected coverage:

- malformed new syntax (`E1001`);
- indirect arity (`E2003`) and function mismatches (`E3001`);
- unsupported function equality/print and non-callable calls (`E3001`);
- local closure self-reference (`E2001`);
- direct/forwarded mutable capture (`E3011`);
- local/imported generic function values (`E3012`);
- non-array iteration and immutable loop assignment (`E3001`/`E2004`);
- bound-method extraction, unknown members, intrinsic arity/type errors
  (`E2005`/`E2003`/`E3001`);
- malformed/overflowing `parseInt` runtime failures; and
- foreign closure/function/builtin IDs, wrong capture layout, non-callable
  indirect values, and invalid intrinsic MIR as structured runtime errors.

Every accepted semantic feature must reach interpreter or CLI execution. Every
rejected source feature requires a stable code and exact source span. Parser-
only coverage does not complete a slice.

## Verification Gates

- `cargo fmt --all -- --check` passes.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
  passes.
- `cargo test --locked --workspace --all-targets` passes.
- `cargo +1.80.0 check --locked --workspace --all-targets` passes.
- `cargo doc --locked --workspace --no-deps` passes.
- `cargo xtask check` passes.
- `pnpm --dir docs build` passes with valid internal links.
- Manual multi-file `nexac check` and `nexac run ... -- 20` for the v0.8
  example pass with exact output.
- A final diff review confirms the parser performs no type checking or I/O,
  typed HIR owns all resolution, MIR reads no CST/source names, and interpreter
  dispatch uses stable enums/IDs rather than `dyn Fn`.

## Deferred Work

Polymorphic function values, explicit generic call arguments, overloads,
optional/rest/default parameters, bound methods, `this`, prototypes, shared
mutable captures, recursive local closures, iterator protocols, custom
iterables, mutable arrays, higher-order collection libraries, grapheme APIs,
implicit coercion, exceptions, async/concurrency, native code generation, UI,
package management, runtime reflection, and TypeScript ecosystem compatibility
remain outside v0.8.
