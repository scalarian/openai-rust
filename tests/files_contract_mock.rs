#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use std::time::Duration;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::files::{
        FileCreateParams, FileDeleteResponse, FileExpiresAfter, FileListParams, FilePurpose,
        FileStatus, FileUpload, WaitForProcessingOptions,
    },
};
use serde_json::json;

#[test]
fn create_preserves_multipart_semantics() {
    let server =
        mock_http::MockHttpServer::spawn(json_response(file_payload("file-created", "uploaded")))
            .unwrap();
    let client = client(&server.url());

    let response = client
        .files()
        .create(FileCreateParams {
            file: FileUpload::new(
                "training.jsonl",
                "application/jsonl",
                br#"{"messages":[]}"#.to_vec(),
            ),
            purpose: FilePurpose::FineTune,
            expires_after: Some(FileExpiresAfter {
                anchor: String::from("created_at"),
                seconds: 3600,
            }),
        })
        .unwrap();

    assert_eq!(response.output.id, "file-created");

    let request = server.captured_request().unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/files");
    let content_type = request.headers.get("content-type").unwrap();
    assert!(content_type.starts_with("multipart/form-data; boundary="));
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    let multipart = multipart_support::parse_multipart(&request.body, boundary).unwrap();
    assert_text_part(&multipart, "purpose", "fine-tune");
    assert_text_part(&multipart, "expires_after[anchor]", "created_at");
    assert_text_part(&multipart, "expires_after[seconds]", "3600");
    let file_part = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some("file"))
        .unwrap();
    assert_eq!(file_part.filename.as_deref(), Some("training.jsonl"));
    assert_eq!(
        file_part.headers.get("content-type").map(String::as_str),
        Some("application/jsonl")
    );
    assert_eq!(file_part.body, br#"{"messages":[]}"#);
}

#[test]
fn list_preserves_cursor_pagination_and_filters() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(files_page_payload(vec!["file-1", "file-2"], true)),
        json_response(files_page_payload(vec!["file-3"], false)),
    ])
    .unwrap();
    let client = client(&server.url());

    let first = client
        .files()
        .list(FileListParams {
            after: Some(String::from("file-0")),
            limit: Some(2),
            order: Some(String::from("asc")),
            purpose: Some(FilePurpose::Batch),
        })
        .unwrap();
    assert_eq!(first.output.data.len(), 2);
    assert_eq!(first.output.next_after(), Some("file-2"));
    assert!(first.output.has_next_page());

    let second = client
        .files()
        .list(FileListParams {
            after: first.output.next_after().map(String::from),
            limit: Some(2),
            order: Some(String::from("asc")),
            purpose: Some(FilePurpose::Batch),
        })
        .unwrap();
    assert_eq!(second.output.data.len(), 1);
    assert!(!second.output.has_next_page());

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(
        requests[0].path,
        "/v1/files?after=file-0&limit=2&order=asc&purpose=batch"
    );
    assert_eq!(
        requests[1].path,
        "/v1/files?after=file-2&limit=2&order=asc&purpose=batch"
    );
}

#[test]
fn retrieve_and_delete() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(file_payload("file-123", "processed")),
        json_response(
            json!({
                "id": "file-123",
                "object": "file",
                "deleted": true
            })
            .to_string(),
        ),
    ])
    .unwrap();
    let client = client(&server.url());

    let retrieved = client.files().retrieve("file-123").unwrap();
    assert_eq!(retrieved.output.id, "file-123");
    assert_eq!(retrieved.output.status, Some(FileStatus::Processed));
    assert_eq!(retrieved.output.filename, "file-123.jsonl");

    let deleted = client.files().delete("file-123").unwrap();
    assert_eq!(
        deleted.output,
        FileDeleteResponse {
            id: String::from("file-123"),
            object: String::from("file"),
            deleted: true,
            extra: Default::default(),
        }
    );

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(requests[0].path, "/v1/files/file-123");
    assert_eq!(requests[1].path, "/v1/files/file-123");

    let error = client.files().retrieve(" ").unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Validation));
}

#[test]
fn content_and_wait_for_processing() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        binary_response(b"file-bytes"),
        json_response(file_payload("file-pending", "uploaded")),
        json_response(file_payload("file-pending", "processed")),
    ])
    .unwrap();
    let client = client(&server.url());

    let content = client.files().content("file-pending").unwrap();
    assert_eq!(content.output, b"file-bytes");

    let processed = client
        .files()
        .wait_for_processing(
            "file-pending",
            WaitForProcessingOptions {
                poll_interval: Duration::from_millis(1),
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert_eq!(processed.output.status, Some(FileStatus::Processed));

    let requests = server.captured_requests(3).unwrap();
    assert_eq!(requests[0].path, "/v1/files/file-pending/content");
    assert_eq!(requests[1].path, "/v1/files/file-pending");
    assert_eq!(requests[2].path, "/v1/files/file-pending");
}

#[test]
fn wait_for_processing_times_out_with_clear_error() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(file_payload("file-pending", "uploaded")),
        delayed_json_response(
            file_payload("file-pending", "uploaded"),
            Duration::from_millis(20),
        ),
    ])
    .unwrap();
    let client = client(&server.url());

    let error = client
        .files()
        .wait_for_processing(
            "file-pending",
            WaitForProcessingOptions {
                poll_interval: Duration::from_millis(1),
                max_wait: Duration::from_millis(5),
            },
        )
        .unwrap_err();

    assert!(matches!(error.kind, ErrorKind::Timeout));
    assert!(
        error
            .message
            .contains("Giving up on waiting for file file-pending")
    );
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn file_payload(id: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "file",
        "bytes": 17,
        "created_at": 1_717_171_717,
        "filename": format!("{id}.jsonl"),
        "purpose": "fine-tune",
        "status": status,
        "expires_at": 1_717_181_717
    })
    .to_string()
}

fn files_page_payload(ids: Vec<&str>, has_more: bool) -> String {
    let data: Vec<_> = ids
        .into_iter()
        .map(|id| {
            json!({
                "id": id,
                "object": "file",
                "bytes": 17,
                "created_at": 1_717_171_717,
                "filename": format!("{id}.jsonl"),
                "purpose": "batch",
                "status": "processed"
            })
        })
        .collect();
    json!({
        "object": "list",
        "data": data,
        "has_more": has_more
    })
    .to_string()
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn delayed_json_response(body: String, delay: Duration) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        delay,
        ..json_response(body)
    }
}

fn binary_response(body: &[u8]) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/octet-stream"),
            ),
        ],
        body: body.to_vec(),
        ..Default::default()
    }
}

fn assert_text_part(multipart: &multipart_support::ParsedMultipart, name: &str, value: &str) {
    let part = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("missing multipart text part `{name}`"));
    assert_eq!(String::from_utf8(part.body.clone()).unwrap(), value);
}
