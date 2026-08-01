# Release-Candidate Validation

Language Core v0.9 is the active stabilization milestone. Language Core v0.8
is the delivered language, and Nexa Language 1.0 has not been released.

The local release-candidate gate is:

```bash
cargo xtask release-check
```

That command validates the candidate without publishing anything. It runs:

1. the locked Rust formatting, lint, test, and rustdoc quality gate;
2. a Rust 1.80 compatibility check;
3. the versioned `conformance/1.0` corpus;
4. stable parser seed replay and a Rust 1.80 fuzz-target compile check;
5. the release-mode performance workload;
6. a release `nexac` build, version smoke, and canonical `check`/`run` smoke;
7. `pnpm --dir docs build`.

The individual repository entry points are:

```bash
cargo xtask check
cargo xtask conformance
cargo xtask fuzz-smoke
cargo xtask perf
cargo xtask release-check
```

The nightly CI fuzz job is intentionally separate: it runs exactly 256
`cargo-fuzz` iterations and uploads crash artifacts. The stable seed replay and
fuzz-target compile check remain part of `release-check`, so local validation
does not require a nightly toolchain.

## Provisional Performance Baseline

The provisional v0.9 release-candidate baseline was recorded on 2026-07-31 on
Windows `x86_64-pc-windows-msvc` with Rust 1.80.0 and release-mode binaries.
The generated source was 14985 bytes with 257 functions. Each workload used
three samples.

| Workload | Samples (microseconds) | Median |
|---|---|---|
| Frontend | 5472, 5618, 6202 | 5618 us |
| Full pipeline | 6293, 7022, 8469 | 7022 us |

Run `cargo xtask perf` to reproduce the workload. This baseline has no timing
threshold, and results from different machines are not directly comparable.
The final v0.9 integration revision must replace the provisional revision in
the release record after the workload is rerun.

## Publication Boundary

`release-check` does not publish crates, create a Git tag, push a branch,
deploy documentation, or package a public binary distribution. Those are
separate, explicitly authorized operations. Passing v0.9 makes the delivered
v0.8 behavior a 1.0 release candidate; it does not declare Nexa Language 1.0
delivered.

Security checks remain separately callable with `cargo xtask security`. That
command treats missing security tools as optional, so a release owner must
install and verify the intended audit tools before treating its result as a
security review.

See the [v0.9 specification](spec/language-core-v0.9.md),
[v0.9 roadmap](project/roadmap/language-core-v0.9.md), and
[Compatibility Policy](compatibility.md).
