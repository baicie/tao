# Language Core v0.5 Roadmap

## Status

Language Core v0.5 is delivered. The executable compiler satisfies every exit
criterion below, and v0.6 multi-file modules are now the active implementation
milestone in the [1.0 roadmap](language-1.0.md).

## Goal

Deliver one complete tagged-union vertical slice: nominal union declarations,
qualified construction, guarded recursive nominal data, exhaustive `match`
expressions, resolved union CFG MIR, deterministic interpretation, and stable
diagnostics from the lossless CST through the CLI.

v0.5 stays single-file and monomorphic. It must preserve v0.4 behavior except
that `match`, `case`, and `default` become reserved words. It must not begin the
v0.6 module loader or the v0.7 generic `Option`/`Result` work.

## Branch Sequence

Implementation proceeds as reviewable branches based on the previous branch
and merged into `codex/language-1.0` in this order:

1. `codex/v0.5-contract`: freeze grammar, name/type rules, exhaustiveness,
   reachability, guarded recursion, runtime depth, diagnostics, and fixtures.
2. `codex/v0.5-syntax`: add union/variant declarations, qualified
   construction, match arms, patterns, `=>`, lossless CST nodes, and recovery.
3. `codex/v0.5-union-hir`: lower declarations and constructors; allocate
   stable `UnionId`/`VariantId`; validate names, payloads, arity, exact types,
   and guarded recursive type graphs.
4. `codex/v0.5-match-check`: lower and check patterns, binder scopes, foreign
   and duplicate cases, match result types, exhaustiveness, and reachability.
5. `codex/v0.5-mir-runtime`: lower resolved construction, tag dispatch, and
   payload extraction into CFG MIR; execute variants and enforce depth 1024.
6. `codex/v0.5-integration`: add CLI fixtures and the accepted example, audit
   v0.4 compatibility, update delivered-status documentation only after all
   executable gates pass, and run the full verification matrix.

Every branch must compile and retain the tests owned by earlier branches.
Syntax branches do not perform semantic checking, and runtime branches do not
repeat source-name lookup.

## Architectural Route

```text
UTF-8 source -> lossless CST -> union/match HIR -> typed HIR -> CFG MIR
                                      |                  |
                                      +-> UnionId        +-> resolved tag dispatch
                                      +-> VariantId      +-> payload positions
                                                        |
                                                        +-> interpreter
```

The parser owns tokens, tree shape, and recovery only. HIR owns namespaces,
nominal identities, type dependencies, constructor/case resolution, binder
scopes, exhaustiveness, and arm result typing. MIR consumes those resolved
facts and never compares source qualifier or variant strings.

Existing `RecordId` and owner-scoped `FieldId` remain unchanged. New union and
variant identities must be source-order stable and able to gain module
ownership in v0.6 without changing v0.5 semantics.

## Delivery Slices

### Contract And Syntax

- Reserve `match`, `case`, and `default`; add the `=>` token without importing
  TypeScript/JavaScript parser types.
- Accept both `type X = A() | B();` and an optional leading `|` layout.
- Require parentheses on every declaration, constructor, and pattern variant.
- Reject trailing pipes/commas, missing names/types/arrows/semicolons, malformed
  payloads, and incomplete arms with `E1001` while recovering into the next
  variant, arm, or top-level declaration.
- Preserve trivia and exact source spans in the CST.

### Union HIR And Type Checking

- Share the existing type namespace between records and unions; collect all
  nominal declarations before checking bodies.
- Allocate `UnionId` in source order and `VariantId` in owner/source order.
- Diagnose unknown and duplicate unions, variants, and payloads with stable
  primary/secondary labels.
- Resolve only qualified construction; require exact arity and payload types,
  propagate contextual record/array types, and allow direct `Unit` payloads.
- Treat union values as immutable nominal values; reject equality, printing,
  arbitrary members, bare constructors, and foreign type use.

### Recursive-Type Validation

- Classify record-field dependencies as unguarded even beneath arrays.
- Classify union-payload dependencies as guarded even beneath arrays.
- Reject `E3005` precisely when the dependency graph still contains a cycle
  after guarded union-payload edges are removed.
- Cover direct union recursion, union/record cycles, forward references,
  record-only self/mutual/long cycles, and cycles beneath arrays.

### Match Checking

- Evaluate and type the scrutinee once; require one resolved union type.
- Resolve qualified case patterns against that exact union and bind positional
  payloads as immutable arm-local values.
- Diagnose binder and case duplication, pattern arity, unknown variants,
  foreign cases, and arm result mismatches independently.
- Require every variant exactly once unless a reachable `default` covers the
  remainder; report missing variants in declaration order with `E3006`.
- Report arms after a default and defaults after complete coverage with
  `E3007`; continue validating unreachable patterns and expressions.

### MIR And Interpreter

- Represent variant construction with resolved union/variant identities and
  declaration-order payload slots.
- Lower matches to CFG tag dispatch and payload extraction such that only the
  selected arm is runtime-reachable.
- Preserve left-to-right constructor arguments, exactly-once scrutinee
  evaluation, selected-arm-only effects, and output before failures.
- Define union depth as one plus the maximum nested union depth transmitted
  through payload records/arrays. Accept depth 1024 and fail construction of
  depth 1025 at the complete constructor span.
- Preserve the global 100000-basic-block step budget and 64-active-call limit.
- Return structured runtime errors for malformed programmatic MIR tags/layouts
  instead of panicking.

## Required Test Matrix

- **Lexer/CST**: all new keywords/tokens, both leading-pipe styles, zero/many
  payloads, trivia, nested match expressions, and exact lossless reconstruction.
- **Recovery**: missing union/variant/payload names, parentheses, colons,
  arrows, arm semicolons, braces, declaration semicolon, trailing pipe/comma,
  and recovery into following arms/declarations without loops.
- **Names/IDs**: forward references, record/union namespace collisions,
  variant scoping, duplicate variants/payloads/binders/cases, stable source
  order IDs, unknown qualifiers, and unknown variants.
- **Typing**: exact constructor and pattern arity, exact payload types, direct
  `Unit` payloads, contextual record/empty-array payloads, arm result unification,
  non-union matches, foreign cases, and rejected union operations.
- **Coverage**: zero-arm match, one/many omitted variants, declaration-order
  missing names, default coverage, default after complete coverage, arms after
  default, and semantic checks inside unreachable arms.
- **Recursion**: direct/mutual/long union recursion, union/record cycles, array
  payload recursion, and unchanged `E3005` record-only cycles through fields
  or arrays with precise primary/secondary spans.
- **Evaluation**: constructor arguments left to right once, scrutinee once,
  only selected arm evaluated, payload binding order, nested matches, early
  runtime failure, and preservation of earlier output.
- **Depth**: iterative construction at exactly 1024 succeeds; construction of
  node 1025 fails at its constructor span with the fixed message and retained
  output; records/arrays transmit but do not increment depth.
- **IR/runtime**: no source-name lookup, owner-correct tags, valid local CFG
  targets, joins preserve result values, unions pass through locals, arrays,
  records, calls, returns, and malformed MIR fails defensively.
- **CLI/compatibility**: accepted and rejected fixtures for every diagnostic,
  exact file/line/column checks for new codes, an executable `42` example, all
  v0.4 behavior, and explicit regressions for the three newly reserved words.

## Diagnostic And Span Gates

- `E1001`: malformed union, constructor, match, or pattern syntax.
- `E2001`: unknown type or qualifier.
- `E2002`: duplicate union/type, variant, payload, binder, or case.
- `E2003`: constructor or pattern arity mismatch.
- `E2005`: unknown variant.
- `E3001`: type mismatch, non-union/foreign case, bare constructor, equality,
  printing, or member operation.
- `E3005`: unguarded recursive record cycle.
- `E3006`: non-exhaustive match.
- `E3007`: unreachable match arm.

Every new path requires a diagnostic-code assertion and a primary-span
assertion. Duplicate diagnostics include a secondary label on the first item.
Coverage and reachability diagnostics identify their match/arm and the
declaration or earlier arm that explains the result.

## Exit Criteria

- `nexac check` accepts the documented recursive `List` program and `nexac run`
  prints `42`.
- Lossless parsing and recovery cover every new grammar boundary without hangs
  or swallowing a following top-level declaration.
- Typed HIR exposes stable `UnionId`/`VariantId` facts and resolves every
  constructor, pattern, payload, and binder before MIR.
- Exhaustiveness, duplicate cases, unreachable arms, foreign cases, and arm
  type mismatches produce the specified diagnostics and stable spans.
- Guarded union recursion succeeds while every unguarded record cycle retains
  `E3005`.
- CFG MIR and the interpreter use resolved tags/positions, execute only the
  selected arm, preserve evaluation order/output, and enforce exact 1024/1025
  depth boundaries.
- v0.4 accepted/rejected fixtures retain their behavior except the documented
  `match`/`case`/`default` keyword compatibility cases.
- `cargo xtask check`, locked Clippy with warnings denied, Rust 1.80 all-target
  checks, the documentation-site build, the accepted CLI example, and all
  milestone fixtures pass.
- v0.5 documentation is marked delivered and the 1.0 roadmap advances v0.6 to
  active only after every gate above passes.

## Deferred Work

Multi-file modules, imports/exports, generic unions, `Option`, `Result`,
function values, closures, advanced patterns, guards, union equality/printing,
reflection, exceptions, native code generation, UI, and TypeScript ecosystem
compatibility remain outside v0.5.
