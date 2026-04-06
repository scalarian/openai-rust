# Architecture note

The public crate remains async-first with one shared runtime for config, auth, request preparation, retries, response parsing, metadata capture, and error mapping.

## Public entry points

- `openai_rust::OpenAI` is the root async client.
- `openai_rust::blocking::OpenAI` is the feature-gated blocking facade.
- `openai_rust::ApiResponse<T>` and `openai_rust::ResponseMetadata` expose typed output plus request metadata.

## Primary and secondary surfaces

- Primary: `client.responses()` and `client.conversations()`
- Secondary compatibility: `client.chat().completions()` and `client.completions()`
- Retrieval/media/files/realtime: `client.embeddings()`, `client.models()`, `client.images()`, `client.audio()`, `client.files()`, `client.uploads()`, `client.vector_stores()`, `client.realtime()`

## Example coverage

Representative examples for the published architecture live under `examples/`:

- `examples/responses_quickstart.rs`
- `examples/responses_streaming.rs`
- `examples/structured_outputs.rs`
- `examples/request_metadata.rs`
- `examples/upload_to_vector_store.rs`
- `examples/chat_completions_migration.rs`
