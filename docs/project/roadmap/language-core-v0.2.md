# Language Core v0.2 Roadmap

## Goal

Deliver the stateful control-flow slice defined by
[Language Core v0.2](../../spec/language-core-v0.2.md): initialized mutable
locals, assignment statements, `while`, `break`, `continue`, and Bool-only
short-circuit `&&` and `||`, executed through CFG-based MIR.

This milestone extends v0.1 without adopting TypeScript or JavaScript
compatibility semantics.

## Delivery Branches

1. `codex/v0.2-spec`: lock grammar, semantics, diagnostics, interpreter
   limits, CFG invariants, accepted programs, rejected programs, and
   non-goals.
2. `codex/v0.2-syntax-hir`: add lossless syntax and recovery for `let`,
   assignment, loop statements, and logical operators; lower them into HIR;
   enforce mutability, scope, loop context, and Bool-only conditions and
   operators.
3. `codex/v0.2-cfg-mir`: represent each function as CFG basic blocks and
   terminators; lower loops, loop control, assignment, returns, and
   short-circuit expressions; enforce the 64-active-call limit and global
   100,000-basic-block-step budget in the reference interpreter.
4. `codex/v0.2-integration`: add accepted and rejected fixtures, CST/HIR/MIR
   regressions, CLI end-to-end coverage, documentation, and final workspace
   verification.

Each branch must preserve v0.1 behavior outside the explicitly reserved
`let`, `while`, `break`, and `continue` identifiers, and include rejected tests
for every new diagnostic path it introduces.

## Architectural Route

```text
source -> lossless CST -> HIR -> typed HIR -> CFG MIR -> interpreter
```

The parser recognizes syntax and recovers malformed input without resolving
names or types. HIR owns binding mutability, lexical resolution, type checking,
and loop-context validation. MIR owns explicit control-flow blocks and
terminators. The interpreter only consumes MIR.

The CFG work must establish these invariants before interpreter execution:

- every reachable basic block ends in exactly one terminator;
- every branch target is a valid block in the same function;
- loop `break` and `continue` target the nearest loop exit and condition blocks;
- short-circuit right operands are reachable only through the required branch;
- return paths terminate explicitly rather than falling through.

## Test Slices

- Accepted: initialized inferred and annotated `let`, repeated assignment,
  nested shadowing, zero-iteration and multi-iteration loops, nested loops,
  nearest-loop `break`/`continue`, and observable short-circuit behavior.
- Rejected: missing `let` initializer, assignment type mismatch, assignment to
  `const` or a parameter, undefined assignment target, non-`Bool` `while`,
  non-`Bool` logical operands, and loop control outside a loop.
- Runtime: exactly 64 active calls are allowed and the 65th is rejected;
  exactly 100,000 global basic-block steps are allowed and step 100,001 is
  rejected; output before either runtime error remains observable.
- Compatibility: v0.1 accepted and rejected fixtures retain their expected
  result and diagnostic codes, with explicit regressions for the newly
  reserved identifiers.

## Exit Criteria

- `nexac check` accepts a program that exercises `let`, assignment, `while`,
  `break`, `continue`, `&&`, and `||` together.
- `nexac run` executes that program with deterministic output.
- `&&` and `||` integration tests prove that skipped right operands have no
  runtime effects.
- `const` and parameter assignments report `E2004` with stable source spans.
- `break` and `continue` outside loops report `E3004` with stable source spans.
- A non-`Bool` `while` condition reports `E3002`; invalid logical operands
  report `E3001`.
- CFG tests cover block termination, branch targets, nested loops,
  short-circuit paths, and explicit returns.
- Interpreter boundary tests cover 64 active calls and the global 100,000-step
  budget without off-by-one behavior.
- Language Core v0.1 regression tests remain green except where v0.2
  intentionally reserves new keywords.
- `cargo xtask check` passes on `codex/v0.2-integration`.

## Deferred Work

`var`, increment/decrement operators, assignment expressions, truthiness,
`for`, strings, arrays, modules, native code generation, LLVM, UI, and
TypeScript parser adapters remain outside v0.2. OXC and SWC must not enter the
core parser or IR dependency graph.
