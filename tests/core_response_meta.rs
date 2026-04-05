use openai_rust::OpenAI;
use serde_json::Value;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn request_id_is_exposed_on_success_and_error() {
    let success_body = br#"{"data":[{"id":"model-1"}]}"#.to_vec();
    let success_server = mock_http::MockHttpServer::spawn_sequence(vec![
        mock_http::ScriptedResponse {
            headers: vec![
                (
                    String::from("content-type"),
                    String::from("application/json"),
                ),
                (String::from("content-length"), success_body.len().to_string()),
                (String::from("x-request-id"), String::from("req_success_123")),
                (String::from("x-trace-id"), String::from("trace-abc")),
            ],
            body: success_body,
            ..Default::default()
        },
        mock_http::ScriptedResponse {
            status_code: 429,
            reason: "Too Many Requests",
            headers: vec![
                (
                    String::from("content-type"),
                    String::from("application/json"),
                ),
                (String::from("content-length"), String::from("69")),
                (String::from("x-request-id"), String::from("req_error_456")),
            ],
            body: br#"{"error":{"message":"slow down","type":"rate_limit_error","code":"rate_limit"}}"#
                .to_vec(),
            ..Default::default()
        },
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("meta-key")
        .base_url(success_server.url())
        .max_retries(0)
        .build();

    let response = client
        .execute_json::<Value>("GET", "/models", Default::default())
        .unwrap();
    assert_eq!(response.request_id(), Some("req_success_123"));
    assert_eq!(response.status_code(), 200);
    assert_eq!(response.header("x-trace-id"), Some("trace-abc"));
    assert_eq!(response.output()["data"][0]["id"], "model-1");

    let error = client
        .execute_json::<Value>("GET", "/models", Default::default())
        .unwrap_err();
    assert_eq!(error.request_id(), Some("req_error_456"));
}

#[test]
fn raw_response_access_preserves_status_headers_and_parsed_data() {
    let body = br#"{"answer":"pong"}"#.to_vec();
    let server = mock_http::MockHttpServer::spawn_sequence(vec![mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
            (String::from("x-request-id"), String::from("req_raw_789")),
            (
                String::from("x-custom-header"),
                String::from("custom-value"),
            ),
        ],
        body,
        ..Default::default()
    }])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("meta-key")
        .base_url(server.url())
        .build();

    let response = client
        .execute_json::<TypedAnswer>("GET", "/models", Default::default())
        .unwrap();
    let (metadata, output) = response.into_parts();

    assert_eq!(metadata.request_id(), Some("req_raw_789"));
    assert_eq!(metadata.status_code(), 200);
    assert_eq!(metadata.header("x-custom-header"), Some("custom-value"));
    assert_eq!(output.answer, "pong");
}

#[test]
fn missing_request_id_header_returns_none_without_failing() {
    let body = br#"{"answer":"pong"}"#.to_vec();
    let server = mock_http::MockHttpServer::spawn_sequence(vec![mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body,
        ..Default::default()
    }])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("meta-key")
        .base_url(server.url())
        .build();

    let response = client
        .execute_json::<TypedAnswer>("GET", "/models", Default::default())
        .unwrap();

    assert_eq!(response.request_id(), None);
    assert_eq!(response.metadata().request_id(), None);
}

#[derive(Debug, serde::Deserialize)]
struct TypedAnswer {
    answer: String,
}
