# Nexa Language Core v0.1

## Product Contract

Nexa Language Core v0.1 is a single-file, statically checked language that
uses familiar TypeScript-shaped syntax while defining independent native
semantics. `nexac check` validates a program and `nexac run` executes its MIR
with the bundled interpreter.

Nexa does not accept TypeScript or JavaScript compatibility as a language
requirement. In particular, it has no `any`, implicit `undefined`, truthiness,
or implicit numeric/string coercion.

## Accepted Program

```nexa
function add(left: Int, right: Int): Int {
  return left + right;
}

function main(): Unit {
  const answer: Int = add(40, 2);

  if (answer === 42) {
    print(answer);
  }
}
```

## Grammar

Whitespace and `//` line comments may appear between all grammar symbols and
remain in the lossless concrete syntax tree.

```text
SourceFile           = FunctionDeclaration*
FunctionDeclaration  = "function" Identifier "(" ParameterList? ")" ":" Type Block
ParameterList        = Parameter ("," Parameter)*
Parameter            = Identifier ":" Type
Type                 = "Int" | "Bool" | "Unit"
Block                = "{" Statement* "}"
Statement            = ConstDeclaration | IfStatement | ReturnStatement | ExpressionStatement
ConstDeclaration     = "const" Identifier (":" Type)? "=" Expression ";"
IfStatement          = "if" "(" Expression ")" Block ("else" Block)?
ReturnStatement      = "return" Expression? ";"
ExpressionStatement  = Expression ";"
Expression           = Equality
Equality             = Comparison ("===" Comparison)*
Comparison           = Addition (("<" | "<=" | ">" | ">=") Addition)*
Addition             = Multiplication (("+" | "-") Multiplication)*
Multiplication       = Unary (("*" | "/") Unary)*
Unary                = ("!" | "-") Unary | Call
Call                 = Primary ("(" ArgumentList? ")")*
ArgumentList         = Expression ("," Expression)*
Primary              = Integer | "true" | "false" | Identifier | "(" Expression ")"
```

Identifiers start with an ASCII letter or `_` and continue with ASCII letters,
digits, or `_`. Integers are ASCII decimal literals.

## Semantics

- `Int` is a signed 64-bit integer. Overflow and division by zero are runtime
  errors in the interpreter.
- `Bool` has exactly `true` and `false`; conditions must have type `Bool`.
- `Unit` is the return type of computations with no value.
- `const` introduces an immutable, lexically scoped binding. A type annotation
  is optional only when the initializer has an inferable type.
- Function parameter and return types are always explicit. Calls require an
  exact argument count and exact argument types.
- `===` compares two values of the same type and returns `Bool`.
- Non-`Unit` functions must return a value on every control-flow path.
- `main` is the only entry point for `nexac run`; it takes no parameters and
  returns `Unit`.
- `print` is the sole v0.1 builtin. It accepts one `Int` and returns `Unit`.

## Required Diagnostics

Diagnostics use stable source spans and stable codes. The initial set is:

| Code | Condition |
|---|---|
| `E1001` | malformed syntax |
| `E2001` | undefined name |
| `E2002` | duplicate binding or function |
| `E2003` | incorrect call arity |
| `E3001` | type mismatch |
| `E3002` | non-boolean condition |
| `E3003` | invalid or missing return |

## Rejected Examples

```nexa
function main(): Unit {
  if (42) {
    print(1);
  }
}
```

```nexa
function main(): Unit {
  print(missing);
}
```

```nexa
function main(): Unit {
  const answer: Bool = 42;
}
```

## Non-Goals

Language Core v0.1 deliberately excludes TypeScript/JavaScript compatibility,
OXC/SWC, imports, multiple source files, mutation, loops, strings, collections,
structs, enums, generics, traits, closures, async execution, garbage
collection, UI, native AOT code generation, and an LLVM dependency.
