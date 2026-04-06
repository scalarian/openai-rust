# openai-rust

Clean-room Rust SDK for the OpenAI API. The crate is async-first, keeps `Responses` as the primary generation surface, and still ships compatibility helpers for Chat Completions and legacy Completions.

## Quickstart

### Validate the checkout

```sh
cargo test --test readme_contract
cargo check --examples --all-features
cargo test --doc
```

### Dry-run the local onboarding flow

These examples do not hit the network. They prove the public API shape from a clean checkout:

```sh
cargo run --example responses_quickstart
cargo run --example responses_streaming
cargo run --example structured_outputs
```

### First API call

Set `OPENAI_API_KEY` in your shell before running a live request:

```rust,no_run
use openai_rust::OpenAI;
use openai_rust::resources::responses::ResponseCreateParams;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAI::builder().build();

    let response = client.responses().create(ResponseCreateParams {
        model: "gpt-4.1-mini".into(),
        input: Some(json!("Say hello from Rust.")),
        ..Default::default()
    })?;

    println!("{}", response.output.output_text());
    Ok(())
}
```

If you want a zero-cost sanity check before making a live call, use `client.prepare_request("GET", "/models")?` to confirm env-based configuration resolves correctly.

## Supported surfaces

The published surface is backed by concrete modules and examples in this repository:

- Responses and structured outputs — `src/resources/responses.rs`, `examples/responses_quickstart.rs`, `examples/structured_outputs.rs`
- Conversations — `src/resources/conversations.rs`, `examples/live_conversations_crud_smoke.rs`
- Chat Completions compatibility — `src/resources/chat.rs`, `examples/chat_completions_migration.rs`
- Legacy Completions compatibility — `src/resources/completions.rs`, `docs/migration-guide.md`
- Embeddings, Models, Moderations — `src/resources/embeddings.rs`, `src/resources/models.rs`, `src/resources/moderations.rs`, `examples/embeddings.rs`
- Images and Audio — `src/resources/images.rs`, `src/resources/audio.rs`, `docs/api-coverage.md`
- Files, Uploads, and Vector Stores — `src/resources/files.rs`, `src/resources/uploads.rs`, `src/resources/vector_stores.rs`, `examples/upload_to_vector_store.rs`
- Realtime GA — `src/realtime`, `docs/architecture-note.md`
- Blocking facade — `src/blocking/mod.rs`

## Migration from compatibility surfaces

`Responses` is the preferred surface for new code. Compatibility namespaces remain available, but they are intentionally secondary:

- `client.chat().completions()` stays available for chat-completions compatibility and stored chat-completion helpers.
- `client.completions()` stays available for legacy `/v1/completions` workflows.
- New structured output, modern streaming, and tool-heavy flows should start from `client.responses()`.

See `docs/migration-guide.md` for a side-by-side migration walkthrough.

## Examples

Representative examples that compile with the shipped crate API:

- `examples/responses_quickstart.rs`
- `examples/responses_streaming.rs`
- `examples/structured_outputs.rs`
- `examples/request_metadata.rs`
- `examples/embeddings.rs`
- `examples/upload_to_vector_store.rs`
- `examples/chat_completions_migration.rs`

Run them with `cargo run --example <name>`.

## Developer validation

The docs and examples are validated with the same commands used by this feature:

```sh
cargo test --test readme_contract
cargo test --test docs_contract
cargo check --examples --all-features
cargo test --doc
```
