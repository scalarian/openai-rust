#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use openai_rust::{
    ErrorKind, OpenAI,
    resources::uploads::{ChunkedUploadSource, UploadChunkedParams, UploadPurpose, UploadStatus},
};
use serde_json::json;

#[test]
fn chunked_helper_preserves_path_and_in_memory_semantics() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(upload_payload("upload-path", "pending")),
        json_response(part_payload("part-1")),
        json_response(part_payload("part-2")),
        json_response(completed_payload(
            "upload-path",
            "from-path.txt",
            vec!["part-1", "part-2"],
        )),
        json_response(upload_payload("upload-bytes", "pending")),
        json_response(part_payload("part-3")),
        json_response(part_payload("part-4")),
        json_response(completed_payload(
            "upload-bytes",
            "memory.bin",
            vec!["part-3", "part-4"],
        )),
    ])
    .unwrap();
    let client = client(&server.url());

    let path = temp_fixture_path();
    let path_bytes = b"abcdef".to_vec();
    fs::write(&path, &path_bytes).unwrap();

    let from_path = client
        .uploads()
        .upload_file_chunked(UploadChunkedParams {
            source: ChunkedUploadSource::Path(path.clone()),
            mime_type: String::from("text/plain"),
            purpose: UploadPurpose::Batch,
            part_size: Some(3),
            md5: Some(String::from("path-md5")),
        })
        .unwrap();
    assert_eq!(from_path.output.status, UploadStatus::Completed);

    let from_bytes = client
        .uploads()
        .upload_file_chunked(UploadChunkedParams {
            source: ChunkedUploadSource::InMemory {
                bytes: b"ghijkl".to_vec(),
                filename: Some(String::from("memory.bin")),
                byte_length: Some(6),
            },
            mime_type: String::from("application/octet-stream"),
            purpose: UploadPurpose::Assistants,
            part_size: Some(4),
            md5: None,
        })
        .unwrap();
    assert_eq!(from_bytes.output.status, UploadStatus::Completed);

    let requests = server.captured_requests(8).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].body).unwrap(),
        json!({
            "bytes": 6,
            "filename": path.file_name().unwrap().to_string_lossy(),
            "mime_type": "text/plain",
            "purpose": "batch"
        })
    );
    assert_part_body(&requests[1], b"abc");
    assert_part_body(&requests[2], b"def");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[3].body).unwrap(),
        json!({"part_ids":["part-1","part-2"],"md5":"path-md5"})
    );

    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[4].body).unwrap(),
        json!({
            "bytes": 6,
            "filename": "memory.bin",
            "mime_type": "application/octet-stream",
            "purpose": "assistants"
        })
    );
    assert_part_body(&requests[5], b"ghij");
    assert_part_body(&requests[6], b"kl");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[7].body).unwrap(),
        json!({"part_ids":["part-3","part-4"]})
    );

    let missing_filename = client
        .uploads()
        .upload_file_chunked(UploadChunkedParams {
            source: ChunkedUploadSource::InMemory {
                bytes: b"abc".to_vec(),
                filename: None,
                byte_length: Some(3),
            },
            mime_type: String::from("application/octet-stream"),
            purpose: UploadPurpose::Assistants,
            part_size: Some(2),
            md5: None,
        })
        .unwrap_err();
    assert!(matches!(missing_filename.kind, ErrorKind::Validation));

    let missing_length = client
        .uploads()
        .upload_file_chunked(UploadChunkedParams {
            source: ChunkedUploadSource::InMemory {
                bytes: b"abc".to_vec(),
                filename: Some(String::from("bytes.bin")),
                byte_length: None,
            },
            mime_type: String::from("application/octet-stream"),
            purpose: UploadPurpose::Assistants,
            part_size: Some(2),
            md5: None,
        })
        .unwrap_err();
    assert!(matches!(missing_length.kind, ErrorKind::Validation));

    let _ = fs::remove_file(path);
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn temp_fixture_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{}-upload-{unique}.txt", env!("CARGO_PKG_NAME")))
}

fn assert_part_body(request: &mock_http::CapturedRequest, expected: &[u8]) {
    let content_type = request.headers.get("content-type").unwrap();
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    let multipart = multipart_support::parse_multipart(&request.body, boundary).unwrap();
    let part = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some("data"))
        .unwrap();
    assert_eq!(part.body, expected);
}

fn upload_payload(id: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "upload",
        "bytes": 6,
        "created_at": 1_717_171_717,
        "expires_at": 1_717_175_317,
        "filename": "fixture.bin",
        "purpose": "assistants",
        "status": status
    })
    .to_string()
}

fn completed_payload(id: &str, filename: &str, part_ids: Vec<&str>) -> String {
    json!({
        "id": id,
        "object": "upload",
        "bytes": 6,
        "created_at": 1_717_171_717,
        "expires_at": 1_717_175_317,
        "filename": filename,
        "purpose": "assistants",
        "status": "completed",
        "part_ids": part_ids,
        "file": {
            "id": format!("file-{id}"),
            "object": "file",
            "bytes": 6,
            "created_at": 1_717_171_717,
            "filename": filename,
            "purpose": "assistants",
            "status": "processed"
        }
    })
    .to_string()
}

fn part_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "upload.part",
        "created_at": 1_717_171_718,
        "upload_id": "upload"
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
