# Nexa Language Core v0.2

## Product Contract

Language Core v0.2 extends [Language Core v0.1](language-core-v0.1.md) with
mutable local state and structured loops. Existing v0.1 programs retain their
behavior unless they used one of the new reserved words (`let`, `while`,
`break`, or `continue`) as an identifier. `nexac check` statically validates a
single source file, and `nexac run` executes its control-flow graph (CFG) MIR
with the reference interpreter.

Nexa continues to use TypeScript-shaped syntax while defining independent
semantics. It is not a TypeScript or JavaScript compatibility layer and does
not acquire JavaScript truthiness, coercion, object, or runtime behavior in
v0.2.

## Accepted Program

```nexa
function sumUntil(limit: Int): Int {
  let current: Int = 0;
  let total: Int = 0;

  while (current < limit && total < 100) {
    current = current + 1;

    if (current === 2) {
      continue;
    }

    total = total + current;
    if (total > 20 || current === limit) {
      break;
    }
  }

  return total;
}

function main(): Unit {
  print(sumUntil(5));
}
```

The program prints `13`.

## Grammar

Whitespace and `//` line comments may appear between grammar symbols and
remain in the lossless concrete syntax tree.

```text
SourceFile           = FunctionDeclaration*
FunctionDeclaration  = "function" Identifier "(" ParameterList? ")" ":" Type Block
ParameterList        = Parameter ("," Parameter)*
Parameter            = Identifier ":" Type
Type                 = "Int" | "Bool" | "Unit"
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
Unary                = ("!" | "-") Unary | Call
Call                 = Primary ("(" ArgumentList? ")")*
ArgumentList         = Expression ("," Expression)*
Primary              = Integer | "true" | "false" | Identifier | "(" Expression ")"
```

`AssignmentStatement` is a statement with an identifier target. Assignment is
not an expression and therefore cannot appear inside an initializer,
condition, argument, return value, or another expression.

## Binding And Assignment Semantics

- `let` introduces a mutable, lexically scoped local binding.
- Every `let` declaration must have an initializer. A declaration such as
  `let count: Int;` is malformed syntax (`E1001`).
- A `let` type annotation is optional only when the initializer has an
  inferable type. Every later assignment must have exactly the binding's type.
- An assignment resolves its target in the current lexical scope and updates
  the nearest matching `let` binding after evaluating the right-hand side.
- `const` bindings and function parameters are immutable. Assigning to either
  produces `E2004` at the assignment target.
- Assigning to an unresolved name produces `E2001`. Duplicate `let`, `const`,
  and parameter names continue to use the v0.1 `E2002` rules.

## Loop Semantics

- `while` evaluates its condition before every iteration and enters the body
  only when the result is `true`.
- A `while` condition must have type `Bool`. A non-`Bool` condition reuses
  `E3002`, the same diagnostic used for a non-`Bool` `if` condition.
- `break` exits the nearest lexically enclosing `while` loop.
- `continue` transfers control to the condition of the nearest lexically
  enclosing `while` loop.
- `break` or `continue` outside a `while` body produces `E3004`.

## Logical Operator Semantics

- `&&` and `||` accept only `Bool` operands and produce `Bool`. A non-`Bool`
  operand produces `E3001`.
- Evaluation is left to right and short-circuited. `false && right` does not
  evaluate `right`; `true || right` does not evaluate `right`.
- Both operands are still statically resolved and type-checked even when a
  constant left operand would skip the right operand at runtime.
- Logical operators do not use truthiness or implicit coercion.

The following program prints only `1`; neither call to `probe` is evaluated:

```nexa
function probe(): Bool {
  print(99);
  return true;
}

function main(): Unit {
  if (false && probe()) {
    print(0);
  }

  if (true || probe()) {
    print(1);
  }
}
```

## CFG MIR Contract

- Every function lowers to a control-flow graph of basic blocks.
- A basic block contains an ordered sequence of MIR operations followed by
  exactly one terminator. There is no implicit fallthrough.
- Terminators represent an unconditional jump, a conditional branch, or a
  function return. Every branch target belongs to the same function.
- A `while` loop has an explicit condition block, body path, and exit block.
  `continue` targets the nearest loop's condition block, and `break` targets
  its exit block.
- `&&` and `||` lower to conditional control flow. The skipped operand must
  not be emitted as an eagerly evaluated binary operation.
- Assignment updates the MIR local associated with the resolved `let`
  binding. The interpreter consumes MIR and does not inspect CST or HIR.

## Reference Interpreter Limits

- The interpreter permits at most 64 active function calls, including
  `main`. An attempt to create a 65th active call reports a runtime error
  before entering that call.
- One MIR step is one execution of a basic block, including the function entry
  block. A run has one global budget of 100,000 MIR basic-block steps shared by
  `main` and every nested call.
- The interpreter permits the first 100,000 steps and reports a runtime error
  before executing step 100,001. Calls and loop iterations do not reset the
  budget.
- Output produced before a runtime error remains observable.

## Required Diagnostics

Diagnostics use stable source spans and stable codes. v0.2 retains all v0.1
codes and adds `E2004` and `E3004`.

| Code | Condition |
|---|---|
| `E1001` | malformed syntax, including an uninitialized `let` |
| `E2001` | undefined name or assignment target |
| `E2002` | duplicate binding or function |
| `E2003` | incorrect call arity |
| `E2004` | assignment to an immutable binding or parameter |
| `E3001` | type mismatch, including non-`Bool` logical operands |
| `E3002` | non-`Bool` `if` or `while` condition |
| `E3003` | invalid or missing return |
| `E3004` | `break` or `continue` outside a loop |

## Rejected Programs

An uninitialized mutable binding is malformed (`E1001`):

```nexa
function main(): Unit {
  let count: Int;
}
```

A `const` binding cannot be assigned (`E2004`):

```nexa
function main(): Unit {
  const answer: Int = 42;
  answer = 0;
}
```

A parameter cannot be assigned (`E2004`):

```nexa
function reset(value: Int): Unit {
  value = 0;
}

function main(): Unit {}
```

A `while` condition cannot use truthiness (`E3002`):

```nexa
function main(): Unit {
  while (1) {
    break;
  }
}
```

Loop control must be inside a loop (`E3004`):

```nexa
function main(): Unit {
  break;
}
```

```nexa
function main(): Unit {
  continue;
}
```

Logical operands must be `Bool` (`E3001`):

```nexa
function main(): Unit {
  const invalid: Bool = true && 1;
}
```

Assignment cannot be used as an expression (`E1001`):

```nexa
function main(): Unit {
  let value: Int = 0;
  const copy: Int = (value = 1);
}
```

## Non-Goals

Language Core v0.2 remains independent from TypeScript and JavaScript. It does
not add TypeScript compatibility, JavaScript runtime semantics, OXC, or SWC.
A future interoperability adapter must remain outside the core parser and IRs.

The v0.2 grammar deliberately excludes `var`, `++`, `--`, assignment
expressions, truthiness, `for`, `String`, arrays, modules and imports,
multiple source files, structs, enums, generics, traits, closures, async
execution, garbage collection, LLVM, native AOT code generation, UI, and
platform adapters.
