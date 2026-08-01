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

## Commit Messages

Use Conventional Commits:

```text
feat(parser): parse const declarations
fix(nexac): report check diagnostics on stderr
docs: update compiler architecture notes
```
