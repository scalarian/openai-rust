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
- The shared mock HTTP harness is serialized rather than concurrency-aware; transport proofs that need overlapping requests currently require a bespoke loopback server in the owning test file.
- Live coverage should prove end-to-end auth, request shaping, metadata capture, and a budget-capped set of representative real API flows.
- Documentation and packaging are part of the validation surface: examples, README snippets, docs guides, and `cargo package` outputs must validate mechanically.
- Observed on `2026-04-06`: when `Cargo.toml` sets `include`, Cargo ignores `exclude` for package filtering, so publish-hygiene validation must rely on the final chosen manifest strategy and verify it with `cargo package --list` rather than assuming both keys compose.
- Observed on `2026-04-06`: `VAL-REALTIME-006` is not directly covered by the committed `realtime_*` test binaries; validating structured realtime `error` recovery currently requires an isolated temporary cargo probe that opens a local websocket, injects an `error` event, and then proves a later request still completes on the same connection.

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
- Stay inside the assigned assertion set and the relevant contract binaries or harnesses for that milestone (for publish-ready this includes `cross_surface_contract`, `upload_to_downstream_contract`, and other explicitly assigned cargo test targets).
- Do not edit source files or `.factory` state from the flow validator; only write the assigned flow report and evidence artifacts.
- Use an isolated `CARGO_TARGET_DIR` under `.factory/validation/<milestone>/user-testing/target/<group>` for each flow report to avoid cargo lock contention between concurrent validators.
- Safe concurrency boundary: mocked cargo validators may run in parallel up to the global ceiling of 4 because they only compile/run isolated test binaries against loopback mocks and do not require shared mutable app state beyond Cargo build artifacts.
- Evidence should record the exact `cargo test` commands, exit codes, and the assertion-relevant observations from the output.

## Flow Validator Guidance: Live API validation
- Work from the repository root: `/Users/staticpayload/Mainframe/openai-rust`.
- Source credentials in the same shell as the live command: `set -a && . ./creds.txt && set +a`.
- Explicitly clear `OPENAI_BASE_URL` before the smoke so the validation proves default-host behavior: `unset OPENAI_BASE_URL`.
- Run only the assigned budget-capped live smoke; do not expand coverage or use extra models/endpoints.
- Live stored chat-completion retrieval may be briefly eventually consistent after a `store=true` create; allow a short retry loop before concluding the stored completion is missing or malformed.
- Use an isolated `CARGO_TARGET_DIR` under `.factory/validation/<milestone>/user-testing/target/<group>` for the live flow so repeated runs do not collide with mocked validators.
- Live validation is serialized at concurrency 1 to avoid rate-limit noise and to keep evidence tied to a single request sequence.
- Capture the command, exit code, resolved default-host proof from output, and any surfaced request ID.
- Advanced-platform entitlement-aware live smokes are embedded as ignored tests inside `tests/containers_contract.rs`, `tests/skills_contract.rs`, and `tests/videos_contract.rs` rather than separate `tests/live_*.rs` binaries.
- Observed on `2026-04-06`: the staged project credentials successfully reached the containers, skills, and videos entitlement-aware live smokes without hitting skip paths, and each surface printed live request IDs.
- Observed on `2026-04-06`: `tests/live_cross_surface_smoke.rs` normalizes the main per-surface request IDs to `request_id:present` in its paired report and only prints the cleanup file-delete request ID verbatim, so publish-ready evidence should preserve both the paired normalized report and the explicitly surfaced cleanup request ID.

## Flow Validator Guidance: Packaging/docs validation
- Work from the repository root: `/Users/staticpayload/Mainframe/openai-rust`.
- Validate only through published-facing commands and cargo-native harnesses such as `cargo test --test readme_contract`, `cargo test --test docs_contract`, `cargo test --doc`, `cargo check --examples --all-features`, `cargo metadata`, `cargo package --allow-dirty`, and `cargo package --list` when assigned.
- Use an isolated `CARGO_TARGET_DIR` under `.factory/validation/<milestone>/user-testing/target/<group>` for commands that compile or test so doc/example validators do not contend with other cargo jobs.
- Treat `README.md`, `docs/*.md`, `Cargo.toml`, and the package file list as the user-visible surface; do not edit repository files from the flow validator.
- Safe concurrency boundary: packaging/docs validators may run in parallel up to the global ceiling of 2 when they cover disjoint assertion groups, but avoid launching more than one `cargo package` command at a time because it rewrites `target/package`.
- Evidence should tie each assertion back to the exact command output, validated snippet/report location, or package-list observation that proves the published artifact is correct.
