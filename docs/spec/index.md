# Nexa Language Spec

This directory records accepted language behavior and rejected alternatives.

The current delivered specification is the
[Nexa Language 1.0 Reference Core](language-1.0.md). It composes the delivered
Language Core milestones into a TypeScript-shaped language with independent
native semantics, not a TypeScript implementation or compatibility layer.

Language 1.0 includes nominal records, tagged unions with exhaustive matching,
explicit multi-file modules, bounded generics, ordinary error values, exact
function types, lexical closures, deterministic iteration, and practical
immutable-data operations. Each feature remains normatively defined by its
versioned milestone specification.

[Language Core v0.8](language-core-v0.8.md) completed the language surface.
The delivered [Language Core v0.9 contract](language-core-v0.9.md) then froze
the keyword, precedence, evaluation, diagnostic, conformance, fuzz,
determinism, performance, documentation, and release-validation boundaries
without adding syntax. The completed gates are recorded in the
[v0.9 roadmap](../project/roadmap/language-core-v0.9.md), the
[1.0 roadmap](../project/roadmap/language-1.0.md), and the
[delivery archive](../project/archive/language-1.0-delivery.md).

Cross-version guarantees are summarized in the
[Compatibility Policy](../compatibility.md). The
[Diagnostic Reference](../reference/diagnostics.md) collects the complete
required code set and normative runtime failures.

OXC and SWC are not part of the core language implementation. If TypeScript
interop is added later, it must be isolated in an adapter crate that lowers
into Nexa HIR and does not make JavaScript runtime semantics or third-party
AST types part of the language contract.

The original integer-binding parser was the bootstrap milestone. Language Core
v0.1 established the first checked and interpreted language slice, v0.2 added
stateful CFG control flow, v0.3 added immutable strings and arrays, v0.4 added
nominal records, v0.5 added tagged unions and matching, and v0.6 added
multi-file modules. v0.7 added bounded generics and ordinary error values; v0.8
added function values, lexical closures, iteration, and practical immutable
data operations.
