# API coverage

This note tracks the public surface that is already shipped in the crate.

## Primary modern surface

- `src/resources/responses.rs` — create, parse, stream, retrieve, cancel, compact, input helpers — `examples/responses_quickstart.rs`, `examples/responses_streaming.rs`, `examples/structured_outputs.rs`
- `src/resources/conversations.rs` — conversation CRUD and item helpers — `examples/live_conversations_crud_smoke.rs`, `examples/live_conversations_items_smoke.rs`

## Compatibility surface

- `src/resources/chat.rs` — chat-completions compatibility and stored chat helpers — `examples/chat_completions_migration.rs`
- `src/resources/completions.rs` — legacy completions compatibility — `docs/migration-guide.md`

## Retrieval and metadata surface

- `src/resources/embeddings.rs` — embeddings — `examples/embeddings.rs`
- `src/resources/models.rs` and `src/resources/moderations.rs`
- `src/core/response.rs` and `src/core/metadata.rs` — raw response metadata and request-id access — `examples/request_metadata.rs`

## Files and downstream workflows

- `src/resources/files.rs`
- `src/resources/uploads.rs`
- `src/resources/vector_stores.rs`
- `examples/upload_to_vector_store.rs`

## Media, advanced platform, and realtime

- `src/resources/images.rs`
- `src/resources/audio.rs`
- `src/resources/fine_tuning.rs`
- `src/resources/evals.rs`
- `src/resources/containers.rs`
- `src/resources/skills.rs`
- `src/resources/videos.rs`
- `src/realtime`
