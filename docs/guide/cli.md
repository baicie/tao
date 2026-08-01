# CLI Guide

`nexac` is the command-line driver for the Nexa reference compiler. It exposes
three commands: `parse`, `check`, and `run`.

## Parse One File

```bash
cargo run -p nexac -- parse examples/practical-core/main.nexa
```

`parse` reads exactly the named source file, tokenizes it, builds a lossless
concrete syntax tree, prints a deterministic developer-oriented tree dump to
standard output, and renders lexical or syntax diagnostics to standard error.
It does not resolve or load imports and does not perform name or type checking.

The command succeeds when parsing produces no error diagnostic. It fails with
a nonzero status after printing the recovered CST when syntax errors exist.
The exact CST debug indentation is an implementation-facing format rather than
a source-language serialization format.

## Check A Program

```bash
cargo run -p nexac -- check examples/practical-core/main.nexa
```

`check` treats the named file as the entry module. The CLI source provider
loads its reachable explicit relative imports, and the compiler session then:

1. parses each source once;
2. builds a deterministic module graph;
3. lowers the lossless CSTs to module HIR;
4. resolves names, imports, visibility, nominal identities, and generic facts;
5. type-checks the complete graph; and
6. retains globally sorted structured diagnostics.

A successful check prints:

```text
ok
```

Any error diagnostic produces a nonzero status. `check` does not execute
`main` and does not require the entry module to declare one.

## Run A Program

```bash
cargo run -p nexac -- run examples/practical-core/main.nexa -- 20
```

`run` performs the complete `check` pipeline, lowers validated typed HIR to CFG
MIR, and invokes the reference interpreter. Source text is never executed
directly, and the interpreter does not resolve source names.

A runnable entry module declares one of these signatures:

```nexa
function main(): Unit {}
function main(args: String[]): Unit {}
```

Arguments after the second `--` are passed unchanged as the `String[]` value
for `main(args: String[])`. With no trailing arguments, that entry receives an
empty array. Passing arguments to parameterless `main()` is a runtime failure.
An imported `main` is not selected as the entry module's `main`.

## Output And Failures

Nexa `print` writes one value per line to standard output. Compiler diagnostics
and runtime errors are written to standard error. A runtime failure preserves
every line already printed, stops later evaluation, and exits unsuccessfully.

A source diagnostic is rendered with its stable code and every label's path,
line, and column. Cross-file secondary labels retain the path of the source that
owns their span. Runtime errors render their message and source location but do
not use an `E` code.

For example, a static diagnostic has this general shape:

```text
E3001 Error: <message>
  --> path/to/file.nexa:line:column: <label>
```

Scripts should rely on process success, documented program output, diagnostic
codes, and source locations. Ordinary English diagnostic wording and host
absolute-path spelling are not stable interfaces.

## Resource Limits

The reference interpreter uses one global budget per run:

- at most 64 active source-function and closure calls;
- at most 100000 executed CFG basic-block steps; and
- at most 1024 recursive-union nesting levels.

The compiler session accepts at most 256 distinct closed generic semantic
instances. Crossing a boundary produces the diagnostic or runtime behavior in
the [Diagnostic Reference](../reference/diagnostics.md).

## Imports And Paths

Source imports are explicit, relative, and end in `.nexa`:

```nexa
import { Result, transform } from "./support.nexa";
```

The source provider owns host file access and canonicalization. Running Nexa
code cannot observe canonical keys or host paths. The parser itself never reads
another file.

## Current Example

The delivered v0.8 example combines modules, records, unions, generics,
function values, a closure, immutable arrays, `for...of`, conversion, and CLI
arguments:

```bash
cargo run -p nexac -- check examples/practical-core/main.nexa
cargo run -p nexac -- run examples/practical-core/main.nexa -- 20
```

Expected output:

```text
42
5
```

See the [Language Guide](language.md),
[Compatibility Policy](../compatibility.md), and
[Language Core v0.8 specification](../spec/language-core-v0.8.md).
