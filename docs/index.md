---
layout: home

hero:
  name: "Futao"
  text: "Nexa 1.0 Bootstrap Compiler"
  tagline: "The current Rust bootstrap for Futao, with independent static semantics and a MIR interpreter."
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/baicie/tao

features:
  - title: Compiler-Shaped Workspace
    details: Crates are organized by compiler phase boundaries instead of generic app layers.
  - title: Stable Source Spans
    details: Diagnostics and parser output share source ranges through nexa_span.
  - title: Checked Execution
    details: nexac loads a reachable module graph, type-checks it, lowers it to CFG MIR, and runs the resolved entry.
  - title: Immutable Data
    details: UTF-8 strings, homogeneous arrays, generic nominal data, closures, ordinary error values, and command-line arguments have explicit native semantics.
  - title: Compatibility Baseline
    details: Nexa Language 1.0 is delivered; v0.9 froze and validated the complete reference-core surface without adding syntax.
  - title: Native Semantics
    details: Familiar TypeScript-shaped syntax does not imply JavaScript runtime compatibility.
  - title: Rust Tooling
    details: fmt, clippy, tests, docs, xtask automation, CI, and security checks stay in place.
---
