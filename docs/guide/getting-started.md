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

To install the self-use CLI from that checkout:

```bash
cargo install --locked --path crates/nexac
nexac --version
```

The installed compiler reports version `0.0.7`. This pre-stable tool version
implements the separately versioned Nexa Language 1.0 compatibility baseline.
After the corresponding tag is published, install the same source revision
without keeping a checkout:

```bash
cargo install --locked --git https://github.com/baicie/nexa --tag v0.0.7 nexac
```

## Test

```bash
cargo test --workspace
```

## Try the CLI

```bash
nexac check examples/practical-core/main.nexa
nexac run examples/practical-core/main.nexa -- 20
nexac parse examples/practical-core/main.nexa
nexac dump examples/futao-2-full-stack/baseline-1.0/main.ft
```

`check` parses, resolves, and type-checks the program. `run` executes the
checked module graph's entry-local `main` function through the MIR interpreter.
The bundled practical-core two-module example prints:

```text
42
5
```

Language Core v0.8 adds exact function values, typed arrow functions,
immutable closure captures, array `for...of`, immutable `append`/`concat`,
Unicode-scalar string length, and explicit integer/string conversion. It
retains bounded generics, deterministic relative imports, private-by-default
exports, cross-file diagnostics, immutable UTF-8 `String`, and homogeneous
`T[]` values. A program may use either `main(): Unit` with no program arguments or
`main(args: String[]): Unit` to receive arguments after a second `--`.

Nexa adopts familiar TypeScript-shaped syntax, but it is not a TypeScript or
JavaScript compatibility layer. It deliberately has no JavaScript runtime
semantics such as `any`, implicit `undefined`, truthiness, or implicit
coercions. Any future TypeScript interop belongs in a separate adapter crate,
not in the core parser or IRs.

## Next Steps

- Read the [Language Guide](/guide/language).
- Read the [CLI Guide](/guide/cli).
- Read [Project Structure](/guide/project-structure).
- Read [Development](/guide/development).
- Review the [Compatibility Policy](/compatibility).
- Look up compiler failures in the [Diagnostic Reference](/reference/diagnostics).
- Track language behavior in [Spec](/spec/).
