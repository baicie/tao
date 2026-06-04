# Development

## Common Commands

```bash
cargo xtask check
cargo xtask fmt
cargo xtask lint
cargo xtask test
cargo xtask security
```

## Adding a Crate

Add a crate only when it represents a real compiler phase boundary. Then:

1. Add it to root `Cargo.toml` workspace members.
2. Add a path entry in `[workspace.dependencies]` if other crates consume it.
3. Keep dependencies flowing in the documented direction.
4. Add accepted and rejected tests for new language behavior.
