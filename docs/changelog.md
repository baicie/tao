# Changelog

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

The active implementation milestone is v0.6: explicit multi-file modules.

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
