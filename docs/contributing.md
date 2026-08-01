# Contributing

## Development Setup

```bash
cargo xtask check
cargo xtask conformance
```

Before changing the Language 1.0 compatibility baseline, also run
`cargo xtask release-check`. The command validates but does not publish.

Optional tools:

```bash
cargo install cargo-deny cargo-audit cargo-machete cargo-llvm-cov git-cliff
```

## Code Standards

- `cargo xtask check` must pass.
- New language behavior needs accepted and rejected tests.
- Parser changes must not introduce type checking.
- Diagnostics must use stable source spans.
- Front-end crates should forbid unsafe code.

## Branch and Release Policy

Create every change on a dedicated branch from the latest `mvp`. Pull requests
must target `mvp`, contain one independently verifiable slice, and pass all
required checks before squash merge. Do not push directly to `mvp`; delete the
remote topic branch after merge.

Compiler toolchain versions remain in `0.0.x` until the
[ADR-011 self-hosting gate](adr/011-toolchain-versioning-and-self-hosting-gate.md)
passes. The [implementation plan](implementation/self-hosting-0.1.0.md) defines
the only path to `0.1.0`.

## Commit Messages

Use Conventional Commits:

```text
feat(parser): parse const declarations
fix(nexac): report check diagnostics on stderr
docs: update compiler architecture notes
```
