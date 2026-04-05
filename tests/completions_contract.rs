use openai_rust::{ErrorKind, OpenAI};
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn legacy_completions_create_preserves_text_completion_shape() {
    let server = mock_http::MockHttpServer::spawn(json_response(completion_payload(
        "cmpl_legacy",
        "Hello from legacy completions",
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .completions()
        .create(
            openai_rust::resources::completions::CompletionCreateParams {
                model: String::from("gpt-3.5-turbo-instruct"),
                prompt: Some(json!("Say hello")),
                max_tokens: Some(8),
                stream: Some(false),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(response.output().id, "cmpl_legacy");
    assert_eq!(response.output().object, "text_completion");
    assert_eq!(response.output().choices.len(), 1);
    assert_eq!(
        response.output().choices[0].text,
        "Hello from legacy completions"
    );
    assert_eq!(
        response.output().choices[0].finish_reason.as_deref(),
        Some("stop")
    );
    assert_eq!(response.output().choices[0].index, 0);
    assert_eq!(
        response.output().usage.as_ref().unwrap()["total_tokens"],
        10
    );

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/completions");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["model"], "gpt-3.5-turbo-instruct");
    assert_eq!(body["prompt"], "Say hello");
    assert_eq!(body["max_tokens"], 8);
    assert_eq!(body["stream"], false);
}

#[test]
fn streaming_best_of_is_rejected_locally() {
    let client = OpenAI::builder().api_key("test-key").build();

    let error = client
        .completions()
        .create(
            openai_rust::resources::completions::CompletionCreateParams {
                model: String::from("gpt-3.5-turbo-instruct"),
                prompt: Some(json!("Say hello")),
                best_of: Some(2),
                stream: Some(true),
                ..Default::default()
            },
        )
        .expect_err("best_of with stream should be rejected locally");

    assert_eq!(error.kind, ErrorKind::Validation);
    assert!(error.message.contains("best_of"));
    assert!(error.message.contains("stream"));
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

fn completion_payload(id: &str, text: &str) -> String {
    json!({
        "id": id,
        "object": "text_completion",
        "created": 1,
        "model": "gpt-3.5-turbo-instruct",
        "choices": [
            {
                "text": text,
                "index": 0,
                "logprobs": null,
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 4,
            "completion_tokens": 6,
            "total_tokens": 10
        }
    })
    .to_string()
}
