# Language Core v0.6 Roadmap

## Status

Language Core v0.6 is delivered. Its executable behavior is defined by the
[v0.6 specification](../../spec/language-core-v0.6.md), and the compiler
satisfies every exit criterion and verification gate below. v0.7 bounded
generics, v0.8 practical-core function values, v0.9 stabilization, and the
[Language 1.0 integration](language-1.0.md) have since been delivered.

## Goal

Deliver one complete multi-file vertical slice: explicit relative imports,
private-by-default exports, deterministic source loading and module graphs,
module-aware resolved identities, cross-file typed HIR and CFG MIR, entry-only
`main` selection, deterministic interpretation, and stable cross-file
diagnostics from the lossless CST through the CLI.

v0.6 must preserve v0.5 behavior except that `import`, `export`, and `from`
become reserved words. It must not begin v0.7 generics/`Option`/`Result`, add a
package manager, or introduce runtime module objects.

## Branch Sequence

Implementation proceeds as reviewable branches based on the previous branch
and merged into `codex/language-1.0` in this order:

1. `codex/v0.6-contract`: freeze grammar, portable paths, source-provider
   ownership, graph order, identities, visibility, entry behavior,
   diagnostics, labels, and the complete fixture matrix.
2. `codex/v0.6-syntax`: add `import`/`export`/`from`, import lists and paths,
   export modifiers, lossless CST nodes, typed syntax accessors, and recovery.
3. `codex/v0.6-source-session`: add canonical `SourceKey` resolution, an
   in-memory and CLI filesystem provider, one session-owned `SourceMap`, DFS
   graph construction, deduplicated loading, SCC detection, `ModuleId`, and
   `E4001`/`E4002`.
4. `codex/v0.6-module-hir`: allocate module-aware function/record/union IDs and
   kind-safe `DefId` values, collect exports, bind imports into the existing
   two namespaces, resolve all
   cross-file references, and implement `E4003`/`E4004`/`E4005` plus import
   collisions.
5. `codex/v0.6-mir-runtime`: lower a complete checked session into
   module-aware CFG MIR, record the resolved entry identity, execute
   cross-module calls/layouts, and reject malformed programmatic MIR safely.
6. `codex/v0.6-integration`: add the diamond example and all CLI fixtures,
   audit v0.5 compatibility, update delivered-status documentation only after
   executable gates pass, and run the full verification matrix.

Every branch must compile and retain tests owned by earlier branches. Syntax
code never reads files or resolves names. The source session never type-checks.
MIR and the interpreter never compare source paths or repeat name lookup.

## Architectural Route

```text
CLI / test provider
        |
        v
SourceKey -> source session -> SourceMap + deterministic module graph
                              |                         |
                              v                         v
                    per-file lossless CST      ModuleId / import edges
                              |                         |
                              +------------+------------+
                                           v
                               module HIR + export tables
                                           |
                                           v
                         typed HIR + module-aware DefId facts
                                           |
                                           v
                              CFG MIR + resolved entry DefId
                                           |
                                           v
                                  reference interpreter
```

The source provider owns I/O and canonical source identity. The session owns
reachability, registration, graph order, caching, and diagnostic aggregation.
Each parser remains a pure one-file consumer. HIR owns internal/external
namespaces, visibility, nominal identity, and entry resolution. MIR consumes
only resolved facts, and the interpreter consumes only MIR.

## Delivery Slices

### Contract And Syntax

- Reserve `import`, `export`, and `from` without importing TypeScript or
  JavaScript parser types.
- Accept non-empty named imports with no aliases or trailing comma and allow
  imports at any top-level source position.
- Accept `export` only before a top-level function, record, or union.
- Preserve trivia, path literal text, and exact byte spans in the CST.
- Recover from every missing brace, comma target, `from`, literal, semicolon,
  and export target into the next import/declaration without loops.
- Keep `nexac parse` one-file-only; seeing an import must never trigger I/O.

### Source Provider And Session

- Define an opaque equality/hashable `SourceKey` and provider boundary that can
  resolve from an importing key and load bytes plus a display path.
- Validate the exact `./`/`../`, `/`, and lowercase `.nexa` path contract
  before provider resolution; never probe alternative filenames.
- Supply an in-memory provider for deterministic tests and a filesystem
  provider only at the CLI boundary.
- Cache successes and failures by `SourceKey`, register valid UTF-8 once, and
  retain the correct file/path for cross-file labels.
- Allocate `ModuleId(0)` to the entry, then allocate modules/files in first
  discovery DFS pre-order while scanning import declarations in source order.
- Deduplicate equal keys across repeated and diamond imports; never request an
  unreachable source.
- Compute SCCs after loading and emit one deterministic `E4002` witness per
  cyclic SCC, including self-imports.

### Module HIR And Resolution

- Extend `FunctionId`, `RecordId`, and `UnionId` with `ModuleId`; allocate each
  kind's module-local index independently in source order and combine them in
  a kind-safe `DefId` sum. Retain owner-scoped field/variant/payload identities.
- Collect local declarations and export tables before checking bodies so
  imports and top-level references are module-wide and source-order independent.
- Keep internal value/type namespaces separate while exposing a single
  external function/record/union export namespace.
- Insert resolved imports into the correct internal namespace without copying
  nominal definitions; a diamond must preserve exact identity.
- Diagnose absent, private, and cross-kind-colliding exports with
  `E4003`/`E4004`/`E4005`; retain `E2002` for same-namespace import/local/import
  collisions.
- Resolve calls, named types, record fields, union constructors, patterns, and
  match coverage across files before MIR.
- Treat only a local `main` in `ModuleId(0)` as an entry candidate. A dependency
  `main` remains an ordinary importable function.

### MIR And Interpreter

- Flatten or index checked module definitions by their resolved module-aware
  identities; no MIR operation may depend on a source name or path.
- Preserve module ownership in function, record, union, field, variant, and
  payload layouts and validate owner/kind relationships while lowering.
- Store the entry module and optional resolved entry function in `MirProgram`.
  The interpreter invokes that identity rather than scanning for `main`.
- Execute imported calls and nominal values with unchanged left-to-right and
  exactly-once behavior.
- Share the step, call-depth, and recursive-union-depth budgets across all
  modules and preserve earlier output on failures.
- Retain defining-file spans for runtime errors and turn forged module/DefId,
  kind, entry, and owner/layout mismatches into structured errors.

### CLI And Integration

- Make `check` and `run` create a source session from the CLI entry and render
  all labels through its complete `SourceMap`.
- Leave `parse` as a one-file CST command with no import loading.
- Check the full reachable graph even when the entry has no `main`; require a
  resolved entry only when running.
- Add a multi-file executable example that imports a nominal type and function
  and prints `42`.
- Sort merged diagnostics by primary `(FileId, start, end, code)` before CLI
  rendering.
- Update v0.6/1.0 status pages only after all compiler, MSRV, documentation,
  fixture, and manual CLI gates pass.

## Required Test Matrix

### Lexer, CST, And Recovery

| Area | Required accepted coverage | Required rejected/recovery coverage |
|---|---|---|
| Keywords | all three keywords, adjacency to identifiers, comments/trivia | each keyword used where an identifier is required |
| Imports | one/many names, imports before/between/after declarations, UTF-8 trivia, exact reconstruction | empty list, leading/trailing/doubled comma, missing name/brace/`from`/literal/semicolon |
| Exports | exported function, record, union; trivia between modifier and declaration | `export import`, repeated `export`, local/statement export, missing or malformed declaration |
| Boundaries | multiple imports and declarations reconstruct byte-for-byte | recovery reaches the next import and next declaration without swallowing or looping |

Every malformed syntax case asserts `E1001`, its primary span, CST recovery,
and lossless reconstruction where reconstruction is defined.

### Path And Provider Matrix

| Case | Expected result |
|---|---|
| `./a.nexa`, `../a.nexa`, `../../a.nexa` | accepted and resolved relative to importer |
| `./dir/../a.nexa`, repeated spelling of same canonical key | accepted; one module/load |
| bare `a.nexa`, `a`, absolute `/a.nexa`, `C:/a.nexa`, URL | `E4001` on complete literal |
| `.\\a.nexa`, `./a.NEXA`, `./a`, `./`, `./a.nexa/`, `./a//b.nexa` | `E4001` on complete literal |
| missing or provider-unresolvable target | `E4001` at each referring import; failed key loaded once |
| unreadable target | `E4001` on importing literal with the unreadable reason |
| invalid UTF-8 bytes | `E4001` on importing literal; source is never parsed |
| malformed literal/escape | `E1001`, no provider resolution |
| unreadable entry file | nonzero CLI I/O error, no synthetic source diagnostic |
| unreachable provider source | never resolved, loaded, parsed, or checked |

The same graph cases run against the in-memory provider. Filesystem-provider
tests use isolated temporary directories and portable `/` source spellings.

### Graph, Order, And Identity Matrix

- one-node graph assigns entry `ModuleId(0)` and matching first `FileId`;
- a chain follows DFS pre-order and import source order;
- a diamond loads the shared key once and preserves one `ModuleId`/`DefId`;
- two normalized spellings and provider-recognized aliases reuse one key;
- repeated imports add edges/bindings without allocating another module;
- reordering names inside braces does not change graph/ID order;
- reordering import declarations changes first-discovery order predictably;
- self, two-node, long, and complex multi-edge SCCs each produce exactly one
  `E4002` per SCC, while disjoint SCCs each produce one;
- the first DFS closing edge is primary and the DFS-tree witness edges are
  secondary in cycle order;
- stable re-runs allocate equal `ModuleId` and same-kind local ID values;
- import/export/error recovery does not renumber unrelated same-kind
  definitions, and cross-kind insertion does not change their indices; and
- record/union nominality, field owners, variant owners, and payload positions
  remain distinct and owner-correct across modules.

### Visibility And Namespace Matrix

| Case | Expected result |
|---|---|
| exported function, record, or union imported by exact name | resolves to target `DefId` and exact kind |
| private function/type used inside defining module | accepted |
| private target imported | `E4004`, import-name primary, declaration-name secondary |
| absent target imported | `E4003`, import-name primary, no secondary |
| two absent/private names in one list | one independently spanned diagnostic per name |
| exported function and type with same name | `E4005`, later-name primary, first-name secondary |
| private function and type with same name | accepted internal dual namespace |
| exported item plus private cross-kind same name | exported item resolves unambiguously |
| same-namespace import/local or import/import collision | `E2002`, later binding primary, first secondary |
| cross-namespace imported/local same spelling | accepted |
| imported name used before textual import | accepted module-wide binding |
| import of an imported but non-declared name | `E4003`; no transitive export |
| aliases, export lists, star/default/side-effect/re-export forms | `E1001` |

Cross-file semantic tests must import and use functions, records, tagged unions,
constructors, payload patterns, arrays of nominal types, and guarded recursive
unions. Equal declarations in different modules must remain nominally unequal.

### Entry, MIR, And Runtime Matrix

- private and exported local entry-module `main` both run;
- both valid v0.3 entry signatures retain their exact CLI argument behavior;
- an invalid local entry signature remains `E3003`;
- `check` accepts a graph whose entry has no local `main`;
- `run` with no local entry fails before execution with the fixed missing-main
  runtime message and no output;
- a dependency-only or imported `main` never becomes entry;
- an exported dependency `main` can be explicitly imported under a
  non-conflicting graph and called as an ordinary function, with no entry
  signature special case;
- MIR's entry identity points to `ModuleId(0)` and the interpreter performs no
  `main` string search;
- cross-module call chains share evaluation order, 100000 steps, 64 active
  calls, and union-depth 1024/1025 boundaries;
- a runtime error in a dependency renders that dependency's file/line/column
  and retains output from earlier modules/calls; and
- forged missing modules/definitions, wrong kinds, invalid entry identities,
  and cross-owner record/union layouts return structured errors, never panics.

### Diagnostic And Ordering Matrix

Each `E4001` through `E4005` case asserts code, message, primary `FileId` and
exact byte range, label style, secondary file/range where required, rendered
display paths, and one-based Unicode line/column. Additional fixtures assert:

- `E4001` separately for path shape, resolve/not-found, unreadable, and UTF-8;
- `E4002` for self, simple, complex, and disjoint SCCs with one diagnostic per
  SCC and the exact deterministic witness labels;
- `E4003` versus `E4004` classification when target declarations occupy the
  value namespace, type namespace, both namespaces, or neither;
- `E4005` only for cross-kind external collisions, without duplicating an
  existing same-namespace `E2002`;
- multiple parse, load, visibility, and type errors across at least three files
  sort strictly by `(FileId, start, end, code)` regardless of phase emission;
- exact-key ties remain stable, and secondary labels never affect ordering; and
- invalid/cyclic imports do not suppress independent later source diagnostics.

### CLI Exit Matrix

| Command/scenario | Exit | stdout | stderr |
|---|---:|---|---|
| `nexac parse entry.nexa` with syntactically valid missing import target | 0 | entry CST only | empty |
| `nexac parse entry.nexa` with malformed import | nonzero | recovered CST as currently defined | rendered `E1001` |
| `nexac check entry.nexa` valid graph, no `main` | 0 | `ok` line | empty |
| `nexac check entry.nexa` valid graph and entry | 0 | `ok` line | empty |
| `nexac check entry.nexa` any `E4001`-`E4005` or source error | nonzero | empty | globally sorted diagnostics |
| `nexac run entry.nexa` valid graph | 0 | program output | empty |
| `nexac run entry.nexa -- args` valid argument-taking entry | 0 | deterministic output | empty |
| `nexac run entry.nexa` missing local entry | nonzero | empty | structured missing-main runtime error |
| `nexac run entry.nexa` compile/module error | nonzero | empty | sorted compile diagnostics; no execution |
| `nexac run entry.nexa` dependency runtime failure | nonzero | all earlier output | dependency-located runtime error |
| any command with unreadable entry | nonzero | empty | CLI I/O error |

Tests must assert process status and exact relevant output, not only an
in-process result type.

### Compatibility Matrix

- every v0.5 accepted and rejected fixture retains its result, code, span, and
  runtime output in a one-node module graph;
- explicit regressions prove `import`, `export`, and `from` are newly reserved;
- all earlier `type`, `match`, `case`, and `default` reservation tests remain;
- v0.5 record/union IDs may gain module ownership without changing nominal
  behavior, layouts, match coverage, or runtime depth; and
- no core crate gains OXC, SWC, JavaScript runtime, LLVM, or host-I/O coupling.

## Diagnostic And Span Gates

- `E4001`: complete import path literal, no secondary label.
- `E4002`: deterministic closing-edge path literal, with only the ordered
  DFS-tree cycle-witness path literals as secondary labels.
- `E4003`: exact imported identifier, no secondary label.
- `E4004`: exact imported identifier, with the earliest matching private
  declaration identifier in the target file as secondary.
- `E4005`: later exact exported declaration identifier, with the first
  exported identifier as secondary.
- `E2002`: later same-namespace import/local/import binding, with the first
  binding as secondary.

All compiler diagnostics must have a primary label. The compiler driver merges
then stable-sorts by `(FileId, start, end, code)` before exposing or rendering
diagnostics. Tests assert byte spans and rendered Unicode columns.

## Exit Criteria

- The documented two-file program passes `nexac check` and prints exactly
  `42` through `nexac run`.
- A four-module diamond demonstrates DFS IDs, one load/definition for the
  shared module, cross-module nominal types, and deterministic execution.
- Lossless CST and recovery tests cover every new grammar boundary without I/O,
  hangs, or swallowing a following top-level item.
- Path/provider tests cover every accepted and rejected row above, including
  cached failures, non-UTF-8 bytes, unreachable sources, and entry I/O.
- Module graph tests prove entry zero, source-order DFS discovery, SourceKey
  deduplication, exact SCC witnesses, and one `E4002` per cyclic SCC.
- Typed HIR exposes stable module-aware `DefId` facts and resolves every import,
  cross-module call/type/layout/constructor/pattern before MIR.
- Visibility tests prove private-by-default declarations, the internal dual
  namespace, the external single namespace, no re-export, and exact
  `E4003`/`E4004`/`E4005` labels.
- MIR records a resolved entry identity and module-aware definitions; the
  interpreter performs no source/path/name lookup and safely rejects every
  malformed identity/layout case.
- Entry tests prove `check` without `main`, run-time missing entry, dependency
  `main` behavior, both valid signatures, argument handling, and `E3003`.
- Cross-file runtime tests preserve evaluation order, global limits, defining
  source spans, and output before failure.
- The global diagnostic ordering and complete CLI exit matrices pass with
  exact process statuses, output, file paths, line/column, and codes.
- All v0.5 behavior remains green except the three documented keyword cases.
- `cargo xtask check` passes.
- `cargo fmt --all -- --check` passes.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
  passes.
- `cargo test --locked --workspace --all-targets` passes.
- `cargo +1.80.0 check --locked --workspace --all-targets` passes.
- `cargo doc --locked --workspace --no-deps` passes.
- `pnpm --dir docs build` passes, and all new internal Markdown links resolve.
- v0.6 documentation is marked delivered and the 1.0 roadmap advances v0.7 to
  active only after every gate above passes.

## Deferred Work

Aliases, re-exports, export lists, star/default/namespace/side-effect/dynamic
imports, module values, top-level execution, cyclic-module execution, package
and remote resolution, manifests, a package manager, generics, `Option`,
`Result`, function values, closures, exceptions, native code generation, UI,
and TypeScript ecosystem compatibility remain outside v0.6.
