# Changelog

All notable changes to this project will be documented in this file.

## 0.0.3 - 2026-08-02

- Accept `.ft` source entries and imports while preserving the `.nexa`
  Language 1.0 compatibility corpus and mixed-extension migration graphs.
- Add a Host-independent `CompilerInput -> CompilerOutput` API with validated
  logical source identities and structured diagnostics, typed HIR, and MIR.
- Add versioned canonical token, CST, diagnostic, HIR, MIR, and explicit
  unavailable-NIR outputs plus an honest Rust/Futao differential adapter.
- Add `nexac dump` and determinism coverage across input order, Host paths,
  working directories, locale, timezone, and repeated compiler executions.

## 0.0.2 - 2026-08-01

- Accept the mixed self-hosting route with target-neutral internal NIR as the
  Stage boundary and `.nexc` as a separate stable component lifecycle.
- Pin the Rust `nexac 0.0.1` Stage 0 source commit, archive and lockfile
  SHA-256 digests, Rust 1.80 toolchain, and locked release build recipe.
- Add `cargo xtask bootstrap-contract` validation and a release-gated clean
  Stage 0 rebuild with accepted and rejected manifest coverage.

## 0.0.1 - 2026-08-01

- Package the complete Nexa Language 1.0 Reference Core as the self-use
  `nexac` CLI.
- Keep Rust crate APIs and distribution pre-stable while preserving the
  Language 1.0 source, diagnostic, and runtime compatibility baseline.
