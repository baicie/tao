# Nexa Language Core v0.4

## Status

Language Core v0.4 is delivered and is the current executable language. Its
implementation satisfies the exit criteria in the
[v0.4 roadmap](../project/roadmap/language-core-v0.4.md). Development continues
with the versioned milestones in the
[Nexa Language 1.0 roadmap](../project/roadmap/language-1.0.md).

## Product Contract

Language Core v0.4 extends v0.3 with nominal, immutable record types. A record
has a declared name and an exact ordered set of named fields. Record literals
use familiar TypeScript-shaped object-literal syntax, but they are checked
against a named expected type and do not introduce structural object semantics.

`nexac check` continues to validate one source file, and `nexac run` continues
to execute CFG MIR with the reference interpreter. Modules, imports, tagged
unions, pattern matching, and generics remain later milestones.

The only newly reserved word is `type`. A v0.3 program that used `type` as an
identifier is not source-compatible with v0.4. All other accepted v0.3 programs
retain their behavior.

## Accepted Program

```nexa
type User = {
  name: String;
  scores: Int[];
};

function total(values: Int[]): Int {
  let index = 0;
  let result = 0;

  while (index < values.length) {
    result = result + values[index];
    index = index + 1;
  }

  return result;
}

function main(): Unit {
  const user: User = {
    name: "Ada",
    scores: [20, 22]
  };

  print(user.name);
  print(total(user.scores));
}
```

The program prints:

```text
Ada
42
```

## Grammar

Whitespace and `//` line comments may appear between grammar symbols and remain
in the lossless CST. These productions replace or extend the corresponding
v0.3 productions; all other v0.3 grammar remains unchanged.

```text
SourceFile            = Declaration*
Declaration           = RecordDeclaration | FunctionDeclaration
RecordDeclaration     = "type" Identifier "=" RecordBody ";"
RecordBody            = "{" RecordField* "}"
RecordField           = Identifier ":" Type ";"
Type                  = PrimaryType ("[" "]")*
PrimaryType           = "Int" | "Bool" | "String" | "Unit" | Identifier
Primary               = Integer
                      | StringLiteral
                      | "true"
                      | "false"
                      | Identifier
                      | ArrayLiteral
                      | RecordLiteral
                      | "(" Expression ")"
RecordLiteral         = "{" FieldInitializerList? "}"
FieldInitializerList  = FieldInitializer ("," FieldInitializer)*
FieldInitializer      = Identifier ":" Expression
```

A record declaration may contain zero or more fields. Every declared field has
a terminating semicolon. Record literal fields are comma-separated; a trailing
comma is not accepted. Field shorthand, computed field names, spreads, method
declarations, optional fields, and field defaults are not part of v0.4.

Record literals and statement blocks are unambiguous from their grammar
position: a block is required after a statement-level control-flow construct or
function declaration, while a record literal appears in an expression.

## Names And Nominality

- Record names occupy a type namespace separate from function and local value
  names. A record and a function may therefore use the same spelling.
- Record declarations are visible throughout their source file, including
  before their textual declaration. Duplicate record names use `E2002`.
- A named type reference that does not resolve to a record uses `E2001`.
- Field names are local to their declaring record. Duplicate declared fields
  and duplicate literal initializers use `E2002` at the later name, with a
  secondary label on the first declaration or initializer.
- Two separately declared record types are different even when their fields
  have the same names and types. Assignments, arguments, and returns require
  the exact nominal type.
- Typed HIR assigns stable source-order identities to records and fields. MIR
  field construction and access use those identities rather than repeating
  source-text lookup. The concrete Rust identifier types are an implementation
  detail, not part of the source-language contract.

## Construction Semantics

- A record literal does not infer a type from its field shape. It requires one
  exact expected record type from a declaration annotation, assignment target,
  function argument, return position, or enclosing record field.
- Every declared field must appear exactly once. A missing field uses `E2006`;
  an unknown field uses `E2005`; a duplicate field uses `E2002`.
- Each initializer must have exactly the declared field type. There is no
  implicit coercion, structural conversion, or width subtyping.
- Field initializers evaluate exactly once in literal source order. Runtime
  storage may use declaration order after evaluation, but storage order is not
  observable.
- Nested record and array expectations propagate into their literals. An empty
  array inside a record field therefore uses the declared field type as its
  v0.3 contextual array type.
- `Unit` is not a record field value type. A field declared as `Unit`, or as an
  array type containing `Unit`, uses `E3001`.

## Field Access And Immutability

- `value.field` first evaluates `value` exactly once, then reads the resolved
  field. The base must have a known record type.
- Accessing a field not declared by that exact record uses `E2005` at the field
  name. Existing array `.length` behavior remains unchanged.
- Record values and fields are immutable. The v0.4 assignment grammar still
  accepts only a bare identifier target, so `value.field = replacement;` is
  malformed syntax (`E1001`).
- Passing, returning, or binding a record may share immutable backing storage.
  Allocation identity, address, representation, and reference counts are not
  observable language behavior.
- Record equality and printing are not defined. Applying `===` to records or
  passing a record to `print` uses `E3001`.

## Recursive Record Rule

v0.4 rejects every direct or indirect record-type cycle. Construct a directed
graph with one node per record declaration and an edge from record `A` to
record `B` whenever any field of `A` contains `B`, including beneath one or
more array suffixes. If that graph contains a cycle, every declaration that
participates in the cycle is invalid and receives `E3005`.

This deliberately rejects both of these shapes:

```nexa
type Node = {
  children: Node[];
};
```

```nexa
type Left = {
  right: Right;
};

type Right = {
  left: Left;
};
```

Rejecting cycles keeps v0.4 value layout and termination rules finite. A later
tagged-union milestone must specify its own recursive-data representation
before any recursive nominal type becomes accepted.

## Evaluation And IR Contract

- Record construction preserves source-order, exactly-once field evaluation,
  including calls or runtime failures in initializers.
- A runtime failure in one initializer prevents evaluation of later
  initializers and preserves earlier `print` output.
- Typed HIR resolves every record literal, field initializer, and field access
  to stable nominal identities before MIR lowering.
- CFG MIR carries only resolved record construction and field-read operations.
  The interpreter does not read CST, compare source names, or perform type
  lookup.
- The v0.2 call-depth and global basic-block step limits remain unchanged.

## Required Diagnostics

v0.4 retains every v0.3 diagnostic and adds `E2006` and `E3005`.

| Code | Condition |
|---|---|
| `E1001` | malformed record syntax or field assignment |
| `E2001` | undefined value or named type |
| `E2002` | duplicate binding, function, record, declared field, or literal field |
| `E2005` | unknown record field or value-type member |
| `E2006` | record literal is missing a required field |
| `E3001` | type mismatch, unconstrained record literal, invalid record operand, or invalid field value type |
| `E3005` | direct or indirect recursive record declaration |

All diagnostics use stable source spans. Missing-field diagnostics primarily
label the complete record literal and may use secondary labels for the missing
field declarations. Recursive-record diagnostics label each participating
record name and identify at least one field edge that closes the cycle.

## Rejected Programs

An unconstrained literal does not create a structural type (`E3001`):

```nexa
function main(): Unit {
  const user = { name: "Ada" };
}
```

Every declared field is required (`E2006`):

```nexa
type User = {
  name: String;
  age: Int;
};

function main(): Unit {
  const user: User = { name: "Ada" };
}
```

Unknown fields are rejected (`E2005`):

```nexa
type User = {
  name: String;
};

function main(): Unit {
  const user: User = { name: "Ada", age: 42 };
}
```

Equal shapes do not erase nominal identity (`E3001`):

```nexa
type Left = {
  value: Int;
};

type Right = {
  value: Int;
};

function main(): Unit {
  const left: Left = { value: 1 };
  const right: Right = left;
}
```

Record cycles are invalid in v0.4 (`E3005`):

```nexa
type First = {
  second: Second;
};

type Second = {
  first: First;
};
```

## Non-Goals

Language Core v0.4 does not add structural object typing, anonymous record
types, type aliases, optional or default fields, spreads, computed properties,
methods, prototypes, classes, inheritance, interfaces, field mutation, record
equality, record printing, destructuring, recursive records, tagged unions,
pattern matching, generics, modules, imports, multiple source files, garbage
collection as a language contract, native AOT, LLVM, WebAssembly, UI, OXC,
SWC, or TypeScript/JavaScript compatibility.
