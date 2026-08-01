# Nexa Conformance Corpus

The corpus is versioned independently from the Rust crates. `cargo xtask
conformance` runs `conformance/1.0/manifest.tsv` by default. A different
manifest can be selected with `--manifest PATH`.

## Manifest Format

The manifest is UTF-8, line-oriented TSV. Blank lines and lines whose first
non-whitespace character is `#` are ignored. The first two meaningful rows are
fixed:

```text
version<TAB>1.0
name<TAB>mode<TAB>entry<TAB>arguments<TAB>expectation<TAB>expected
```

Every following row has exactly six columns:

| Column | Contract |
| --- | --- |
| `name` | Unique ASCII letters, digits, `.`, `_`, or `-`. |
| `mode` | `check` or `run`. |
| `entry` | Forward-slash relative path below the manifest directory, ending in `.nexa`. Absolute paths, `.`/`..`, backslashes, drive prefixes, and empty segments are rejected. Canonical paths are checked again at execution time so symlinks cannot escape the suite. |
| `arguments` | `-` for no arguments, otherwise a comma-separated list. Check cases must use `-`. |
| `expectation` | `stdout` for an accepted case, `diagnostics` for a rejected check case, or `runtime` for an expected run failure. |
| `expected` | Exact escaped stdout; an ordered comma-separated diagnostic sequence; or an exact runtime message and prior stdout separated by the first decoded tab. Successful `check` output is `ok\n`; successful `run` output contains one trailing `\n` for every printed line. |

Rows run and report in manifest order. The runner executes every row and emits
one `PASS` or `FAIL` record before the summary.

## Escapes

Literal tabs and newlines cannot appear in a row. Text fields support `\\`,
`\t`, `\n`, `\r`, and `\,`. In `arguments`, an unescaped comma starts the next
argument; `\,` is a literal comma, and `\-` represents one literal `-`
argument. Unknown escapes and trailing backslashes are errors with manifest
line numbers.

Diagnostic sequences are compared exactly, so missing, extra, or reordered
codes fail. A runtime expectation uses a value such as `division by zero\t7\n`:
the first decoded tab separates the exact runtime message from stdout emitted
before the failure.

Add source modules beside the manifest under `accepted/` or `rejected/`, then
add one manifest row. Imported modules must remain inside the same versioned
suite.
