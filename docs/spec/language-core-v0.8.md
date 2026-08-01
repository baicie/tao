# Nexa Language Core v0.8

## Status

Language Core v0.8 is delivered. This document defines its normative source,
static, MIR, and reference-interpreter contract. It is the final feature
milestone composed into the [Language 1.0 contract](language-1.0.md).

## Product Contract

Language Core v0.8 extends v0.7 with monomorphic function values, arrow
functions, immutable lexical captures, array `for...of` iteration, practical
immutable array operations, Unicode-scalar string length, and explicit integer
and string conversions.

The new features retain Nexa's TypeScript-shaped surface without adopting
JavaScript runtime semantics. Function values have one exact static signature.
Closures copy resolved lexical values when the arrow expression is evaluated.
Arrays remain immutable values, iteration is restricted to arrays, and all
conversions are explicit.

v0.8 reserves `for`. A v0.7 program that used `for` as an identifier must be
renamed. `of` is contextual only inside a `for...of` header and remains an
ordinary identifier everywhere else. No other word becomes reserved.

## Accepted Multi-File Program

`support.nexa` exports ordinary generic data and a higher-order function:

```nexa
export type Result<T, E> =
  | Ok(value: T)
  | Err(error: E);

export type Batch<T> = {
  values: T[];
};

export function ok<T>(value: T): Result<T, String> {
  return Result.Ok(value);
}

export function apply(value: Int, transform: (value: Int) => Int): Int {
  return transform(value);
}
```

`main.nexa` imports those definitions and uses every v0.8 practical-core
feature:

```nexa
import { Batch, Result, apply, ok } from "./support.nexa";

function read(result: Result<Int, String>): Int {
  return match (result) {
    case Result.Ok(value) => value;
    case Result.Err(message) => 0;
  };
}

function main(args: String[]): Unit {
  const factor = 2;
  const scale: (value: Int) => Int =
    (value: Int): Int => value * factor;
  const input = parseInt(args[0]);
  const batch: Batch<Int> = {
    values: [input].append(1).concat([])
  };
  let total = 0;

  for (const value of batch.values) {
    total = total + apply(value, scale);
  }

  const result = ok(total);
  print(toString(read(result)));
  print("Nexa🙂".length);
}
```

Running `nexac run main.nexa -- 20` prints:

```text
42
5
```

The emoji occupies multiple UTF-8 bytes but one Unicode scalar value.

## Grammar

Whitespace and `//` line comments may occur between grammar symbols and remain
in the lossless CST. These productions replace or extend their v0.7
counterparts; all other v0.7 grammar remains unchanged.

```text
Type                    = FunctionType | PostfixType
FunctionType            = "(" FunctionTypeParameterList? ")" "=>" Type
FunctionTypeParameterList
                        = FunctionTypeParameter
                          ("," FunctionTypeParameter)*
FunctionTypeParameter   = Identifier ":" Type
PostfixType             = TypeAtom ArraySuffix*
TypeAtom                = "Int"
                        | "Bool"
                        | "String"
                        | "Unit"
                        | NamedType
                        | "(" FunctionType ")"

Statement               = ConstDeclaration
                        | LetDeclaration
                        | AssignmentStatement
                        | IfStatement
                        | WhileStatement
                        | ForOfStatement
                        | BreakStatement
                        | ContinueStatement
                        | ReturnStatement
                        | ExpressionStatement
ForOfStatement          = "for" "(" "const" Identifier "of" Expression ")"
                          Block

Primary                 = Integer
                        | StringLiteral
                        | "true"
                        | "false"
                        | Identifier
                        | ArrayLiteral
                        | RecordLiteral
                        | MatchExpression
                        | ArrowExpression
                        | "(" Expression ")"
ArrowExpression         = "(" ArrowParameterList? ")" ":" Type "=>" ArrowBody
ArrowParameterList      = ArrowParameter ("," ArrowParameter)*
ArrowParameter          = Identifier ":" Type
ArrowBody               = Expression | Block
```

Function-type parameter names document positions but do not participate in
type equality. Lists are comma-separated, do not accept trailing commas, and
may be empty only where the grammar has `?`. Malformed delimiters, missing
annotations, and trailing commas use `E1001`.

The arrow result annotation is mandatory even when an expected function type
is available. In `=> { ... }`, braces always begin a block body. An arrow that
returns a record literal uses a parenthesized expression, for example
`() : Box<Int> => ({ value: 42 })` with ordinary trivia allowed around the
tokens.

Function type arrows associate to the right. Parentheses are required to apply
an array suffix to a function type:

```nexa
const compose: (next: (value: Int) => Int) => (value: Int) => Int =
  (next: (value: Int) => Int): (value: Int) => Int =>
    (value: Int): Int => next(value);

const handlers: ((value: Int) => Int)[] = [];
```

The parser recognizes `of` contextually only after the binding identifier in a
`for (const ...` header. `const of = 1;`, a parameter named `of`, and a function
named `of` remain valid.

## Function Types

A function type consists of an ordered parameter-type list and one result
type. Parameter names are not semantic:

```text
(value: Int) => Int == (input: Int) => Int
```

Function types are exact and invariant. Parameter count, each parameter type,
and the result type must match. There is no subtyping, optional parameter,
rest parameter, overload set, contextual parameter inference, covariance, or
contravariance.

Function types are ordinary non-`Unit` value types. They may appear in
parameters, returns, local annotations, array elements, record fields, union
payloads, and generic type arguments wherever the surrounding v0.7 layout rule
permits a value. Generic constraint collection and substitution traverse
function parameter and result types structurally.

Function values do not support `===`, ordering, `print`, string conversion, or
reflection. An attempted unsupported operation uses `E3001`. Programs cannot
observe whether two values refer to the same source function, arrow site, or
capture environment.

## Named Function Values And Calls

A reference to a non-generic source function evaluates to a value with the
function's declared signature. This applies to module-local functions and
explicitly imported functions. Passing, returning, storing, and indirectly
calling such a value preserves its module-aware `FunctionId`.

Call evaluation is uniform:

1. Evaluate the callee expression exactly once.
2. Evaluate arguments exactly once from left to right.
3. Validate no types at runtime; invoke the source function or closure selected
   by the typed MIR value.

Static checking requires the callee to have a function type, exact arity, and
exact argument types. Existing `E2003` handles arity and `E3001` handles type
mismatches or non-callable values.

A generic function remains callable directly through v0.7 local inference:

```nexa
const value = identity(42);
```

It cannot be converted to a value because v0.8 has no polymorphic function
type, explicit call-site specialization, or runtime generic dispatch:

```nexa
const invalid: (value: Int) => Int = identity; // E3012
```

Only a syntactic name in direct callee position receives generic call
inference. Parenthesizing, returning, storing, or passing that generic name
places it in value position and uses `E3012`. Non-generic functions remain
first-class across module boundaries.

Variant constructors and record member-call intrinsics are not first-class
function values. Existing constructor syntax and validation remain unchanged.

## Arrow Functions And Captures

Every arrow parameter and result type is explicit. An expression body returns
the expression value. A block body follows ordinary function rules: `Unit`
may fall through, while a non-`Unit` result must return a value on every path.
Arrow parameters and locals use the same lexical duplicate, mutability, and
scope rules as source functions.

An arrow expression creates a closure value when evaluation reaches it. The
closure captures each referenced binding declared in an enclosing callable or
block by value at that moment:

- enclosing parameters and `const` bindings may be captured;
- top-level and imported function references are resolved identities and are
  not capture slots;
- a binding declared inside the arrow is local, not captured; and
- an enclosing `let` binding may not be captured, even when the arrow only
  reads it. That rejection uses `E3011`.

By-value capture means a returned closure retains its copied values after the
creating call has returned. Arrays, records, unions, strings, function values,
and other closures may be captured. No capture exposes an address, shared
mutable cell, or environment identity.

Nested arrows capture from their immediate lexical environment. When an inner
arrow needs a binding from a more distant source function, intermediate
closures forward the copied value. Capturing a distant `let` remains `E3011`;
nesting does not bypass the rule.

Capture slots are deterministic. Semantic analysis walks each arrow body in
source order, records a free binding at its first reference, and deduplicates
later references. A nested arrow's forwarded requirements are visited at the
nested arrow expression's source position. Runtime closure creation copies
values in that recorded order.

Each arrow site has the stable identity:

```text
ClosureId = { owner: FunctionId, source_index: zero-based source-order index }
```

`owner` is the module-aware source function containing the arrow, including
arrows nested inside other arrows. `source_index` is assigned by preorder over
arrow expressions in that owner's source HIR, ignoring trivia and recovery
nodes. Re-evaluating one site creates values with the same `ClosureId` and new
capture snapshots. A `ClosureId` is not a runtime object identity.

An initializer cannot refer to the binding it is declaring, so an arrow cannot
create local self-recursion through its own `const` or `let`. Such a reference
uses the ordinary `E2001`. Mutually recursive local closures are likewise not
introduced. Named source functions retain their existing direct recursion,
and indirect recursion through already available function values is bounded by
the global call-depth limit.

## Array For-Of Iteration

`for (const value of values) { ... }` accepts only an array expression. The
loop variable's exact type is the array element type; v0.8 does not allow a
separate annotation in the header. A non-array iterable uses `E3001` at the
iterable expression.

Execution follows these steps:

1. Evaluate the array expression exactly once before the first iteration.
2. Visit its fixed elements in ascending index and source-value order.
3. Create a fresh immutable loop binding for the current element, then execute
   the body.
4. `continue` advances to the next element, `break` exits the nearest loop,
   and `return` exits the enclosing callable.

An empty array executes no body. The loop binding is scoped only to the body
and cannot be assigned. A closure created during an iteration captures that
iteration's element value, not a shared loop slot. Existing nearest-loop rules
for nested `while` and `for...of` loops remain unchanged.

There is no iterable protocol, iterator object, callback desugaring, index
binding, `for...in`, asynchronous iteration, or user-defined iteration hook.
MIR lowers `for...of` directly to array indexing and CFG branches using typed
array facts.

## Immutable Array Operations

v0.8 adds two intrinsic member-call forms for an array of `T`:

```text
array.append(value: T): T[]
array.concat(other: T[]): T[]
```

- `append` returns a new array containing every base element followed by the
  argument.
- `concat` returns a new array containing every base element followed by every
  argument-array element.
- The original arrays remain unchanged and have no observable capacity or
  allocation identity.
- The base expression is evaluated first and exactly once, followed by call
  arguments from left to right.
- Element and array types must match exactly; no widening or conversion is
  attempted.

These forms are resolved statically only when the member is immediately
called. They do not create bound methods, and `const add = values.append;` uses
`E2005`. Unknown array members retain `E2005`; incorrect intrinsic arity uses
`E2003`; argument type errors use `E3001`.

An implementation may share immutable storage, but sharing is unobservable.
No `push`, element assignment, capacity mutation, prototype member, or dynamic
method lookup is added.

## String Length And Explicit Conversions

`string.length` has type `Int` and counts decoded Unicode scalar values, not
UTF-8 bytes and not user-perceived grapheme clusters. No normalization occurs.
For example:

```nexa
print("é".length);       // 1
print("e\u{301}".length); // represented here as two source scalars: 2
print("🙂".length);      // 1
```

The second comment states the semantic case; v0.8 does not add a `\u{...}`
string escape. Source text must contain the intended scalar directly or use
the existing five escapes.

Two direct builtins provide explicit conversion:

```text
toString(Int): String
parseInt(String): Int
```

`toString` returns the canonical ASCII base-10 representation. Zero is `"0"`,
negative values have one leading `-`, and no plus sign, grouping, whitespace,
or locale-dependent digit is emitted.

`parseInt` accepts a complete string matching `-?[0-9]+`. Leading zeroes and
`-0` are accepted. Empty strings, a lone `-`, leading or trailing whitespace,
`+`, decimal points, separators, non-ASCII digits, and trailing characters are
malformed. The mathematically parsed value must fit the signed 64-bit `Int`
range.

Malformed input stops execution with exactly:

```text
parseInt expected a complete ASCII decimal integer
```

Syntactically valid but out-of-range input stops execution with exactly:

```text
parseInt result is outside the Int range
```

Both runtime errors label the complete `parseInt(...)` call and preserve all
prior `print` output. `toString` and `parseInt` do not add implicit coercion,
exception handling, fallback values, radix arguments, locale rules, or
JavaScript-compatible partial parsing.

## Evaluation Order

v0.8 retains deterministic left-to-right, exactly-once evaluation and adds the
following required order:

- a call evaluates its callee before its arguments;
- an arrow expression snapshots captures when that expression is evaluated;
- `append` and `concat` evaluate the base before arguments;
- `.length` evaluates its base once;
- `for...of` evaluates its array once before visiting elements; and
- `parseInt` evaluates its argument before validating the complete value.

A runtime failure prevents every later effect. Output emitted before an array
bounds, integer arithmetic, call-depth, step-budget, recursive-union, or
`parseInt` failure remains observable.

## Typed HIR, MIR, And Runtime Contract

Typed HIR resolves all function-value and closure facts before MIR lowering.
Its logical additions are:

```text
Type = ... | Function { parameters: [Type], result: Type }

CallFact = DirectFunction { function: FunctionId, type_arguments: [Type] }
         | Indirect { callee_type: Type::Function }
         | Builtin(BuiltinId)

ClosureFact = {
  id: ClosureId,
  signature: Type::Function,
  captures: [ResolvedLocalId]
}
```

Generic function bodies may contain function types and arrow expressions.
Their symbolic parameter types are substituted under the existing v0.7 rules.
A reference to the generic source function itself is still `E3012`; v0.8 does
not synthesize a polymorphic value.

CFG MIR lowers one closure body per `ClosureId` and addresses capture slots by
their checked index. Runtime callable values use stable enums and IDs, with a
logical representation such as:

```text
CallableValue = Function(FunctionId)
              | Closure { id: ClosureId, captures: [Value] }
CallTarget    = Direct(FunctionId)
              | Indirect(Value)
              | Builtin(BuiltinId)
```

The concrete Rust API may choose equivalent names, but dispatch must remain a
closed enum/ID operation. It must not use `dyn Fn`, source-name strings,
filesystem paths, CST tokens, runtime overload search, or runtime type
inference.

Array append/concat, string length, conversions, and `for...of` lower to stable
MIR intrinsic or CFG forms selected by typed HIR. The interpreter does not
perform source-level member lookup. Malformed programmatic MIR with a foreign
`FunctionId`, `ClosureId`, capture slot, builtin ID, or incompatible callable
value produces a structured runtime error rather than a panic.

## Runtime And Resource Rules

The existing global limits remain part of the language contract:

- at most 64 active source-function and closure calls;
- at most 100000 executed CFG basic-block steps per program run;
- at most 1024 active recursive-union traversal levels where that v0.5 limit
  applies; and
- at most 256 distinct closed generic semantic instances per compiler session.

Direct and indirect calls share one call-depth counter. Calls, closures, and
`for...of` loops do not reset the step budget. Each loop progresses through
ordinary MIR blocks, so an infinite or excessively long loop reaches the same
deterministic step error as `while`.

A closure environment has exactly the finite capture slots established by
typed HIR. v0.8 adds no separate closure count, environment-depth counter,
garbage collector, or observable allocation failure. Implementations may share
immutable values and reclaim unreachable environments without observable
effect.

## Required Diagnostics And Labels

v0.8 retains every v0.7 diagnostic and adds `E3011` and `E3012`.

| Code | Condition | Primary label | Secondary label(s) |
|---|---|---|---|
| `E1001` | malformed function type, arrow, or `for...of` syntax | unexpected or missing syntax position | none |
| `E2001` | unresolved value, including attempted local closure self-reference | unresolved name | none |
| `E2003` | direct, indirect, builtin, or array-intrinsic arity mismatch | complete call | source declaration when available |
| `E2004` | assignment to a `const`, parameter, or `for...of` binding | assignment target | binding declaration |
| `E2005` | unknown member or attempted extraction of `append`/`concat` | member name | none |
| `E3001` | function mismatch, non-callable value, non-array iteration, intrinsic mismatch, or unsupported function operation | invalid expression | existing expected/declaration context when available |
| `E3011` | an arrow captures an enclosing `let` binding | first reference requiring that capture | captured `let` declaration |
| `E3012` | a generic source or imported function is used as a value | function-name reference in value position | generic function declaration name |

One arrow emits at most one `E3011` for each prohibited captured binding;
later references to the same binding do not duplicate it. An outer arrow that
must forward a prohibited capture for a nested arrow reports the nested first
reference as primary. `E3012` suppresses derivative function-type mismatch
noise at the same expression while unrelated expressions continue checking.

Malformed and overflowing `parseInt` values are runtime errors, not static
diagnostics. Cross-file secondary labels and global `(FileId, start, end,
code)` sorting retain the v0.6 contract.

## Required Accepted Coverage

The v0.8 conformance slice must include executable coverage for:

- zero-, one-, and many-parameter function types, nested returns, and arrays of
  functions;
- module-local and imported non-generic function values passed, returned,
  stored, and indirectly called;
- expression- and block-bodied arrows;
- capture of parameters, `const`, immutable data, function values, and nested
  forwarded captures;
- repeated evaluation of one arrow site producing independent snapshots;
- ordinary direct generic calls and generic functions returning closures;
- `for...of` over empty and non-empty arrays, nested loop control, return, and
  per-iteration closure snapshots;
- immutable `append` and `concat`, including empty contextual arrays and
  left-to-right effects;
- array length and Unicode-scalar string length with ASCII, multibyte scalars,
  and combining scalar sequences;
- `toString` for zero, positive, negative, minimum, and maximum `Int` values;
- `parseInt` round trips, leading zeroes, `-0`, minimum, and maximum `Int`;
- call-depth and step-budget sharing across direct functions, closures, and
  loops; and
- one multi-file CLI program combining modules, records, unions, exhaustive
  matching, generics, explicit `Result`, function values, captures, arrays,
  iteration, conversions, and CLI arguments.

## Required Rejected Coverage

The conformance slice must assert diagnostic codes and exact source spans for:

- malformed function types, arrows, and `for...of` headers (`E1001`);
- function arity and exact signature mismatches (`E2003`/`E3001`);
- calling non-functions and using function equality or `print` (`E3001`);
- local arrow self-reference (`E2001`);
- direct and forwarded capture of an enclosing `let` (`E3011`);
- local and imported generic function values (`E3012`);
- iterating a non-array and assigning the loop binding (`E3001`/`E2004`);
- extracting `append` or `concat`, unknown collection members, wrong arity,
  wrong element type, and wrong concat array type (`E2005`/`E2003`/`E3001`);
- malformed and overflowing `parseInt` input as runtime failures with exact
  call spans and prior-output retention; and
- forged MIR callable identities, capture layouts, intrinsics, and indirect
  value kinds as structured runtime failures.

Parser recovery must retain later statements and top-level declarations after
each malformed form. Semantic recovery must suppress derivative callable noise
without hiding independent errors.

## Non-Goals

Language Core v0.8 does not add polymorphic function values, explicit generic
call arguments, bound methods, method values, `this`, prototypes, structural
objects, structural records, function overloads, optional/rest parameters,
default parameters, parameter inference, function equality, shared mutable
captures, capture-by-reference, recursive local closures, an iterator protocol,
custom iterables, `for...in`, async iteration, mutable arrays, array element
assignment, callbacks such as `map`, string indexing, grapheme segmentation,
Unicode normalization, interpolation, implicit coercion, JavaScript-compatible
`parseInt`, exceptions, native code generation, runtime reflection, UI, or
TypeScript ecosystem compatibility.
