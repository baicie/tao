# Nexa Language Spec

This directory records accepted language behavior and rejected alternatives.

The initial compiler milestone is intentionally small:

- stable source spans
- structured diagnostics
- lossless tokenization
- parser entry points
- `nexac parse` and `nexac check`

## MVP Grammar

The first grammar slice accepts source files containing zero or more integer
bindings:

```text
SourceFile   = LetStatement*
LetStatement = "let" Identifier "=" Integer ";"
```

`Identifier` starts with an ASCII letter or `_` and continues with ASCII
letters, digits, or `_`. `Integer` is one or more ASCII decimal digits.
Whitespace and line comments may appear between grammar symbols and remain in
the lossless concrete syntax tree.

Malformed statements produce source-spanned diagnostics. Parsing resumes at a
semicolon or the next `let` keyword so later statements remain available in
the tree.

Accepted:

```nexa
let answer = 42;
let _next = 7;
```

Rejected:

```nexa
let = 42;
let answer = ;
```

This slice does not define expressions, types, name resolution, or runtime
semantics.
