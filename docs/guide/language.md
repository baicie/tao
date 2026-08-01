# Language Guide

Nexa Language Core is a TypeScript-shaped, statically typed language with its
own deterministic semantics. The delivered v0.8 implementation checks
multi-file command-line programs and executes resolved CFG MIR with a reference
interpreter.

This guide introduces the programming model. The versioned files under
[Language Spec](../spec/index.md) remain normative when a concise example here
omits a rule.

## Values And Types

The primitive types are `Int`, `Bool`, `String`, and `Unit`.

- `Int` is a signed 64-bit integer. Arithmetic overflow is a runtime failure.
- `Bool` is exactly `true` or `false`; Nexa has no truthiness.
- `String` is immutable UTF-8 text. `.length` counts Unicode scalar values.
- `Unit` is the result of a callable that returns no value.
- `T[]` is an immutable homogeneous array.

There is no implicit `any`, nullability, numeric/string coercion, or implicit
error propagation. Conversions are explicit:

```nexa
const text: String = toString(42);
const value: Int = parseInt(text);
```

`parseInt` requires a complete ASCII decimal integer. It does not copy
JavaScript's partial parsing behavior.

## Bindings And Control Flow

`const` creates an immutable local. `let` creates a mutable local and requires
an initializer.

```nexa
function total(values: Int[]): Int {
  let result = 0;

  for (const value of values) {
    result = result + value;
  }

  return result;
}
```

Assignments can target only a visible `let`. Parameters, `const`, record
fields, array elements, and `for...of` bindings are immutable. `if` and `while`
conditions must have type `Bool`. `break` and `continue` target the nearest
enclosing `while` or `for...of` loop.

`&&` and `||` accept `Bool` operands and short-circuit. Nexa evaluates operands
and arguments from left to right exactly once.

## Functions And Function Values

Top-level functions declare every parameter and return type:

```nexa
function add(left: Int, right: Int): Int {
  return left + right;
}
```

A non-generic function is a first-class value with one exact signature:

```nexa
function apply(value: Int, transform: (value: Int) => Int): Int {
  return transform(value);
}

const increment: (value: Int) => Int =
  (value: Int): Int => value + 1;
```

Function parameter names do not affect type equality. Arity, parameter types,
and result type must match exactly; there is no overloading, optional or rest
parameter, subtyping, or variance.

An arrow may capture an enclosing parameter or `const` by value. The snapshot
is taken when the arrow expression executes, so a returned closure keeps its
captured immutable data after the creating call returns. Capturing an enclosing
mutable `let` is rejected. Local recursive closures are not implicit; named
source functions retain ordinary recursion.

Generic source functions remain directly callable through local inference but
are not polymorphic first-class values:

```nexa
function identity<T>(value: T): T {
  return value;
}

const answer = identity(42);
```

## Immutable Arrays And Strings

Array literals are homogeneous. An empty literal requires an expected array
type.

```nexa
const first: Int[] = [20, 21];
const values = first.append(22).concat([]);
print(values.length);
print(values[2]);
```

`append` and `concat` return new arrays and do not modify their inputs. They are
immediate intrinsic calls, not extractable method values. Array indexing uses
`Int`; an out-of-bounds index is a runtime failure.

String `+` concatenates strings. String equality is value equality.
`String.length` counts Unicode scalar values rather than UTF-8 bytes or
grapheme clusters.

## Nominal Records

A record declaration defines one nominal immutable type:

```nexa
type Pair<T> = {
  left: T;
  right: T;
};

const pair: Pair<Int> = {
  left: 20,
  right: 22
};
```

Construction requires every declared field exactly once with its exact type.
Unknown, duplicate, missing, or mistyped fields are rejected. Equal field
shapes from separately declared records are not interchangeable. Fields cannot
be assigned after construction.

## Tagged Unions And Error Values

Tagged unions define nominal variants with typed payloads:

```nexa
type Result<T, E> =
  | Ok(value: T)
  | Err(error: E);
```

Construct a variant through its qualified name and inspect it with exhaustive
`match`:

```nexa
function unwrap(result: Result<Int, String>): Int {
  return match (result) {
    case Result.Ok(value) => value;
    case Result.Err(message) => 0;
  };
}
```

Every variant must be covered unless a reachable `default` arm handles the
remainder. Only the selected arm executes. `Option<T>` and `Result<T, E>` are
ordinary source-defined unions; Nexa does not inject an implicit prelude or add
exception semantics.

Recursive unions are allowed only through the guarded shapes defined by the
tagged-union specification. Records cannot contain an unguarded direct or
indirect recursive layout.

## Generics

Functions, records, and tagged unions may declare bounded compile-time type
parameters:

```nexa
function first<T>(values: T[]): T {
  return values[0];
}
```

Named generic type references provide complete type arguments. Calls and
constructors infer type arguments only from local arguments and an exact
expected result context. Nexa has no expression-level explicit type arguments,
generic defaults, higher-kinded types, trait constraints, or runtime generic
dispatch.

Type arguments are resolved in typed HIR and erased to one definition-level
MIR body or layout. A compiler session accepts at most 256 distinct closed
semantic instances.

## Modules

Declarations are private by default. Prefix a top-level function, record, or
union with `export` to make it available to an explicit named import:

```nexa
// support.nexa
export function answer(): Int {
  return 42;
}
```

```nexa
// main.nexa
import { answer } from "./support.nexa";

function main(): Unit {
  print(answer());
}
```

Import paths are relative, use `/`, and end in `.nexa`. Cycles, missing files,
unknown names, private imports, and export-name collisions are diagnosed with
source spans in the owning files. The command-line entry module alone owns the
runnable `main` selection.

## Program Entry And Execution

A runnable program uses one of these entry signatures:

```nexa
function main(): Unit {}
function main(args: String[]): Unit {}
```

The compiler parses each reachable file into a lossless CST, lowers and checks
typed HIR, lowers resolved facts to CFG MIR, and then runs the interpreter.
Runtime call-depth, basic-block step, recursive-union, and generic-instance
limits are deterministic parts of the reference contract.

Use the [CLI Guide](cli.md) to run programs and the
[Diagnostic Reference](../reference/diagnostics.md) to interpret failures.

## What Nexa Is Not

Familiar braces, `function`, `const`, and type annotations do not make Nexa a
TypeScript implementation. The 1.0 reference core deliberately excludes
JavaScript runtime semantics, TypeScript/npm compatibility, classes,
interfaces, exceptions, `Float`, mutable heap objects, async/concurrency,
native code generation, UI, FFI, package management, LSP, and a complete
standard library.

See the [Compatibility Policy](../compatibility.md) and
[Nexa Language 1.0 target](../spec/language-1.0.md) for the exact boundary.
