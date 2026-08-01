# Nexa 2.0 Full-Stack Example

This project is a contract-first example for ADR-001 through ADR-009. It
models an incident-response field console that synchronizes incidents,
captures field evidence, renders a cross-platform UI, and exports a signed
component.

## Status

| Area | Status | Validation |
|------|--------|------------|
| `baseline-1.0` | Runnable now | `nexac check` and `nexac run` |
| `target-2.0/src` | Accepted target design | ADR coverage and fixture review |
| Host/component IDL | Accepted target design | Schema fixture review |
| `nexa.toml` / `nexa.lock` | Accepted target design | Package fixture review |
| Wasm/UI/release metadata | Accepted target design | Loader-policy fixture review |

The current compiler is `nexac 0.0.1` and implements Language 1.0 only. It
must reject or fail to parse many files under `target-2.0`; that is expected.
Those files are examples of the accepted 2.0 contracts, not claims that the
features are implemented.

## Scenario

The field console uses every Host capability family accepted by ADR-005:

* filesystem for offline import and report export;
* network for incident synchronization;
* time and random for deadlines and correlation IDs;
* environment and process for an explicitly scoped support tool;
* UI and clipboard for the operator workflow;
* camera and microphone for field evidence;
* GPU for the incident timeline;
* database for the local cache.

The application keeps these capabilities behind typed Host interfaces. It
does not expose OS handles, DOM nodes, JavaScript objects, or raw pointers to
safe Nexa code.

## Layout

```text
baseline-1.0/             runnable compatibility anchor
target-2.0/src/           cohesive Nexa 2.0 application sources
target-2.0/host/          versioned Host IDL
target-2.0/component/     stable component import/export IDL
target-2.0/tests/         accepted and compile-fail contracts
nexa.toml                 package, target, capability, and build policy
nexa.lock                 illustrative canonical locked graph
release/                  component and signing policy inputs
ADR-COVERAGE.md           decision-to-file coverage matrix
```

## Run The Delivered Baseline

```bash
cargo run -p nexac -- check examples/nexa-2-full-stack/baseline-1.0/main.nexa
cargo run -p nexac -- run examples/nexa-2-full-stack/baseline-1.0/main.nexa
```

Expected output:

```text
Incident Field Console
22
3
```

## Future Nexa 2.0 Flow

The following commands document the intended workflow. They are not available
in `nexac 0.0.1`:

```text
nexac package resolve --locked
nexac check --target field-console
nexac test --accepted --compile-fail
nexac build --target wasm32-component --release --reproducible
nexac component verify release/component.toml
nexac component sign --policy release/signing-policy.toml
```

No private key, access token, generated signature, binary component, MIR, or
NIR is checked into this example. Signing is deliberately represented as a
policy and build step rather than by fake security material.
