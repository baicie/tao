# Futao 0.0.3 Canonical Differential Contract

## Status and Scope

This document freezes the comparison boundary delivered by toolchain `0.0.3`.
It is an internal pre-`0.1.0` compiler schema, not a public package artifact or
stable component format. Schema changes require a version increment and fixture
updates; consumers fail closed on an unknown schema.

The available implementation is the Rust reference compiler. The Futao
self-hosted implementation does not exist yet and is reported as unavailable.
NIR is planned for `0.0.5` and is also reported as unavailable. Neither is
represented by generated placeholder output.

## Pure Compiler Input

`nexa_compiler::compile` accepts only a `CompilerInput` value:

```text
CompilerInput
  entry: logical source identity
  sources: unordered UTF-8 CompilerSource collection
  options.languageVersion: 1.0
```

A logical source identity is a non-empty, relative UTF-8 path using `/`. It:

* is already lexically normalized and contains no empty, `.` or `..` segment;
* contains no `\\`, NUL, query, fragment, scheme/drive separator, or absolute root;
* ends in lowercase `.ft` or `.nexa`; and
* is unique in the source collection, with the selected entry present exactly once.

The compiler rejects invalid, duplicate, and missing-entry inputs before parsing.
It does not read the filesystem, environment, current directory, clock, random
source, process state, locale, or timezone. A Host shell resolves physical paths
and builds the explicit source collection.

`.ft` is the source identity for new Futao code. `.nexa` remains accepted for the
Language 1.0 corpus. Imports may cross the two suffixes during migration; removing
that compatibility requires a separate accepted migration decision.

The current compiler has one language option, Nexa Language 1.0. Target and
optimization options are added only when a real NIR/backend boundary exists.

## Structured Output

`CompilerOutput` owns:

* the source map used by stable spans;
* ordered canonical diagnostics;
* checked typed HIR when semantic analysis succeeds;
* complete CFG MIR when lowering succeeds; and
* `CanonicalDumps` for all six planned phases.

Every phase has a discriminated state:

```text
Produced(canonical JSON)
SkippedDueToDiagnostics
Unavailable(planned version)
```

An empty string or `None` never stands for multiple states. Rejected sources still
produce token, CST, and diagnostic artifacts. Their HIR and MIR are skipped. NIR
is unavailable with planned version `0.0.5`.

## Canonical JSON

The root document printed by `nexac dump` has this logical shape:

```json
{
  "schemaVersion": 1,
  "languageVersion": "1.0",
  "compilationProfile": "application",
  "artifacts": [
    { "phase": "tokens", "artifact": { "state": "produced", "content": "..." } },
    { "phase": "cst", "artifact": { "state": "produced", "content": "..." } },
    { "phase": "diagnostics", "artifact": { "state": "produced", "content": "..." } },
    { "phase": "hir", "artifact": { "state": "produced", "content": "..." } },
    { "phase": "mir", "artifact": { "state": "produced", "content": "..." } },
    { "phase": "nir", "artifact": { "state": "unavailable", "planned_version": "0.0.5" } }
  ]
}
```

`compilationProfile` is `application` or `futao-bootstrap-v1`; otherwise identical
source graphs compiled under different capability surfaces remain distinct canonical
builds. The `content` field is itself a compact JSON envelope with `schemaVersion`,
`phase`, and `value`. Object fields, arrays, and phase entries have fixed order.
Strings use JSON escaping; byte offsets, source ordinals, module ordinals, local
slots, and block IDs are checked unsigned integers.

### Tokens

Token entries are grouped by module discovery ordinal. Each token records the
frozen numeric `SyntaxKind`, half-open byte range, and exact source text. Trivia
is retained. No Rust `Debug` name is part of the schema.

### CST

CST entries are a lossless preorder stream. Each entry records depth, whether it
is a node or token, numeric `SyntaxKind`, half-open byte range, and token text when
applicable. Depth plus preorder defines parentage and child order, including
parser recovery nodes.

### Diagnostics

Canonical diagnostics contain stable code, severity, and every ordered label's
style, source discovery ordinal, and half-open byte span. Diagnostic and label
wording remains available in structured `CompilerOutput`, but is deliberately
excluded from canonical comparison as allowed presentation text under ADR-000.

Diagnostics have a total deterministic order: primary source ordinal/range,
code, severity, complete label sequence, then presentation text for the in-memory
structured list. Host paths and provider error strings do not enter either the
canonical artifact or stable diagnostic messages.

### Typed HIR

Canonical HIR is an independent DTO rather than serialized Rust fields. It covers
the complete lowered module/declaration/statement/expression structure and all
typed semantic facts: expression types, name resolutions, function and closure
slots, record/union layouts, generic parameters, constructor instantiations,
direct calls, and match dispatch/bindings.

All map-backed facts are converted to vectors sorted by `(source, start, end)`.
Definition identities are structural module/declaration ordinals. Rust memory
addresses, private layout, hash insertion order, and implementation-only IDs are
not observable.

### MIR

Canonical MIR is a separate DTO covering modules, entry identity, nominal layouts,
function and closure frames, ordered basic blocks, every statement, terminator,
expression, callee, type, local slot, and source span. It is constructed only from
validated typed HIR and does not read tokens or CST.

### NIR

Schema version 1 reserves the NIR phase name but emits no NIR content. `0.0.5`
must replace `Unavailable` with a typed, independently verified canonical NIR
schema; it must not reinterpret the absence marker as an artifact.

## Differential Reports

`DifferentialHarness` runs two real `CompilerAdapter` values over the same
explicit input and compares every phase artifact byte-for-byte. A missing
implementation uses `CompilerAdapterState::Unavailable`; it is not emulated and
the outcome is not `Match`.

Each corpus run has a stable case ID. A phase difference records reference and
candidate SHA-256 digests and starts as `Unclassified`, which fails the gate.
Review may assign exactly one ADR-000 category:

```text
Rust Compiler Bug
Futao Compiler Bug
Specification Ambiguity
Canonicalization Bug
Allowed Diagnostic Text Difference
```

Because wording is absent from canonical diagnostic artifacts, a structural
phase difference cannot be classified as allowed diagnostic text. No wildcard,
case-prefix, or phase-wide suppression exists.

## Determinism Gate

The `0.0.3` tests compare complete canonical output across:

* repeated execution of accepted and rejected inputs;
* forward, reverse, and unrelated source insertion order;
* a 32-module graph containing map-backed typed facts;
* different logical and physical root paths;
* separate processes with different working directories, locale, timezone, and
  unrelated environment values; and
* stable rejected diagnostic codes, severities, label styles, and byte spans.

Local validation uses:

```bash
cargo test -p nexa_compiler --test core_api --test differential
cargo test -p nexac --test cli
cargo run -p nexac -- dump examples/futao-2-full-stack/baseline-1.0/main.ft
make check
cargo xtask release-check
```

The release check parses the `nexac dump` result and requires schema version 1
with all six phases. This gate proves the Rust comparison boundary only; it does
not claim Rust/Futao equivalence before a real Futao adapter is available.
