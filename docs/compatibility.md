# Compatibility Policy

## Status

Language Core v0.8 is the current delivered language. Language Core v0.9 is
the active stabilization contract, and Nexa Language 1.0 has not been released.
This page defines the compatibility boundary that v0.9 must make executable
before 1.0.

Before 1.0, a versioned milestone specification may make an intentional
breaking change only when it names the change, records any newly reserved word,
and adds regression coverage. v0.9 adds no such change.

## The 1.0 Compatibility Baseline

At the 1.0 release, the following become the source compatibility baseline:

- the reserved and contextual word sets;
- lexical rules, grammar, precedence, and associativity;
- name resolution, type checking, generic inference, and module visibility;
- left-to-right evaluation, short-circuiting, capture snapshots, and loop
  control;
- value representations visible to Nexa programs and the absence of observable
  allocation identity;
- reference-interpreter resource limits and normative runtime failures;
- required diagnostic codes, severity, label roles, source spans, and global
  ordering;
- `nexac parse`, `nexac check`, and `nexac run` command behavior; and
- every expected result in the versioned `conformance/1.0` corpus.

The normative language is the ordered composition of the Language Core
milestone specifications. The
[v0.9 stabilization contract](spec/language-core-v0.9.md) records the frozen
surface, and the [Language 1.0 target](spec/language-1.0.md) records the release
acceptance boundary.

## Reserved Words

The 1.0 reserved-word set is:

```text
function const let if else while break continue for return
true false Int Bool String Unit type match case default
import export from
```

`of` is contextual in a `for...of` header and remains an identifier elsewhere.
`print`, `toString`, and `parseInt` are predefined names rather than keywords.

The compatibility baseline begins with v0.3 source. Later intentional keyword
changes were:

| Milestone | Newly reserved words |
|---|---|
| v0.4 | `type` |
| v0.5 | `match`, `case`, `default` |
| v0.6 | `import`, `export`, `from` |
| v0.7 | none |
| v0.8 | `for` |
| v0.9 | none |

A 1.x release must not turn an ordinary 1.0 identifier into an unconditional
keyword. Additive syntax in 1.x must be contextual and must preserve the parse
and behavior of every valid 1.0 program. A change that cannot meet that rule
requires a new major language version.

## Diagnostics

The stable machine-readable diagnostic surface is:

- code and severity;
- primary and secondary label roles;
- each label's source file and UTF-8 byte span;
- deterministic diagnostic ordering; and
- suppression rules explicitly named by the specifications.

Ordinary diagnostic and label prose may become clearer in a compatible patch
release unless a specification quotes exact wording. Tooling must key behavior
off codes and structured labels rather than parsing English text. The complete
code list is in the [Diagnostic Reference](reference/diagnostics.md).

This surface is guarded jointly. The `conformance/1.0` manifest compares
ordered diagnostic code sequences, successful standard output, and normative
runtime messages together with prior output. Structured crate and CLI
regressions separately assert severity, label roles, owning files, exact byte
spans, rendered locations, suppression, and global ordering. The manifest is
not a serialization format for every structured diagnostic field.

Normative runtime messages are different: messages explicitly listed in the
reference are part of the interpreter contract because runtime failures do not
have `E` codes. Output emitted before a runtime failure remains observable.

## CLI Compatibility

The command names, required operands, `--` argument boundary, success/failure
meaning, standard-output program lines, and structured diagnostic locations
are stable at 1.0. The [CLI Guide](guide/cli.md) defines the user-facing
contract.

The following presentation details are not frozen unless represented in the
conformance corpus:

- exact whitespace or indentation in the developer-oriented CST dump;
- absolute path spelling supplied by the host operating system;
- host I/O error text;
- colors or future terminal decoration; and
- ordinary explanatory diagnostic prose.

Scripts should use process success, stable diagnostic codes, and documented
program output. They should not parse Rust `Debug` output.

## Versioning Rules

After 1.0:

- patch releases may fix implementation bugs, improve non-normative prose, and
  improve performance without changing conforming behavior;
- minor releases may add behavior only when valid 1.0 programs retain their
  parse, type result, evaluation, diagnostics, and output;
- a conformance case may change only with an explicit compatibility review;
- removing or changing a valid source form, evaluation rule, runtime limit,
  required diagnostic code, or normative runtime message requires a major
  version; and
- behavior outside the documented language is not made stable merely because
  one compiler revision happened to accept it.

When implementation behavior conflicts with the normative specification, the
specification and conformance corpus define the intended contract. A bug fix
must include a regression case and release note.

## Explicitly Unstable Surfaces

Nexa Language 1.0 does not stabilize:

- Rust crate APIs, Rust type layouts, or `Debug` representations;
- HIR or MIR serialization, because no public serialization format exists;
- internal numeric IDs across separate compiler sessions;
- memory allocation strategy or immutable-storage sharing;
- benchmark timings as portable guarantees;
- unpublished package or crate distribution; or
- behavior listed as a Language 1.0 non-goal.

Stable semantic identities such as `FunctionId` and `ClosureId` remain
deterministic within the compiler contract where the specifications require
them, but their concrete Rust representation is not a language API.

## Scope Of The Reference Core

Compatibility does not imply TypeScript, JavaScript, npm, Node.js, or browser
compatibility. Nexa has no implicit `any`, `null`, `undefined`, truthiness,
coercion, prototype lookup, exceptions, or JavaScript runtime object model.

Native code generation, UI, FFI, package management, LSP, mutable heap
collections, `Float`, async execution, threads, and a complete standard library
remain possible post-1.0 projects. They are not gaps that v0.9 may fill by
changing the frozen core.
