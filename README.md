# Futao

> **Futao is a systems and application programming language created in memory of Fu Tao.**

See the bilingual [dedication](DEDICATION.md) and
[ADR-010](docs/adr/010-futao-language-name.md) for the naming decision.

This repository is the Rust bootstrap compiler workspace for the delivered
[Nexa Language 1.0 Reference Core](docs/spec/language-1.0.md): a
TypeScript-shaped language with independently specified native semantics.
The existing `nexac 0.0.x`, Cargo crate names, and `.nexa` corpus retain their
historical identifiers until the separately verified Futao toolchain rename.

Language 1.0 is a statically checked, deterministic, multi-file command-line
language executed by the CFG MIR reference interpreter. Its versioned
contracts compose into the [Language 1.0 specification](docs/spec/language-1.0.md),
and the completed integration evidence is retained in the
[delivery archive](docs/project/archive/language-1.0-delivery.md).

The language compatibility version and compiler package version are separate.
The complete Language 1.0 reference core currently ships as the self-use
`nexac 0.0.3`; Rust crate APIs and distribution remain intentionally unstable.

## Layout

```txt
crates/
  nexa_span/         # FileId, TextRange, and source spans
  nexa_source/       # Source file ownership and line/column lookup
  nexa_diagnostics/  # Structured errors, warnings, and source labels
  nexa_syntax/       # Tokens, SyntaxKind, and lossless CST support
  nexa_parser/       # Parser entry points and recovery diagnostics
  nexa_hir/          # HIR lowering, name resolution, and type checking
  nexa_mir/          # MIR lowering and interpreter
  nexa_compiler/     # Compiler driver
  nexac/              # CLI entry point
xtask/               # Repository automation commands
bootstrap/           # Pinned Stage 0 provenance and contract-fail fixtures
docs/spec/           # Versioned Language 1.0 contracts
docs/adr/            # Accepted architecture decisions and history
crates/nexac/tests/  # CLI integration tests
```

## Dependency Direction

```txt
nexac -> nexa_source -> nexa_span
  |
  \-> nexa_compiler -> nexa_source
                     +-> nexa_parser -> nexa_syntax -> nexa_span
                     |                \-> nexa_diagnostics -> nexa_span
                     +-> nexa_hir -> nexa_syntax
                     |             \-> nexa_diagnostics -> nexa_span
                     \-> nexa_mir -> nexa_hir
```

## Commands

```bash
cargo xtask check
cargo xtask bootstrap-contract
cargo run -p nexac -- check examples/practical-core/main.nexa
cargo run -p nexac -- run examples/practical-core/main.nexa -- 20
cargo run -p nexac -- parse examples/practical-core/main.nexa
cargo run -p nexac -- dump examples/futao-2-full-stack/baseline-1.0/main.ft
```

The `0.0.3` explicit compiler input and canonical differential schema is
documented in
[docs/implementation/differential-0.0.3.md](docs/implementation/differential-0.0.3.md).

Install the self-use CLI from a local checkout with:

```bash
cargo install --locked --path crates/nexac
nexac --version
```

Version tags publish checked Linux, macOS, and Windows archives with SHA-256
files through [GitHub Releases](https://github.com/baicie/nexa/releases). The
compiler remains a prerelease and is not published to crates.io.

After `v0.0.3` is published, the same version can be installed reproducibly
from its tag:

```bash
cargo install --locked --git https://github.com/baicie/nexa --tag v0.0.3 nexac
```

Nexa intentionally borrows familiar TypeScript surface syntax without
accepting TypeScript or JavaScript compatibility as a goal. A future TypeScript
interop layer, if needed, belongs in an isolated adapter crate and must lower
into Nexa HIR without leaking a third-party AST or JavaScript runtime semantics
into the core compiler.

Language 1.0 deliberately excludes native AOT, UI, package management,
exceptions, and full TypeScript compatibility. v0.9 adds conformance,
determinism, recovery, fuzz-smoke, stress, and release-baseline coverage without
expanding the delivered language syntax.
