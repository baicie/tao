# Nexa Language Core v0.9

## Status

Language Core v0.9 is delivered. It is the stabilization contract layered over
[Language Core v0.8](language-core-v0.8.md). Its complete release gate passed
on 2026-08-01 and is retained in the
[Language 1.0 delivery archive](../project/archive/language-1.0-delivery.md).

v0.9 adds no source syntax, value type, static rule, MIR operation, builtin, or
runtime behavior. Its purpose is to freeze, exercise, document, and measure the
language that v0.8 already delivers before the Nexa Language 1.0 integration
release.

## Product Contract

The v0.9 product is the v0.8 language with a release-quality compatibility
boundary:

- the lexical surface, grammar, precedence, and evaluation order are fixed;
- required diagnostic codes, label roles, source spans, and ordering are
  cataloged;
- a versioned accepted/rejected corpus exercises source, module, semantic, and
  runtime behavior;
- parser fuzz seeds and recovery stress cases enforce losslessness, progress,
  determinism, and bounded source spans;
- larger deterministic programs and malformed programmatic MIR cases exercise
  phase boundaries without adding language behavior;
- repeatable performance workloads establish a baseline without making
  machine-specific timings part of language semantics; and
- the language guide, CLI guide, compatibility policy, and release gates are
  complete enough to support a 1.0 release candidate.

The detailed compatibility boundary is recorded in the
[Compatibility Policy](../compatibility.md). The complete diagnostic catalog is
the [Diagnostic Reference](../reference/diagnostics.md).

## Normative Language Base

The source and runtime language remain the transitive contract of Language
Core v0.1 through v0.8. Read the milestone specifications in version order. A
later milestone's explicit replacement or extension takes precedence over an
earlier production or rule.

For the 1.0 compatibility baseline, v0.3 is the oldest preserved source level.
The only post-v0.3 keyword compatibility exceptions are the words explicitly
reserved by v0.4, v0.5, v0.6, and v0.8. v0.9 introduces no exception.

## Frozen Lexical Surface

The complete reserved-word set is:

```text
function const let if else while break continue for return
true false Int Bool String Unit type match case default
import export from
```

`of` is contextual only in the binding header of `for...of`. It remains an
ordinary identifier everywhere else. `print`, `toString`, and `parseInt` are
predefined callable names, not keywords; their v0.8 name-resolution behavior
is unchanged.

Identifiers begin with an ASCII letter or `_` and continue with ASCII letters,
digits, or `_`. Integer literals contain ASCII digits. Strings, `//` line
comments, whitespace trivia, delimiters, and operators retain their v0.8
lexical rules. The lossless CST must retain every source byte represented by a
valid UTF-8 source string.

No 1.x release may turn an ordinary 1.0 identifier into an unconditional
reserved word. A future additive form must be contextual and must not change
the parse of a valid 1.0 program.

## Frozen Expression Binding

Binary operators are left-associative. From lowest to highest precedence they
are:

| Precedence | Operators |
|---|---|
| 1 | `||` |
| 2 | `&&` |
| 3 | `===` |
| 4 | `<`, `<=`, `>`, `>=` |
| 5 | `+`, `-` |
| 6 | `*`, `/` |

Prefix `!` and unary `-` bind more tightly than binary operators. Calls,
indexing, and member access bind more tightly than prefix operators and remain
left-to-right postfix operations. Parenthesized expressions override ordinary
binding. Function-type arrows associate to the right under the v0.8 function
type grammar. Assignment remains a statement and is not an expression.

The parser's generic `>` splitting, match-arm `=>`, arrow-expression `=>`,
record literals, and comparison operators retain the disambiguation rules
already specified by v0.5 through v0.8.

## Frozen Evaluation And Runtime Rules

Expressions evaluate exactly once from left to right. This includes callee
before arguments, binary left before right, array and record elements in source
order, constructor payloads in source order, match scrutinee before the selected
arm, member base before intrinsic arguments, and `for...of` iterable before any
iteration.

`&&` and `||` short-circuit. Only the selected `match` arm executes. An arrow
expression snapshots captures when evaluation reaches that expression.
`for...of` visits one already-evaluated immutable array in ascending index
order. A runtime failure stops later effects and preserves output already
emitted.

The reference interpreter retains these global limits:

- 64 active source-function and closure calls;
- 100000 executed CFG basic-block steps per program run;
- 1024 active recursive-union traversal levels; and
- 256 distinct closed generic semantic instances per compiler session.

Calls and closures share one call-depth counter. All functions, closures, and
loops share one execution-step budget. These limits and their boundary behavior
are part of the reference-interpreter compatibility contract.

## Diagnostic Freeze

The required code set is `E1001`, `E2001` through `E2006`, `E3001` through
`E3012`, and `E4001` through `E4005`. v0.9 adds no diagnostic code.

Across the versioned corpus and structured crate/CLI regression tests,
compatibility covers:

- the diagnostic code and severity;
- the ordered primary and secondary label roles;
- each label's owning source file and UTF-8 byte range;
- global ordering by primary `(FileId, start byte, end byte, code)` with stable
  emission order for exact ties; and
- suppression of derivative diagnostics explicitly required by the milestone
  specifications.

Ordinary explanatory prose is not frozen unless a specification explicitly
quotes exact text. This permits clearer wording without weakening machine
readable codes and source locations. Exact reference-interpreter messages that
are listed as normative runtime failures remain fixed.

## Versioned Conformance Contract

The 1.0 compatibility corpus lives under `conformance/1.0`. Each case is
self-contained and declares its entry source, command, arguments, expectation
kind, and expected observable result. Diagnostic cases record the ordered code
sequence; successful execution cases record standard output; runtime cases
record the normative failure message together with output emitted before the
failure. Discovery and execution order are deterministic.

The corpus must include:

- rejected source cases covering every required diagnostic code;
- representative multi-file visibility and module-cycle failures;
- an exact runtime failure with prior-output retention; and
- one canonical multi-file 1.0 acceptance program that combines every feature
  named by the [1.0 target](language-1.0.md).

The complete release-validation matrix combines that public corpus with crate
and CLI regressions. Together they cover every delivered feature, cross-file
labels, diamond identity, generic values, dependency runtime failures, exact
boundaries for all four global resource limits, malformed and overflowing
`parseInt`, and representative `nexac parse`, `check`, and `run` behavior.

The corpus runner compares every field represented by the manifest. It fails
on missing, extra, or reordered diagnostic codes, mismatched standard output,
or a mismatched runtime message and prior output. Structured crate and CLI
regression tests complement that manifest by checking severity, primary and
secondary label roles, owning files, exact UTF-8 byte spans, rendered source
locations, suppression, and global ordering. A diagnostic relocation is
therefore a joint-gate failure even though byte spans are not duplicated in the
manifest.

## Robustness And Determinism Contract

Parser fuzz smoke and its checked-in seed corpus must assert, for every valid
UTF-8 input:

- token text and CST text reconstruct the original source exactly;
- token and diagnostic ranges remain ordered, in bounds, and on UTF-8
  boundaries;
- parsing terminates without panic and always makes recovery progress; and
- a repeated parse produces the same tokens, CST, and diagnostics.

Stable Rust 1.80 must compile the fuzz target. A separate nightly
`cargo-fuzz` CI job runs exactly 256 bounded iterations and uploads any crash
artifacts for investigation.

Recovery stress inputs include truncated delimiters, malformed lists, unknown
tokens, Unicode text, dense trivia, consecutive errors, and valid declarations
after malformed constructs. Later declarations must remain available whenever
the documented recovery boundary permits it.

Repeated compilation of the same virtual module graph, including different
provider insertion orders, must produce equal module order, diagnostics, typed
HIR, CFG MIR, output, and runtime failure. Stress coverage must include larger
module graphs, diagnostic sets, generic instances, closures, and loops. Stress
tests use semantic counts and equality, not wall-clock assertions.

## Performance Baseline Contract

Performance measurements use one fixed, generated workload for the aggregate
frontend check path and for the complete compile, MIR-lowering, and interpreted
execution path. Broader module, generic-limit, and phase-boundary behavior is
validated by deterministic semantic-count tests rather than represented as a
timing threshold. The baseline record identifies the source revision, Rust
version, build mode, host platform, workload size, and reported statistic.

Performance numbers are engineering evidence, not language semantics. Shared
CI compiles the workloads and verifies their results but does not fail on an
absolute millisecond threshold. A regression decision must compare equivalent
release-mode runs on a controlled host and include profiling evidence.

## Delivery Gates

v0.9 is delivered only when:

- the versioned corpus runner and structured crate/CLI diagnostic regressions
  pass;
- parser fuzz seeds, recovery stress, determinism, and larger-program tests
  pass;
- a documented performance baseline can be reproduced;
- the language, CLI, diagnostic, compatibility, and release documentation
  builds with valid internal links;
- the Rust 1.80 workspace checks, current stable checks, rustdoc, and complete
  repository quality gate pass; and
- a final dependency-direction audit confirms that parser, HIR, MIR,
  interpreter, compiler session, and CLI responsibilities remain separated.

These gates passed on 2026-08-01. The subsequent integration gate also passed,
so the stabilized behavior is published as the delivered
[Nexa Language 1.0 Reference Core](language-1.0.md).

## Non-Goals

v0.9 does not add or change source syntax, keywords, builtins, type inference,
data models, runtime values, platform access, or code generation. It does not
stabilize Rust crate APIs, exact CST debug formatting, host operating-system
error text, absolute diagnostic paths, or benchmark timings as language APIs.

All Nexa Language 1.0 non-goals remain in force, including TypeScript or npm
compatibility, JavaScript runtime semantics, exceptions, async/concurrency,
mutable heap collections, native code generation, UI, FFI, package management,
LSP, and a complete standard library.
