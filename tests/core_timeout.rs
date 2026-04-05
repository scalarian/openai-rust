use std::time::Duration;

use openai_rust::{ErrorKind, OpenAI};
use serde_json::Value;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn timeout_policy_honors_defaults_and_per_request_overrides() {
    let default_client = OpenAI::builder().api_key("test-key").build();
    assert_eq!(
        default_client
            .resolve_request_options(&Default::default())
            .unwrap()
            .timeout,
        Duration::from_secs(600)
    );

    let client_with_override = OpenAI::builder()
        .api_key("test-key")
        .timeout(Duration::from_secs(12))
        .build();
    assert_eq!(
        client_with_override
            .resolve_request_options(&Default::default())
            .unwrap()
            .timeout,
        Duration::from_secs(12)
    );

    let request_override = openai_rust::core::request::RequestOptions {
        timeout: Some(Duration::from_millis(250)),
        ..Default::default()
    };
    assert_eq!(
        client_with_override
            .resolve_request_options(&request_override)
            .unwrap()
            .timeout,
        Duration::from_millis(250)
    );
}

#[test]
fn timeout_expiry_produces_a_dedicated_timeout_error_after_retry_budget() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        mock_http::ScriptedResponse {
            delay: Duration::from_millis(150),
            ..json_ok()
        },
        mock_http::ScriptedResponse {
            delay: Duration::from_millis(150),
            ..json_ok()
        },
        mock_http::ScriptedResponse {
            delay: Duration::from_millis(150),
            ..json_ok()
        },
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("timeout-key")
        .base_url(server.url())
        .timeout(Duration::from_millis(40))
        .max_retries(2)
        .build();

    let error = client
        .execute_json::<Value>("GET", "/models", Default::default())
        .unwrap_err();

    assert_eq!(error.kind, ErrorKind::Timeout);
    assert!(error.status_code().is_none());
    assert!(error.api_error().is_none());
    assert!(error.source().is_some());

    let captured = server.captured_requests(3).unwrap();
    assert_eq!(captured.len(), 3);
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
