# Bootstrap Contract

`stage0/bootstrap-manifest.json` pins the source-only Rust Stage 0 used to
start Futao self-hosting. It records the exact `nexac 0.0.1` commit, canonical
Git archive and `Cargo.lock` SHA-256 digests, Rust 1.80 toolchain, and locked
release build recipe.

Futao bootstrap compiler source version `0.0.3` uses compiler manifest schema 2.
The manifest recursively binds the declared `src` and `typecheck` source roots,
their sorted `.ft` files, and the source-tree digest. It also records three
snapshot schemas, the 11-case lexer, 21-case parser, and 6-case resolver
differential corpora, implemented `lexer`, `parser`, and `resolver` phases, and
the Rust-reference default-path boundary. The Stage 0 schema 2 contract repeats
the compiler manifest schema, source roots, digest, and phase values and rejects
any mismatch.

The compiler source tree digest for this milestone is:

```text
sha256:1982421cdc786e056ca420aad0cc0410b3d790253391e7ced4d4f605d9c8da10
```

Schema 2 computes that digest over the following byte sequence, where
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
