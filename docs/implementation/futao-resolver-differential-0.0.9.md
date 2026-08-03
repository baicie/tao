# Futao 0.0.9 Resolver Differential Contract

## Status and Objective

Toolchain `0.0.9` delivers the third executable Futao-written compiler phase:
name resolution. It consumes the verified lexer and parser phases from `0.0.7`
and `0.0.8`, then compares the resolver observables needed by the self-hosting
roadmap. The Rust resolver remains the reference and production implementation.

This milestone adds no language syntax and does not make the Futao resolver the
default compiler path. It proves that a real Futao implementation can build the
same deterministic semantic graph and diagnostics for the checked-in resolver
corpus.

## Boundary

The resolver accepts a validated explicit source graph. Host code owns source
loading and supplies the entry identity and source contents; the Futao phase has
no filesystem, network, clock, process, or ambient output capability. Parsing
and lexical validation are complete before resolution starts. Type checking,
HIR/MIR/NIR lowering, and compiler-driver migration remain later milestones.

The resolver runs under `futao-bootstrap-v1` and emits a strict private protocol
with header `FUTAO-RESOLVER-2`. The Rust adapter validates every field, integer
bound, source span, identity, ordering rule, enum tag, and terminal status before
a snapshot can enter the differential comparison. Unknown internal enum values
are emitted as invalid tags and fail closed instead of being coerced to a legal
observable value.

## Resolver Snapshot Schema 1

The canonical snapshot contains these ordered collections:

```text
ResolverSnapshot
  schemaVersion: 1
  modules: module identity and stable module id
  edges: importer/imported module ids and import path span
  symbols: module-owned definition/layout identities, kind, visibility, and name span
  scopes: module/function/block/match-arm/closure/for scopes and parent links
  bindings: local and generic bindings with declaration spans
  names: every resolved or unresolved name occurrence and target identity
  diagnostics: resolver-only code, severity, labels, and stable source spans
```

Module ids, symbol ids, scope ids, binding ids, and name ids are assigned from
canonical source and traversal order, never from hash-map iteration or the order
in which the caller supplied equivalent source collections. All spans use stable
source ordinals and UTF-8 byte offsets. Diagnostics preserve source order and
include deterministic cycle witnesses. Owner-local child identities for record
fields, union variants, named payloads, and type parameters use contiguous
indices; payloads must reference a declared variant identity.

The snapshot compares six observables independently: module graph, symbols,
scopes, bindings, resolved names, and diagnostics. A difference in any one is a
gate failure; there is no suppression list or expected-divergence escape hatch.

## Implemented Resolver Surface

The Futao resolver now covers:

* deterministic module graph construction, imports, visibility, and cycle paths;
* module, function, block, match-arm, closure, and `for` scopes;
* local bindings and generic type parameters;
* record fields, union variants, and payload bindings;
* qualified and unqualified name resolution;
* resolver diagnostics `E2001`, `E2002`, and `E4002` through `E4005`.

The implementation is intentionally limited to the resolver contract. It does
not infer types, perform ownership checking, lower syntax to MIR, or emit public
artifacts.

## Corpus and Fuzz Boundary

The checked-in corpus has six cases in stable order:

| Category | Cases |
|---|---:|
| accepted graph | 1 |
| rejected graph | 1 |
| resolver fuzz seeds | 4 |
| total | 6 |

The accepted graph exercises cross-module imports, symbols, scopes, bindings,
and name targets. The rejected graph covers missing imports, visibility,
unknown names, duplicate definitions, and a deterministic two-module cycle.
Fuzz seeds cover data declarations, empty input, nested functions, and nested
scopes. Every case runs the real Rust and Futao adapters.

On mismatch, `cargo xtask resolver-differential` retains the source graph and
both canonical snapshots under `target/resolver-differential/<case-id>/`.
Malformed protocol output, adapter failure, invalid snapshots, and execution
limits fail closed as gate errors.

## Content Addressing and Stage 0

`bootstrap/compiler/bootstrap-compiler.json` binds the resolver source files,
protocol entry points, schema version, and exact corpus counts. The source-tree
digest for this milestone is:

```text
sha256:085e030264fd56e3232ce0fcef480dc2b3f0fe79bb41d8bb7b7823d2a026ded8
```

`bootstrap/stage0/bootstrap-manifest.json` repeats the digest and records
`resolver-differential`, compiler version `0.0.3`, and resolver schema 1. The
private NIR verifier remains versioned `0.0.9`; public `.nexc` components and
any stable ABI remain outside this milestone.

## Validation

Run the focused checks:

```bash
cargo test --locked -p nexa_compiler --test resolver_differential
cargo clippy --locked -p nexa_compiler --lib --tests -- -D warnings
cargo xtask resolver-differential
```

The release path additionally runs the workspace, MSRV, Bootstrap Profile,
Stage 0 rebuild, NIR, fuzz, security, and documentation gates. The Rust
resolver remains the default implementation until the type checker, lowering,
driver, and ADR-011 fixed-point stages are complete.

## Rollback and Deferred Work

Rollback is the squash revert of the `0.0.9` PR. The `0.0.7` lexer and `0.0.8`
parser contracts remain usable independently. An incompatible snapshot change
must increment the schema and update the manifest and fixtures atomically.

Deferred to later slices:

* inferred types, generic substitution, match and ownership checking (`0.0.10`);
* HIR/MIR/NIR and compiler-driver equivalence (`0.0.11`);
* C1/C2/C3 construction and normalized fixed-point comparison (`0.0.12+`);
* public compiler protocols, Host ABI, package resolution, and signing.
