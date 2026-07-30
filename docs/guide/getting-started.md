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
cargo run -p nexac -- check examples/tagged_unions.nexa
cargo run -p nexac -- run examples/tagged_unions.nexa
cargo run -p nexac -- parse examples/tagged_unions.nexa
```

`check` parses, resolves, and type-checks the program. `run` executes the
checked program's `main` function through the MIR interpreter. The bundled
tagged-union example prints:

```text
42
```

Language Core v0.5 adds nominal tagged unions, qualified variant construction,
guarded recursive data, and exhaustive `match` expressions. It retains nominal
records, immutable UTF-8 `String`, and homogeneous `T[]` values. A program may
use either `main(): Unit` with no program arguments or
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
