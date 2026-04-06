#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use std::time::Duration;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::{
        files::FileUpload,
        vector_stores::{
            VectorStoreFileBatchCreateParams, VectorStoreFileBatchFile,
            VectorStoreFileBatchListFilesParams, VectorStoreFileBatchPollOptions,
            VectorStoreFileBatchStatus, VectorStoreFileBatchUploadAndPollParams,
            VectorStoreFileStatus,
        },
    },
};
use serde_json::json;

#[test]
fn create_retrieve_cancel_and_list_files_cover_batch_contract() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(vector_store_file_batch_payload(
            "vsfb_shared",
            "in_progress",
            counts(2, 0, 0, 0, 2),
        )),
        json_response(vector_store_file_batch_payload(
            "vsfb_per_file",
            "completed",
            counts(0, 2, 0, 0, 2),
        )),
        json_response(vector_store_file_batch_payload(
            "vsfb_shared",
            "failed",
            counts(0, 1, 1, 0, 2),
        )),
        json_response(vector_store_file_batch_payload(
            "vsfb_shared",
            "cancelled",
            counts(0, 1, 0, 1, 2),
        )),
        json_response(vector_store_file_page_payload()),
    ])
    .unwrap();
    let client = client(&server.url());

    let shared = client
        .vector_stores()
        .file_batches()
        .create(
            "vs_123",
            VectorStoreFileBatchCreateParams {
                attributes: Some(json!({"department": "support"})),
                chunking_strategy: None,
                file_ids: vec![String::from("file_1"), String::from("file_2")],
                files: Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(shared.output.status, VectorStoreFileBatchStatus::InProgress);
    assert_eq!(shared.output.file_counts.total, 2);

    let per_file = client
        .vector_stores()
        .file_batches()
        .create(
            "vs_123",
            VectorStoreFileBatchCreateParams {
                attributes: None,
                chunking_strategy: None,
                file_ids: Vec::new(),
                files: vec![
                    VectorStoreFileBatchFile {
                        file_id: String::from("file_a"),
                        attributes: Some(json!({"topic": "faq"})),
                        chunking_strategy: None,
                    },
                    VectorStoreFileBatchFile {
                        file_id: String::from("file_b"),
                        attributes: Some(json!({"topic": "policy"})),
                        chunking_strategy: None,
                    },
                ],
            },
        )
        .unwrap();
    assert_eq!(
        per_file.output.status,
        VectorStoreFileBatchStatus::Completed
    );

    let retrieved = client
        .vector_stores()
        .file_batches()
        .retrieve("vs_123", "vsfb_shared")
        .unwrap();
    assert_eq!(retrieved.output.status, VectorStoreFileBatchStatus::Failed);
    assert_eq!(retrieved.output.file_counts.failed, 1);

    let cancelled = client
        .vector_stores()
        .file_batches()
        .cancel("vs_123", "vsfb_shared")
        .unwrap();
    assert_eq!(
        cancelled.output.status,
        VectorStoreFileBatchStatus::Cancelled
    );
    assert_eq!(cancelled.output.file_counts.cancelled, 1);

    let listed = client
        .vector_stores()
        .file_batches()
        .list_files(
            "vs_123",
            "vsfb_shared",
            VectorStoreFileBatchListFilesParams {
                after: Some(String::from("vsf_000")),
                before: Some(String::from("vsf_999")),
                filter: Some(String::from("failed")),
                limit: Some(2),
                order: Some(String::from("desc")),
            },
        )
        .unwrap();
    assert_eq!(listed.output.data.len(), 2);
    assert_eq!(listed.output.next_after(), Some("vsf_222"));
    assert_eq!(
        listed.output.data[1].status,
        Some(VectorStoreFileStatus::Failed)
    );

    let requests = server.captured_requests(5).unwrap();
    assert_eq!(requests[0].path, "/v1/vector_stores/vs_123/file_batches");
    assert_eq!(requests[1].path, "/v1/vector_stores/vs_123/file_batches");
    assert_eq!(
        requests[2].path,
        "/v1/vector_stores/vs_123/file_batches/vsfb_shared"
    );
    assert_eq!(
        requests[3].path,
        "/v1/vector_stores/vs_123/file_batches/vsfb_shared/cancel"
    );
    assert_eq!(
        requests[4].path,
        "/v1/vector_stores/vs_123/file_batches/vsfb_shared/files?after=vsf_000&before=vsf_999&filter=failed&limit=2&order=desc"
    );
    for request in &requests {
        assert_eq!(
            request.headers.get("openai-beta").map(String::as_str),
            Some("assistants=v2")
        );
    }

    let shared_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(shared_body["file_ids"], json!(["file_1", "file_2"]));
    assert_eq!(shared_body["attributes"]["department"], json!("support"));
    assert!(shared_body.get("files").is_none());

    let per_file_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(per_file_body["files"][0]["file_id"], json!("file_a"));
    assert_eq!(
        per_file_body["files"][1]["attributes"]["topic"],
        json!("policy")
    );
    assert!(per_file_body.get("file_ids").is_none());

    let blank_retrieve = client
        .vector_stores()
        .file_batches()
        .retrieve("vs_123", " ")
        .unwrap_err();
    assert!(matches!(blank_retrieve.kind, ErrorKind::Validation));

    let blank_cancel = client
        .vector_stores()
        .file_batches()
        .cancel(" ", "vsfb_shared")
        .unwrap_err();
    assert!(matches!(blank_cancel.kind, ErrorKind::Validation));
}

#[test]
fn upload_and_poll_merges_existing_file_ids_and_rejects_empty_work() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(file_payload("file_uploaded")),
        json_response(vector_store_file_batch_payload(
            "vsfb_upload",
            "in_progress",
            counts(1, 0, 0, 0, 1),
        )),
        json_response_with_headers(
            vector_store_file_batch_payload("vsfb_upload", "in_progress", counts(1, 0, 0, 0, 1)),
            vec![(String::from("openai-poll-after-ms"), String::from("10"))],
        ),
        json_response(vector_store_file_batch_payload(
            "vsfb_upload",
            "completed",
            counts(0, 1, 0, 0, 1),
        )),
    ])
    .unwrap();
    let client = client(&server.url());

    let response = client
        .vector_stores()
        .file_batches()
        .upload_and_poll(
            "vs_123",
            VectorStoreFileBatchUploadAndPollParams {
                files: vec![FileUpload::new(
                    "knowledge.txt",
                    "text/plain",
                    b"support policy".to_vec(),
                )],
                file_ids: vec![String::from("file_existing")],
            },
            VectorStoreFileBatchPollOptions {
                poll_interval: None,
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert_eq!(
        response.output.status,
        VectorStoreFileBatchStatus::Completed
    );

    let requests = server.captured_requests(4).unwrap();
    assert_eq!(requests[0].path, "/v1/files");
    assert_eq!(requests[1].path, "/v1/vector_stores/vs_123/file_batches");
    assert_eq!(
        requests[2].path,
        "/v1/vector_stores/vs_123/file_batches/vsfb_upload"
    );
    assert_eq!(
        requests[3].path,
        "/v1/vector_stores/vs_123/file_batches/vsfb_upload"
    );

    let content_type = requests[0].headers.get("content-type").unwrap();
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    let multipart = multipart_support::parse_multipart(&requests[0].body, boundary).unwrap();
    let purpose = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some("purpose"))
        .unwrap();
    assert_eq!(
        String::from_utf8(purpose.body.clone()).unwrap(),
        "assistants"
    );

    let batch_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(
        batch_body["file_ids"],
        json!(["file_existing", "file_uploaded"])
    );
    assert_eq!(
        requests[2]
            .headers
            .get("x-stainless-poll-helper")
            .map(String::as_str),
        Some("true")
    );
    assert!(requests[3].received_after >= requests[2].received_after + Duration::from_millis(8));

    let empty = client
        .vector_stores()
        .file_batches()
        .upload_and_poll(
            "vs_123",
            VectorStoreFileBatchUploadAndPollParams {
                files: Vec::new(),
                file_ids: vec![String::from("file_existing")],
            },
            VectorStoreFileBatchPollOptions::default(),
        )
        .unwrap_err();
    assert!(matches!(empty.kind, ErrorKind::Validation));
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

fn vector_store_file_page_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "id": "vsf_111",
                "object": "vector_store.file",
                "created_at": 1_717_171_717,
                "usage_bytes": 1024,
                "vector_store_id": "vs_123",
                "status": "completed",
                "last_error": null
            },
            {
                "id": "vsf_222",
                "object": "vector_store.file",
                "created_at": 1_717_171_718,
                "usage_bytes": 512,
                "vector_store_id": "vs_123",
                "status": "failed",
                "last_error": {"code": "server_error", "message": "parse failed"}
            }
        ],
        "has_more": true
    })
    .to_string()
}

fn file_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "file",
        "bytes": 14,
        "created_at": 1_717_171_717,
        "filename": "knowledge.txt",
        "purpose": "assistants",
        "status": "processed"
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
