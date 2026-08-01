# Development

## Commands

```bash
cargo xtask check
cargo xtask fmt
cargo xtask lint
cargo xtask test
cargo xtask doc
cargo xtask conformance
cargo xtask bootstrap-contract
cargo xtask fuzz-smoke
cargo xtask perf
cargo xtask release-check
cargo xtask security
```

`cargo xtask check` runs formatting, clippy, tests, Rust docs, and the Stage 0
bootstrap contract.
`conformance` runs the versioned language corpus. `fuzz-smoke` replays stable
parser seeds and checks the fuzz target on Rust 1.80. `perf` runs release-mode
reference workloads without a machine-independent threshold. `release-check`
combines the required local release-candidate gates and has no publishing side
effects.

`bootstrap-contract` validates the pinned Stage 0 source and internal NIR
boundary. Add `--rebuild-stage0` to recreate and smoke-test `nexac 0.0.1` with
Rust 1.80 from its fixed source commit.

## Manual Checks

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

## Dependency Policy

All shared versions live in root `[workspace.dependencies]`.
Individual crate manifests use `.workspace = true`.

Before adding a dependency, check that it is needed for the current compiler milestone and that it supports MSRV 1.80.

## Testing Policy

- Unit tests live beside the implementation.
- Integration tests live under the crate that owns the behavior.
- New language behavior needs accepted and rejected tests.
- Compile-fail behavior is a first-class test target.
