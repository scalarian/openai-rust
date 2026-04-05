# User Testing

Testing surface findings, validation tools, and resource-cost guidance for this mission.

**What belongs here:** how validators should exercise the SDK, what tools they should use, live-test budget rules, and per-surface concurrency limits.

---

## Validation Surface
- Primary surface: cargo-based unit, integration, doc, example, and env-gated live tests for the Rust crate API.
- Primary tools: `cargo test`, `cargo check`, `cargo doc`, `cargo package`, mock HTTP/WebSocket harnesses, and representative `cargo run --example ...` smoke checks.
- Browser/TUI tooling is not part of the main validation path for this mission.
- Live tests must source credentials via `set -a && . ./creds.txt && set +a` in the same shell invocation.

## Validation Strategy
- Broad coverage should come from mocked HTTP/WebSocket tests, transcript fixtures, multipart inspection, and parser/unit tests.
- Live coverage should prove end-to-end auth, request shaping, metadata capture, and a budget-capped set of representative real API flows.
- Documentation and packaging are part of the validation surface: examples, README snippets, docs guides, and `cargo package` outputs must validate mechanically.

## Validation Concurrency
### Mocked cargo validation
- Max concurrent validators: **4**
- Rationale: machine has 10 CPU cores and 16 GiB RAM; mocked cargo tests are CPU-bound but typically fit comfortably within ~70% of available headroom at concurrency 4.

### Live API validation
- Max concurrent validators: **1**
- Rationale: prevents rate-limit noise, simplifies request-id/evidence capture, and avoids accidental overspend.

### Packaging/docs validation
- Max concurrent validators: **2**
- Rationale: doc/package checks are lighter than compile-heavy test runs but still compete for disk and rustc resources.

## Budget Guardrails
- Media live coverage is capped to one image-generation smoke and one transcription smoke unless the orchestrator explicitly expands scope.
- Realtime live coverage should stay text-first unless a specific assertion explicitly requires audio/tool live proof.
- Advanced surfaces that are entitlement-sensitive should record explicit entitlement failures rather than silently downgrading to mock-only validation.
- Use the cheapest suitable models and the smallest meaningful fixtures.

## Evidence Requirements
- Always capture the exact command run.
- For live successes and failures, preserve request IDs whenever the SDK exposes them.
- For mocked transport tests, capture request bodies/headers and parser outputs.
- For examples and docs snippets, capture compile/run output proving API-shape accuracy.
- For cargo-based validation, the stdout usually names only the passing test functions; assertion-to-contract mapping may require inspecting the corresponding test source to confirm the exact behavior covered.

## Flow Validator Guidance: Mocked cargo validation
- Work from the repository root: `/Users/staticpayload/Mainframe/openai-rust`.
- Treat the Rust crate API as the user surface; validate behavior only through cargo-driven tests and their observable output.
- Stay inside the assigned assertion set and the existing core-foundation tests (`core_config`, `core_headers`, `core_timeout`, `core_retry`, `core_errors`, `core_response_meta`).
- Do not edit source files or `.factory` state from the flow validator; only write the assigned flow report and evidence artifacts.
- Use an isolated `CARGO_TARGET_DIR` under `.factory/validation/<milestone>/user-testing/target/<group>` for each flow report to avoid cargo lock contention between concurrent validators.
- Safe concurrency boundary: mocked cargo validators may run in parallel up to the global ceiling of 4 because they only compile/run isolated test binaries against loopback mocks and do not require shared mutable app state beyond Cargo build artifacts.
- Evidence should record the exact `cargo test` commands, exit codes, and the assertion-relevant observations from the output.

## Flow Validator Guidance: Live API validation
- Work from the repository root: `/Users/staticpayload/Mainframe/openai-rust`.
- Source credentials in the same shell as the live command: `set -a && . ./creds.txt && set +a`.
- Explicitly clear `OPENAI_BASE_URL` before the smoke so the validation proves default-host behavior: `unset OPENAI_BASE_URL`.
- Run only the assigned budget-capped live smoke; do not expand coverage or use extra models/endpoints.
- Use an isolated `CARGO_TARGET_DIR` under `.factory/validation/<milestone>/user-testing/target/<group>` for the live flow so repeated runs do not collide with mocked validators.
- Live validation is serialized at concurrency 1 to avoid rate-limit noise and to keep evidence tied to a single request sequence.
- Capture the command, exit code, resolved default-host proof from output, and any surfaced request ID.
