#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use std::time::Duration;

use openai_rust::{
    OpenAI,
    resources::{
        batches::{BatchCompletionWindow, BatchCreateParams, BatchEndpoint},
        files::{FileCreateParams, FilePurpose, FileUpload},
        vector_stores::{
            VectorStoreFileBatchCreateParams, VectorStoreFileBatchPollOptions,
            VectorStoreFileBatchStatus,
        },
    },
};
use serde_json::{Value, json};

#[test]
fn files_flow_into_downstream_batches_and_poll_helpers_honor_server_intervals() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(file_object_payload("file_batch_seed")).with_request_id("req_file_seed"),
        json_response(vector_store_file_batch_payload("vsfb_123", "in_progress")),
        json_response_with_headers(
            vector_store_file_batch_payload("vsfb_123", "in_progress"),
            vec![(String::from("openai-poll-after-ms"), String::from("10"))],
        ),
        json_response(vector_store_file_batch_payload("vsfb_123", "completed")),
    ])
    .unwrap();
    let client = client(&server.url());

    let file = client
        .files()
        .create(FileCreateParams {
            file: FileUpload::new("batch-input.jsonl", "application/jsonl", b"{}".to_vec()),
            purpose: FilePurpose::Batch,
            expires_after: None,
        })
        .unwrap();

    let batch = client
        .vector_stores()
        .file_batches()
        .create_and_poll(
            "vs_123",
            VectorStoreFileBatchCreateParams {
                file_ids: vec![file.output.id.clone()],
                ..Default::default()
            },
            VectorStoreFileBatchPollOptions {
                poll_interval: None,
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert_eq!(batch.output.status, VectorStoreFileBatchStatus::Completed);

    let requests = server.captured_requests(4).unwrap();
    assert_eq!(requests[0].path, "/v1/files");
    assert_eq!(requests[1].path, "/v1/vector_stores/vs_123/file_batches");
    assert_eq!(
        requests[2].path,
        "/v1/vector_stores/vs_123/file_batches/vsfb_123"
    );
    assert_eq!(
        requests[3].path,
        "/v1/vector_stores/vs_123/file_batches/vsfb_123"
    );
    assert_eq!(
        requests[2]
            .headers
            .get("x-stainless-poll-helper")
            .map(String::as_str),
        Some("true")
    );
    assert!(
        requests[3].received_after >= requests[2].received_after + Duration::from_millis(8),
        "expected poll helper to honor openai-poll-after-ms"
    );

    let create_body: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(create_body["file_ids"][0], Value::String(file.output.id));
}

#[test]
fn uploads_complete_file_ids_feed_downstream_batches_without_manual_glue() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(upload_payload("upload_123", "pending", None))
            .with_request_id("req_upload_create"),
        json_response(upload_part_payload("part_1", "upload_123"))
            .with_request_id("req_upload_part"),
        json_response(upload_payload(
            "upload_123",
            "completed",
            Some(file_object_payload("file_from_upload")),
        ))
        .with_request_id("req_upload_complete"),
        json_response(batch_payload("batch_123", "file_from_upload"))
            .with_request_id("req_batch_create"),
    ])
    .unwrap();
    let client = client(&server.url());

    let upload = client
        .uploads()
        .upload_file_chunked(openai_rust::resources::uploads::UploadChunkedParams {
            source: openai_rust::resources::uploads::ChunkedUploadSource::InMemory {
                bytes: b"{\"custom_id\":\"row-1\"}".to_vec(),
                filename: Some(String::from("batch.jsonl")),
                byte_length: Some(21),
            },
            mime_type: String::from("application/jsonl"),
            purpose: openai_rust::resources::uploads::UploadPurpose::Batch,
            part_size: Some(64),
            md5: Some(String::from("md5-upload")),
        })
        .unwrap();

    let uploaded_file_id = upload
        .output
        .file
        .as_ref()
        .expect("completed upload should include file object")
        .id
        .clone();
    let batch = client
        .batches()
        .create(BatchCreateParams {
            completion_window: BatchCompletionWindow::Hours24,
            endpoint: BatchEndpoint::Responses,
            input_file_id: uploaded_file_id.clone(),
            metadata: Some(json!({"source": "upload"})),
            output_expires_after: None,
        })
        .unwrap();
    assert_eq!(batch.output.input_file_id, uploaded_file_id);

    let requests = server.captured_requests(4).unwrap();
    assert_eq!(requests[0].path, "/v1/uploads");
    assert_eq!(requests[1].path, "/v1/uploads/upload_123/parts");
    assert_eq!(requests[2].path, "/v1/uploads/upload_123/complete");
    assert_eq!(requests[3].path, "/v1/batches");

    let part_content_type = requests[1].headers.get("content-type").unwrap();
    let boundary = part_content_type.split("boundary=").nth(1).unwrap();
    let multipart = multipart_support::parse_multipart(&requests[1].body, boundary).unwrap();
    let part = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some("data"))
        .unwrap();
    assert_eq!(part.filename.as_deref(), Some("part.bin"));
    assert_eq!(part.body, b"{\"custom_id\":\"row-1\"}".to_vec());

    let complete_body: Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(
        complete_body["part_ids"][0],
        Value::String(String::from("part_1"))
    );
    assert_eq!(
        complete_body["md5"],
        Value::String(String::from("md5-upload"))
    );

    let batch_body: Value = serde_json::from_slice(&requests[3].body).unwrap();
    assert_eq!(
        batch_body["input_file_id"],
        Value::String(String::from("file_from_upload"))
    );
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .max_retries(0)
        .build()
}

fn json_response(body: Value) -> mock_http::ScriptedResponse {
    let bytes = serde_json::to_vec(&body).unwrap();
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), bytes.len().to_string()),
        ],
        body: bytes,
        ..Default::default()
    }
}

fn json_response_with_headers(
    body: Value,
    mut extra_headers: Vec<(String, String)>,
) -> mock_http::ScriptedResponse {
    let mut response = json_response(body);
    response.headers.append(&mut extra_headers);
    response
}

fn file_object_payload(id: &str) -> Value {
    json!({
        "id": id,
        "object": "file",
        "bytes": 21,
        "created_at": 1,
        "filename": "batch-input.jsonl",
        "purpose": "batch",
        "status": "processed"
    })
}

fn vector_store_file_batch_payload(id: &str, status: &str) -> Value {
    json!({
        "id": id,
        "object": "vector_store.file_batch",
        "created_at": 1,
        "vector_store_id": "vs_123",
        "status": status,
        "file_counts": {
            "in_progress": if status == "completed" { 0 } else { 1 },
            "completed": if status == "completed" { 1 } else { 0 },
            "cancelled": 0,
            "failed": 0,
            "total": 1
        }
    })
}

fn upload_payload(id: &str, status: &str, file: Option<Value>) -> Value {
    json!({
        "id": id,
        "object": "upload",
        "bytes": 21,
        "created_at": 1,
        "expires_at": 2,
        "filename": "batch.jsonl",
        "purpose": "batch",
        "status": status,
        "file": file
    })
}

fn upload_part_payload(id: &str, upload_id: &str) -> Value {
    json!({
        "id": id,
        "object": "upload.part",
        "created_at": 1,
        "upload_id": upload_id
    })
}

fn batch_payload(id: &str, input_file_id: &str) -> Value {
    json!({
        "id": id,
        "object": "batch",
        "created_at": 1,
        "completion_window": "24h",
        "endpoint": "/v1/responses",
        "input_file_id": input_file_id,
        "status": "validating",
        "request_counts": {
            "completed": 0,
            "failed": 0,
            "total": 1
        }
    })
}

trait RequestIdResponseExt {
    fn with_request_id(self, request_id: &str) -> Self;
}

impl RequestIdResponseExt for mock_http::ScriptedResponse {
    fn with_request_id(mut self, request_id: &str) -> Self {
        self.headers
            .push((String::from("x-request-id"), String::from(request_id)));
        self
    }
}
