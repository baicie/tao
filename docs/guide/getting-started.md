# Getting Started

## Prerequisites

- Rust stable with MSRV 1.80
- cargo

Optional tools:

```bash
cargo install cargo-deny cargo-audit cargo-machete cargo-llvm-cov git-cliff
```

## Build

```bash
git clone https://github.com/baicie/nexa.git
cd nexa
cargo build --workspace
```

## Test

```bash
cargo test --workspace
```

## Try the CLI

```bash
cargo run -p nexac -- check examples/basic.nexa
cargo run -p nexac -- parse examples/basic.nexa
```

## Next Steps

- Read [Project Structure](/guide/project-structure).
- Read [Development](/guide/development).
- Track language behavior in [Spec](/spec/).
