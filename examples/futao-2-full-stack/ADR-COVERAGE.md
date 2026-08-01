# ADR Coverage

This matrix maps each accepted design decision to a concrete part of the
Incident Field Console example. It is a review map, not an implementation
status claim.

| ADR | Capability demonstrated | Primary fixtures | Negative proof |
|-----|--------------------------|------------------|----------------|
| ADR-001 | TypeScript-shaped records, classes, interfaces, profiles, explicit capabilities | `target-2.0/src/model.ft`, `futao.toml` | Dynamic JavaScript and ambient authority are absent |
| ADR-002 | Copy/Move, default borrow, `mut`, `take`, Drop, Shared/Weak | `model.ft`, `memory.ft`, `accepted/ownership-memory.ft` | `E4101` use after Move |
| ADR-003 | Stable descriptors, typed handles, ownership modes, private NIR/MIR | Host/component IDL | `E5750` internal layout export |
| ADR-004 | allocator provenance, String/Vec/Array, Box, Shared/Weak, DropArena/PlainArena | `memory.ft` | `E5730` Arena escape, `E5731` Drop in PlainArena |
| ADR-005 | versioned Host modules, stable function IDs, rights-bearing resources | `field-host.futao-idl`, `platform.ft` | No fd, pointer, DOM node, JS object, or ambient capability |
| ADR-006 | Result, `?`, try/catch, reverse cleanup, defer, explicit close, panic boundary | `errors.ft`, `workflows.ft`, `accepted/error-cleanup.ft` | `E5820` fallible defer |
| ADR-007 | lazy task ownership, structured scope, cancellation, deadline, bounded channel, worker and actor | `main.ft`, `workflows.ft`, `accepted/async-boundaries.ft` | `E5901` borrow across await, `E5950` unbounded channel |
| ADR-008 | Wasm MemoryView, generated JS mapping, patch/event batches, main-thread UI | component IDL, `ui.ft` | `E6050` UI handle sent to worker |
| ADR-009 | package/lock graph, reproducible component plan, signing and trust policy | `futao.toml`, `futao.lock`, `release/` | No payload, fake signature, private key, credential, MIR, or NIR |
| ADR-010 | Futao, `.ft`, `futao` command family and migration boundary | project README, root dedication, ADR-010 | Historical `nexac 0.0.1` is not relabeled |

## Host Capability Coverage

| Capability | Application use | Manifest scope | Host module |
|------------|-----------------|----------------|-------------|
| filesystem | Export the final report | `offline-cache`, `reports` | `filesystem@1` |
| network | Synchronize incident batches | incident service origin | `network@1` |
| time | Bound synchronization by a monotonic deadline | `monotonic`, `timer` | `time@1` |
| random | Generate correlation IDs | `correlation-id` | `random@1` |
| environment | Read the explicit support mode only | `FUTAO_SUPPORT_MODE` | `environment@1` |
| process | Launch the allowlisted support tool | `field-support-v1` | `process@1` |
| ui | Render validated patch batches | `primary-window` | `ui@1` |
| clipboard | Copy a correlation ID after an event | `write-text` | `clipboard@1` |
| camera | Capture bounded still evidence | `still-image` | `camera@1` |
| microphone | Capture bounded audio evidence | `bounded-recording` | `microphone@1` |
| gpu | Upload a typed timeline buffer | `timeline-buffer` | `gpu@1` |
| database | Commit the local synchronization state | `field-cache` | `database@1` |

## Validation Boundary

The files under `baseline-1.0` are executed by the delivered bootstrap
compiler. The files under `target-2.0` are contract fixtures for accepted
2.0 designs. Until a future Futao compiler runs the manifest test matrix, the
expected diagnostics are specifications rather than observed compiler output.

Release inputs intentionally contain no `.ftc` payload, generated ABI hash,
signature, key, token, MIR, NIR, LLVM IR, or provenance attestation. A release
build must compute those values from locked inputs and an external signer.
