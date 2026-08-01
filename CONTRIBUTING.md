# Contributing

## Requirements

- Rust stable
- `rustfmt`
- `clippy`

Recommended:

```bash
cargo install cargo-deny cargo-audit cargo-machete cargo-llvm-cov git-cliff
```

## Before opening a PR

```bash
cargo xtask check
cargo xtask security
```

Bootstrap contract changes must also run the clean Stage 0 rebuild:

```bash
cargo xtask bootstrap-contract --rebuild-stage0
```

## Branch and merge workflow

- Start each change from the latest `mvp` on a dedicated branch.
- Open the pull request with `mvp` as its base; do not push directly to `mvp`.
- Keep one independently verifiable delivery slice in each pull request.
- Merge only after required checks pass, using squash merge, then delete the remote branch.
- Keep toolchain releases below `0.1.0` until the ADR-011 self-hosting gate is fully satisfied.

## Commit style

Conventional Commits are recommended:

```txt
feat(parser): parse let statements
fix(nexac): report check diagnostics on stderr
docs: update usage
chore: update dependencies
```
