# Examples

The examples are split by compatibility level:

* Existing `.nexa` files and directories target the delivered
  [Nexa Language 1.0](../docs/spec/language-1.0.md) interpreter.
* [`nexa-2-full-stack`](nexa-2-full-stack/README.md) is a contract-first
  showcase for the accepted Nexa 2.0 ADRs. Its `baseline-1.0` subdirectory is
  runnable today; its `target-2.0` sources require the future 2.0 toolchain.

Run the current practical-core example with:

```bash
cargo run -p nexac -- run examples/practical-core/main.nexa -- 20
```

Run the 1.0 baseline of the full-stack showcase with:

```bash
cargo run -p nexac -- run examples/nexa-2-full-stack/baseline-1.0/main.nexa
```
