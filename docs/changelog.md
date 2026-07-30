# Changelog

## Language Core v0.7 (Delivered)

- Added bounded generic functions, nominal records, and tagged unions with
  owner-scoped type-parameter identities and complete named type arguments.
- Added deterministic local inference for calls and constructors, regular
  generic recursion, and a session-wide limit of 256 unique closed instances.
- Added ordinary source-defined `Option<T>` and `Result<T, E>` unions without
  an implicit prelude, exception handling, or propagation semantics.
- Added definition-level CFG MIR erasure with validation of resolved generic
  facts, including cross-module generic construction, calls, and matching.
- Added stable `E3008`, `E3009`, and `E3010` diagnostics, accepted/rejected
  conformance coverage, and executable single-file and multi-file examples.

The active implementation milestone is v0.8: function values, lexical
closures, `for...of`, and practical immutable-data operations.

## Language Core v0.6 (Delivered)

- Added explicit named imports and private-by-default exports for functions,
  nominal records, and tagged unions across reachable `.nexa` files.
- Added canonical source providers, deterministic DFS module graphs, cached
  loads, cycle detection, complete cross-file source maps, and `E4001` through
  `E4005` diagnostics.
- Added module-owned definition and layout identities through typed HIR, CFG
  MIR, and the interpreter, including resolved entry selection and diamond
  identity preservation.
- Added file-system and in-memory provider tests, multi-file CLI fixtures, and
  the executable `examples/modules/main.nexa` four-module example.

This milestone remains a supported predecessor of v0.7.

## Language Core v0.5 (Delivered)

- Added nominal tagged union declarations, qualified constructors, positional
  payloads, and exhaustive `match` expressions.
- Added stable `UnionId`, `VariantId`, and `PayloadId` identities through typed
  HIR and CFG MIR, including dense resolved tag dispatch.
- Added guarded recursive nominal types and a deterministic recursive-union
  runtime depth limit of 1024.
- Added `E3006` non-exhaustive-match and `E3007` unreachable-arm diagnostics,
  parser recovery, CLI fixtures, malformed-MIR checks, and the executable
  `examples/tagged_unions.nexa` example.

This milestone remains a supported predecessor of v0.6.

## Language Core v0.4 (Delivered)

- Added nominal immutable record declarations, contextually typed record
  literals, exact field validation, and immutable field access.
- Added stable `RecordId` and `FieldId` identities through typed HIR and CFG
  MIR, with resolved record layouts in the reference interpreter.
- Added diagnostics for unknown and missing fields, invalid record operations,
  and direct or indirect recursive record declarations.
- Added accepted and rejected parser, semantic, MIR, interpreter, compiler,
  and CLI coverage plus the `examples/named_records.nexa` example.

This milestone remains a supported predecessor of v0.5. Release notes will be
versioned further when Nexa has a stable release workflow.
