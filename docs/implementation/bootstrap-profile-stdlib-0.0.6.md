# Futao 0.0.6 Bootstrap Profile and Stdlib

## Status and Boundary

Toolchain `0.0.6` freezes the first source capability contract for the future
self-hosted compiler and delivers Bootstrap Stdlib `0.0.1`. The contract is
machine-readable, selected explicitly by the compiler core, content-addressed
in the Stage 0 manifest, and required by local, CI, and release gates.

This milestone does not deliver a Futao-written compiler phase, a general
application standard library, or a new Host capability. The default
`application` profile continues to implement the separately versioned Language
1.0 behavior.

| Area | `0.0.6` contract |
|---|---|
| Profile | `futao-bootstrap-v1`, Language 1.0 semantics, `.ft` source identities |
| Purity | immutable compiler core with no ambient Host effects |
| Diagnostics | `E6201` mutation, `E6202` unbounded loop control, `E6203` ambient output |
| Stdlib | version `0.0.1`, 12 sorted source files, no required capabilities |
| Determinism | source-tree SHA-256 plus profile-aware canonical build SHA-256 |
| Stage 0 | rebuilt `nexac 0.0.1` checks a parser-guided compatibility projection |
| Evolution | canonical three-part versions advance strictly; rollback fixture fails closed |

## Frozen Profile

`CompilerOptions::bootstrap_v1()` selects `futao-bootstrap-v1`; default options
select `application`. The bootstrap input boundary accepts only `.ft` logical
source identities. The selected profile is serialized as `compilationProfile`
in every canonical dump, so an otherwise identical source graph compiled under
different capability surfaces cannot share a canonical build identity.

The profile manifest freezes these source capabilities:

- records, tagged unions, functions, generics, closures, and explicit relative
  imports;
- immutable `const`, `if`, bounded `for...of`, return, expressions, and pattern
  matching;
- `Int`, `Bool`, `String`, `Unit`, arrays, `Option`, `Result`, nominal records,
  tagged unions, and function values;
- pure array length/append/concat, string length, `toString`, and `parseInt`;
- Copy semantics for `Int`, `Bool`, and `Unit`, with immutable owned aggregate
  values and no source-level ownership operations yet;
- deterministic array-index, source-order, or sorted-key iteration policies.

The profile-specific HIR pass emits stable source diagnostics after ordinary
Language 1.0 checking succeeds:

| Code | Rejected source behavior | Required alternative |
|---|---|---|
| `E6201` | mutable `let` or assignment | immutable value construction and return |
| `E6202` | `while`, `break`, or `continue` | bounded `for...of` or explicit recursion |
| `E6203` | ambient `print` | structured data returned to the Host shell |

The machine-readable deny list also contains filesystem, network, process,
clock, environment, random, thread, async, UI, dynamic loading, plugin, and
reflection capabilities. Most do not exist in Language 1.0, so the manifest is
the fail-closed contract and `E6203` is the current observable Host-effect
rejection. Any future capability requires profile lint and accepted/rejected
coverage before compiler source may use it.

## Bootstrap Stdlib 0.0.1

The checked-in stdlib is ordinary Futao source under `bootstrap/stdlib/src`.
Its public surface is deliberately small:

- `Option<T>`, `Result<T, E>`, `Ordering`, `SourceId`, `Span`, `Severity`, and
  structured `Diagnostic`;
- pure array `map`, `filter`, `fold`, `find`, `sortBy`, and `binarySearch`;
- persistent `StringBuilder` and explicit `SourceText` byte, code-point, and
  byte-offset tables;
- persistent `StringMap`, `IntMap`, `StringSet`, `IntSet`, and `BitSet` with
  stable first-insertion order and replacement without key reordering;
- persistent `Arena<T>` handles checked by arena identity, generation, and
  index, with structured wrong-arena, stale-generation, and invalid-index
  errors.

The APIs avoid mutation and implicit I/O. Their implementations favor direct,
deterministic behavior over a premature runtime representation: maps and sets
use persistent arrays, and arena identity is a source value rather than an
unforgeable runtime handle. Resource-bounded allocation, target layout, Drop,
and physical arena storage remain the separate `nexa_storage`, ADR-004, and
later lowering responsibilities.

Behavior fixtures keep test-only `print` calls outside the stdlib source tree.
They verify array/text results, insertion order and replacement, set/bit-set
membership, and arena rejection categories through the existing reference
interpreter. Array transformation, stable sorting, map lookup/replacement, set
membership, and StringBuilder finish also run over 80 elements, crossing the
reference interpreter's 64-active-call boundary. Divide-and-conquer helpers
and bounded `for...of` traversal keep these operations within that resource
contract without weakening the Bootstrap Profile.

## Content Addressing

The profile and stdlib are pinned by these checked-in values:

```text
profile manifest:
  sha256:8f906ed909566e098ee8cd5029ded3b34bfb174619d198ab68bf26e57e63a3cd

stdlib source tree:
  sha256:d302a92827072fdb6c9ef85ffc53cca9472f9dfe69f604132d5acd50a6f92da2

stdlib canonical build:
  sha256:03c183622af98e72f667443a1995f96f0314ab8cf9c7e1eaf70dbe114175538b
```

The tree hash uses a domain separator followed by length-delimited portable
path bytes and source bytes in manifest order. The build hash uses a separate
domain separator and the complete schema 1 canonical compiler dump produced
under `futao-bootstrap-v1`. The gate compiles both forward and reverse input
insertion orders and requires byte-identical dumps before checking the digest.

Package version metadata is deliberately not part of this canonical compiler
dump, so the stdlib build digest does not change merely because `nexac` moves
from `0.0.5` to `0.0.6`. The private `FUTAO-NIR` artifact has a different
contract and does bind the exact compiler version.

The top-level Stage 0 manifest repeats the profile ID/digest and stdlib
version/profile/tree/build values. `bootstrap-contract` reads the referenced
manifests and rejects any mismatch rather than trusting duplicated metadata.

## Fixed C0 Compatibility

The pinned C0 is `nexac 0.0.1` at commit
`9293a7b59ff3b6625ca09091f7ad50234638981f`. It predates the Futao rename and
accepts relative imports ending only in `.nexa`; changing the checked-in `.ft`
source contract to satisfy that historical parser would violate the profile.

`bootstrap-contract --rebuild-stage0` therefore performs a narrow temporary
projection inside its detached worktree:

1. parse every manifest-listed `.ft` source with the current lossless CST;
2. rewrite only string tokens owned by `ImportDeclaration` from `.ft` to
   `.nexa` and rename the projected files accordingly;
3. preserve ordinary string literals and all other source bytes;
4. run the rebuilt pinned C0 over the projected stdlib entry and require exact
   `ok` output with no diagnostics;
5. remove the detached worktree and projection.

The checked-in `.ft` tree and its digest remain authoritative. This proof shows
that fixed C0 accepts the stdlib language semantics despite the historical
extension spelling. The profile-aware canonical build digest is separately
produced by the current Rust reference compiler because `nexac 0.0.1` has no
canonical dump or profile-selection command.

## Version Transitions

Stdlib versions use canonical `major.minor.patch` unsigned decimal components.
The accepted fixture advances `0.0.0` to `0.0.1`; the rejected fixture attempts
`0.0.1` to `0.0.0`. Candidate versions must compare greater than installed
versions, both digests must be non-zero lowercase SHA-256 values, and the
profile ID must remain exact.

These fixtures prove fail-closed ordering, not package-manager resolution or
artifact rollback. ADR-009 owns the later signed package lifecycle.

## Validation

Run the milestone-specific gates with:

```bash
cargo test --locked -p nexa_compiler --test bootstrap_profile
cargo test --locked -p xtask --all-targets --all-features
cargo xtask bootstrap-profile
cargo xtask bootstrap-contract
cargo xtask bootstrap-contract --rebuild-stage0
```

Repository-wide `make check`, Rust 1.80, the private NIR gate, security tools,
and `pnpm --dir docs build` remain release requirements. CI runs profile,
manifest, and private NIR validation explicitly under the MSRV job.

## Deferred Work

The following are explicit non-goals for `0.0.6`:

- a Futao lexer, parser, resolver, type checker, lowering pass, or driver;
- C1/C2/C3 construction or normalized Stage comparison;
- mutable compiler data structures or new source-level ownership operations;
- resource budgets, physical allocation, unsafe memory, or target object layout;
- filesystem, network, process, clock, async, UI, plugin, or package APIs;
- a general standard library or a stable public component format;
- changing the pinned Stage 0 source, Rust verifier/backend, or `.nexc` lifecycle.

The next milestone is `0.0.7`: implement the Futao lexer slice and compare its
token kinds, text ranges, trivia, and lexical diagnostics against the Rust
reference compiler over accepted, rejected, and fuzz corpora.
