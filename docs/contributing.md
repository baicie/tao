# Contributing

## Development Setup

```bash
cargo xtask check
```

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

## Commit Messages

Use Conventional Commits:

```text
feat(parser): parse let statements
fix(nexac): report check diagnostics on stderr
docs: update compiler architecture notes
```
