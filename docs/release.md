# Nexa Language 1.0 Release Validation

Language Core v0.9 and the Nexa Language 1.0 integration release are delivered.
The complete gate passed on 2026-08-01. This records the reproducible
validation boundary; it does not claim that the Rust crates have stable 1.0
APIs or have been published to a package registry.

The local compatibility gate is:

```bash
cargo xtask release-check
```

The command requires the current stable Rust toolchain, an installed Rust
1.80.0 toolchain, Node.js 22, and pnpm 9. Install the documentation dependencies
once with `pnpm --dir docs install --frozen-lockfile`. The local gate compiles
the fuzz target but does not require `cargo-fuzz`; the separate nightly CI job
owns bounded coverage-guided execution.

That command validates the candidate without publishing anything. It runs:

1. the locked Rust formatting, lint, test, rustdoc, Bootstrap Profile/Stdlib,
   and Rust/Futao lexer/parser/resolver differential quality gates;
2. a Rust 1.80 compatibility check;
3. Stage 0 manifest/digest verification, a clean Rust 1.80 locked rebuild, and
   a complete Bootstrap Stdlib check through the deterministic `.ft` to
   historical `.nexa` compatibility projection;
4. the versioned `conformance/1.0` corpus;
5. stable front-end seed replay, the checked-in lexer, parser, and resolver differential
   corpora, and a Rust 1.80 front-end fuzz-target compile check;
6. the release-mode performance workload;
7. the private NIR artifact accepted/rejected contract;
8. a release `nexac` build, version smoke, canonical `check`/`run` smoke, and
   a six-phase scalar `.ft` dump whose NIR phase is `produced`;
9. `pnpm --dir docs build`.

The checkout must include the pinned Stage 0 commit history. In a shallow clone,
fetch that history before running the gate.

The individual repository entry points are:

```bash
cargo xtask check
cargo xtask conformance
cargo xtask bootstrap-contract
cargo xtask bootstrap-profile
cargo xtask lexer-differential
cargo xtask parser-differential
cargo xtask resolver-differential
cargo xtask nir-artifact
cargo xtask fuzz-smoke
cargo xtask perf
cargo xtask release-check
```

The nightly CI fuzz matrix is intentionally separate: it runs exactly 256
`cargo-fuzz` iterations per front-end target, including the resolver, and uploads crash artifacts. The
stable seed replay and fuzz-target compile check remain part of `release-check`,
so local validation does not require a nightly toolchain.

## Delivery Performance Baseline

The final Language 1.0 baseline was recorded on 2026-08-01 from implementation
commit `4a38329` on
Windows `x86_64-pc-windows-msvc` with Rust 1.80.0 and release-mode binaries.
The generated source was 14985 bytes with 257 functions. Each workload used
three samples.

| Workload | Samples (microseconds) | Median |
|---|---|---|
| Frontend | 5615, 5621, 6014 | 5621 us |
| Full pipeline | 6078, 6899, 7712 | 6899 us |

Run `cargo xtask perf` to reproduce the workload. This baseline has no timing
threshold, and results from different machines are not directly comparable.
The samples are an engineering baseline only; the conformance gate validates
results, not elapsed time.

## Publication Boundary

`release-check` does not publish crates, create a Git tag, push a branch,
deploy documentation, or publish a binary distribution. Publishing remains a
separate, explicitly authorized operation. The completed integration process,
not the command by itself, declares Nexa Language 1.0 Reference Core delivered.

The self-use compiler is versioned independently as `nexac 0.0.9`. After a
release commit is squash-merged to `mvp`, create and push the matching annotated tag:

```bash
git switch mvp
git pull --ff-only
git tag -a v0.0.9 -m "nexac 0.0.9"
git push origin v0.0.9
```

The [release workflow](https://github.com/baicie/tao/actions/workflows/release.yml)
rejects a tag that does not exactly match the Cargo package version or does not
point to `mvp`. It then runs the complete release and security gates, builds
and smoke-tests Rust 1.80 binaries on Linux, macOS, and Windows, verifies the
archives after extraction, attaches `.tar.gz` archives and SHA-256 files, and
publishes the result as a GitHub prerelease. It does not publish any crate to
crates.io; every workspace package explicitly disables registry publication.

All validation and platform builds finish before GitHub Release creation, so
failures in those jobs can be retried without moving the tag. If publication
is interrupted and leaves a draft, delete that draft with
`gh release delete v0.0.9 --yes` before rerunning the workflow. If the tagged
source itself needs correction, increment the package version and create a new
tag rather than rewriting the existing tag. Reinstall any earlier tag to roll
back, or remove the CLI with `cargo uninstall nexac`.

Security checks remain separately callable with `cargo xtask security`. That
command treats missing security tools as optional, so a release owner must
install and verify the intended audit tools before treating its result as a
security review.

See the [Language 1.0 specification](spec/language-1.0.md),
[delivery archive](project/archive/language-1.0-delivery.md), and
[Compatibility Policy](compatibility.md).
