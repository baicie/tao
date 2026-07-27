# Nexa Language Core v0.3

## Product Contract

Language Core v0.3 extends
[Language Core v0.2](language-core-v0.2.md) with immutable UTF-8 strings,
immutable homogeneous arrays, and command-line arguments. `nexac check`
continues to validate one source file, while `nexac run` executes typed data
values through CFG MIR and the reference interpreter.

Nexa continues to use TypeScript-shaped syntax with independent semantics. A
Nexa `String` is not a JavaScript string, and a Nexa array is not a JavaScript
`Array`. v0.3 does not add implicit coercion, `undefined`, object identity, or
mutable collection behavior.

## Accepted Program

```nexa
function sum(values: Int[]): Int {
  let index = 0;
  let total = 0;

  while (index < values.length) {
    total = total + values[index];
    index = index + 1;
  }

  return total;
}

function main(args: String[]): Unit {
  const values: Int[] = [20, 22];
  print("Hello, " + args[0]);
  print(args[0] === "Nexa");
  print(values.length);
  print(sum(values));
}
```

Running this program with `nexac run file.nexa -- Nexa` prints:

```text
Hello, Nexa
true
2
42
```

## Grammar

Whitespace and `//` line comments may appear between grammar symbols and
remain in the lossless concrete syntax tree. The following grammar includes
the v0.3 additions while retaining all v0.2 statements and operators.

```text
SourceFile           = FunctionDeclaration*
FunctionDeclaration  = "function" Identifier "(" ParameterList? ")" ":" Type Block
ParameterList        = Parameter ("," Parameter)*
Parameter            = Identifier ":" Type
Type                 = PrimaryType ("[" "]")*
PrimaryType          = "Int" | "Bool" | "String" | "Unit"
Block                = "{" Statement* "}"
Statement            = ConstDeclaration
                     | LetDeclaration
                     | AssignmentStatement
                     | IfStatement
                     | WhileStatement
                     | BreakStatement
                     | ContinueStatement
                     | ReturnStatement
                     | ExpressionStatement
ConstDeclaration     = "const" Identifier (":" Type)? "=" Expression ";"
LetDeclaration       = "let" Identifier (":" Type)? "=" Expression ";"
AssignmentStatement  = Identifier "=" Expression ";"
IfStatement          = "if" "(" Expression ")" Block ("else" Block)?
WhileStatement       = "while" "(" Expression ")" Block
BreakStatement       = "break" ";"
ContinueStatement    = "continue" ";"
ReturnStatement      = "return" Expression? ";"
ExpressionStatement  = Expression ";"
Expression           = LogicalOr
LogicalOr            = LogicalAnd ("||" LogicalAnd)*
LogicalAnd           = Equality ("&&" Equality)*
Equality             = Comparison ("===" Comparison)*
Comparison           = Addition (("<" | "<=" | ">" | ">=") Addition)*
Addition             = Multiplication (("+" | "-") Multiplication)*
Multiplication       = Unary (("*" | "/") Unary)*
Unary                = ("!" | "-") Unary | Postfix
Postfix              = Primary PostfixSuffix*
PostfixSuffix        = "(" ArgumentList? ")"
                     | "[" Expression "]"
                     | "." Identifier
ArgumentList         = Expression ("," Expression)*
Primary              = Integer
                     | StringLiteral
                     | "true"
                     | "false"
                     | Identifier
                     | ArrayLiteral
                     | "(" Expression ")"
ArrayLiteral         = "[" ArgumentList? "]"
```

Array element assignment is not part of `AssignmentStatement`; its target
remains a bare identifier. Trailing commas are not accepted in parameter,
argument, or array-element lists.

## String Lexical Contract

- Source files and string values are valid UTF-8.
- A string literal begins and ends with `"` and cannot contain an unescaped
  quote, backslash, carriage return, or line feed.
- Exactly five escapes are accepted: `\\` for a backslash, `\"` for a quote,
  `\n` for line feed, `\r` for carriage return, and `\t` for horizontal tab.
  Any other escape is malformed syntax (`E1001`) with a stable label covering
  the complete literal token.
- Escape decoding does not perform Unicode normalization. Raw UTF-8 text is
  preserved as written, so canonically equivalent but differently encoded
  Unicode scalar sequences remain distinct values.

## String Semantics

- `String` values are immutable.
- `left + right` concatenates when both operands are `String`. Integer
  addition remains `Int + Int`; mixed operands produce `E3001`. There is no
  implicit conversion from `Int`, `Bool`, arrays, or `Unit` to `String`.
- `left === right` compares two `String` values by their exact decoded UTF-8
  contents and returns `Bool`. Because there is no normalization, equality
  does not collapse canonically equivalent Unicode sequences.
- Concatenation and equality evaluate their left operand before their right
  operand, exactly once each.
- v0.3 does not add string indexing, string `.length`, interpolation, or
  additional escape forms.

## Array Semantics

- `T[]` is an immutable, homogeneous, fixed-length array whose elements have
  exactly type `T`. `Unit[]` is not a valid value type.
- A non-empty array literal evaluates each element exactly once from left to
  right. Every element must have the same type; no numeric, string, or other
  coercion is applied.
- An empty literal `[]` has no element type by itself. It is accepted only
  when its surrounding declaration, assignment target, function argument, or
  return position supplies an exact array type. An unconstrained empty array
  produces `E3001` at the literal.
- `array[index]` evaluates `array` first and `index` second, exactly once. The
  base must have array type and the index must have type `Int`.
- `array.length` has type `Int` and is fixed when the array value is created.
  No other member is defined for arrays in v0.3; member lookup failures use
  `E2005`.
- A negative index or an index greater than or equal to `.length` is a runtime
  error. Its source span is the complete indexing expression, such as
  `values[index]`. Output emitted before the error remains observable.
- Array `===` is not defined in v0.3. Arrays have no observable identity,
  capacity, allocation address, or reference count.

An implementation may share immutable backing storage when arrays are bound,
passed to functions, returned, or indexed. Since v0.3 exposes neither mutation
nor identity comparison, this storage sharing cannot be observed by a Nexa
program and is not part of the language contract.

## Builtin And Entry-Point Semantics

- `print` has three accepted one-argument forms: `print(Int)`, `print(Bool)`,
  and `print(String)`. It returns `Unit`. Arrays and `Unit` cannot be printed.
- A runnable program has exactly one of these entry-point signatures:
  `function main(): Unit` or `function main(args: String[]): Unit`. Other
  `main` signatures produce `E3003`.
- `nexac run file.nexa` invokes a parameterless `main` only when no program
  arguments were supplied. Supplying arguments to that entry point is a
  runtime error before `main` begins.
- `nexac run file.nexa -- first second` passes `["first", "second"]` to
  `main(args: String[])`. Invoking that signature without arguments passes an
  empty array. The `--` separator is not included in the array.
- Program arguments are immutable strings. Nexa does not perform shell
  splitting or numeric/boolean conversion on them.

## Evaluation And IR Contract

- Observable expression evaluation is deterministic and left to right. This
  includes function arguments, array elements, string concatenation, equality,
  an indexing base, and its index.
- A runtime error stops subsequent evaluation. Lines emitted by `print` before
  the error remain observable, including for an array-bounds failure.
- Typed HIR records the resolved type of string, array, member, and index
  expressions. MIR lowering does not repeat source-level name or type lookup.
- CFG MIR carries string and array constants or constructed values without
  reading CST tokens. The interpreter consumes only MIR.
- The v0.2 call-depth limit and global basic-block step budget remain in
  effect.

## Required Diagnostics

Diagnostics use stable source spans and stable codes. v0.3 retains all v0.2
codes and adds `E2005`.

| Code | Condition |
|---|---|
| `E1001` | malformed syntax, including an invalid or unterminated string literal |
| `E2001` | undefined lexical name or assignment target |
| `E2002` | duplicate binding or function |
| `E2003` | incorrect function or builtin call arity |
| `E2004` | assignment to an immutable binding or parameter |
| `E2005` | unknown member for a value type |
| `E3001` | type mismatch, invalid indexing, heterogeneous array, or unconstrained empty array |
| `E3002` | non-`Bool` `if` or `while` condition |
| `E3003` | invalid return or `main` signature |
| `E3004` | `break` or `continue` outside a loop |

Array-bounds failures and passing program arguments to a parameterless `main`
are runtime errors, not static diagnostics.

## Rejected Programs

Unknown string escapes are malformed (`E1001`):

```nexa
function main(): Unit {
  print("invalid: \x");
}
```

Array elements must be homogeneous (`E3001`):

```nexa
function main(): Unit {
  const values = [1, true];
}
```

An empty array needs an exact contextual element type (`E3001`):

```nexa
function main(): Unit {
  const values = [];
}
```

An array index must be `Int` (`E3001`):

```nexa
function main(): Unit {
  const values = [20, 22];
  print(values[false]);
}
```

Only `.length` is defined for arrays (`E2005`):

```nexa
function main(): Unit {
  const values = [20, 22];
  print(values.capacity);
}
```

Array elements cannot be assigned (`E1001`):

```nexa
function main(): Unit {
  const values = [20, 22];
  values[0] = 0;
}
```

Array equality is outside v0.3 (`E3001`):

```nexa
function main(): Unit {
  print([1] === [1]);
}
```

The argument-taking entry point has exactly one `String[]` parameter
(`E3003`):

```nexa
function main(args: Int[]): Unit {}
```

## Non-Goals

Language Core v0.3 remains independent from TypeScript and JavaScript. It does
not add TypeScript compatibility, JavaScript runtime semantics, OXC, or SWC.

The v0.3 grammar and runtime deliberately exclude element assignment, mutable
arrays, array equality, array methods, slicing, growable collections, string
indexing, string interpolation, records or objects, `null`, `undefined`,
modules and imports, multiple source files, garbage collection as a language
contract, native AOT code generation, LLVM, UI, and platform adapters.
