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

## Commit style

Conventional Commits are recommended:

```txt
feat(parser): parse let statements
fix(nexac): report check diagnostics on stderr
docs: update usage
chore: update dependencies
```
