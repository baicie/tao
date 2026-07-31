# Language Core v0.7 Roadmap

## Status

Language Core v0.7 is delivered. Its normative executable behavior is defined
by the [v0.7 specification](../../spec/language-core-v0.7.md), and the compiler
satisfies every exit criterion and verification gate below. v0.8 function
values and the practical core have since been delivered; v0.9 stabilization is
active in the [1.0 roadmap](language-1.0.md).

## Goal

Deliver one complete vertical slice for bounded generics and explicit error
values: generic functions, nominal generic records and unions, local exact
type-argument inference, deterministic instance limits, regular recursion,
ordinary source-defined `Option<T>` and `Result<T, E>`, typed HIR substitution,
definition-level CFG MIR erasure, cross-module execution, and stable
diagnostics through the CLI.

v0.7 adds no reserved words and must preserve every valid v0.6 program.

## Frozen Boundaries

- Named type references use explicit complete arguments such as `Box<Int>`.
- Function calls and union constructors use local inference; v0.7 has no
  expression-level `call<Type>(...)` syntax.
- Every generic parameter must constrain an input, field, or payload.
- Inference is exact first-order unification with no conversion or subtyping.
- Typed HIR distinguishes definition identity from instantiated type
  arguments.
- One compiler session permits 256 unique closed generic instances.
- Recursive generic SCCs pass their parameters unchanged and in order.
- CFG MIR and runtime values erase type arguments and retain definition-level
  IDs; no runtime generic dispatch is introduced.
- `Option` and `Result` are explicitly declared/imported ordinary tagged
  unions, not compiler builtins.
- Traits, interfaces, bounds, exceptions, implicit propagation, function
  values, and native code generation remain outside this milestone.

## Delivery Slices

### 1. Specification And Syntax Model

- Keep this roadmap and the v0.7 specification aligned before implementation.
- Add CST node kinds for type parameter lists, type parameters, and type
  argument lists.
- Reuse existing `<`, `>`, identifier, comma, and type tokens; add no keyword.
- Extend function, record, union, and named-type grammar.
- Recover from empty lists, trailing commas, missing `>`, and malformed nested
  arguments without losing later top-level declarations.
- Prove generic type parsing does not change comparison-expression precedence.

Exit criteria:

- Lossless CST tests cover nested types and trivia.
- Accepted/rejected parser tests cover every new delimiter and separator rule.
- `identity<Int>(1)` is not silently accepted as a generic-call syntax.

### 2. HIR Surface And Stable Identities

- Add declaration-order type parameters to functions, records, and unions.
- Lower named type arguments recursively into `TypeReference`.
- Introduce owner-scoped `TypeParameterId`.
- Change resolved nominal types to carry a source definition ID plus arguments;
  monomorphic applications use an empty list.
- Preserve module ownership, export tables, and import resolution by `DefId`.
- Record generic call, construction, match, field, and payload facts without
  leaking CST tokens into semantic APIs.

Exit criteria:

- HIR lowering tests assert names, parameter order, nested arguments, and spans.
- Cross-module lowering proves imported generic declarations retain the
  exporting definition ID.
- No new crate or dependency cycle is introduced.

### 3. Generic Declaration Checking

- Build a declaration-local type-parameter namespace for each generic owner.
- Resolve parameters before module/imported named types inside that owner.
- Diagnose duplicates, out-of-scope names, type-argument arity, and arguments
  on monomorphic declarations.
- Reject unused and function-result-only parameters with `E3008`.
- Reject generic `main` using the existing entry diagnostic `E3003`.
- Recheck existing `Unit` and invalid-array restrictions after substitution.

Exit criteria:

- Each rejection has an exact diagnostic-code and primary/secondary-span test.
- All v0.6 type/name/entry fixtures retain their result.
- Generic definitions from separate modules remain nominally distinct.

### 4. Local Inference And Typed Facts

- Implement structural constraint collection for function arguments and union
  payloads.
- Solve repeated parameter occurrences by exact equality.
- Use expected result types only for still-unresolved parameters.
- Defer context-dependent literals until a complete expected type is known.
- Emit one `E3009` for unresolved or conflicting inference and suppress
  derivative mismatch noise.
- Substitute resolved arguments through function results, record fields, union
  payloads, member access, constructors, match binders, and nested type
  applications.
- Check generic function bodies once with symbolic parameter types.

Exit criteria:

- `identity`, `same`, generic records, `Option`, `Result`, and nested contextual
  construction have focused accepted tests.
- Conflict, missing-context, concrete-operation, and substituted-layout
  failures have focused rejected tests.
- Expression evaluation order remains unchanged and is tested separately from
  inference order.

### 5. Bounded Instances And Recursion

- Intern canonical `(DefId, arguments)` semantic instance keys.
- Discover instances deterministically in module/source/argument order.
- Deduplicate repeated applications and enforce the session-wide limit of 256.
- Extend recursive type analysis to generic data SCCs.
- Analyze recursive generic function-call SCCs after local inference.
- Require unchanged positional parameters on every recursive SCC edge.
- Preserve `E3005` precedence for unguarded record cycles and use `E3010` for
  generic expansion or instance 257.

Exit criteria:

- Regular `List<T>` and same-argument generic function recursion are accepted.
- `Grow<T[]>`, transformed recursive calls, and instance 257 are rejected at
  deterministic spans.
- Repeated equal instances and diamond imports consume the budget once.

### 6. CFG MIR And Interpreter Erasure

- Lower one MIR layout or function body per source definition.
- Keep call, field, variant, and payload operations on resolved definition-level
  IDs selected by typed HIR.
- Erase generic arguments from runtime values and dispatch.
- Ensure symbolic HIR types never cause source-name lookup in MIR or runtime.
- Retain structured validation failures for malformed programmatic MIR.
- Prove call depth, execution steps, recursive-union depth, evaluation order,
  and prior-output retention remain global and unchanged.

Exit criteria:

- One generic function body executes with at least two concrete source types.
- Generic records and unions construct, project, match, and cross module
  boundaries through CFG MIR.
- The interpreter contains no inference, source-path lookup, or type-name
  dispatch.

### 7. Ordinary Error Values And CLI Integration

- Add a multi-file example that explicitly exports/imports the canonical
  ordinary `Option<T>` and `Result<T, E>` declarations.
- Exercise `Some`, `None`, `Ok`, `Err`, nested exhaustive matches, generic
  functions, and cross-module calls in one flow.
- Add an equivalent `Maybe<T>` test to prove the canonical names are not
  intrinsic.
- Add CLI accepted/rejected fixtures for `E3008`, `E3009`, and `E3010` with
  rendered source locations.
- Verify private and missing generic imports retain `E4004` and `E4003`.

Exit criteria:

- `nexac check` accepts the multi-file example.
- `nexac run` prints its specified deterministic output.
- Recoverable `Result.Err` execution is handled only by ordinary `match`.
- No implicit prelude, exception, or propagation behavior is present.

### 8. Stabilization And Delivery

- Update the language guide, architecture document, changelog, navigation, and
  examples only after executable behavior is complete.
- Run the complete v0.7 accepted/rejected conformance matrix.
- Review public Rust APIs and rustdoc for the new semantic identities and
  facts.
- v0.7 is delivered and the 1.0 roadmap has advanced to v0.8 after every gate
  below passed.

## Required Test Matrix

Accepted coverage:

- nested generic CST and ordinary comparisons;
- one generic function instantiated with `Int` and `String`;
- generic record construction/access/return;
- payload and expected-result inference;
- ordinary `Option` and `Result` with exhaustive matches;
- regular recursive generic union and function;
- cross-module and diamond generic imports;
- one erased MIR body serving distinct source types; and
- a non-canonical generic union with identical behavior.

Rejected coverage:

- malformed parameter and argument lists (`E1001`);
- duplicate parameters (`E2002`);
- wrong type arity (`E2003`);
- unused and result-only parameters (`E3008`);
- conflicting or unresolved inference (`E3009`);
- generic nominal mismatches and invalid concrete-only operations (`E3001`);
- invalid substituted `Unit` layouts (`E3001`);
- transformed recursive instances and instance 257 (`E3010`);
- generic entry (`E3003`); and
- missing/private generic imports (`E4003`/`E4004`).

Every accepted behavior requires an executable test. Every rejected behavior
requires the stable diagnostic code and exact source span. Parser-only coverage
does not complete a semantic slice.

## Verification Gates

- `cargo fmt --all -- --check` passes.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
  passes.
- `cargo test --locked --workspace --all-targets` passes.
- `cargo +1.80.0 check --locked --workspace --all-targets` passes.
- `cargo doc --locked --workspace --no-deps` passes.
- `cargo xtask check` passes.
- `pnpm --dir docs build` passes with valid internal links.
- Manual multi-file `nexac check` and `nexac run` for the v0.7 example pass.
- A final diff review confirms parser, HIR, MIR, and interpreter phase
  boundaries remain intact.

## Deferred Work

Expression-level explicit type arguments, generic defaults, traits,
interfaces, bounds, higher-kinded types, variance, subtyping, overloads,
specialization, dynamic dispatch, function values, closures, implicit
preludes, exception syntax, implicit error propagation, native
monomorphization, runtime type reflection, package management, UI, and
TypeScript ecosystem compatibility remain outside v0.7.
