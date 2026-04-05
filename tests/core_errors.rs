use std::time::Duration;

use openai_rust::{ApiErrorKind, ErrorKind, OpenAI};
use serde_json::Value;

#[path = "support/mock_http.rs"]
mod mock_http;

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct ExpectedShape {
    answer: String,
}

#[test]
fn api_status_errors_preserve_taxonomy_and_server_payload() {
    let cases = [
        (400, "Bad Request", ApiErrorKind::BadRequest),
        (401, "Unauthorized", ApiErrorKind::Authentication),
        (403, "Forbidden", ApiErrorKind::PermissionDenied),
        (404, "Not Found", ApiErrorKind::NotFound),
        (409, "Conflict", ApiErrorKind::Conflict),
        (
            422,
            "Unprocessable Entity",
            ApiErrorKind::UnprocessableEntity,
        ),
        (429, "Too Many Requests", ApiErrorKind::RateLimit),
        (500, "Internal Server Error", ApiErrorKind::Server),
    ];

    for (status, reason, kind) in cases {
        let server =
            mock_http::MockHttpServer::spawn_sequence(vec![error_response(status, reason)])
                .unwrap();
        let client = OpenAI::builder()
            .api_key("error-key")
            .base_url(server.url())
            .max_retries(0)
            .build();

        let error = client
            .execute_json::<Value>("GET", "/models", Default::default())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Api(kind));
        assert_eq!(error.status_code(), Some(status));
        assert_eq!(error.request_id(), Some("req_test_123"));
        assert_eq!(error.header("x-extra-header"), Some("extra-value"),);
        let payload = error.api_error().unwrap();
        assert_eq!(payload.message, "status specific failure");
        assert_eq!(payload.code.as_deref(), Some("status_error"));
        assert_eq!(payload.param.as_deref(), Some("model"));
        assert_eq!(server.captured_requests(1).unwrap()[0].path, "/v1/models");
    }
}

#[test]
fn transport_failures_stay_distinct_from_api_status_failures() {
    let client = OpenAI::builder()
        .api_key("error-key")
        .base_url("http://127.0.0.1:1")
        .max_retries(0)
        .timeout(Duration::from_millis(50))
        .build();

    let error = client
        .execute_json::<Value>("GET", "/models", Default::default())
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Transport);
    assert!(error.source().is_some());
    assert_eq!(error.status_code(), None);
    assert!(error.api_error().is_none());
}

#[test]
fn malformed_success_responses_raise_parse_errors() {
    let malformed_server =
        mock_http::MockHttpServer::spawn_sequence(vec![mock_http::ScriptedResponse {
            headers: vec![
                (
                    String::from("content-type"),
                    String::from("application/json"),
                ),
                (
                    String::from("content-length"),
                    br#"{"ok":invalid}"#.len().to_string(),
                ),
            ],
            body: br#"{"ok":invalid}"#.to_vec(),
            ..Default::default()
        }])
        .unwrap();
    let client = OpenAI::builder()
        .api_key("error-key")
        .base_url(malformed_server.url())
        .build();
    let error = client
        .execute_json::<Value>("GET", "/models", Default::default())
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Parse);
    assert_eq!(error.status_code(), Some(200));
    assert_eq!(
        malformed_server.captured_requests(1).unwrap()[0].path,
        "/v1/models"
    );

    let wrong_shape_server =
        mock_http::MockHttpServer::spawn_sequence(vec![mock_http::ScriptedResponse {
            headers: vec![
                (
                    String::from("content-type"),
                    String::from("application/json"),
                ),
                (String::from("content-length"), String::from("11")),
            ],
            body: br#"{"ok":true}"#.to_vec(),
            ..Default::default()
        }])
        .unwrap();
    let client = OpenAI::builder()
        .api_key("error-key")
        .base_url(wrong_shape_server.url())
        .build();
    let error = client
        .execute_json::<ExpectedShape>("GET", "/models", Default::default())
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Parse);
    assert_eq!(error.status_code(), Some(200));
    assert_eq!(
        wrong_shape_server.captured_requests(1).unwrap()[0].path,
        "/v1/models"
    );
}

#[test]
fn validation_failures_are_distinct() {
    let client = OpenAI::builder().api_key("error-key").build();

    let method_error = client
        .execute_json::<Value>("   ", "/models", Default::default())
        .unwrap_err();
    assert_eq!(method_error.kind, ErrorKind::Validation);

    let path_error = client
        .execute_json::<Value>("GET", "   ", Default::default())
        .unwrap_err();
    assert_eq!(path_error.kind, ErrorKind::Validation);
}

fn error_response(status_code: u16, reason: &'static str) -> mock_http::ScriptedResponse {
    let body = br#"{"error":{"message":"status specific failure","type":"request_error","code":"status_error","param":"model"}}"#
        .to_vec();
    mock_http::ScriptedResponse {
        status_code,
        reason,
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
            (String::from("x-request-id"), String::from("req_test_123")),
            (String::from("x-extra-header"), String::from("extra-value")),
        ],
        body,
        ..Default::default()
    }
}
