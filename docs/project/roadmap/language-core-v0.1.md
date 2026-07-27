# Language Core v0.1 Roadmap

## Goal

Deliver a TS-shaped, independently specified Nexa language core that checks
and interprets a single source file. The proof is a program with typed
functions, immutable local values, arithmetic, conditions, and deterministic
diagnostics.

## Delivery Branches

1. `codex/mvp-spec-roadmap`: lock this contract and its non-goals.
2. `codex/source-diagnostics`: add source ownership, diagnostic codes, and
   user-readable locations.
3. `codex/ts-syntax`: parse the v0.1 grammar into a lossless CST with recovery.
4. `codex/hir-typecheck`: lower CST into HIR, resolve names, and check types.
5. `codex/mir-interpreter`: lower typed HIR to MIR and implement `nexac run`.
6. `codex/mvp-integration`: add end-to-end fixtures, documentation, and final
   verification.

## Architectural Route

```text
nexac -> compiler driver -> parser -> syntax / diagnostics -> span
                         -> HIR -> typed HIR -> MIR -> interpreter
```

The parser only creates a CST and parser diagnostics. Name resolution and type
checking live after lowering. The interpreter only receives MIR. New crates
are introduced only when a phase has real behavior to own.

## Exit Criteria

- `nexac check examples/language_core.nexa` completes with no diagnostics.
- `nexac run examples/language_core.nexa` prints `42`.
- Rejected fixtures exercise every required diagnostic code.
- The CST, HIR, typed HIR, and MIR have focused regression tests.
- `cargo xtask check` passes on the integration branch.

## Deferred Work

The next decisions, after v0.1 is demonstrated, are mutation and loops,
strings and collections, multi-file modules, a native backend, and finally UI
libraries and platform adapters. None are prerequisites for this milestone.
