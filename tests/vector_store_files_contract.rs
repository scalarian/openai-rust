#[path = "support/mock_http.rs"]
mod mock_http;

use std::time::Duration;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::vector_stores::{
        VectorStoreFileContentPage, VectorStoreFileCreateParams, VectorStoreFileDeleteResponse,
        VectorStoreFileListParams, VectorStoreFilePollOptions, VectorStoreFileStatus,
        VectorStoreFileUpdateParams,
    },
};
use serde_json::json;

#[test]
fn crud_and_content_flows() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(vector_store_file_payload("vsf_123", "in_progress")),
        json_response(vector_store_file_payload("vsf_123", "completed")),
        json_response(vector_store_file_with_attributes_payload(
            "vsf_123",
            "completed",
            json!({"department": "support"}),
        )),
        json_response(vector_store_file_page_payload()),
        json_response(vector_store_file_content_payload()),
        json_response(
            json!({
                "id": "vsf_123",
                "object": "vector_store.file.deleted",
                "deleted": true
            })
            .to_string(),
        ),
    ])
    .unwrap();
    let client = client(&server.url());

    let created = client
        .vector_stores()
        .files()
        .create(
            "vs_123",
            VectorStoreFileCreateParams {
                file_id: String::from("file_123"),
                attributes: Some(json!({"department": "support"})),
                chunking_strategy: None,
            },
        )
        .unwrap();
    assert_eq!(created.output.id, "vsf_123");
    assert_eq!(
        created.output.status,
        Some(VectorStoreFileStatus::InProgress)
    );

    let retrieved = client
        .vector_stores()
        .files()
        .retrieve("vs_123", "vsf_123")
        .unwrap();
    assert_eq!(
        retrieved.output.status,
        Some(VectorStoreFileStatus::Completed)
    );

    let updated = client
        .vector_stores()
        .files()
        .update(
            "vs_123",
            "vsf_123",
            VectorStoreFileUpdateParams {
                attributes: Some(json!({"department": "support"})),
            },
        )
        .unwrap();
    assert_eq!(
        updated.output.attributes,
        Some(json!({"department": "support"}))
    );

    let listed = client
        .vector_stores()
        .files()
        .list(
            "vs_123",
            VectorStoreFileListParams {
                after: Some(String::from("vsf_000")),
                before: Some(String::from("vsf_999")),
                filter: Some(String::from("completed")),
                limit: Some(2),
                order: Some(String::from("asc")),
            },
        )
        .unwrap();
    assert_eq!(listed.output.data.len(), 2);
    assert_eq!(listed.output.next_after(), Some("vsf_222"));

    let content = client
        .vector_stores()
        .files()
        .content("vs_123", "vsf_123")
        .unwrap();
    assert_eq!(
        content.output,
        VectorStoreFileContentPage {
            object: String::from("list"),
            data: vec![
                openai_rust::resources::vector_stores::VectorStoreFileContentPart {
                    r#type: Some(String::from("text")),
                    text: Some(String::from("First parsed chunk")),
                    extra: Default::default(),
                }
            ],
            has_more: false,
            extra: Default::default(),
        }
    );

    let deleted = client
        .vector_stores()
        .files()
        .delete("vs_123", "vsf_123")
        .unwrap();
    assert_eq!(
        deleted.output,
        VectorStoreFileDeleteResponse {
            id: String::from("vsf_123"),
            object: String::from("vector_store.file.deleted"),
            deleted: true,
            extra: Default::default(),
        }
    );

    let requests = server.captured_requests(6).unwrap();
    assert_eq!(requests[0].path, "/v1/vector_stores/vs_123/files");
    assert_eq!(requests[1].path, "/v1/vector_stores/vs_123/files/vsf_123");
    assert_eq!(requests[2].path, "/v1/vector_stores/vs_123/files/vsf_123");
    assert_eq!(
        requests[3].path,
        "/v1/vector_stores/vs_123/files?after=vsf_000&before=vsf_999&filter=completed&limit=2&order=asc"
    );
    assert_eq!(
        requests[4].path,
        "/v1/vector_stores/vs_123/files/vsf_123/content"
    );
    assert_eq!(requests[5].path, "/v1/vector_stores/vs_123/files/vsf_123");
    for request in requests {
        assert_eq!(
            request.headers.get("openai-beta").map(String::as_str),
            Some("assistants=v2")
        );
    }
}

#[test]
fn polling_helpers_respect_explicit_and_server_intervals() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(vector_store_file_payload("vsf_explicit", "in_progress")),
        json_response_with_headers(
            vector_store_file_payload("vsf_explicit", "in_progress"),
            vec![(String::from("openai-poll-after-ms"), String::from("50"))],
        ),
        json_response(vector_store_file_payload("vsf_explicit", "completed")),
        json_response_with_headers(
            vector_store_file_payload("vsf_header", "in_progress"),
            vec![(String::from("openai-poll-after-ms"), String::from("15"))],
        ),
        json_response(vector_store_file_payload("vsf_header", "completed")),
        json_response(vector_store_file_failed_payload(
            "vsf_failed",
            "file_too_large",
            "The file exceeded the processing limit.",
        )),
        json_response(vector_store_file_payload("vsf_cancelled", "in_progress")),
        json_response(vector_store_file_payload("vsf_cancelled", "cancelled")),
    ])
    .unwrap();
    let client = client(&server.url());

    let explicit = client
        .vector_stores()
        .files()
        .create_and_poll(
            "vs_123",
            VectorStoreFileCreateParams {
                file_id: String::from("file_123"),
                attributes: None,
                chunking_strategy: None,
            },
            VectorStoreFilePollOptions {
                poll_interval: Some(Duration::from_millis(5)),
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert_eq!(
        explicit.output.status,
        Some(VectorStoreFileStatus::Completed)
    );

    let header_driven = client
        .vector_stores()
        .files()
        .poll(
            "vs_123",
            "vsf_header",
            VectorStoreFilePollOptions {
                poll_interval: None,
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert_eq!(
        header_driven.output.status,
        Some(VectorStoreFileStatus::Completed)
    );
    let failed = client
        .vector_stores()
        .files()
        .poll(
            "vs_123",
            "vsf_failed",
            VectorStoreFilePollOptions {
                poll_interval: None,
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert_eq!(failed.output.status, Some(VectorStoreFileStatus::Failed));
    assert_eq!(
        failed.output.last_error,
        Some(
            openai_rust::resources::vector_stores::VectorStoreFileLastError {
                code: Some(String::from("file_too_large")),
                message: Some(String::from("The file exceeded the processing limit.")),
                extra: Default::default(),
            }
        )
    );

    let cancelled = client
        .vector_stores()
        .files()
        .create_and_poll(
            "vs_123",
            VectorStoreFileCreateParams {
                file_id: String::from("file_456"),
                attributes: None,
                chunking_strategy: None,
            },
            VectorStoreFilePollOptions {
                poll_interval: None,
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert_eq!(
        cancelled.output.status,
        Some(VectorStoreFileStatus::Cancelled)
    );

    let requests = server.captured_requests(8).unwrap();
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
    assert_eq!(
        requests[5]
            .headers
            .get("x-stainless-poll-helper")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        requests[5].path,
        "/v1/vector_stores/vs_123/files/vsf_failed"
    );
    assert_eq!(requests[6].path, "/v1/vector_stores/vs_123/files");
    assert_eq!(
        requests[7].path,
        "/v1/vector_stores/vs_123/files/vsf_cancelled"
    );
    assert_eq!(
        requests[7]
            .headers
            .get("x-stainless-poll-helper")
            .map(String::as_str),
        Some("true")
    );

    let blank_id = client
        .vector_stores()
        .files()
        .retrieve("vs_123", " ")
        .unwrap_err();
    assert!(matches!(blank_id.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn vector_store_file_payload(id: &str, status: &str) -> String {
    vector_store_file_with_attributes_payload(id, status, json!({"department": "support"}))
}

fn vector_store_file_failed_payload(id: &str, code: &str, message: &str) -> String {
    json!({
        "id": id,
        "object": "vector_store.file",
        "created_at": 1_717_171_717,
        "usage_bytes": 1024,
        "vector_store_id": "vs_123",
        "status": "failed",
        "last_error": {
            "code": code,
            "message": message
        },
        "attributes": {"department": "support"}
    })
    .to_string()
}

fn vector_store_file_with_attributes_payload(
    id: &str,
    status: &str,
    attributes: serde_json::Value,
) -> String {
    json!({
        "id": id,
        "object": "vector_store.file",
        "created_at": 1_717_171_717,
        "usage_bytes": 1024,
        "vector_store_id": "vs_123",
        "status": status,
        "last_error": null,
        "attributes": attributes
    })
    .to_string()
}

fn vector_store_file_page_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&vector_store_file_payload("vsf_111", "completed")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&vector_store_file_payload("vsf_222", "completed")).unwrap()
        ],
        "has_more": true
    })
    .to_string()
}

fn vector_store_file_content_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "type": "text",
                "text": "First parsed chunk"
            }
        ],
        "has_more": false
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
