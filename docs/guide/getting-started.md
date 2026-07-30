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
cargo run -p nexac -- check examples/generic-modules/main.nexa
cargo run -p nexac -- run examples/generic-modules/main.nexa
cargo run -p nexac -- parse examples/generics.nexa
```

`check` parses, resolves, and type-checks the program. `run` executes the
checked module graph's entry-local `main` function through the MIR interpreter.
The bundled generic two-module example prints:

```text
42
```

Language Core v0.7 adds bounded generic functions, records, and tagged unions;
local type-argument inference; ordinary source-defined `Option<T>` and
`Result<T, E>` values; and deterministic instance limits. It retains
deterministic relative imports, private-by-default exports, cross-file
diagnostics, immutable UTF-8 `String`, and homogeneous `T[]` values. A program
may use either `main(): Unit` with no program arguments or
`main(args: String[]): Unit` to receive arguments after a second `--`.

Nexa adopts familiar TypeScript-shaped syntax, but it is not a TypeScript or
JavaScript compatibility layer. It deliberately has no JavaScript runtime
semantics such as `any`, implicit `undefined`, truthiness, or implicit
coercions. Any future TypeScript interop belongs in a separate adapter crate,
not in the core parser or IRs.

## Next Steps

- Read [Project Structure](/guide/project-structure).
- Read [Development](/guide/development).
- Track language behavior in [Spec](/spec/).
