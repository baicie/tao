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
cargo run -p nexac -- check examples/named_records.nexa
cargo run -p nexac -- run examples/named_records.nexa
cargo run -p nexac -- parse examples/named_records.nexa
```

`check` parses, resolves, and type-checks the program. `run` executes the
checked program's `main` function through the MIR interpreter. The bundled
record example prints:

```text
Ada
42
```

Language Core v0.4 adds nominal immutable record declarations, contextually
typed record literals, exact field checking, and immutable field access. It
retains the v0.3 immutable UTF-8 `String` and homogeneous `T[]` values. A
program may use either `main(): Unit` with no program arguments or
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
