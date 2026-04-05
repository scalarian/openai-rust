use std::time::Duration;

use openai_rust::{ErrorKind, OpenAI};
use serde::Deserialize;

#[path = "support/mock_http.rs"]
mod mock_http;

#[derive(Debug, Deserialize)]
struct OkResponse {
    ok: bool,
}

#[test]
fn default_retry_policy_covers_retryable_failures() {
    let status_cases = [
        (408, "Request Timeout"),
        (409, "Conflict"),
        (429, "Too Many Requests"),
    ];

    for (status, reason) in status_cases {
        let server = mock_http::MockHttpServer::spawn_sequence(vec![
            error_response(status, reason),
            error_response(status, reason),
            json_ok(),
        ])
        .unwrap();

        let client = OpenAI::builder()
            .api_key("retry-key")
            .base_url(server.url())
            .build();

        let response = client
            .execute_json::<OkResponse>("GET", "/models", Default::default())
            .unwrap();
        assert!(response.output.ok);
        assert_eq!(server.captured_requests(3).unwrap().len(), 3);
    }

    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        error_response(500, "Internal Server Error"),
        error_response(500, "Internal Server Error"),
        json_ok(),
    ])
    .unwrap();
    let client = OpenAI::builder()
        .api_key("retry-key")
        .base_url(server.url())
        .build();
    let response = client
        .execute_json::<OkResponse>("GET", "/models", Default::default())
        .unwrap();
    assert!(response.output.ok);
    assert_eq!(server.captured_requests(3).unwrap().len(), 3);
}

#[test]
fn retry_after_guidance_is_honored() {
    let retry_after_server = mock_http::MockHttpServer::spawn_sequence(vec![
        error_response_with_headers(
            429,
            "Too Many Requests",
            vec![(String::from("retry-after"), String::from("1"))],
        ),
        json_ok(),
    ])
    .unwrap();
    let client = OpenAI::builder()
        .api_key("retry-key")
        .base_url(retry_after_server.url())
        .build();
    let response = client
        .execute_json::<OkResponse>("GET", "/models", Default::default())
        .unwrap();
    assert!(response.output.ok);
    let retry_after_requests = retry_after_server.captured_requests(2).unwrap();
    let retry_after_gap =
        retry_after_requests[1].received_after - retry_after_requests[0].received_after;
    assert!(retry_after_gap >= Duration::from_millis(900));

    let retry_after_ms_server = mock_http::MockHttpServer::spawn_sequence(vec![
        error_response_with_headers(
            429,
            "Too Many Requests",
            vec![(String::from("retry-after-ms"), String::from("150"))],
        ),
        json_ok(),
    ])
    .unwrap();
    let client = OpenAI::builder()
        .api_key("retry-key")
        .base_url(retry_after_ms_server.url())
        .build();
    let response = client
        .execute_json::<OkResponse>("GET", "/models", Default::default())
        .unwrap();
    assert!(response.output.ok);
    let retry_after_ms_requests = retry_after_ms_server.captured_requests(2).unwrap();
    let retry_after_ms_gap =
        retry_after_ms_requests[1].received_after - retry_after_ms_requests[0].received_after;
    assert!(retry_after_ms_gap >= Duration::from_millis(130));
    assert!(retry_after_ms_gap < Duration::from_millis(500));
}

#[test]
fn disabling_retries_surfaces_the_first_failure_immediately() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        error_response(429, "Too Many Requests"),
        json_ok(),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("retry-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let error = client
        .execute_json::<OkResponse>("GET", "/models", Default::default())
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::Api(openai_rust::ApiErrorKind::RateLimit)
    );
    assert_eq!(server.captured_request().unwrap().path, "/v1/models");
}

#[test]
fn non_retryable_client_errors_do_not_retry() {
    let cases = [
        (400, "Bad Request", openai_rust::ApiErrorKind::BadRequest),
        (
            401,
            "Unauthorized",
            openai_rust::ApiErrorKind::Authentication,
        ),
        (
            403,
            "Forbidden",
            openai_rust::ApiErrorKind::PermissionDenied,
        ),
        (404, "Not Found", openai_rust::ApiErrorKind::NotFound),
        (
            422,
            "Unprocessable Entity",
            openai_rust::ApiErrorKind::UnprocessableEntity,
        ),
    ];

    for (status, reason, kind) in cases {
        let server = mock_http::MockHttpServer::spawn_sequence(vec![
            error_response(status, reason),
            json_ok(),
        ])
        .unwrap();
        let client = OpenAI::builder()
            .api_key("retry-key")
            .base_url(server.url())
            .build();

        let error = client
            .execute_json::<OkResponse>("GET", "/models", Default::default())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Api(kind));
        assert_eq!(server.captured_request().unwrap().path, "/v1/models");
    }
}

#[test]
fn server_retry_directives_override_generic_backoff() {
    let do_not_retry_server = mock_http::MockHttpServer::spawn_sequence(vec![
        error_response_with_headers(
            500,
            "Internal Server Error",
            vec![(String::from("x-should-retry"), String::from("false"))],
        ),
        json_ok(),
    ])
    .unwrap();
    let client = OpenAI::builder()
        .api_key("retry-key")
        .base_url(do_not_retry_server.url())
        .build();
    let error = client
        .execute_json::<OkResponse>("GET", "/models", Default::default())
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::Api(openai_rust::ApiErrorKind::Server)
    );
    assert_eq!(
        do_not_retry_server.captured_request().unwrap().path,
        "/v1/models"
    );

    let do_retry_server = mock_http::MockHttpServer::spawn_sequence(vec![
        error_response_with_headers(
            400,
            "Bad Request",
            vec![(String::from("x-should-retry"), String::from("true"))],
        ),
        json_ok(),
    ])
    .unwrap();
    let client = OpenAI::builder()
        .api_key("retry-key")
        .base_url(do_retry_server.url())
        .build();
    let response = client
        .execute_json::<OkResponse>("GET", "/models", Default::default())
        .unwrap();
    assert!(response.output.ok);
    assert_eq!(do_retry_server.captured_requests(2).unwrap().len(), 2);
}

#[test]
fn loopback_requests_preserve_query_strings() {
    let server = mock_http::MockHttpServer::spawn(json_ok()).unwrap();
    let client = OpenAI::builder()
        .api_key("retry-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .execute_json::<OkResponse>(
            "GET",
            "/models?limit=2&after=cursor&filter=name%3Ddemo",
            Default::default(),
        )
        .unwrap();

    assert!(response.output.ok);
    let captured = server.captured_request().unwrap();
    println!("captured loopback path: {}", captured.path);
    assert_eq!(
        captured.path,
        "/v1/models?limit=2&after=cursor&filter=name%3Ddemo"
    );
}

fn json_ok() -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), String::from("11")),
        ],
        body: br#"{"ok":true}"#.to_vec(),
        ..Default::default()
    }
}

fn error_response(status_code: u16, reason: &'static str) -> mock_http::ScriptedResponse {
    error_response_with_headers(status_code, reason, Vec::new())
}

fn error_response_with_headers(
    status_code: u16,
    reason: &'static str,
    extra_headers: Vec<(String, String)>,
) -> mock_http::ScriptedResponse {
    let body =
        r#"{"error":{"message":"retry me","type":"request_error","code":"retryable","param":"model"}}"#
            .as_bytes()
            .to_vec();
    let mut headers = vec![
        (
            String::from("content-type"),
            String::from("application/json"),
        ),
        (String::from("content-length"), body.len().to_string()),
    ];
    headers.extend(extra_headers);

    mock_http::ScriptedResponse {
        status_code,
        reason,
        headers,
        body,
        ..Default::default()
    }
}
