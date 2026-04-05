# Architecture

This SDK is a single Rust crate with one shared core for configuration, transport, parsing, and error handling, plus public resource families layered on top. The design is async-first, Responses-first, and clean-room: workers extend the SDK by adding typed resources and models on top of shared runtime contracts rather than creating endpoint-specific transport stacks.

## What belongs here
- Stable boundaries between the public API, shared runtime, transport, parsing, and endpoint families.
- The request/response flow from typed Rust parameters to wire format and back to typed results.
- Cross-cutting concerns that every worker must reuse: auth, retries, timeouts, request IDs, pagination, polling, multipart, streaming, and webhook verification.
- The distinction between first-class surfaces, compatibility surfaces, and feature-gated subsystems.
- Rules for extending the crate without changing the architectural shape or re-implementing shared behavior.

## Worker Mental Model
- Think in four layers: public client/resource handles, family-owned endpoint types and helpers, shared REST/runtime infrastructure, and protocol-specific streaming or realtime machinery.
- Most feature work should land in one resource family plus its tests. Shared core should change only when the need is truly cross-family or transport-level.
- Family modules own endpoint semantics: paths, query/body shapes, operation-specific headers, typed request/response models, and family-scoped helpers.
- Shared core owns mechanics: auth, base URL joining, request execution, retry/timeout policy, metadata capture, content-type decoding, and common helper primitives.
- Shared model types should exist only when multiple families genuinely speak the same concept; otherwise keep models local to the owning family to avoid an accidental god-module.
- A worker should be able to add or extend an API without inventing a new stack: define the typed contract in the family, describe the operation in the shared runtime's terms, and reuse an existing execution shape.

## Logical Module Map
- **Root/client layer:** crate entry points, configuration builder, feature flags, and the `OpenAI` handle that exposes resource families.
- **Shared core layer:** config/auth loading, transport, retry/timeout policy, error taxonomy, response metadata, and generic response decoding.
- **Shared protocol/helper layer:** pagination, polling, multipart/upload composition, SSE framing and accumulation, structured-output parsing, webhook verification, and blocking adapters.
- **Resource-family layer:** one cohesive module per API family, each owning its public request/response types plus family-specific convenience helpers.
- **Realtime layer:** websocket session and event handling kept separate from REST transport while still sharing the same public error and event-modeling principles.
- **Test layer:** core contract tests, family-specific mocked integration suites, streaming transcript suites, and opt-in live smoke coverage.

## Public API Shape
- The root entry point is an immutable configuration plus an `OpenAI` client handle that owns shared runtime state.
- Public resources are grouped by platform family, not by transport detail. Each family hangs off the root client and exposes cohesive operations for that API area.
- Every operation is represented by typed Rust request and response types. Ergonomic helpers may sit on top, but the wire-visible contract must always remain expressible through stable public types.
- Success paths return typed data first. Raw status, headers, and request IDs are available through explicit wrappers or metadata accessors rather than mixed into every model.
- Compatibility APIs such as Chat Completions and legacy Completions stay available, but they remain visibly secondary to the modern Responses surface.
- Pagination, polling, streaming, and binary-download behavior are modeled as distinct result forms instead of ad hoc booleans inside generic response objects.

## Runtime Architecture
- The configuration layer loads and validates API keys, base URL overrides, org/project routing, retry policy, timeout policy, user agent, and feature-gated execution options.
- The client core owns immutable shared state and vends resource-family handles; resource handles do not own independent HTTP stacks.
- Each resource operation produces a canonical request description: method, path, query, body mode, expected response mode, and any operation-specific execution hints such as extra headers, polling semantics, or response parsing strategy.
- One shared transport layer executes those request descriptions, applying auth, common headers, timeout and retry behavior, multipart encoding, raw-response capture, and request-id extraction.
- One shared parsing layer decodes the response mode into typed JSON, pages, bytes, text, SSE event streams, websocket events, or empty success results, and maps failures into a single error taxonomy.
- Helper layers sit above transport and parsing for pagination, polling, structured-output parsing, output-text aggregation, webhook verification, upload composition, and blocking adapters.
- The core data flow is: caller -> typed params -> request description -> shared transport -> shared parser -> typed result or typed error -> optional helper adaptation.

## Resource Families
- **Modern primary surface:** Responses, Conversations, input-items or input-token helpers, structured outputs, tool calling, and streamed response helpers.
- **Compatibility surface:** Chat Completions, stored chat completions and message listing, and legacy Completions.
- **Core retrieval surface:** Models, Embeddings, and Moderations.
- **Media surface:** Images plus audio transcription, translation, and speech, alongside shared multimodal input types used by primary and compatibility APIs.
- **File and retrieval workflows:** Files, Uploads, Vector Stores, Vector Store Files and File Batches, Batches, and Webhooks.
- **Advanced platform surface:** Fine-tuning, Evals with runs and output items, Containers with container files, Videos, and any remaining public OpenAI-only families confirmed during artifacting.
- **Realtime surface:** Realtime client secrets, Realtime calls, and the GA websocket session and event model.
- Each family owns family-specific request and response types plus any family-scoped helpers, but all families share the same configuration, transport, parsing, metadata, and error contracts.
- Out of scope for this architecture: Azure-specific branches and deprecated Assistants/Threads/Runs-style surfaces.

## Transport and Parsing Boundaries
- Resources own endpoint semantics; they do not own HTTP clients, retry loops, header policy, or content-type decoding.
- Transport owns authentication, base-URL joining, common headers, timeout and retry behavior, idempotent replay rules, multipart assembly, and raw metadata extraction such as `x-request-id`.
- Operation-specific headers or wire toggles required by a family, such as beta contracts or alternate `Accept` behavior, belong in the resource-owned request description and flow through transport as data rather than as ad hoc family-specific HTTP code.
- Parsing owns response-shape discrimination across JSON objects, paginated lists, text bodies, binary bodies, empty success bodies, SSE frames, and realtime websocket events.
- Local validation stops at deterministic SDK-owned invariants such as missing credentials, blank path identifiers, invalid base URL or API key forms, explicitly documented mutually exclusive options, and feature misuse.
- Platform-owned validation stays server-owned. The SDK forwards documented fields faithfully and surfaces typed API errors instead of re-implementing platform business rules locally.
- Unknown additive fields must never turn a valid response into a client failure. Forward-compatible data is ignored safely or preserved through raw or unknown variants.
- Response metadata is orthogonal to model payloads: typed models describe API data, while headers, status, and request IDs live in explicit metadata surfaces.

## Streaming Model
- REST streaming and Realtime are separate runtime paths with shared design rules: typed events first, derived helpers second.
- REST streamed endpoints use SSE; Realtime uses websocket event streams. They do not share connection code, but they do share error taxonomy, event-first modeling, and terminal-state rules.
- Stream state is assembled incrementally from server events keyed by stable ids or indices. Final snapshots, `output_text`, structured outputs, and parsed tool arguments are derived views over that event stream.
- A stream is successful only when the documented terminal event or terminal response state is observed. `[DONE]`, EOF, or socket close alone never imply a completed success value.
- Structured outputs and parsed tool arguments are validated only at a completion boundary, never from partial JSON fragments.
- Unknown or future events must remain lossless through raw or unknown variants, or be safely skipped without corrupting accumulation of known state.

## Blocking Model
- Async REST execution is the source of truth for operation definitions, models, helpers, and error behavior.
- The `blocking` feature adds a synchronous facade over the same request descriptions, transport policies, parsers, and helper logic; it must not duplicate endpoint business logic or fork model definitions.
- Blocking pagination, polling, raw-response access, uploads, and streaming expose the same user-facing contracts as async, translated only into blocking control flow.
- Feature gating happens at subsystem boundaries: blocking execution lives behind `blocking`, while shared request and response types stay available to the base crate.
- Realtime remains its own boundary. If a capability cannot be supported meaningfully in blocking form, it stays explicitly async-only rather than growing a divergent partial clone.

## Testing and Validation Architecture
- Unit tests cover configuration loading, local validation, URL and header formation, retry decisions, parser behavior, metadata extraction, and helper logic.
- Mocked integration tests cover request shaping, pagination, multipart bodies, polling behavior, binary and text decoding, streaming transcripts, and error mapping for each resource family.
- Live tests are opt-in, budget-aware smoke checks that prove the shared runtime works against the real API without attempting exhaustive coverage of server behavior.
- Streaming has dedicated transcript-driven tests that verify fragmentation, ordering, terminal states, accumulation rules, and forward-compatible unknown-event handling independently of any one endpoint family.
- Cross-area tests prove shared contracts such as env-based configuration, request-id exposure, upload-to-downstream flows, async and blocking parity, and REST-to-Realtime bootstrap.
- Documentation and packaging validation are part of the architecture because public docs, examples, and publish artifacts must reflect the actual crate surface.
- Workers should extend validation along the same seam as the feature: family behavior in family tests, shared transport/parser behavior in core tests, and live smoke only when the family needs real API proof that mocks cannot provide.

## Invariants
- Clean-room rule: reference SDKs are surface-inventory inputs only; no copied code, generated layouts, or structural mirroring.
- One shared transport, parsing, metadata, and error core serves every REST family.
- Public resource families depend inward on shared core modules; shared core never depends on endpoint families.
- Responses is the primary modern generation surface; compatibility APIs do not dictate the overall architecture.
- Typed success values, typed API errors, and raw metadata access remain consistent across families.
- Unknown additive fields and event variants must not break existing callers.
- Streaming accumulation is deterministic: the same event transcript always yields the same final snapshot.
- Async and blocking clients are contract-equivalent for supported REST flows.
- Feature flags may add execution modes or optional subsystems, but they must not change the wire semantics of an already-supported API surface.
- Local client validation covers only deterministic SDK-owned invariants; platform policy, entitlement, and business-rule failures stay server-sourced.

## Extension Rules
- Add new APIs by fitting them into an existing resource family or creating a new family with the same layer boundaries: public resource handle -> typed operation description -> shared transport and parser -> typed helper surfaces.
- Choose the execution shape first: unary JSON, paginated list, multipart upload, pollable job, SSE stream, binary or text response, or realtime event stream. Reuse the matching shared helper path instead of inventing a family-specific mechanism.
- Promote behavior into shared core only when it is transport-level or is required by multiple families; otherwise keep the semantics inside the owning family.
- Prefer additive modeling: new fields, event types, or helpers should extend public types without invalidating existing construction or parsing rules.
- When an API has both modern and compatibility forms, keep one canonical runtime path where possible and enforce the compatibility labeling at the public boundary.
- Every extension must define its validation boundary up front: which failures are local preflight errors, which are transport or parser errors, and which must surface as typed API contract errors from the server.
- The canonical worker path is: place or extend the feature in its owning family, pick the existing execution shape, reuse shared helpers for cross-cutting behavior, add validation at the same seam, and only then consider whether a new shared abstraction is warranted.
