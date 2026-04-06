# Responses guide

`Responses` is the primary generation surface in this crate.

## Basic request

```rust
use openai_rust::resources::responses::ResponseCreateParams;
use serde_json::json;

let params = ResponseCreateParams {
    model: "gpt-4.1-mini".into(),
    input: Some(json!("Summarize the Rust ownership rules.")),
    ..Default::default()
};

let _ = params;
```

## Streaming

Run the executable walkthrough:

```sh
cargo run --example responses_streaming
```

The example uses `ResponseStream::from_sse_chunks(...)` so the streaming API can be exercised without network setup.

## Structured outputs

Run the structured-output example:

```sh
cargo run --example structured_outputs
```

The example builds `ResponseParseParams`, `ResponseTextConfig`, `ResponseFormatTextConfig`, and `FunctionTool` with the public exports shipped by `src/resources/responses.rs`.

## Response metadata

Run the metadata walkthrough:

```sh
cargo run --example request_metadata
```

That example demonstrates the stable `ApiResponse<T>` and `ResponseMetadata` accessors used to inspect request IDs and headers after a typed response returns.
