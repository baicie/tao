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

## Versioned Contracts

- [v0.1](language-core-v0.1.md): checked functions and CFG MIR execution
- [v0.2](language-core-v0.2.md): mutable locals and control flow
- [v0.3](language-core-v0.3.md): strings, arrays, and CLI arguments
- [v0.4](language-core-v0.4.md): nominal records
- [v0.5](language-core-v0.5.md): tagged unions and matching
- [v0.6](language-core-v0.6.md): multi-file modules
- [v0.7](language-core-v0.7.md): bounded generics and error values
- [v0.8](language-core-v0.8.md): function values, closures, and iteration
- [v0.9](language-core-v0.9.md): the frozen 1.0 compatibility surface

The completed integration evidence is retained in the
[Language 1.0 delivery archive](../project/archive/language-1.0-delivery.md).

Cross-version guarantees are summarized in the
[Compatibility Policy](../compatibility.md). The
[Diagnostic Reference](../reference/diagnostics.md) collects the complete
required code set and normative runtime failures.

OXC and SWC are not part of the core language implementation. If TypeScript
interop is added later, it must be isolated in an adapter crate that lowers
into Nexa HIR and does not make JavaScript runtime semantics or third-party
AST types part of the language contract.
