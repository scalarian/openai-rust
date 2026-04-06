#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use std::time::Duration;

use openai_rust::{
    OpenAI,
    error::ErrorKind,
    resources::{
        files::FileUpload,
        vector_stores::{
            VectorStoreFilePollOptions, VectorStoreFileStatus, VectorStoreFileUploadParams,
        },
    },
};
use serde_json::json;

#[test]
fn blank_vector_store_id_fails_before_file_upload() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![json_response(file_payload(
        "file_should_not_upload",
    ))])
    .unwrap();
    let client = client(&server.url());

    let error = client
        .vector_stores()
        .files()
        .upload(
            "   ",
            VectorStoreFileUploadParams {
                file: FileUpload::new("knowledge.txt", "text/plain", b"support policy".to_vec()),
                attributes: None,
                chunking_strategy: None,
            },
        )
        .unwrap_err();

    assert_eq!(error.kind, ErrorKind::Validation);
    assert_eq!(error.message, "vector_store_id cannot be blank");
    assert!(server.captured_request().is_none());
}

#[test]
fn upload_composes_files_create_and_attach() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(file_payload("file_uploaded")),
        json_response(vector_store_file_payload("vsf_uploaded", "completed")),
    ])
    .unwrap();
    let client = client(&server.url());

    let response = client
        .vector_stores()
        .files()
        .upload(
            "vs_123",
            VectorStoreFileUploadParams {
                file: FileUpload::new("knowledge.txt", "text/plain", b"support policy".to_vec()),
                attributes: Some(json!({"department": "support"})),
                chunking_strategy: None,
            },
        )
        .unwrap();
    assert_eq!(
        response.output.status,
        Some(VectorStoreFileStatus::Completed)
    );

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(requests[0].path, "/v1/files");
    assert_eq!(requests[1].path, "/v1/vector_stores/vs_123/files");

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

    let attach_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(attach_body["file_id"], json!("file_uploaded"));
    assert_eq!(attach_body["attributes"]["department"], json!("support"));
}

#[test]
fn upload_and_poll_reuses_file_upload_and_attach_flow() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(file_payload("file_polled")),
        json_response(vector_store_file_payload("vsf_polled", "in_progress")),
        json_response_with_headers(
            vector_store_file_payload("vsf_polled", "in_progress"),
            vec![(String::from("openai-poll-after-ms"), String::from("10"))],
        ),
        json_response(vector_store_file_payload("vsf_polled", "completed")),
    ])
    .unwrap();
    let client = client(&server.url());

    let response = client
        .vector_stores()
        .files()
        .upload_and_poll(
            "vs_123",
            VectorStoreFileUploadParams {
                file: FileUpload::new("knowledge.txt", "text/plain", b"support policy".to_vec()),
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
        response.output.status,
        Some(VectorStoreFileStatus::Completed)
    );

    let requests = server.captured_requests(4).unwrap();
    assert_eq!(requests[0].path, "/v1/files");
    assert_eq!(requests[1].path, "/v1/vector_stores/vs_123/files");
    assert_eq!(
        requests[2].path,
        "/v1/vector_stores/vs_123/files/vsf_polled"
    );
    assert_eq!(
        requests[3].path,
        "/v1/vector_stores/vs_123/files/vsf_polled"
    );
    assert_eq!(
        requests[2]
            .headers
            .get("x-stainless-poll-helper")
            .map(String::as_str),
        Some("true")
    );
    assert!(requests[3].received_after >= requests[2].received_after + Duration::from_millis(8));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
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

fn vector_store_file_payload(id: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "vector_store.file",
        "created_at": 1_717_171_717,
        "usage_bytes": 1024,
        "vector_store_id": "vs_123",
        "status": status,
        "last_error": null
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
