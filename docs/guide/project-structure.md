# Project Structure

| Path | Purpose |
|------|---------|
| `crates/nexa_span` | source identity and text ranges |
| `crates/nexa_source` | source file ownership and line/column lookup |
| `crates/nexa_diagnostics` | diagnostics and source labels |
| `crates/nexa_syntax` | token and syntax primitives |
| `crates/nexa_parser` | parser entry points |
| `crates/nexa_hir` | CST lowering, name resolution, and type checking |
| `crates/nexa_mir` | middle IR lowering and interpreter |
| `crates/nexa_storage` | bootstrap ownership and bounded storage contracts |
| `crates/nexa_compiler` | compiler-driver entry points |
| `crates/nexac` | compiler CLI and diagnostic rendering |
| `xtask` | development automation |
| `docs/spec` | language design notes |
| `crates/*/tests` | crate-level integration tests |

## Dependency Direction

```text
nexac -> nexa_source -> nexa_span
  |
  \-> nexa_compiler -> nexa_parser -> nexa_syntax -> nexa_span
                     |                \-> nexa_diagnostics -> nexa_span
                     +-> nexa_hir -> nexa_syntax
                     |             \-> nexa_diagnostics -> nexa_span
                     \-> nexa_mir -> nexa_hir

nexa_storage -> safe Host-backed bootstrap ownership/storage contracts
```

`nexa_compiler` owns phase orchestration; `nexa_parser` only builds CST and
parser diagnostics, while semantic checks live in `nexa_hir` and execution
receives only MIR. `nexa_storage` is an independent reference kernel until the
Futao compiler starts consuming its frozen contract. Keep the graph acyclic.
Avoid catch-all crates.

Nexa's surface syntax is TypeScript-shaped, not TypeScript-compatible. A
future OXC or SWC integration must live in a dedicated adapter crate and lower
into Nexa HIR without exposing external AST types or JavaScript runtime
semantics to the core compiler.
