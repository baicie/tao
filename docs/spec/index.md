# Nexa Language Spec

This directory records accepted language behavior and rejected alternatives.

The active target is [Language Core v0.1](language-core-v0.1.md): a small,
TypeScript-shaped language with its own native semantics. It is not a
TypeScript implementation or compatibility layer.

OXC and SWC are not part of the core language implementation. If TypeScript
interop is added later, it must be isolated in an adapter crate that lowers
into Nexa HIR and does not make JavaScript runtime semantics or third-party
AST types part of the language contract.

The original integer-binding parser was the bootstrap milestone. Language Core
v0.1 is the active implementation target.
