# Nexa Language Core v0.7

## Status

Language Core v0.7 is delivered and is the current executable language. Its
implementation satisfies every exit criterion and verification gate in the
[v0.7 roadmap](../project/roadmap/language-core-v0.7.md). Development continues
with the v0.8 function-values and practical-core milestone in the
[1.0 roadmap](../project/roadmap/language-1.0.md).

## Product Contract

Language Core v0.7 extends v0.6 with bounded parametric polymorphism for
top-level functions, nominal immutable records, and nominal tagged unions.
Type arguments are inferred locally at calls and union construction, while
named type references always state their complete type arguments.

Generics do not add TypeScript or JavaScript compatibility. There is no
`any`, structural typing, overload resolution, runtime type reflection,
implicit conversion, exception handling, or implicit error propagation.
Generic bodies are checked parametrically and may use a type parameter only in
operations valid for every possible substitution.

`Option<T>` and `Result<T, E>` are ordinary tagged unions written in Nexa
source. The compiler does not reserve their names, inject an implicit prelude,
or give their variants privileged behavior.

v0.7 adds no reserved words. Every valid v0.6 program retains its syntax,
static meaning, diagnostics, and runtime behavior.

## Accepted Program

`outcome.nexa` defines and exports ordinary generic unions:

```nexa
export type Option<T> =
  | Some(value: T)
  | None();

export type Result<T, E> =
  | Ok(value: T)
  | Err(error: E);
```

`main.nexa` imports and uses them:

```nexa
import { Option, Result } from "./outcome.nexa";

function identity<T>(value: T): T {
  return value;
}

function read(value: Result<Option<Int>, String>): Int {
  return match (value) {
    case Result.Ok(optional) => match (optional) {
      case Option.Some(number) => number;
      case Option.None() => 0;
    };
    case Result.Err(message) => 0;
  };
}

function main(): Unit {
  const value: Result<Option<Int>, String> =
    Result.Ok(Option.Some(identity(42)));
  print(read(value));
}
```

The program prints `42`. Recoverable errors remain ordinary values selected by
an exhaustive `match`; no branch is propagated implicitly.

## Grammar

Whitespace and `//` line comments may occur between grammar symbols and remain
in the lossless CST. These productions replace or extend the corresponding
v0.6 productions; all other v0.6 grammar remains unchanged.

```text
FunctionDeclaration   = "function" Identifier TypeParameterList?
                        "(" ParameterList? ")" ":" Type Block
RecordDeclaration     = "type" Identifier TypeParameterList?
                        "=" RecordBody ";"
UnionDeclaration      = "type" Identifier TypeParameterList?
                        "=" LeadingPipe?
                        UnionVariant ("|" UnionVariant)* ";"

TypeParameterList     = "<" TypeParameter ("," TypeParameter)* ">"
TypeParameter         = Identifier

Type                  = TypeAtom ArraySuffix*
TypeAtom              = "Int"
                      | "Bool"
                      | "String"
                      | "Unit"
                      | NamedType
NamedType             = Identifier TypeArgumentList?
TypeArgumentList      = "<" Type ("," Type)* ">"
ArraySuffix           = "[" "]"
```

Type parameter and argument lists are non-empty and do not accept a trailing
comma. Missing delimiters, empty lists, and trailing commas use `E1001`.
Nested closing `>` tokens remain separate tokens, so
`Result<Option<Int>, String>` needs no whitespace between its closing
delimiters.

Type arguments are expression-free type syntax. v0.7 deliberately has no
expression-level explicit type arguments: `identity<Int>(1)` and
`Option<Int>.None()` are not call or constructor forms. Calls remain
`identity(1)`, and union construction remains `Option.Some(1)` or
contextually typed `Option.None()`. This keeps generic syntax in type contexts
and preserves the existing unambiguous meaning of `<` and `>` as expression
comparison operators.

No declaration accepts a `where`, `extends`, constraint, default type, or
variance annotation.

## Type Parameter Declarations And Scope

A type parameter is identified by its owner definition and declaration-order
index:

```text
TypeParameterId = { owner: DefId, index: declaration-order index }
```

- Function type parameters are visible in the complete function signature,
  body, and local type annotations in that body.
- Record type parameters are visible only in that record's field types.
- Union type parameters are visible in every payload type of that union.
- A type parameter occupies a declaration-local type namespace. It may have
  the same spelling as a value and shadows an imported or module-local named
  type only within its owner.
- Type parameters from different declarations are distinct even when their
  spellings and positions are equal.
- A duplicate type parameter in one list uses `E2002`, with the later name as
  primary and the first declaration as secondary.
- A type parameter name outside its owner is an ordinary unresolved named type
  and uses `E2001`.
- Built-in type keywords cannot be parameter names because a parameter is an
  `Identifier` token.

Functions, records, and unions remain the only generic declarations. Local
bindings, fields, variants, and payloads cannot declare their own parameters.
Local bindings are monomorphic; v0.7 performs no implicit generalization.

## Declaration Constraints

Every declared parameter must constrain a value-bearing part of its owner:

- a function parameter must occur in at least one function value-parameter
  type, not only in its result type or body;
- a record parameter must occur in at least one field type; and
- a union parameter must occur in at least one variant payload type across the
  complete union.

An occurrence nested beneath arrays or other named type arguments counts. An
unused or result-only parameter uses `E3008`, primarily labels that parameter,
and secondarily labels the owner name.

The rule ensures ordinary calls and construction have a local source of type
constraints. A selected zero-payload union variant may still leave parameters
unresolved at one constructor; an exact expected result type can supply them.

`main` must remain non-generic and retain one of the v0.3 entry signatures. A
generic entry declaration uses `E3003` independently of generic inference.

## Type References And Arity

A reference to a generic record or union must supply exactly its declared
number of type arguments. A bare generic name, too few or too many arguments,
or type arguments applied to a non-generic named type use `E2003`. The primary
label covers the complete named type reference and a secondary label identifies
the referenced declaration.

Generic applications are invariant and nominal:

- `Box<Int>` and `Box<String>` are distinct types;
- equal type arguments do not make separately declared generic definitions
  interchangeable; and
- there is no subtype, union-widening, covariance, or contravariance relation.

`Unit` is a valid type argument when the substituted position already permits
`Unit`. Existing restrictions are rechecked after substitution. For example,
`Option<Unit>` is valid because a direct union payload may be `Unit`, while a
generic record instance whose field becomes `Unit`, or an instance containing
`Unit[]`, uses `E3001`.

## Local Type-Argument Inference

Inference is first-order exact unification over the closed v0.7 type grammar.
It never searches other functions, declarations, or call sites and never
chooses a subtype or conversion.

For a generic function call or union constructor, the checker performs these
steps:

1. Collect equations between declared parameter or payload types and the
   corresponding argument expression types.
2. Solve those equations structurally. Arrays must match arrays; nominal
   applications must have the same definition and arity; repeated occurrences
   of one type parameter must resolve to exactly one type.
3. After argument equations are solved, use the exact expected result type, if
   one exists, only to fill parameters that remain unresolved. Expected context
   never overwrites a substitution already established by an argument.
4. Substitute the complete argument list into parameter, payload, and result
   types, then contextually check every argument exactly once.
5. Record the resolved definition identity and complete inferred argument list
   in typed HIR.

Constraint collection is not source-order biased. A contextual literal that
cannot determine its own type may be deferred until another argument or the
result context determines its exact expected type. Runtime evaluation order
remains left to right and is unrelated to inference order.

Examples:

```nexa
function same<T>(left: T, right: T): T { return left; }

const integer = same(1, 2);                  // T = Int
const none: Option<Int> = Option.None();     // T from result context
const error: Result<Int, String> =
  Result.Err("failed");                      // E from payload, T from context
```

`same(1, true)` has conflicting constraints and uses `E3009`. A standalone
`Option.None()` has an unresolved parameter and also uses `E3009`. If arguments
fully infer a result that conflicts with an outer annotation, the call is
well-instantiated and the outer incompatibility is the ordinary `E3001` type
mismatch.

Record literals remain contextual. A literal does not search record
declarations by field shape, so a generic record literal requires an exact
expected type:

```nexa
type Box<T> = { value: T; };

const box: Box<Int> = { value: 42 };
```

## Parametric Body Checking

A generic function body is checked once with symbolic
`Type::Parameter(TypeParameterId)` values. It may bind, pass, return, store, and
place such values into declared generic data structures. An operation that
requires a concrete type remains invalid:

- arithmetic, ordering, string concatenation, and boolean operators require
  their existing exact operand types;
- `print` accepts only its existing concrete printable types; and
- record fields, union variants, calls, and equality retain their existing
  resolved-type rules.

Consequently `value + value` where `value: T` uses `E3001`. v0.7 does not infer
or synthesize a trait bound from that expression.

## Typed HIR Representation

The logical typed HIR representation is:

```text
Type = Int | Bool | String | Unit
     | Parameter(TypeParameterId)
     | Array(Type)
     | Record { definition: RecordId, arguments: [Type] }
     | Union  { definition: UnionId, arguments: [Type] }
```

Monomorphic records and unions use an empty argument list. Definition IDs keep
their v0.6 module-aware source identities; an instance does not allocate a new
source definition.

Resolved function-call facts contain the source `FunctionId` and inferred type
arguments. Record, constructor, field, match, and payload-binding facts retain
their resolved definition/field/variant/payload IDs plus the complete
instantiated type needed for substitution. Imported generic definitions refer
directly to the exporting module's `DefId`, so diamond imports never duplicate
generic identity.

## Bounded Semantic Instantiation

The checker canonicalizes each closed generic application as:

```text
GenericInstance = { definition: DefId, arguments: [Type] }
```

One compiler session may contain at most 256 distinct closed generic instances
across functions, records, and unions. Monomorphic definitions do not consume
this budget, and repeated equal applications consume it once.

Discovery is deterministic: modules use compiler-session order, declarations
and expressions use source order, and nested arguments are visited left to
right. The application that would create instance 257 uses `E3010`; its full
type reference, call, or constructor is primary and the generic definition is
secondary. An error prevents typed output and MIR lowering.

The budget is a compiler-resource and language-determinism contract. It is not
a runtime counter and is not reset per module or function.

## Regular Generic Recursion

v0.5 guarded recursive-data rules remain in force. v0.7 additionally requires
every recursive generic dependency to be regular.

Within a recursive strongly connected component, a reference or call from an
owner declared with `<P0, ..., Pn>` to another member must pass exactly
`<P0, ..., Pn>` in the same order and with the same arity. Wrapping, swapping,
duplicating, dropping, or replacing a parameter on a recursive edge uses
`E3010`.

Accepted regular data recursion:

```nexa
type List<T> =
  | Empty()
  | Node(head: T, tail: List<T>);
```

Rejected expanding recursion:

```nexa
type Grow<T> =
  | Next(value: Grow<T[]>);
```

The same rule applies to recursive generic function-call SCCs. A call from
`loop<T>` to `loop<T>` is regular; a call that infers `loop<T[]>` is not. The
rule is intentionally conservative: some mathematically finite permutations
are rejected so v0.7 does not require type-level evaluation or orbit analysis.

An unguarded record cycle still uses `E3005`. When one cycle violates both the
guard rule and generic regularity, `E3005` is primary and derivative `E3010`
noise for the same edge is suppressed.

## Ordinary Option And Result

The canonical v0.7 spellings are ordinary declarations:

```nexa
type Option<T> =
  | Some(value: T)
  | None();

type Result<T, E> =
  | Ok(value: T)
  | Err(error: E);
```

A program must declare them or explicitly import exported declarations from a
relative source module. Their names may be reused by another module under the
ordinary namespace rules. Constructors, payload typing, exhaustiveness,
visibility, and runtime union-depth limits are exactly those of any other
generic union.

There is no `?`, `throw`, `catch`, implicit return, implicit propagation,
special method set, hidden discriminant, or compiler-provided conversion.
Programs inspect these values with ordinary exhaustive `match` expressions.

## MIR And Runtime Representation

v0.7 performs semantic instantiation in typed HIR but does not duplicate code
or data layouts in CFG MIR.

- Each generic function lowers once under its source `FunctionId`.
- Each generic record or union lowers one definition-level layout under its
  source `RecordId` or `UnionId`.
- Calls, construction, projection, and matching use the resolved definition,
  field, variant, and payload IDs already selected by typed HIR.
- Type parameters and instance arguments are erased from runtime values. MIR
  may retain them as non-observable validation/debug metadata, but the
  interpreter does not dispatch on them.
- Runtime record and union values retain their definition-level nominal tags.

Erasure is sound for the reference interpreter because all values use the
existing uniform `Value` representation and generic bodies cannot select
operations from a type argument. There is no runtime reflection, specialization,
operator overloading, or observable allocation identity.

Left-to-right exactly-once evaluation, prior-output retention, the 100000-step
limit, 64-call-depth limit, and 1024 recursive-union-depth limit remain
unchanged. Generic calls do not reset any runtime limit.

Malformed programmatic MIR with unknown definition, field, variant, payload,
or function identities remains a structured runtime error rather than a
panic. The interpreter does not repeat generic inference or source-name lookup.

## Required Diagnostics And Labels

v0.7 retains every v0.6 diagnostic and adds `E3008`, `E3009`, and `E3010`.

| Code | Condition | Primary label | Secondary label(s) |
|---|---|---|---|
| `E1001` | malformed type parameter/argument delimiter or separator | unexpected or missing syntax position | none |
| `E2001` | unknown named type or out-of-scope type parameter | unresolved name | none |
| `E2002` | duplicate type parameter | later parameter name | first parameter name |
| `E2003` | missing, extra, or invalidly applied type arguments | complete named type reference | referenced declaration name |
| `E3001` | substituted type mismatch or concrete-only operation on a parameter | invalid expression or type | existing contextual label(s) |
| `E3003` | generic or otherwise invalid `main` signature | `main` name | none |
| `E3005` | unguarded recursive record layout | recursive field type | record declaration name |
| `E3008` | unconstrained or result-only declared type parameter | parameter name | owner declaration name |
| `E3009` | unresolved or conflicting local type inference | complete call/constructor when unresolved; later conflicting argument when inconsistent | parameter declaration and, for conflicts, first constraining argument |
| `E3010` | non-regular generic recursion or instance-budget overflow | expanding recursive use or instance 257 | generic definition name |

Generic failures suppress derivative argument/result mismatches when no stable
substitution exists. Independent errors inside arguments and later
declarations are still checked and reported. Cross-file labels and global
diagnostic sorting retain the v0.6 `(FileId, start, end, code)` contract.

## Required Accepted Coverage

The v0.7 conformance slice must contain focused accepted tests for:

- nested generic type CST and preservation of ordinary `<`/`>` comparisons;
- one generic function called with both `Int` and `String`;
- generic record construction, field access, parameter passing, and return;
- payload inference for `Option.Some` and result-context inference for
  `Option.None`;
- `Result.Ok`/`Result.Err` plus exhaustive nested matching;
- regular `List<T>` recursion and execution;
- cross-module generic function, record, and union imports;
- a diamond import retaining one source definition and nominal identity;
- one erased generic MIR function body executing with distinct source types;
  and
- a user-defined `Maybe<T>` behaving like `Option<T>`, proving that canonical
  error-value names have no compiler privilege.

Every accepted semantic feature must reach `nexac run`, not only parser or HIR
tests.

## Required Rejected Coverage

The conformance slice must contain source-span assertions for:

- empty, trailing-comma, and unterminated parameter/argument lists (`E1001`);
- duplicate parameters (`E2002`);
- bare generic types, wrong arity, and arguments on monomorphic types
  (`E2003`);
- result-only and unused parameters (`E3008`);
- conflicting arguments such as `same(1, true)` (`E3009`);
- a zero-payload generic constructor without an expected type (`E3009`);
- mixing `Box<Int>` with `Box<String>` (`E3001`);
- arithmetic or `print` on an unconstrained parameter (`E3001`);
- substitution that creates a record `Unit` field or `Unit[]` (`E3001`);
- expanding data and function recursion (`E3010`);
- deterministic rejection of instance 257 (`E3010`);
- a generic entry point (`E3003`); and
- unknown and private generic imports retaining `E4003` and `E4004`.

Parser recovery tests must prove that malformed generic syntax retains later
top-level declarations. Semantic recovery must avoid duplicate inference noise
while continuing to check unrelated declarations.

## Non-Goals

Language Core v0.7 does not add expression-level explicit type arguments,
generic defaults, trait/interface bounds, `where` clauses, higher-kinded
types, variance, subtyping, overloads, specialization, dynamic dispatch,
generic local declarations, polymorphic local bindings, first-class
constructors, function values, closures, implicit preludes, exceptions,
implicit error propagation, native code monomorphization, runtime type
reflection, TypeScript compatibility, or JavaScript runtime semantics.
