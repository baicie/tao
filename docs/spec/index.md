# Nexa Language Spec

This directory records accepted language behavior and rejected alternatives.

The current delivered core is
[Language Core v0.7](language-core-v0.7.md). It extends
[Language Core v0.6](language-core-v0.6.md) with bounded generic functions and
nominal data, local type-argument inference, ordinary source-defined
`Option`/`Result` unions, deterministic instance limits, and definition-level
MIR erasure. Nexa remains a TypeScript-shaped language with its own native
semantics, not a TypeScript implementation or compatibility layer.

The active target is the
[Nexa Language 1.0 Reference Core](language-1.0.md). It adds nominal records,
tagged unions with exhaustive matching, explicit multi-file modules, bounded
generics, error values, and a practical immutable-data core through versioned
milestones. The target document does not describe already-delivered behavior;
each feature becomes normative only when its milestone is delivered.

Language Core v0.7 is normative and delivered; its completed gates remain in
the [v0.7 roadmap](../project/roadmap/language-core-v0.7.md). The active
implementation milestone is v0.8, which targets typed function values,
lexical closures, deterministic array iteration, and practical immutable-data
operations under the broader
[Nexa Language 1.0 roadmap](../project/roadmap/language-1.0.md).

OXC and SWC are not part of the core language implementation. If TypeScript
interop is added later, it must be isolated in an adapter crate that lowers
into Nexa HIR and does not make JavaScript runtime semantics or third-party
AST types part of the language contract.

The original integer-binding parser was the bootstrap milestone. Language Core
v0.1 established the first checked and interpreted language slice, v0.2 added
stateful CFG control flow, v0.3 added immutable strings and arrays, v0.4 added
nominal records, v0.5 added tagged unions and matching, and v0.6 added
multi-file modules. v0.7 added bounded generics and ordinary error values; v0.8
is active and is not yet delivered.
