---
layout: home

hero:
  name: "Nexa"
  text: "Rust bootstrap compiler workspace"
  tagline: A small first compiler front end: spans, diagnostics, syntax, parser, and nexac.
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
  - title: Front-End Loop
    details: nexac can parse and check source files while the grammar grows.
  - title: Rust Tooling
    details: fmt, clippy, tests, docs, xtask automation, CI, and security checks stay in place.
---
