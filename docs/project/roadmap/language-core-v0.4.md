# Language Core v0.4 Roadmap

## Status

Language Core v0.4 is delivered. The executable compiler satisfies every exit
criterion below; v0.5 through v0.8 have since been delivered in the
[Nexa Language 1.0 roadmap](language-1.0.md).

## Goal

Deliver the nominal immutable-record slice defined by
[Language Core v0.4](../../spec/language-core-v0.4.md): named record
declarations, contextually typed record literals, field access, exact field
validation, finite record-type graphs, and resolved record operations in MIR.

This milestone is the first vertical slice toward the
[Nexa Language 1.0 Reference Core](../../spec/language-1.0.md). It does not add
modules, tagged unions, generics, structural objects, or a native backend.

## Delivery Slices

1. **Record contract**: lock the grammar, nominal identity, contextual literal
   rules, field immutability, source-order evaluation, recursion rejection,
   diagnostics, compatibility exception, and non-goals.
2. **Record syntax**: add lossless `type`, record declaration, record field,
   and record literal CST nodes with recovery around missing names, colons,
   separators, braces, and declaration terminators.
3. **Record HIR and semantics**: collect record declarations before checking
   functions, allocate module-ready record and field identities, detect record
   cycles, propagate expected record types, validate exact fields, and preserve
   all resolved facts for MIR.
4. **Record MIR and runtime**: lower construction and access by resolved IDs,
   preserve literal source-order evaluation, store immutable record values,
   and retain output before an initializer failure.
5. **Integration**: add accepted and rejected parser, HIR, MIR, interpreter,
   compiler, and CLI fixtures; update documentation and examples only after
   the executable behavior exists; run full workspace and documentation checks.

Each slice preserves v0.3 behavior except that `type` becomes a reserved word.
Every accepted behavior requires an executable test, and every rejection path
requires a stable diagnostic-code and source-span assertion.

## Architectural Route

```text
UTF-8 source -> lossless CST -> record HIR -> typed HIR -> CFG MIR -> interpreter
                                      |             |
                                      +-> record ID  +-> resolved record / field IDs
                                      +-> field ID
```

The parser recognizes record syntax but does not resolve names, infer literal
types, validate fields, or detect recursion. HIR owns nominal declarations,
the record dependency graph, expected-type propagation, and field resolution.
MIR consumes only validated identities and never compares source field names.

The new identities must be source-order stable and designed to gain module
ownership in v0.6 without changing record semantics. v0.4 must not implement a
placeholder module loader or a new crate without a current phase responsibility.

## Static-Semantic Invariants

- Every accepted record reference resolves to one exact nominal `RecordId`.
- Every accepted construction provides every declared `FieldId` exactly once
  and provides no unknown field.
- Every accepted field initializer has exactly its declared type.
- Every accepted field access is resolved in typed HIR before MIR lowering.
- Every record dependency graph is acyclic, including dependencies nested
  beneath array types.
- A record literal without one exact expected record type never reaches MIR.
- `Unit` and types containing `Unit` are not accepted as record field types.

## Test Slices

- **Syntax**: empty and non-empty declarations, literals, nested literals,
  trivia preservation, missing punctuation, invalid separators, and recovery
  into the next top-level declaration.
- **Names**: forward type references, unknown types, duplicate type names,
  separate type/value namespaces, duplicate fields, and unknown members.
- **Typing**: annotated declarations, assignment/call/return context, nested
  record and empty-array context, missing fields, wrong field types,
  unconstrained literals, nominally distinct equal shapes, and invalid `Unit`
  fields.
- **Recursion**: direct self-reference, mutual cycles, cycles through arrays,
  longer cycles, acyclic forward references, and precise cycle labels.
- **Evaluation**: literal fields evaluated once in source order, field bases
  evaluated once, early runtime failure, and preservation of earlier output.
- **IR/runtime**: construction and access use stable IDs, storage layout is not
  observable, records pass through calls and returns, and v0.3 limits remain.
- **Compatibility**: all v0.3 fixtures retain their result and diagnostic code
  except an explicit fixture proving that `type` is newly reserved.

## Exit Criteria

- `nexac check` accepts the documented record program, and `nexac run` prints
  `Ada` followed by `42`.
- The parser produces lossless record CST nodes and recovers into subsequent
  declarations after malformed record syntax.
- Typed HIR exposes stable nominal type and field identities; MIR record
  operations contain no source-text lookup.
- Missing, duplicate, unknown, mistyped, and unconstrained fields produce the
  specified diagnostics at stable spans.
- Direct, mutual, longer, and array-mediated record cycles produce `E3005`;
  acyclic forward references succeed.
- Tests prove nominally distinct equal-shaped records are not interchangeable.
- Tests prove exact, left-to-right initializer evaluation and output retention
  before runtime failure.
- Field assignment, record equality, and record printing remain rejected.
- All v0.3 accepted and rejected behavior remains green except the documented
  `type` keyword compatibility case.
- `cargo xtask check`, locked Clippy with warnings denied, Rust 1.80 all-target
  checks, and the documentation-site build pass.

## Deferred Work

Recursive nominal data, tagged unions, pattern matching, generics, modules,
imports, multiple source files, structural object typing, optional fields,
field mutation, record equality, reflection, native code generation, UI, and
TypeScript interoperability remain outside v0.4.
