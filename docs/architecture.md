# Architecture

Nexa starts as a small compiler front end. The workspace keeps each crate aligned to a compiler responsibility rather than a generic application layer.

## Crate Responsibilities

| Crate | Responsibility |
|-------|----------------|
| `nexa_span` | `FileId`, `TextRange`, and source spans |
| `nexa_diagnostics` | structured errors, warnings, and source labels |
| `nexa_syntax` | tokens, `SyntaxKind`, and future lossless CST support |
| `nexa_parser` | parser entry points and recovery diagnostics |
| `nexac` | command-line interface |

## Dependency Graph

```text
nexac -> nexa_parser -> nexa_syntax -> nexa_span
                    \-> nexa_diagnostics -> nexa_span
```

Rules:

- Parser code does not perform type checking.
- Diagnostics are expressed in terms of stable source spans.
- Code generation crates are deferred until the middle representation exists.
- Placeholder crates are avoided until a phase boundary has real behavior.

## Unsafe Code Policy

Front-end crates use `#![forbid(unsafe_code)]`. The workspace lint is `warn` so future low-level crates can isolate and document necessary `unsafe` without weakening the front end.
