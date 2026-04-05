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
