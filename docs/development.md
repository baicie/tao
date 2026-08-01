# Development

## Common Commands

```bash
cargo xtask check
cargo xtask fmt
cargo xtask lint
cargo xtask test
cargo xtask conformance
cargo xtask fuzz-smoke
cargo xtask perf
cargo xtask release-check
cargo xtask security
```

`cargo xtask release-check` validates a release candidate without publishing,
tagging, pushing, or deploying. See [Release-Candidate Validation](release.md)
for the complete gate and toolchain boundary.

## Adding a Crate

Add a crate only when it represents a real compiler phase boundary. Then:

1. Add it to root `Cargo.toml` workspace members.
2. Add a path entry in `[workspace.dependencies]` if other crates consume it.
3. Keep dependencies flowing in the documented direction.
4. Add accepted and rejected tests for new language behavior.
