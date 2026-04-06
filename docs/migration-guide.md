# Migration guide

Responses is the primary surface for new code.

## Surface mapping

| Compatibility workflow | Preferred Responses workflow |
| --- | --- |
| `client.chat().completions().create(...)` | `client.responses().create(...)` |
| `client.chat().completions().stream(...)` | `client.responses().stream(...)` |
| `client.completions().create(...)` | `client.responses().create(...)` with text input |
| Stored compatibility retrieval | Stored `Responses` retrieval and conversation helpers |

## Chat Completions to Responses

```rust
use openai_rust::resources::chat::ChatCompletionCreateParams;
use openai_rust::resources::responses::ResponseCreateParams;
use serde_json::json;

let compatibility = ChatCompletionCreateParams {
    model: "gpt-4.1-mini".into(),
    messages: vec![json!({"role":"user","content":"Say hello"})],
    ..Default::default()
};

let preferred = ResponseCreateParams {
    model: "gpt-4.1-mini".into(),
    input: Some(json!("Say hello")),
    ..Default::default()
};

let _ = (compatibility, preferred);
```

`client.chat().completions()` remains available for compatibility-only flows such as stored chat completion CRUD and stored message listing, but new work should prefer `client.responses()`.

## Legacy Completions to Responses

```rust
use openai_rust::resources::completions::CompletionCreateParams;
use openai_rust::resources::responses::ResponseCreateParams;
use serde_json::json;

let legacy = CompletionCreateParams {
    model: "gpt-3.5-turbo-instruct".into(),
    prompt: Some(json!("Say hello")),
    ..Default::default()
};

let preferred = ResponseCreateParams {
    model: "gpt-4.1-mini".into(),
    input: Some(json!("Say hello")),
    ..Default::default()
};

let _ = (legacy, preferred);
```

`client.completions()` remains available for compatibility-only `/v1/completions` workflows. Keep it as a secondary surface while using `Responses` for new structured output, streaming, and tool-driven flows.

## Runnable migration example

```sh
cargo run --example chat_completions_migration
```
