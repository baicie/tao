# Nexa Language Spec

This directory records accepted language behavior and rejected alternatives.

The active target is [Language Core v0.2](language-core-v0.2.md). It extends
[Language Core v0.1](language-core-v0.1.md) with initialized mutable bindings,
assignment statements, structured loops, and Bool-only short-circuit logical
operators. Nexa remains a TypeScript-shaped language with its own native
semantics, not a TypeScript implementation or compatibility layer.

OXC and SWC are not part of the core language implementation. If TypeScript
interop is added later, it must be isolated in an adapter crate that lowers
into Nexa HIR and does not make JavaScript runtime semantics or third-party
AST types part of the language contract.

The original integer-binding parser was the bootstrap milestone. Language Core
v0.1 established the first checked and interpreted language slice; v0.2 is the
current delivered language core.
