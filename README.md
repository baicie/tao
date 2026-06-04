# Nexa

Nexa is a Rust bootstrap compiler workspace.

## Layout

```txt
crates/
  nexa_span/         # FileId, TextRange, and source spans
  nexa_diagnostics/  # Structured errors, warnings, and source labels
  nexa_syntax/       # Tokens, SyntaxKind, and future lossless CST support
  nexa_parser/       # Parser entry points and recovery diagnostics
  nexac/             # CLI entry point
xtask/               # Repository automation commands
docs/spec/           # Language notes and accepted design decisions
crates/nexac/tests/  # CLI integration tests
```

## Dependency Direction

```txt
nexac -> nexa_parser -> nexa_syntax -> nexa_span
                    \-> nexa_diagnostics -> nexa_span
```

## Commands

```bash
cargo xtask check
cargo run -p nexac -- check examples/basic.nexa
cargo run -p nexac -- parse examples/basic.nexa
```

The first milestone is a small front-end loop: stable spans, diagnostics,
lossless tokens, parser boundaries, and a CLI that can `parse` and `check`.
