# Futao CLI E2E Tests (0.0.10)

## Status

Implemented as a CLI-level acceptance suite for executable `.ft` programs. The
suite starts the `nexac` process for every case, so it covers argument parsing,
source loading, compilation, execution, output, exit status, and rendered
diagnostics together.

## Corpus

The fixtures live under `crates/nexac/tests/e2e/`:

* `accepted/` checks and runs control flow, generic and tagged-union values,
  multi-file `.ft` imports, and `main(args: String[])` CLI arguments.
* `rejected/` runs `nexac check` and verifies non-zero exit status plus the
  expected diagnostic codes from adjacent `.diagnostics` files.
* `runtime/` verifies output emitted before a failure, non-zero exit status,
  and a runtime source location originating in an imported `.ft` module.

Expected stdout and diagnostic fragments are sidecar files next to each
fixture. This keeps the language source readable and makes output changes
reviewable without editing Rust assertions.

## Verification

Run the suite with the repository-built CLI:

```bash
cargo test --locked -p nexac --test e2e
```

To exercise an installed binary instead, set `NEXAC_E2E_BIN`:

```bash
NEXAC_E2E_BIN=/usr/local/bin/nexac \
  cargo test --locked -p nexac --test e2e
```

The normal workspace test and CI commands include this integration test
automatically. The suite intentionally targets the executable Language 1.0
surface; the `examples/futao-2-full-stack/target-2.0` files remain design
fixtures until their corresponding self-hosting gates are implemented.
