# API coverage

This note tracks the public surface that is already shipped in the crate.

## Primary modern surface

- `src/resources/responses.rs` — create, parse, stream, persistent websocket connect with parse/callback dispatch helpers, retrieve, cancel, compact, input helpers — `examples/responses_quickstart.rs`, `examples/responses_streaming.rs`, `examples/structured_outputs.rs`, `tests/responses_websocket_contract.rs`
- `src/resources/conversations.rs` — conversation CRUD and item helpers — `examples/live_conversations_crud_smoke.rs`, `examples/live_conversations_items_smoke.rs`

## Compatibility surface

- `src/resources/chat.rs` — chat-completions compatibility and stored chat helpers — `examples/chat_completions_migration.rs`
- `src/resources/completions.rs` — legacy completions compatibility — `docs/migration-guide.md`

## Retrieval and metadata surface

- `src/resources/embeddings.rs` — embeddings — `examples/embeddings.rs`
- `src/resources/models.rs` and `src/resources/moderations.rs`
- `src/core/response.rs` and `src/core/metadata.rs` — raw response metadata and request-id access — `examples/request_metadata.rs`

## Files and downstream workflows

- `src/resources/files.rs` — file CRUD, content/retrieve-content downloads, and processing wait helpers
- `src/resources/uploads.rs` — upload lifecycle, nested parts helper, and chunked upload helper
- `src/resources/vector_stores.rs`
- `examples/upload_to_vector_store.rs`

## Media, advanced platform, and realtime

- `src/resources/images.rs`
- `src/resources/audio.rs`
- `src/resources/admin.rs` — admin organization operations for audit logs, admin API keys, usage/costs, invites, users, groups, roles, data retention, spend alerts, certificates, projects, and nested project permissions — `tests/admin_contract.rs`
- `src/resources/beta.rs` — beta ChatKit sessions and threads, deprecated beta Assistants/Threads/Runs/Run Steps compatibility with upstream-shaped stream and poll helpers, and beta realtime session/transcription-session token issuance, including required `OpenAI-Beta` headers — `tests/chatkit_contract.rs`, `tests/beta_assistants_contract.rs`, `tests/beta_realtime_contract.rs`
- `src/resources/fine_tuning.rs`
- `src/resources/evals.rs`
- `src/resources/containers.rs`
- `src/resources/skills.rs`
- `src/resources/videos.rs`
- `src/realtime` — Realtime client-secret/calls REST helpers, persistent websocket connect, upstream-shaped connection event helpers, and parse/callback dispatch helpers — `tests/realtime_connection_contract.rs`, `tests/realtime_audio_contract.rs`, `tests/realtime_decode_contract.rs`
