#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::{
        files::{FileExpiresAfter, FilePurpose, FileStatus},
        uploads::{UploadCompleteParams, UploadCreateParams, UploadPartInput, UploadStatus},
    },
};
use serde_json::json;

#[test]
fn lifecycle_and_chunking() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(upload_payload("upload-123", "pending", None)),
        json_response(part_payload("part-1")),
        json_response(
            json!({
                "id": "upload-123",
                "object": "upload",
                "bytes": 12,
                "created_at": 1_717_171_717,
                "expires_at": 1_717_175_317,
                "filename": "parts.jsonl",
                "purpose": "assistants",
                "status": "completed",
                "file": {
                    "id": "file-from-upload",
                    "object": "file",
                    "bytes": 12,
                    "created_at": 1_717_171_717,
                    "filename": "parts.jsonl",
                    "purpose": "assistants",
                    "status": "processed"
                }
            })
            .to_string(),
        ),
        json_response(upload_payload("upload-cancel", "cancelled", None)),
    ])
    .unwrap();
    let client = client(&server.url());

    let created = client
        .uploads()
        .create(UploadCreateParams {
            bytes: 12,
            filename: String::from("parts.jsonl"),
            mime_type: String::from("application/jsonl"),
            purpose: FilePurpose::Assistants,
            expires_after: Some(FileExpiresAfter {
                anchor: String::from("created_at"),
                seconds: 3600,
            }),
        })
        .unwrap();
    assert_eq!(created.output.status, UploadStatus::Pending);

    let part = client
        .uploads()
        .add_part(
            "upload-123",
            UploadPartInput::new(
                "part-1.bin",
                "application/octet-stream",
                b"hello world!".to_vec(),
            ),
        )
        .unwrap();
    assert_eq!(part.output.id, "part-1");

    let completed = client
        .uploads()
        .complete(
            "upload-123",
            UploadCompleteParams {
                part_ids: vec![String::from("part-1")],
                md5: Some(String::from("md5-value")),
            },
        )
        .unwrap();
    assert_eq!(completed.output.status, UploadStatus::Completed);
    assert_eq!(
        completed.output.file.as_ref().unwrap().status,
        Some(FileStatus::Processed)
    );

    let cancelled = client.uploads().cancel("upload-cancel").unwrap();
    assert_eq!(cancelled.output.status, UploadStatus::Cancelled);

    let requests = server.captured_requests(4).unwrap();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/uploads");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].body).unwrap(),
        json!({
            "bytes": 12,
            "filename": "parts.jsonl",
            "mime_type": "application/jsonl",
            "purpose": "assistants",
            "expires_after": {"anchor":"created_at","seconds":3600}
        })
    );

    let content_type = requests[1].headers.get("content-type").unwrap();
    assert!(content_type.starts_with("multipart/form-data; boundary="));
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    let multipart = multipart_support::parse_multipart(&requests[1].body, boundary).unwrap();
    let file_part = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some("data"))
        .unwrap();
    assert_eq!(file_part.filename.as_deref(), Some("part-1.bin"));
    assert_eq!(file_part.body, b"hello world!");

    assert_eq!(requests[2].path, "/v1/uploads/upload-123/complete");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[2].body).unwrap(),
        json!({"part_ids":["part-1"],"md5":"md5-value"})
    );
    assert_eq!(requests[3].path, "/v1/uploads/upload-cancel/cancel");

    let error = client
        .uploads()
        .add_part(" ", UploadPartInput::default())
        .unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn upload_payload(id: &str, status: &str, file: Option<serde_json::Value>) -> String {
    json!({
        "id": id,
        "object": "upload",
        "bytes": 12,
        "created_at": 1_717_171_717,
        "expires_at": 1_717_175_317,
        "filename": "parts.jsonl",
        "purpose": "assistants",
        "status": status,
        "file": file
    })
    .to_string()
}

fn part_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "upload.part",
        "created_at": 1_717_171_718,
        "upload_id": "upload-123"
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
