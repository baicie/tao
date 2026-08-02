# Diagnostic Reference

## Contract

Nexa source diagnostics use a stable code, severity, and one or more source
labels. A primary label identifies the rejected source construct. Secondary
labels identify declarations, earlier occurrences, missing alternatives, or
cross-file context.

The 1.0 baseline freezes code, severity, label role, owning source, byte span,
and ordering. Ordinary English prose is informative unless a specification
quotes exact wording. All currently required `E` codes have severity `Error`.
No warning code is part of the Language 1.0 required set.

Diagnostics from a compiler session are sorted by the primary label's
`(FileId, start byte, end byte, code)`. Exact ties retain emission order.
`FileId` follows deterministic first-discovery module order. Secondary labels
do not affect sorting.

## Syntax

| Code | Condition | Primary label | Secondary labels |
|---|---|---|---|
| `E1001` | unknown token, malformed syntax, invalid string escape, missing delimiter, or disallowed source form | unexpected token or zero-width missing-syntax position | none |

Parser recovery may emit more than one `E1001` when independent malformed
constructs remain. Recovery must make progress and retain later statements or
top-level declarations at documented synchronization boundaries.

## Names, Declarations, And Shape

| Code | Condition | Primary label | Secondary labels |
|---|---|---|---|
| `E2001` | unresolved value, assignment target, named type, type parameter, function, or union qualifier | unresolved name | none |
| `E2002` | duplicate binding, function, type, field, variant, payload, type parameter, pattern binder, import, export, or match case | later duplicate | first declaration or occurrence when available |
| `E2003` | wrong function, builtin, constructor, pattern, intrinsic, or type-argument arity | complete call, pattern, constructor, or named type reference | referenced declaration when available |
| `E2004` | assignment to `const`, parameter, or `for...of` binding | assignment target | binding declaration |
| `E2005` | unknown member, record field, union variant, or extraction of a non-first-class intrinsic member | member, field, or variant name | none unless a milestone requires declaration context |
| `E2006` | record literal omits a required field | complete record literal | each missing field declaration |

## Types And Control Flow

| Code | Condition | Primary label | Secondary labels |
|---|---|---|---|
| `E3001` | type mismatch or unsupported typed operation, including invalid indexing, construction, matching, callable use, iteration, or intrinsic arguments | invalid expression or type | expected type or declaration context when available |
| `E3002` | non-`Bool` `if` or `while` condition | condition expression | none |
| `E3003` | invalid or missing return, or invalid `main` signature | invalid return, function body, or `main` name | function declaration context when available |
| `E3004` | `break` or `continue` outside a loop | complete control statement | none |
| `E3005` | unguarded direct or indirect recursive record layout | recursive field type | record declaration name |
| `E3006` | non-exhaustive match | complete match expression | every missing variant declaration |
| `E3007` | match arm unreachable after `default` or complete variant coverage | unreachable arm | earlier default or coverage-completing arm |
| `E3008` | unconstrained or result-only declared type parameter | type parameter name | owner declaration name |
| `E3009` | unresolved or conflicting local generic inference | complete call/constructor when unresolved; later conflicting argument otherwise | type parameter and first constraining argument when applicable |
| `E3010` | non-regular generic recursion or the 257th distinct closed semantic instance | expanding use or instance 257 | generic definition name |
| `E3011` | arrow captures an enclosing mutable `let` | first reference requiring the capture | captured `let` declaration |
| `E3012` | generic local or imported source function used as a first-class value | function-name reference | generic function declaration name |

Generic inference failures suppress derivative type mismatches when no stable
substitution exists. `E3011` is emitted at most once for each arrow and
prohibited captured binding. `E3012` suppresses a derivative function-value
type mismatch at the same expression. Independent errors continue checking.

## Modules

| Code | Condition | Primary label | Secondary labels |
|---|---|---|---|
| `E4001` | invalid relative import path, unresolved or missing import, unreadable import, or non-UTF-8 imported source | complete import path literal | none |
| `E4002` | cyclic module strongly connected component | deterministic closing import path | DFS-tree witness import paths in cycle order |
| `E4003` | imported name does not exist in the target module | imported identifier | none |
| `E4004` | matching target declaration exists but is private | imported identifier | earliest matching private declaration |
| `E4005` | exported function/type name collision | later exported declaration | first exported declaration |

Same-namespace local/import collisions use `E2002`. Malformed import grammar
uses `E1001`. Entry-source failures that occur before a source span exists are
host-level compiler errors rather than `E4001` diagnostics.

## Bootstrap Profile

These diagnostics are emitted only when compiler options select
`futao-bootstrap-v1`. The default `application` profile preserves the Language
1.0 behavior above.

| Code | Condition | Primary label | Secondary labels |
|---|---|---|---|
| `E6201` | mutable `let` binding or assignment in compiler-core source | complete declaration or assignment | none |
| `E6202` | `while`, `break`, or `continue` in compiler-core source | complete loop/control statement | none |
| `E6203` | ambient `print` output from compiler-core source | resolved builtin name | none |

Bootstrap source identities that do not end in `.ft` fail before a stable
source span exists and are reported as host-level compiler input errors. The
machine-readable profile also denies filesystem, network, process, clock,
environment, random, thread, async, UI, dynamic-loading, plugin, and reflection
capabilities; profile lint must grow before any such capability enters the
language surface.

## Normative Runtime Failures

Runtime failures occur only after static checking succeeds. They have a source
span and message rather than an `E` code. Later evaluation stops, while earlier
`print` output remains visible.

| Condition | Normative message or template | Labeled span |
|---|---|---|
| runnable graph has no entry | ``program has no `main` entry point`` | program span |
| arguments supplied to parameterless entry | ``parameterless `main` does not accept command-line arguments`` | `main` span |
| attempted 65th active call | `maximum call depth of 64 exceeded` | rejected call |
| attempted CFG step 100001 | `execution step limit of 100000 exceeded` | block that would execute |
| recursive union depth 1025 | `recursive union nesting limit of 1024 exceeded` | complete constructor |
| integer unary or binary overflow | `integer <operation> overflow` | complete operation |
| integer division by zero | `division by zero` | complete division |
| invalid array position | `array index <index> out of bounds for length <length>` | complete index expression |
| malformed decimal conversion | `parseInt expected a complete ASCII decimal integer` | complete call |
| decimal result outside signed 64-bit range | `parseInt result is outside the Int range` | complete call |

The concrete overflow operations are `negation`, `addition`, `subtraction`,
`multiplication`, and `division`. Angle-bracket terms in the table are runtime
values, not literal output characters.

Malformed programmatic MIR must also return a structured runtime error instead
of panicking. Those defensive errors are compiler API safeguards; their exact
English text is not a Nexa source-language compatibility guarantee.

## Tooling Guidance

Tools should consume structured diagnostics from compiler APIs when possible.
CLI integrations should key behavior off stable codes and source locations,
not English prose. Cross-file tools must resolve every label through its own
`FileId`; they must not assume all labels belong to the command-line entry.

See the [Compatibility Policy](../compatibility.md),
[CLI Guide](../guide/cli.md), and
[v0.9 stabilization contract](../spec/language-core-v0.9.md).
