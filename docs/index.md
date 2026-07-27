---
layout: home

hero:
  name: "Nexa"
  text: "Language Core v0.3"
  tagline: "A TS-shaped language with independent static semantics and a MIR interpreter."
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/baicie/nexa

features:
  - title: Compiler-Shaped Workspace
    details: Crates are organized by compiler phase boundaries instead of generic app layers.
  - title: Stable Source Spans
    details: Diagnostics and parser output share source ranges through nexa_span.
  - title: Checked Execution
    details: nexac parses, type-checks, lowers to CFG MIR, and runs a single source file.
  - title: Immutable Data
    details: UTF-8 strings, homogeneous arrays, and command-line arguments have explicit native semantics.
  - title: Native Semantics
    details: Familiar TypeScript-shaped syntax does not imply JavaScript runtime compatibility.
  - title: Rust Tooling
    details: fmt, clippy, tests, docs, xtask automation, CI, and security checks stay in place.
---
