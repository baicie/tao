# Language Core v0.9 Roadmap

## Status

Language Core v0.9 is delivered. Its delivery contract is the
[v0.9 specification](../../spec/language-core-v0.9.md). Its complete gate and
the subsequent Nexa Language 1.0 integration gate passed on 2026-08-01.

This milestone freezes and hardens existing behavior. No slice may add a token,
reserved word, grammar production, type, builtin, semantic rule, MIR operation,
or runtime value.

## Goal

Turn the delivered v0.8 implementation into an auditable 1.0 release candidate
through a versioned conformance corpus, parser robustness work, deterministic
stress coverage, reproducible performance workloads, complete user and
compatibility documentation, and one repeatable release-validation command.

## Frozen Boundaries

- The complete reserved-word set and contextual `of` behavior are fixed by the
  v0.9 specification.
- v0.8 grammar, precedence, evaluation order, static semantics, MIR behavior,
  runtime messages, and resource limits remain unchanged.
- Diagnostic codes, severities, label roles/spans, and ordering are stable;
  ordinary prose is not stable unless a specification quotes exact text.
- The versioned corpus exercises public language and CLI behavior. It does not
  freeze internal Rust layouts or CST debug formatting.
- Fuzzing and stress tests enforce existing invariants rather than accepting
  new source forms.
- Performance results are recorded evidence, not machine-independent pass/fail
  semantics.
- Release validation builds and tests artifacts but does not publish crates,
  create tags, push branches, or deploy packages.

## Delivery Slices

### 1. Contract And Reference Freeze

Suggested branch: `codex/v0.9-contract`.

- Publish the v0.9 specification and this roadmap.
- Publish the complete reserved/contextual word table, operator precedence,
  evaluation rules, resource limits, diagnostic catalog, and compatibility
  policy.
- Define which diagnostic and CLI details are stable and which remain
  presentation or Rust implementation details.
- Align the Language 1.0 target, status pages, navigation, guide, and release
  documentation without marking 1.0 delivered.

Exit criteria:

- Every frozen rule points to delivered behavior in v0.8 or an earlier
  milestone.
- No document promises a syntax or runtime feature absent from v0.8.
- Documentation builds with no broken internal links.

### 2. Versioned Conformance Corpus

Suggested branch: `codex/v0.9-conformance`.

- Establish `conformance/1.0` as the self-contained compatibility corpus.
- Give every case structured metadata for command, entry, arguments,
  expectation kind, ordered diagnostic codes, standard output, or runtime
  failure plus prior output.
- Use a root-bounded source provider and compiler session for self-contained
  corpus cases; retain in-memory crate and CLI regressions for exact labels,
  spans, rendered paths, exit status, standard output, and standard error.
- Compare every manifest field and keep case order deterministic.
- Keep corpus cases self-contained. A minimal source may also appear in a
  structured crate or CLI regression only when that test owns additional span,
  label, rendering, or process-boundary assertions.

Exit criteria:

- Every required diagnostic code is in the corpus, and structured crate/CLI
  regressions cover its required label roles and exact source spans.
- Every delivered feature is represented by the canonical program, a targeted
  accepted corpus case, or the structured compatibility regression matrix.
- Cross-file primary/secondary labels and runtime locations are covered.
- The canonical 1.0 program checks and runs with its specified arguments and
  output.

### 3. Parser Fuzz And Recovery Stress

Suggested branch: `codex/v0.9-parser-robustness`.

- Add a coverage-guided parser fuzz target outside the production workspace.
- Check in a minimized seed corpus spanning full syntax, malformed syntax,
  Unicode, trivia, unknown tokens, and truncation.
- Replay every seed under stable Rust as an ordinary workspace test.
- Add deterministic generated recovery cases for missing delimiters, malformed
  lists, dense errors, and later-declaration recovery.
- Assert losslessness, range validity, UTF-8 boundaries, determinism, progress,
  and absence of panic.

Exit criteria:

- Stable seed replay runs in the normal quality gate.
- Rust 1.80 compiles the fuzz target, while a separate nightly `cargo-fuzz` CI
  job runs exactly 256 iterations and uploads crashing artifacts.
- Every previously identified recovery loop or swallowed-declaration class has
  a regression seed.

### 4. Determinism And Larger Programs

Suggested branch: `codex/v0.9-determinism`.

- Rebuild identical virtual module graphs after changing provider insertion
  order and compare modules, diagnostics, typed HIR, CFG MIR, output, and
  failures.
- Exercise larger chains and diamonds while preserving first-discovery module
  identity and one load per canonical key.
- Produce multiple independent diagnostics across files and assert complete
  global ordering.
- Combine generic limits, closures, iteration, nominal data, and runtime limits
  in programs larger than feature-focused unit fixtures.
- Retain structured failures for malformed programmatic MIR.

Exit criteria:

- Repeated runs are structurally equal rather than merely producing matching
  text output.
- No stress test depends on hash iteration, host path ordering, locale, clock,
  or wall-clock performance.
- Existing exact resource boundaries remain unchanged.

### 5. Performance Baseline

Suggested branch: `codex/v0.9-performance`.

- Replace the inert workspace-root benchmark placeholder with benchmarks owned
  by the compiler phase they measure.
- Measure one fixed release-mode generated program through the aggregate
  frontend check path and the complete compile, MIR-lowering, and interpreted
  execution path.
- Generate workload source outside measured regions and validate results in
  ordinary tests.
- Record the revision, toolchain, host, workload, and statistics used for the
  release-candidate baseline.
- Compile benchmarks in CI without enforcing machine-specific timing limits.

Exit criteria:

- One documented command reproduces every workload.
- Baseline data identifies enough environment detail for a meaningful later
  comparison.
- A claimed regression or improvement is supported by equivalent runs and
  profiling, not one noisy sample.

### 6. Guide And Compatibility Policy

Suggested branch: `codex/v0.9-guides`.

- Complete a language guide organized around the delivered programming model.
- Document `nexac parse`, `check`, and `run`, entry signatures, arguments,
  output, diagnostics, runtime failures, and exit behavior.
- Publish a diagnostic-code reference with primary and secondary label roles.
- Define source, semantic, diagnostic, CLI, conformance, Rust API, and
  performance compatibility boundaries.
- State the 1.x rule that no ordinary 1.0 identifier becomes newly reserved.

Exit criteria:

- A user can build, check, and run the canonical program from the guide.
- Every reference page links to the normative milestone specification instead
  of inventing a second semantic definition.
- The compatibility policy distinguishes guaranteed behavior from internal or
  presentation details.

### 7. Release Validation

Suggested branch: `codex/v0.9-release-check`.

- Add `conformance`, `fuzz-smoke`, `perf`, and `release-check` automation to the
  existing `xtask` command surface.
- Run locked formatting/lint/test/doc checks, Rust 1.80 checking, the
  documentation-site build, conformance, parser smoke, release binary build,
  and a version/CLI smoke.
- Keep optional security tools separately callable; do not silently skip a
  required release gate.
- Add CI wiring for the release check without replacing the existing
  cross-platform and documentation jobs.

Exit criteria:

- One documented command reproduces the local release-candidate gate.
- CI installs every required tool rather than relying on developer state.
- Validation performs no publication, tag, push, or deployment side effect.

### 8. Integration And Delivery

Suggested branch: `codex/v0.9-integration`.

- Run the complete corpus, fuzz seed replay, stress suite, performance
  workloads, Rust checks, rustdoc, documentation build, and release validation.
- Review dependency direction and public Rust documentation for every phase.
- Verify the canonical multi-file acceptance program manually through `parse`,
  `check`, and `run`.
- Update status pages only after every gate passes.

Exit criteria:

- v0.9 is marked delivered.
- The separate Nexa Language 1.0 integration gates pass and publish the
  reference-core compatibility baseline.

## Required Test Matrix

Conformance corpus and structured diagnostic regressions:

- every primitive type, operator class, binding, branch, loop, and return;
- strings, arrays, records, unions, exhaustive matching, and error values;
- direct/generic/indirect calls, closures, capture forwarding, and recursion;
- modules, visibility, cycles, diamonds, entry selection, and CLI arguments;
- every `E` diagnostic code in the corpus, with exact label roles and byte
  spans in crate/CLI regressions; and
- every normative runtime failure and resource boundary with prior output.

Parser robustness:

- accepted full-surface source and lossless trivia;
- one-token and delimiter truncation at representative grammar boundaries;
- invalid strings, unknown tokens, malformed separators, and nested recovery;
- Unicode before and inside labeled ranges; and
- later statements and declarations after malformed constructs.

Determinism and stress:

- provider insertion-order permutations;
- larger module chains and diamonds;
- multiple cross-file diagnostics and exact ordering;
- repeated typed HIR/MIR equality; and
- combined direct/indirect calls, loops, generic instances, and immutable data.

## Verification Gates

- `cargo fmt --all -- --check` passes.
- locked Clippy passes for the workspace, all targets, and all features with
  warnings denied.
- locked workspace tests pass for all targets.
- Rust 1.80 checks the locked workspace and all targets.
- locked workspace rustdoc builds without dependency docs.
- `cargo xtask check` passes.
- the versioned 1.0 conformance runner and structured crate/CLI diagnostic
  regressions pass.
- stable parser seed replay and the bounded fuzz smoke pass.
- the performance workloads compile, run, and have a recorded baseline.
- `pnpm --dir docs build` passes with valid internal links.
- the release binary reports its version and the canonical CLI program has the
  exact expected output.
- a final diff audit confirms that no source or runtime behavior was added.

## Deferred Work

Every Nexa Language 1.0 non-goal remains deferred. In particular, v0.9 does
not add `Float`, exceptions, async/concurrency, mutable heap collections,
native code generation, UI, FFI, package management, LSP, TypeScript
compatibility, or JavaScript runtime semantics.
