# Language Core v0.3 Roadmap

## Goal

Deliver the immutable-data slice defined by
[Language Core v0.3](../../spec/language-core-v0.3.md): UTF-8 strings,
immutable homogeneous arrays, array indexing and `.length`, widened `print`,
and optional command-line arguments through `main(args: String[])`.

This milestone makes Nexa useful for small data-processing programs without
introducing JavaScript collection semantics, mutable heap objects, modules, or
a native backend.

## Delivery Slices

1. **Data contract**: lock grammar, UTF-8 and escape behavior, array
   immutability, contextual empty-array typing, left-to-right evaluation,
   entry-point signatures, runtime bounds failures, diagnostics, and
   non-goals.
2. **Data syntax**: add lossless string tokens, brackets, dot member access,
   array type suffixes, array literals, indexing, and recovery tests. Invalid
   escapes and malformed data syntax remain parser diagnostics.
3. **Typed HIR**: represent `String` and `T[]`, preserve resolved expression
   types, infer homogeneous non-empty literals, use expected types for empty
   literals, resolve `.length`, validate indexes, and add `E2005` for unknown
   members.
4. **Data MIR and runtime**: lower typed string and array values without
   reading CST, preserve left-to-right evaluation, implement concatenation,
   string equality, immutable array storage, bounds checks, `print` overloads,
   and command-line argument delivery.
5. **Integration**: add accepted and rejected fixtures, precise source-span
   assertions, CLI argument tests, output-before-failure tests, documentation,
   examples, and full workspace verification.

All slices preserve v0.2 behavior and its call-depth and basic-block step
limits. New language behavior requires both accepted and rejected coverage.

## Architectural Route

```text
UTF-8 source -> lossless CST -> HIR -> typed HIR -> CFG MIR -> interpreter
                                                    ^
CLI arguments --------------------------------------+
```

The parser owns string and postfix syntax but does not infer element types or
perform member lookup. Typed HIR owns expected-type propagation, exact
expression types, builtin overload selection, and entry-point validation. MIR
owns evaluation order and bounds-check operations. The interpreter receives
already resolved MIR plus immutable command-line argument values.

The data work must establish these invariants:

- every typed expression consumed by MIR has a resolved type;
- every array value has one exact element type and a fixed length;
- empty literals reach MIR only after contextual element-type resolution;
- array elements, call arguments, binary operands, and index operands preserve
  source-order evaluation;
- an index bounds error points at the complete indexing expression;
- immutable array storage may be shared, but storage identity is not exposed;
- the renderer for diagnostics remains source-span based and independent of
  runtime value storage.

## Test Slices

- **Strings**: raw UTF-8, all five escapes, invalid and unterminated literals,
  concatenation, exact equality without normalization, and rejected mixed
  operands.
- **Arrays**: inferred and annotated literals, contextual empty literals,
  nested access, `.length`, function arguments and returns, heterogeneous
  rejection, invalid index types, unknown members, and element-assignment
  rejection.
- **Evaluation**: left-to-right side effects for literal elements,
  concatenation, calls, base/index expressions, and early runtime failure.
- **Runtime**: first and last valid index, negative and upper-bound failures,
  exact index-expression spans, and preservation of preceding output.
- **CLI**: `main()`, empty `main(args: String[])`, multiple arguments after
  `--`, preserved argument text, invalid entry signatures, and arguments
  rejected for a parameterless entry point.
- **Compatibility**: every v0.1 and v0.2 accepted/rejected fixture retains its
  expected behavior and diagnostic code.

## Exit Criteria

- `nexac check` accepts the bundled immutable-data example.
- `nexac run examples/immutable_data.nexa -- Nexa` prints the documented
  deterministic output.
- String tests cover `\\`, `\"`, `\n`, `\r`, and `\t`, plus invalid escapes
  and lack of Unicode normalization.
- Empty arrays succeed in every supported contextual position and fail without
  an expected array type.
- MIR and interpreter tests prove left-to-right evaluation and immutable
  storage behavior without exposing allocation identity.
- Bounds failures report the complete indexing-expression span and retain all
  earlier output.
- `print` accepts exactly `Int`, `Bool`, and `String` values.
- Both valid `main` signatures work, and program arguments passed to `main()`
  fail before the function executes.
- Unknown members report `E2005`; malformed strings use `E1001`; data type
  mismatches use `E3001`; invalid entry signatures use `E3003`.
- `cargo xtask check` and the documentation build pass on the v0.3 integration
  branch.

## Deferred Work

Element assignment, mutable arrays, array equality, growable collections,
objects and records, string indexing and interpolation, modules and imports,
multiple source files, native code generation, LLVM, UI, and TypeScript parser
adapters remain outside v0.3. OXC and SWC must not enter the core parser or IR
dependency graph.
