# Front-End Fuzzing

The `parser` target checks five invariants for every valid UTF-8 input up to
512 bytes:

- parsing does not fail internally;
- the CST reproduces the complete source text;
- tokens and diagnostics are deterministic; and
- the line-oriented CST dump is deterministic; and
- the real Rust and Futao parser snapshots match exactly.

Parser adapter, protocol, or snapshot-validation failures are crashes.

The `lexer` target compares the real Rust and Futao lexer adapters over valid
UTF-8 inputs up to 512 bytes. Every token, trivia bit, byte range, and lexical
diagnostic must match; adapter or protocol failures are crashes.

The checked-in lockfile keeps the target buildable with the workspace MSRV:

```text
cargo +1.80.0 check --locked --manifest-path fuzz/Cargo.toml
```

Bounded corpus replay is part of the normal workspace tests:

```text
cargo test --locked -p nexa_parser --test robustness
```

Continuous mutation fuzzing is an explicit maintainer task and requires
`cargo-fuzz` plus its supported nightly toolchain:

```text
cargo install cargo-fuzz
cargo fuzz run parser
cargo fuzz run lexer
```

Crash artifacts, coverage output, and the fuzz build directory are ignored.
Only minimized regression inputs belong in `fuzz/corpus/parser` or the normal
parser robustness corpus.
