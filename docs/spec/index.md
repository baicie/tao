# Nexa Language Spec

This directory records accepted language behavior and rejected alternatives.

The current delivered core is
[Language Core v0.4](language-core-v0.4.md). It extends
[Language Core v0.3](language-core-v0.3.md) with nominal immutable records,
contextually typed record literals, exact field checking, and immutable field
access. Nexa remains a TypeScript-shaped language with its own native
semantics, not a TypeScript implementation or compatibility layer.

The active target is the
[Nexa Language 1.0 Reference Core](language-1.0.md). It adds nominal records,
tagged unions with exhaustive matching, explicit multi-file modules, bounded
generics, error values, and a practical immutable-data core through versioned
milestones. The target document does not describe already-delivered behavior;
each feature becomes normative only when its milestone is delivered.

Language Core v0.4 is normative and delivered. The active implementation
milestone is v0.5, which adds nominal tagged unions and exhaustive matching as
specified by the [1.0 roadmap](../project/roadmap/language-1.0.md).

OXC and SWC are not part of the core language implementation. If TypeScript
interop is added later, it must be isolated in an adapter crate that lowers
into Nexa HIR and does not make JavaScript runtime semantics or third-party
AST types part of the language contract.

The original integer-binding parser was the bootstrap milestone. Language Core
v0.1 established the first checked and interpreted language slice, v0.2 added
stateful CFG control flow, v0.3 added immutable strings and arrays, and v0.4 is
the current nominal-record core.
