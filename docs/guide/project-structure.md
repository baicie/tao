# Project Structure

| Path | Purpose |
|------|---------|
| `crates/nexa_span` | source identity and text ranges |
| `crates/nexa_diagnostics` | diagnostics and source labels |
| `crates/nexa_syntax` | token and syntax primitives |
| `crates/nexa_parser` | parser entry points |
| `crates/nexac` | compiler CLI |
| `xtask` | development automation |
| `docs/spec` | language design notes |
| `crates/*/tests` | crate-level integration tests |

## Dependency Direction

```text
nexac -> nexa_parser -> nexa_syntax -> nexa_span
                    \-> nexa_diagnostics -> nexa_span
```

Keep the graph acyclic. Avoid catch-all crates.
