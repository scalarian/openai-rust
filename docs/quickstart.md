# Quickstart

Start from the repository root.

## Local validation first

```sh
cargo test --test readme_contract
cargo check --examples --all-features
cargo test --doc
```

## Dry-run onboarding flow

These commands compile and run without credentials:

```sh
cargo run --example responses_quickstart
cargo run --example responses_streaming
cargo run --example structured_outputs
cargo run --example request_metadata
cargo run --example embeddings
cargo run --example upload_to_vector_store
cargo run --example chat_completions_migration
```

## First live request

Export `OPENAI_API_KEY`, then use the `Responses` family:

```rust
use openai_rust::OpenAI;
use openai_rust::resources::responses::ResponseCreateParams;
use serde_json::json;

let client = OpenAI::builder().build();
let params = ResponseCreateParams {
    model: "gpt-4.1-mini".into(),
    input: Some(json!("Say hello from Rust.")),
    ..Default::default()
};

// client.responses().create(params)?;
let _ = params;
```

For a zero-cost config check, call `client.prepare_request("GET", "/models")?` before issuing a live request.
