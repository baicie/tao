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
cargo run -p nexac -- check examples/language_core.nexa
cargo run -p nexac -- run examples/language_core.nexa
cargo run -p nexac -- parse examples/language_core.nexa
```

`check` parses, resolves, and type-checks the program. `run` executes the
checked program's `main` function through the MIR interpreter and prints `42`
for the bundled example.

Nexa adopts familiar TypeScript-shaped syntax, but it is not a TypeScript or
JavaScript compatibility layer. It deliberately has no JavaScript runtime
semantics such as `any`, implicit `undefined`, truthiness, or implicit
coercions. Any future TypeScript interop belongs in a separate adapter crate,
not in the core parser or IRs.

## Next Steps

- Read [Project Structure](/guide/project-structure).
- Read [Development](/guide/development).
- Track language behavior in [Spec](/spec/).
