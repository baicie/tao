# Bootstrap Contract

`stage0/bootstrap-manifest.json` pins the source-only Rust Stage 0 used to
start Futao self-hosting. It records the exact `nexac 0.0.1` commit, canonical
Git archive and `Cargo.lock` SHA-256 digests, Rust 1.80 toolchain, and locked
release build recipe.

The Bootstrap Stdlib does not exist at this milestone. Its status is pinned as
`not-defined` with null version and digest so an omitted input cannot be
mistaken for a released library. Milestone `0.0.6` replaces that absence with a
real versioned library contract.

Validate the manifest, referenced Git objects, and digests:

```bash
cargo xtask bootstrap-contract
```

Rebuild Stage 0 from a temporary detached worktree and smoke-test its version:

```bash
cargo xtask bootstrap-contract --rebuild-stage0
```

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
