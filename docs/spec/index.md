# Nexa Language Spec

This directory records accepted language behavior and rejected alternatives.

The current delivered core is
[Language Core v0.3](language-core-v0.3.md). It extends
[Language Core v0.2](language-core-v0.2.md) with immutable UTF-8 strings,
immutable homogeneous arrays, indexing and `.length`, widened `print`, and
command-line arguments. Nexa remains a TypeScript-shaped language with its own
native semantics, not a TypeScript implementation or compatibility layer.

The active target is the
[Nexa Language 1.0 Reference Core](language-1.0.md). It adds nominal records,
tagged unions with exhaustive matching, explicit multi-file modules, bounded
generics, error values, and a practical immutable-data core through versioned
milestones. The target document does not describe already-delivered behavior;
each feature becomes normative only when its milestone is delivered.

OXC and SWC are not part of the core language implementation. If TypeScript
interop is added later, it must be isolated in an adapter crate that lowers
into Nexa HIR and does not make JavaScript runtime semantics or third-party
AST types part of the language contract.

The original integer-binding parser was the bootstrap milestone. Language Core
v0.1 established the first checked and interpreted language slice, v0.2 added
stateful CFG control flow, and v0.3 is the current immutable-data core.
