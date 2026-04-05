use openai_rust::{ApiErrorKind, ErrorKind, OpenAI};
use serde_json::json;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn retrieve_and_list_preserve_model_ids_without_inventing_pagination() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(model_payload("gpt-4.1-mini")),
        json_response(list_payload()),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let retrieved = client.models().retrieve("ft:gpt-4.1-mini:custom").unwrap();
    assert_eq!(retrieved.output().id, "gpt-4.1-mini");
    assert_eq!(retrieved.output().owned_by.as_deref(), Some("openai"));

    let listed = client.models().list().unwrap();
    assert_eq!(listed.output().data.len(), 2);
    assert_eq!(listed.output().data[0].id, "gpt-4.1-mini");
    assert_eq!(listed.output().data[1].id, "omni-moderation-latest");
    assert!(!listed.output().has_next_page());
    assert_eq!(listed.output().next_after(), None);

    let requests = server.captured_requests(2).expect("captured requests");
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/v1/models/ft:gpt-4.1-mini:custom");
    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].path, "/v1/models");

    let blank_id = client
        .models()
        .retrieve("   ")
        .expect_err("blank model id should be rejected locally");
    assert_eq!(blank_id.kind, ErrorKind::Validation);
}

#[test]
fn delete_owned_or_denied() {
    let success_server =
        mock_http::MockHttpServer::spawn(json_response(delete_payload("model-owned"))).unwrap();
    let success_client = OpenAI::builder()
        .api_key("test-key")
        .base_url(success_server.url())
        .max_retries(0)
        .build();

    let deleted = success_client.models().delete("model-owned").unwrap();
    assert_eq!(deleted.output().id, "model-owned");
    assert!(deleted.output().deleted);
    let request = success_server.captured_request().expect("captured request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.path, "/v1/models/model-owned");

    let denied_server = mock_http::MockHttpServer::spawn(error_response(
        403,
        "Forbidden",
        json!({
            "error": {
                "message": "You do not own this model.",
                "type": "invalid_request_error",
                "code": "permission_denied",
                "param": "model"
            }
        })
        .to_string(),
    ))
    .unwrap();
    let denied_client = OpenAI::builder()
        .api_key("test-key")
        .base_url(denied_server.url())
        .max_retries(0)
        .build();
    let denied = denied_client
        .models()
        .delete("model-denied")
        .expect_err("permission error should surface");
    assert_eq!(denied.kind, ErrorKind::Api(ApiErrorKind::PermissionDenied));
    assert_eq!(
        denied.api_error().unwrap().code.as_deref(),
        Some("permission_denied")
    );

    let missing_server = mock_http::MockHttpServer::spawn(error_response(
        404,
        "Not Found",
        json!({
            "error": {
                "message": "No such model",
                "type": "invalid_request_error",
                "code": "model_not_found",
                "param": "model"
            }
        })
        .to_string(),
    ))
    .unwrap();
    let missing_client = OpenAI::builder()
        .api_key("test-key")
        .base_url(missing_server.url())
        .max_retries(0)
        .build();
    let missing = missing_client
        .models()
        .delete("model-missing")
        .expect_err("not found error should surface");
    assert_eq!(missing.kind, ErrorKind::Api(ApiErrorKind::NotFound));
    assert_eq!(
        missing.api_error().unwrap().code.as_deref(),
        Some("model_not_found")
    );

    let blank_id = success_client
        .models()
        .delete(" ")
        .expect_err("blank model id should be rejected locally");
    assert_eq!(blank_id.kind, ErrorKind::Validation);
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

fn error_response(
    status_code: u16,
    reason: &'static str,
    body: String,
) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        status_code,
        reason,
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("x-request-id"),
                String::from("req_model_error"),
            ),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn model_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "model",
        "created": 1710000000,
        "owned_by": "openai"
    })
    .to_string()
}

fn list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "id": "gpt-4.1-mini",
                "object": "model",
                "created": 1710000000,
                "owned_by": "openai"
            },
            {
                "id": "omni-moderation-latest",
                "object": "model",
                "created": 1710000001,
                "owned_by": "openai"
            }
        ]
    })
    .to_string()
}

fn delete_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "model.deleted",
        "deleted": true
    })
    .to_string()
}
