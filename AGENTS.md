# Nexa Compiler Agent Guide

Nexa is a Rust bootstrap compiler workspace. Keep the repository shaped around compiler phases, not generic application layers.

## Project Structure

```text
crates/nexa_span         -> FileId, TextRange, and source spans
crates/nexa_diagnostics  -> errors, warnings, and source labels
crates/nexa_syntax       -> tokens, SyntaxKind, and future lossless CST
crates/nexa_parser       -> parser entry points and recovery diagnostics
crates/nexac             -> CLI entry point
xtask                    -> development automation
docs/spec                -> language design notes
crates/*/tests           -> crate-level integration tests
```

## Dependency Graph

```text
nexac -> nexa_parser -> nexa_syntax -> nexa_span
                    \-> nexa_diagnostics -> nexa_span
```

Do not introduce circular dependencies. Add new crates only when a real phase boundary exists.

## Key Commands

| Command | What it does |
|---------|--------------|
| `make check` | fmt + clippy + test + docs |
| `cargo test --workspace` | Run all workspace tests |
| `cargo doc --workspace --no-deps` | Build documentation |
| `cargo xtask security` | Run optional dependency and security checks |
| `cargo xtask bootstrap-contract` | Validate the pinned Stage 0 provenance and artifact boundary |

Never commit code that fails `make check`.

## Development Workflow

- `mvp` is the default development and release branch.
- Create a new branch from the latest `mvp` for every change; automated branches use the `codex/` prefix.
- Do not push changes directly to `mvp`.
- Every pull request targets `mvp`, contains one independently verifiable slice, and uses squash merge after required checks pass.
- Delete the remote topic branch after merge.
- Toolchain releases remain in `0.0.x` until the ADR-011 self-hosting gate passes. Only the release PR that proves that gate may set version `0.1.0`.

## Compiler Rules

- Parser code must not perform type checking.
- Code generation must not consume syntax tokens or AST directly.
- Diagnostics must use stable source spans.
- Do not add LLVM dependencies before a middle IR exists.
- Do not add JavaScript runtime semantics solely for TypeScript compatibility.
- New language behavior requires accepted and rejected tests.
- Compile-fail tests are first-class tests.
- Avoid empty placeholder crates.

## Rust Conventions

- MSRV is Rust 1.80, so the workspace stays on edition 2021 until MSRV is raised.
- Dependency versions live in root `[workspace.dependencies]`.
- Crate manifests should use `.workspace = true` for shared dependencies.
- Front-end crates should use `#![forbid(unsafe_code)]`.
- Future low-level crates that need `unsafe` must isolate it, document it, and test it.
- Library crates use structured errors; binary crates can use `anyhow`.
