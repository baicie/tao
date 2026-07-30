# Nexa Language Core v0.5

## Status

Language Core v0.5 is delivered and is the current executable language. Its
implementation satisfies the exit criteria in the
[v0.5 roadmap](../project/roadmap/language-core-v0.5.md). Development continues
with the v0.6 multi-file module milestone in the
[1.0 roadmap](../project/roadmap/language-1.0.md).

## Product Contract

Language Core v0.5 extends v0.4 with nominal tagged unions, qualified variant
construction, and exhaustive `match` expressions. Variants may carry zero or
more typed positional payloads. Matching tests one resolved union tag, binds
that variant's payloads, and evaluates exactly one arm.

The surface remains TypeScript-shaped, but unions are not JavaScript objects,
classes, enums, or structural discriminated unions. Tags, payload layouts,
construction, exhaustiveness, and recursion are Nexa language semantics.

`nexac check` and `nexac run` continue to accept one source file. Modules,
generics, `Option`, and `Result` remain later milestones.

The newly reserved words are `match`, `case`, and `default`. A v0.4 program
that used one of those words as an identifier is not source-compatible with
v0.5. All other accepted v0.4 programs retain their behavior.

## Accepted Program

```nexa
type List =
  | Empty()
  | Node(head: Int, tail: List);

function sum(values: List): Int {
  return match (values) {
    case List.Empty() => 0;
    case List.Node(head, tail) => head + sum(tail);
  };
}

function main(): Unit {
  const values = List.Node(20, List.Node(22, List.Empty()));
  print(sum(values));
}
```

The program prints `42`.

## Grammar

Whitespace and `//` line comments may appear between grammar symbols and remain
in the lossless CST. These productions replace or extend the corresponding
v0.4 productions; all other v0.4 grammar remains unchanged.

```text
SourceFile              = Declaration*
Declaration             = RecordDeclaration
                        | UnionDeclaration
                        | FunctionDeclaration
UnionDeclaration        = "type" Identifier "=" LeadingPipe?
                          UnionVariant ("|" UnionVariant)* ";"
LeadingPipe             = "|"
UnionVariant            = Identifier "(" UnionPayloadList? ")"
UnionPayloadList        = UnionPayload ("," UnionPayload)*
UnionPayload            = Identifier ":" Type
Primary                 = Integer
                        | StringLiteral
                        | "true"
                        | "false"
                        | Identifier
                        | ArrayLiteral
                        | RecordLiteral
                        | UnionConstruction
                        | MatchExpression
                        | "(" Expression ")"
UnionConstruction       = Identifier "." Identifier
                          "(" ArgumentList? ")"
MatchExpression         = "match" "(" Expression ")"
                          "{" MatchArm* "}"
MatchArm                = CaseArm | DefaultArm
CaseArm                 = "case" QualifiedVariantPattern
                          "=>" Expression ";"
DefaultArm              = "default" "=>" Expression ";"
QualifiedVariantPattern = Identifier "." Identifier
                          "(" PatternBindingList? ")"
PatternBindingList      = Identifier ("," Identifier)*
```

The leading `|` is optional. A trailing `|` is never accepted. Variant payload
declarations, construction arguments, and pattern binders do not accept a
trailing comma. Parentheses are required for every variant, including a
zero-payload variant. Every match arm ends in a semicolon.

Both union declaration styles are therefore accepted:

```nexa
type Flag = Off() | On();

type Direction =
  | Left()
  | Right();
```

The form `| Left() | Right() |;` is malformed syntax (`E1001`). Record and
union declarations share the `type` introducer and are distinguished after
`=` by `{` or a variant name/leading `|`.

## Names, Types, And Nominality

- Union names occupy the same type namespace as record names. A later record
  or union with a duplicate type name uses `E2002`, with a secondary label on
  the first declaration. Type names remain separate from function and local
  value names.
- Union declarations are visible throughout the source file, including before
  their textual declaration. An unknown named type or construction/pattern
  qualifier uses `E2001`.
- Variant names are scoped to their declaring union. Duplicate variants use
  `E2002`; equal variant names in different unions are allowed.
- Payload names document declaration positions and must be unique within one
  variant. A duplicate payload name uses `E2002`. Constructor arguments and
  pattern binders are positional rather than named.
- Two separately declared unions are different types even when their variants
  have equal names and payload types. Union and record types are also always
  distinct.
- A union payload may have any v0.5 value type. In particular, a direct `Unit`
  payload is valid and still occupies one constructor argument and one pattern
  position. Existing rules continue to reject `Unit[]`.
- Typed HIR introduces a stable source-order `UnionId` and an owner-scoped,
  source-order `VariantId`. `Type::Union(UnionId)` is distinct from
  `Type::Record(RecordId)`. Payload positions and pattern bindings are resolved
  before MIR; their source names are not runtime lookup keys.

The concrete numeric values and runtime tag representation of these IDs are
not observable source-language behavior. They must nevertheless be stable for
the same source so tests and later module ownership can rely on them.

## Variant Construction

A constructor is always qualified and always called:

```nexa
const empty = List.Empty();
const node = List.Node(42, empty);
```

- `Union.Variant(arguments)` resolves `Union` in the type namespace and
  `Variant` within that exact union. A qualifier that is not a union uses
  `E3001`; an unknown qualifier uses `E2001`; an unknown variant uses `E2005`.
- A variant is not inserted into the value namespace. `Node(...)` is an
  ordinary function/value lookup and does not construct `List.Node`.
- A qualified constructor without its required call, such as `List.Empty`, is
  not a first-class value and uses `E3001`.
- Constructor arity must equal the declared payload count. A mismatch uses
  `E2003`. Every argument must have the exact declared type; a mismatch uses
  `E3001`.
- Arguments evaluate exactly once from left to right. A failure stops later
  argument evaluation and preserves earlier output.
- The declared payload type supplies contextual typing to record and array
  literals in that argument position. Construction itself needs no expected
  type because the qualified union name determines the result type.

Union values are immutable. Passing, returning, binding, storing in records,
or storing in arrays may share backing storage, but identity, address, tag
numbers, and allocation strategy are not observable.

## Match Expressions

`match` is an expression, not a statement. Its scrutinee is evaluated exactly
once before any arm is selected. The scrutinee must have one exact union type;
matching a non-union value uses `E3001`.

A case pattern is qualified by the scrutinee's union and lists one binder for
each positional payload:

```nexa
const result = match (value) {
  case List.Empty() => 0;
  case List.Node(head, tail) => head;
};
```

- A case qualifier or variant is resolved by the same namespace rules as a
  constructor. A case from another existing union is a foreign case and uses
  `E3001`. An unknown qualifier uses `E2001`; an unknown variant uses `E2005`.
- Pattern arity must equal the variant payload count and uses `E2003` on a
  mismatch. Binders are immutable values with the corresponding payload types
  and are scoped only to that arm expression.
- Binder names must be unique within one pattern. A later duplicate uses
  `E2002` with a secondary label on the first binder. A binder may shadow an
  outer local under the existing lexical-scope rules.
- Nested patterns, literals, wildcards, guards, alternatives, and binding type
  annotations are not accepted. A pattern only binds the selected variant's
  immediate payload positions.
- A repeated case for the same resolved variant uses `E2002` and does not add
  exhaustiveness coverage.
- Without `default`, every variant of the scrutinee union must have one valid
  case. Missing variants use `E3006`. A reachable `default` covers every
  variant not covered by an earlier valid case.
- Every arm after a `default` is unreachable and uses `E3007`. A `default`
  that follows valid cases covering every declared variant is also unreachable
  and uses `E3007`.
- Unreachable arm expressions and patterns are still name- and type-checked;
  unreachable code does not hide independent diagnostics.
- Only the selected case/default expression executes. Pattern bindings are
  created before that expression. No expression from an unselected arm runs.

An expected type for the complete match propagates to every arm expression.
Without an expected type, the first reachable arm with a known type establishes
the result type. Every other arm must have exactly that type. Type mismatches
use `E3001`. Invalid scrutinee or case resolution suppresses derivative
exhaustiveness noise when the checker cannot determine a reliable union or
coverage set.

`E3006` primarily labels the complete match expression and reports missing
variants in declaration order. It may use secondary labels on their variant
declarations. `E3007` primarily labels the unreachable arm and uses a
secondary label on the earlier `default` or final covering case that made it
unreachable.

## Guarded Recursive Types

v0.5 admits recursive nominal data only when every type-dependency cycle
passes through at least one union variant payload boundary. Record fields,
including fields beneath any number of array suffixes, are unguarded edges.
Union variant payloads, including payloads beneath arrays, are guarded edges.

Equivalently, construct the named-type dependency graph and remove every edge
originating at a union payload. The remaining graph must be acyclic. Any cycle
that remains is an unguarded recursive record cycle and uses `E3005` under the
v0.4 diagnostic rules.

This direct union recursion is accepted:

```nexa
type List =
  | Empty()
  | Node(head: Int, tail: List);
```

This record/union cycle is also accepted because the return edge crosses a
variant payload:

```nexa
type Node = {
  next: Chain;
};

type Chain =
  | End()
  | More(node: Node);
```

This record-only cycle remains rejected, even through an array (`E3005`):

```nexa
type Node = {
  children: Node[];
};
```

The rule guarantees finite layout without requiring every recursive union to
have an inhabitable base variant. It does not make records structurally typed
or mutable.

## Runtime Union-Nesting Limit

The reference interpreter accepts a maximum recursive union nesting depth of
1024. Depth is defined structurally over an already evaluated value:

- scalars and `Unit` have depth zero;
- a record or array has the maximum depth of its contained values, or zero
  when it contains none; and
- a union value has depth one plus the maximum depth of its payload values, or
  one when it has no payloads.

Records and arrays therefore transmit depth but do not add to it. Constructing
a union value of depth 1024 succeeds. After all constructor arguments have
evaluated left to right, an attempted construction of depth 1025 fails at the
complete qualified-constructor span with the runtime message
`recursive union nesting limit of 1024 exceeded`. Output produced before the
failure remains observable and no later expression executes.

Matching, binding, passing, returning, or storing an existing union value does
not add depth. The v0.2 global 100000-basic-block execution-step limit and
64-active-call limit remain independent and unchanged.

## Evaluation And IR Contract

- Typed HIR resolves every construction, case, payload position, and binder to
  its `UnionId`, `VariantId`, type, and local identity before MIR lowering.
- CFG MIR constructs variants and dispatches on resolved variant identities.
  Payload extraction uses resolved positions. It never compares qualifier,
  variant, or binder source strings.
- A match lowers to control flow in which only the selected arm is reachable
  at runtime. Joining arms preserves the one exact match result type.
- Constructor arguments, the match scrutinee, and the selected arm obey the
  existing left-to-right, exactly-once and output-retention rules.
- Runtime tag/layout mismatches in malformed programmatic MIR are structured
  interpreter errors, not panics. Valid typed HIR must not produce them.
- Existing record construction/access, arrays, calls, loops, and runtime limits
  retain their v0.4 behavior.

## Required Diagnostics

v0.5 retains every v0.4 diagnostic and adds `E3006` and `E3007`.

| Code | Condition |
|---|---|
| `E1001` | malformed union, constructor, match, case, default, pattern, arrow, delimiter, or separator syntax |
| `E2001` | undefined value, function, named type, or union qualifier |
| `E2002` | duplicate binding, type, variant, payload name, pattern binder, or resolved variant case |
| `E2003` | function, constructor, or pattern arity mismatch |
| `E2005` | unknown record field, value member, or union variant |
| `E3001` | type mismatch, non-union/foreign case, invalid union operation, bare constructor, or invalid match result |
| `E3005` | unguarded direct or indirect recursive record declaration |
| `E3006` | non-exhaustive match expression |
| `E3007` | match arm unreachable after a default or complete coverage |

Union equality, union printing, arbitrary member access on a union value, a
non-union qualifier, a foreign case, and a bare qualified constructor all use
`E3001`. Diagnostic recovery must continue checking later declarations and
unreachable arm bodies without inventing duplicate coverage.

## Rejected Programs

A match must cover every variant (`E3006`):

```nexa
type Flag = | Off() | On();

function value(flag: Flag): Int {
  return match (flag) {
    case Flag.Off() => 0;
  };
}
```

A default after complete coverage is unreachable (`E3007`):

```nexa
type Flag = | Off() | On();

function value(flag: Flag): Int {
  return match (flag) {
    case Flag.Off() => 0;
    case Flag.On() => 1;
    default => 2;
  };
}
```

A repeated variant case is a duplicate (`E2002`):

```nexa
type Flag = | Off() | On();

function value(flag: Flag): Int {
  return match (flag) {
    case Flag.Off() => 0;
    case Flag.Off() => 1;
    default => 2;
  };
}
```

Constructor and pattern arity are exact (`E2003`):

```nexa
type Value = | Integer(value: Int);

function main(): Unit {
  const value = Value.Integer();
}
```

Cases must belong to the scrutinee union (`E3001`):

```nexa
type Left = | Value(value: Int);
type Right = | Value(value: Int);

function read(value: Left): Int {
  return match (value) {
    case Right.Value(inner) => inner;
    default => 0;
  };
}
```

Union values do not gain object operations (`E3001`):

```nexa
type Flag = | Off() | On();

function main(): Unit {
  print(Flag.On());
}
```

## Non-Goals

Language Core v0.5 does not add structural or anonymous unions, unqualified or
first-class constructors, named constructor arguments, optional payloads,
variant defaults, wildcard/literal/nested/or patterns, guards, destructuring
outside match, union equality, union printing, reflection, exposed tag values,
generic unions, `Option`, `Result`, modules, imports, multiple source files,
exceptions, garbage collection as an observable language contract, native
AOT, LLVM, WebAssembly, UI, OXC, SWC, or TypeScript/JavaScript compatibility.
