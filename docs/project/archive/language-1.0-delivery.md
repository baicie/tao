# Nexa Language 1.0 Delivery Archive

## Status

Nexa Language 1.0 Reference Core was delivered on 2026-08-01. The stabilized
implementation was committed as `4a38329`; the final acceptance matrix was
completed as `466eeba` on the `codex/language-1.0` branch before publication
through the repository's squash-merge workflow.

This is a language-compatibility milestone. The Rust workspace packages and
`nexac` binary remain version `0.1.0`; Rust crate APIs, HIR/MIR layouts, and
package-registry distribution are not declared stable or published by this
delivery.

## Delivered Milestones

| Milestone | Delivered capability |
|---|---|
| v0.1 | Functions, immutable bindings, static checking, CFG MIR, and interpreted execution |
| v0.2 | Mutable locals, assignment, loops, loop control, and short-circuit Boolean operators |
| v0.3 | UTF-8 strings, immutable arrays, indexing, CLI arguments, and entry-point validation |
| v0.4 | Nominal immutable records with resolved field identities |
| v0.5 | Nominal tagged unions, exhaustive matching, and guarded recursive data |
| v0.6 | Deterministic multi-file modules, imports, exports, visibility, and module diagnostics |
| v0.7 | Bounded generics plus source-defined `Option` and `Result` error values |
| v0.8 | Exact function types, named function values, lexical closures, `for...of`, immutable collection operations, and explicit conversion |
| v0.9 | Frozen compatibility surface, conformance, fuzz/recovery coverage, determinism, performance evidence, guides, and release automation |

The resulting language is a deterministic, statically typed, multi-file CLI
language with TypeScript-shaped syntax and independently specified semantics.
It parses to a lossless CST, resolves and checks typed HIR, lowers to CFG MIR,
and executes through the reference interpreter. It does not inherit JavaScript
coercion, nullability, prototypes, exceptions, or runtime object semantics.

## Acceptance Evidence

The complete command below passed on 2026-08-01:

```bash
cargo xtask release-check
```

That successful gate included:

- locked formatting, Clippy with warnings denied, all-target workspace tests,
  and rustdoc;
- a locked Rust 1.80 workspace and fuzz-target compatibility check;
- all 31 ordered cases in `conformance/1.0`, including every required
  diagnostic code and a runtime failure that preserves prior output;
- six checked-in parser robustness seeds, three coverage-guided fuzz seeds,
  generated UTF-8 truncation and recovery stress, deterministic replay, and a
  bounded fuzz smoke target; the nightly CI job is configured for 256 runs;
- provider-order, typed-HIR, CFG-MIR, diagnostic-order, output, and runtime
  determinism checks over a larger module graph;
- a release `nexac` build, version smoke, and manual `parse`, `check`, and `run`
  validation of the practical multi-file program; and
- the VitePress documentation build and internal-link validation.

The canonical program was checked and run with argument `20`, producing:

```text
ok
42
5
```

VitePress reported only the known non-blocking fallback that `nexa` fenced
code is rendered as plain text because a dedicated highlighter is not yet
registered.

## Performance Record

The release-mode reference workload used Rust 1.80.0 on Windows
`x86_64-pc-windows-msvc`. It generated 14985 bytes of source containing 257
functions and reported three samples per workload:

| Workload | Samples (microseconds) | Median |
|---|---|---|
| Frontend | 5615, 5621, 6014 | 5621 us |
| Full pipeline | 6078, 6899, 7712 | 6899 us |

These numbers are reproducible engineering evidence, not conformance
thresholds. Comparisons require the same workload, build mode, toolchain, and
controlled host.

## Compatibility Boundary

The normative baseline is the ordered composition of the v0.1 through v0.9
specifications, the [Language 1.0 specification](../../spec/language-1.0.md),
the [Compatibility Policy](../../compatibility.md), and expected results in
`conformance/1.0`. Stable behavior includes source grammar, static semantics,
evaluation order, interpreter resource limits, diagnostic codes and structured
labels, and documented `nexac parse`, `check`, and `run` behavior.

## Deferred Beyond 1.0

This delivery intentionally excludes TypeScript/JavaScript compatibility,
native AOT or LLVM, UI, FFI, package management, LSP, formatting, exceptions,
async/concurrency, mutable heap collections, `Float`, and a complete standard
library. Those are independent post-1.0 projects and may not weaken the
compiler phase boundaries or the published Language 1.0 compatibility
contract.
