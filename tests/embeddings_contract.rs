use openai_rust::OpenAI;
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn embeddings_default_transport_decodes_base64_but_preserves_explicit_formats() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(default_base64_embedding_payload()),
        json_response(float_embedding_payload()),
        json_response(explicit_base64_embedding_payload()),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let default_response = client
        .embeddings()
        .create(openai_rust::resources::embeddings::EmbeddingCreateParams {
            model: String::from("text-embedding-3-small"),
            input: json!(["first", "second"]),
            dimensions: Some(3),
            user: Some(String::from("user-123")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(default_response.output().data.len(), 2);
    assert_eq!(default_response.output().data[0].index, 0);
    assert_eq!(default_response.output().data[1].index, 1);
    assert_eq!(
        default_response.output().data[0].embedding.as_float_slice(),
        Some(&[1.0_f32, 2.0, 3.5][..])
    );
    assert_eq!(
        default_response.output().data[1].embedding.as_float_slice(),
        Some(&[-4.0_f32, 0.25, 8.0][..])
    );

    let float_response = client
        .embeddings()
        .create(openai_rust::resources::embeddings::EmbeddingCreateParams {
            model: String::from("text-embedding-3-small"),
            input: json!("just one"),
            encoding_format: Some(
                openai_rust::resources::embeddings::EmbeddingEncodingFormat::Float,
            ),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        float_response.output().data[0].embedding.as_float_slice(),
        Some(&[0.5_f32, 1.5, -2.0][..])
    );

    let base64_response = client
        .embeddings()
        .create(openai_rust::resources::embeddings::EmbeddingCreateParams {
            model: String::from("text-embedding-3-small"),
            input: json!("raw base64 please"),
            encoding_format: Some(
                openai_rust::resources::embeddings::EmbeddingEncodingFormat::Base64,
            ),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        base64_response.output().data[0].embedding.as_base64(),
        Some("AACAPwAAAEAAAGBA")
    );

    let requests = server.captured_requests(3).expect("captured requests");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/embeddings");
    let first_body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(first_body["model"], "text-embedding-3-small");
    assert_eq!(first_body["input"], json!(["first", "second"]));
    assert_eq!(first_body["dimensions"], 3);
    assert_eq!(first_body["user"], "user-123");
    assert_eq!(first_body["encoding_format"], "base64");

    let second_body: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(second_body["encoding_format"], "float");

    let third_body: Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(third_body["encoding_format"], "base64");
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn default_base64_embedding_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "object": "embedding",
                "index": 0,
                "embedding": "AACAPwAAAEAAAGBA"
            },
            {
                "object": "embedding",
                "index": 1,
                "embedding": "AACAwAAAgD4AAABB"
            }
        ],
        "model": "text-embedding-3-small",
        "usage": {"prompt_tokens": 6, "total_tokens": 6}
    })
    .to_string()
}

fn float_embedding_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "object": "embedding",
                "index": 0,
                "embedding": [0.5, 1.5, -2.0]
            }
        ],
        "model": "text-embedding-3-small",
        "usage": {"prompt_tokens": 3, "total_tokens": 3}
    })
    .to_string()
}

fn explicit_base64_embedding_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "object": "embedding",
                "index": 0,
                "embedding": "AACAPwAAAEAAAGBA"
            }
        ],
        "model": "text-embedding-3-small",
        "usage": {"prompt_tokens": 4, "total_tokens": 4}
    })
    .to_string()
}
