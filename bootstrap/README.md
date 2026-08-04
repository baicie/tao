# Bootstrap Contract

`stage0/bootstrap-manifest.json` pins the source-only Rust Stage 0 used to
start Futao self-hosting. It records the exact `nexac 0.0.1` commit, canonical
Git archive and `Cargo.lock` SHA-256 digests, Rust 1.80 toolchain, and locked
release build recipe.

Futao bootstrap compiler source version `0.0.3` uses compiler manifest schema 4.
The manifest recursively binds the declared `src` and `typecheck` source roots,
their sorted `.ft` files, and the source-tree digest. Ordered phase records bind
the lexer, parser, resolver, and historical typecheck expression kernel profile
and driver entries, observation and protocol schemas, accepted and rejected
corpora, fuzz seeds, and nullable target-layout descriptor. The compiler still
implements only `lexer`, `parser`, and `resolver`; Rust remains the default
implementation. Schema 4 also binds candidate identity
`futao-bootstrap-candidate` and complete observation schema 1; the reserved
`futao-self-hosted` identity remains unavailable before Stage 1. The Stage 0
schema 4 contract repeats the compiler manifest schema, source roots, digest,
phase records, candidate identity, and observation schema and rejects any
mismatch.

Candidate observation schema 1 is a compare-only transport boundary. It binds
the complete explicit source table and bytes, structured diagnostics, fixed
phase order, state transitions, JSON envelopes, and resource ceilings. HIR,
MIR, and NIR bodies remain opaque canonical bytes until their separately
versioned strict validators construct verified phase values in later slices.

The compiler source tree digest for this milestone is:

```text
sha256:15e4eafa43625e26ae03af2b14d5bd02c94d4239d08fbc3a6ce0cd6bcbea3465
```

TREE-V2 computes that digest over the following byte sequence, where
`frame(bytes) = u64be(bytes.length) || bytes` and every count is encoded as
unsigned big-endian bytes before framing:

```text
SHA256(
  "FUTAO-BOOTSTRAP-COMPILER-TREE-V2\0"
  || frame(u32be(schemaVersion))
  || frame(u64be(sourceRoots.length))
  || each source root as frame(UTF-8 path), in manifest order
  || frame(u64be(sourceFiles.length))
  || each source file as frame(UTF-8 path) || frame(raw UTF-8 contents),
     in manifest order
)
```

Paths are normalized portable relative paths. File contents participate as
checked in, including line endings; the digest performs no text normalization.

Each phase-input digest uses the same framing and the following independent
domain. Resolver accepted and rejected directories each count as one source
graph and must contain `main.ft`; other present inputs count one case per file.
The typecheck expression kernel's absent fuzz corpus binds an empty root, zero
cases, zero files, and the digest of that explicit empty observation. The
reserved `fuzz/corpus/typecheck` path must remain absent, so adding a future seed
cannot bypass a manifest update.

```text
SHA256(
  "FUTAO-BOOTSTRAP-PHASE-INPUTS-V1\0"
  || frame(UTF-8 phase name)
  || frame(UTF-8 category: accepted | rejected | fuzz)
  || frame(UTF-8 declared root, or empty when absent)
  || frame(u64be(case count))
  || frame(u64be(file count))
  || each file as frame(UTF-8 root-relative path)
                  || frame(raw UTF-8 contents),
     sorted by portable root-relative path
)
```

Corpus roots and intermediate directories must be real directories rather than
symlinks. Inputs are discovered recursively; every leaf must be a regular UTF-8
file with the declared extension and a unique portable root-relative path,
including on case-insensitive file systems.

Validate the manifest, referenced bootstrap inputs, Git objects, and digests:

```bash
cargo xtask bootstrap-contract
```

Rebuild Stage 0 from a temporary detached worktree and smoke-test its version:

```bash
cargo xtask bootstrap-contract --rebuild-stage0
```

Run the executable Rust/Futao lexer comparison independently with:

```bash
cargo xtask lexer-differential
```

Run the executable Rust/Futao parser comparison independently with:

```bash
cargo xtask parser-differential
```

Run the executable Rust/Futao resolver comparison independently with:

```bash
cargo xtask resolver-differential
```

The fixed `nexac 0.0.1` predates the Futao rename and accepts only `.nexa`
imports. The rebuild gate therefore creates a temporary compatibility
projection using the current lossless parser: only import-specifier suffixes
are rewritten from `.ft` to `.nexa`, ordinary string contents are preserved,
and the rebuilt compiler must check the complete projected stdlib. The checked
in `.ft` files and their tree digest remain the source of truth.

The contract deliberately separates two artifact classes:

* Bootstrap Stage output is target-neutral internal NIR consumed only by the
  pinned Rust verifier/backend. Schema 1 binds `FUTAO-NIR`, the exact verifier
  and toolchain version, `target-neutral-v1`, private feature flags, strict
  canonical JSON, and SHA-256 content integrity. It has no public extension or
  stable ABI.
* Public stable components use `.nexc`, do not expose MIR/NIR, and evolve under
  ADR-009's independent package/signing lifecycle.

Validation fails closed on unknown fields, unsupported schema values, zero or
malformed digests, public NIR, and mixed component lifecycle declarations. The
checked-in `tests/rejected/public-nir-artifact.json` fixture exercises the
public-NIR and component-lifecycle boundary together.

Validate the private NIR fixture set independently:

```bash
cargo xtask nir-artifact
```

`tests/accepted/minimal-nir.json` is the exact compact compiler serialization.
The rejected fixtures independently cover content mutation, an unknown NIR
field, and an unsupported NIR schema. `xtask` removes one text-file line feed
before loading fixtures; the production loader still requires exact canonical
artifact bytes and rejects trailing whitespace.
