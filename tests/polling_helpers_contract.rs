#[path = "support/mock_http.rs"]
mod mock_http;

use std::time::Duration;

use openai_rust::{
    OpenAI,
    resources::vector_stores::{
        VectorStoreFileBatchCreateParams, VectorStoreFileBatchPollOptions,
        VectorStoreFileBatchStatus,
    },
};
use serde_json::json;

#[test]
fn vector_store_file_batch_polling_respects_explicit_and_server_intervals() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(vector_store_file_batch_payload(
            "vsfb_explicit",
            "in_progress",
            counts(2, 0, 0, 0, 2),
        )),
        json_response_with_headers(
            vector_store_file_batch_payload("vsfb_explicit", "in_progress", counts(1, 1, 0, 0, 2)),
            vec![(String::from("openai-poll-after-ms"), String::from("50"))],
        ),
        json_response(vector_store_file_batch_payload(
            "vsfb_explicit",
            "completed",
            counts(0, 2, 0, 0, 2),
        )),
        json_response_with_headers(
            vector_store_file_batch_payload("vsfb_header", "in_progress", counts(1, 0, 0, 0, 1)),
            vec![(String::from("openai-poll-after-ms"), String::from("15"))],
        ),
        json_response(vector_store_file_batch_payload(
            "vsfb_header",
            "cancelled",
            counts(0, 0, 0, 1, 1),
        )),
    ])
    .unwrap();
    let client = client(&server.url());

    let explicit = client
        .vector_stores()
        .file_batches()
        .create_and_poll(
            "vs_123",
            VectorStoreFileBatchCreateParams {
                attributes: None,
                chunking_strategy: None,
                file_ids: vec![String::from("file_123")],
                files: Vec::new(),
            },
            VectorStoreFileBatchPollOptions {
                poll_interval: Some(Duration::from_millis(5)),
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert_eq!(
        explicit.output.status,
        VectorStoreFileBatchStatus::Completed
    );

    let header_driven = client
        .vector_stores()
        .file_batches()
        .poll(
            "vs_123",
            "vsfb_header",
            VectorStoreFileBatchPollOptions {
                poll_interval: None,
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert_eq!(
        header_driven.output.status,
        VectorStoreFileBatchStatus::Cancelled
    );
    assert_eq!(header_driven.output.file_counts.cancelled, 1);

    let requests = server.captured_requests(5).unwrap();
    assert_eq!(
        requests[1]
            .headers
            .get("x-stainless-poll-helper")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        requests[1]
            .headers
            .get("x-stainless-custom-poll-interval")
            .map(String::as_str),
        Some("5")
    );
    assert!(requests[2].received_after >= requests[1].received_after + Duration::from_millis(4));

    assert_eq!(
        requests[3]
            .headers
            .get("x-stainless-poll-helper")
            .map(String::as_str),
        Some("true")
    );
    assert!(
        !requests[3]
            .headers
            .contains_key("x-stainless-custom-poll-interval")
    );
    assert!(requests[4].received_after >= requests[3].received_after + Duration::from_millis(10));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn counts(
    in_progress: u64,
    completed: u64,
    failed: u64,
    cancelled: u64,
    total: u64,
) -> serde_json::Value {
    json!({
        "in_progress": in_progress,
        "completed": completed,
        "failed": failed,
        "cancelled": cancelled,
        "total": total
    })
}

fn vector_store_file_batch_payload(
    id: &str,
    status: &str,
    file_counts: serde_json::Value,
) -> String {
    json!({
        "id": id,
        "object": "vector_store.files_batch",
        "created_at": 1_717_171_717,
        "vector_store_id": "vs_123",
        "status": status,
        "file_counts": file_counts
    })
    .to_string()
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    json_response_with_headers(body, Vec::new())
}

fn json_response_with_headers(
    body: String,
    mut extra_headers: Vec<(String, String)>,
) -> mock_http::ScriptedResponse {
    let mut headers = vec![
        (String::from("content-length"), body.len().to_string()),
        (
            String::from("content-type"),
            String::from("application/json"),
        ),
    ];
    headers.append(&mut extra_headers);
    mock_http::ScriptedResponse {
        headers,
        body: body.into_bytes(),
        ..Default::default()
    }
}
