#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::batches::{
        BatchCompletionWindow, BatchCreateParams, BatchEndpoint, BatchListParams,
        BatchOutputExpiresAfter, BatchStatus,
    },
};
use serde_json::json;

#[test]
fn create_and_cancel_lifecycle() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(batch_payload("batch_123", "validating")),
        json_response(batch_payload("batch_123", "cancelling")),
    ])
    .unwrap();
    let client = client(&server.url());

    let created = client
        .batches()
        .create(BatchCreateParams {
            completion_window: BatchCompletionWindow::Hours24,
            endpoint: BatchEndpoint::Responses,
            input_file_id: String::from("file_input"),
            metadata: Some(json!({"job": "nightly"})),
            output_expires_after: Some(BatchOutputExpiresAfter {
                anchor: String::from("created_at"),
                seconds: 3600,
            }),
        })
        .unwrap();
    assert_eq!(created.output.status, BatchStatus::Validating);

    let cancelled = client.batches().cancel("batch_123").unwrap();
    assert_eq!(cancelled.output.status, BatchStatus::Cancelling);

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(requests[0].path, "/v1/batches");
    assert_eq!(requests[1].path, "/v1/batches/batch_123/cancel");

    let create_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["completion_window"], json!("24h"));
    assert_eq!(create_body["endpoint"], json!("/v1/responses"));
    assert_eq!(create_body["input_file_id"], json!("file_input"));
    assert_eq!(create_body["metadata"]["job"], json!("nightly"));
    assert_eq!(
        create_body["output_expires_after"]["anchor"],
        json!("created_at")
    );
    assert_eq!(create_body["output_expires_after"]["seconds"], json!(3600));
}

#[test]
fn retrieve_lifecycle() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(batch_payload("batch_123", "completed")),
        json_response(batch_list_payload()),
    ])
    .unwrap();
    let client = client(&server.url());

    let retrieved = client.batches().retrieve("batch_123").unwrap();
    assert_eq!(retrieved.output.status, BatchStatus::Completed);
    assert_eq!(
        retrieved.output.output_file_id.as_deref(),
        Some("file_output")
    );
    assert_eq!(
        retrieved.output.error_file_id.as_deref(),
        Some("file_error")
    );
    assert_eq!(retrieved.output.request_counts.as_ref().unwrap().failed, 1);

    let listed = client
        .batches()
        .list(BatchListParams {
            after: Some(String::from("batch_000")),
            limit: Some(2),
        })
        .unwrap();
    assert_eq!(listed.output.data.len(), 2);
    assert_eq!(listed.output.next_after(), Some("batch_222"));

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(requests[0].path, "/v1/batches/batch_123");
    assert_eq!(requests[1].path, "/v1/batches?after=batch_000&limit=2");

    let blank = client.batches().retrieve(" ").unwrap_err();
    assert!(matches!(blank.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn batch_payload(id: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "batch",
        "completion_window": "24h",
        "created_at": 1_717_171_717,
        "endpoint": "/v1/responses",
        "input_file_id": "file_input",
        "status": status,
        "request_counts": {
            "completed": 3,
            "failed": 1,
            "total": 4
        },
        "output_file_id": "file_output",
        "error_file_id": "file_error",
        "metadata": {"job": "nightly"}
    })
    .to_string()
}

fn batch_list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&batch_payload("batch_111", "in_progress")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&batch_payload("batch_222", "completed")).unwrap()
        ],
        "has_more": true
    })
    .to_string()
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    let headers = vec![
        (String::from("content-length"), body.len().to_string()),
        (
            String::from("content-type"),
            String::from("application/json"),
        ),
    ];
    mock_http::ScriptedResponse {
        headers,
        body: body.into_bytes(),
        ..Default::default()
    }
}
