#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use openai_rust::{
    ApiErrorKind, ErrorKind, OpenAI,
    resources::containers::{
        ContainerFileCreateParams, ContainerFileDeleteResponse, ContainerFileListParams,
        ContainerFileOrder, ContainerFileSource, ContainerFileUpload,
    },
};
use serde_json::json;

#[test]
fn container_files_support_upload_metadata_lookup_and_binary_content_download() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(container_file_payload(
            "cfile_upload",
            "/mnt/data/uploaded.txt",
            "user",
        )),
        json_response(container_file_payload(
            "cfile_copy",
            "/mnt/data/copied.txt",
            "assistant",
        )),
        json_response(container_file_payload(
            "cfile_upload",
            "/mnt/data/uploaded.txt",
            "user",
        )),
        json_response(container_files_page_payload()),
        binary_response(b"\x00container-bytes{json:false}"),
        json_response(
            json!({
                "id": "cfile_upload",
                "object": "container.file.deleted",
                "deleted": true,
                "container_id": "cntr_123"
            })
            .to_string(),
        ),
    ])
    .unwrap();
    let client = client(&server.url());

    let uploaded = client
        .containers()
        .files()
        .create(
            "cntr_123",
            ContainerFileCreateParams::Upload(ContainerFileUpload::new(
                "uploaded.txt",
                "text/plain",
                b"hello from the container".to_vec(),
            )),
        )
        .unwrap();
    assert_eq!(uploaded.output.id, "cfile_upload");
    assert_eq!(uploaded.output.source, Some(ContainerFileSource::User));

    let copied = client
        .containers()
        .files()
        .create(
            "cntr_123",
            ContainerFileCreateParams::FileId(String::from("file_original")),
        )
        .unwrap();
    assert_eq!(copied.output.id, "cfile_copy");
    assert_eq!(copied.output.path, "/mnt/data/copied.txt");

    let retrieved = client
        .containers()
        .files()
        .retrieve("cntr_123", "cfile_upload")
        .unwrap();
    assert_eq!(retrieved.output.bytes, 23);

    let listed = client
        .containers()
        .files()
        .list(
            "cntr_123",
            ContainerFileListParams {
                after: Some(String::from("cfile_prev")),
                limit: Some(2),
                order: Some(ContainerFileOrder::Desc),
            },
        )
        .unwrap();
    assert_eq!(listed.output.data.len(), 2);
    assert_eq!(listed.output.next_after(), Some("cfile_copy"));
    assert!(listed.output.has_next_page());

    let content = client
        .containers()
        .files()
        .content("cntr_123", "cfile_upload")
        .unwrap();
    assert_eq!(content.output, b"\x00container-bytes{json:false}");

    let deleted = client
        .containers()
        .files()
        .delete("cntr_123", "cfile_upload")
        .unwrap();
    assert_eq!(
        deleted.output,
        ContainerFileDeleteResponse {
            id: String::from("cfile_upload"),
            object: String::from("container.file.deleted"),
            deleted: true,
            container_id: String::from("cntr_123"),
            extra: Default::default(),
        }
    );

    let requests = server.captured_requests(6).unwrap();
    assert_eq!(requests[0].path, "/v1/containers/cntr_123/files");
    let upload_content_type = requests[0].headers.get("content-type").unwrap();
    assert!(upload_content_type.starts_with("multipart/form-data; boundary="));
    let boundary = upload_content_type.split("boundary=").nth(1).unwrap();
    let multipart = multipart_support::parse_multipart(&requests[0].body, boundary).unwrap();
    let upload_part = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some("file"))
        .unwrap();
    assert_eq!(upload_part.filename.as_deref(), Some("uploaded.txt"));
    assert_eq!(
        upload_part.headers.get("content-type").map(String::as_str),
        Some("text/plain")
    );
    assert_eq!(upload_part.body, b"hello from the container");

    let copied_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(copied_body, json!({"file_id": "file_original"}));
    assert_eq!(
        requests[2].path,
        "/v1/containers/cntr_123/files/cfile_upload"
    );
    assert_eq!(
        requests[3].path,
        "/v1/containers/cntr_123/files?after=cfile_prev&limit=2&order=desc"
    );
    assert_eq!(
        requests[4].path,
        "/v1/containers/cntr_123/files/cfile_upload/content"
    );
    assert_eq!(
        requests[4].headers.get("accept").map(String::as_str),
        Some("application/binary")
    );
    assert_eq!(
        requests[5].path,
        "/v1/containers/cntr_123/files/cfile_upload"
    );
    assert_eq!(
        requests[5].headers.get("accept").map(String::as_str),
        Some("*/*")
    );

    let blank = client
        .containers()
        .files()
        .retrieve("cntr_123", " ")
        .unwrap_err();
    assert!(matches!(blank.kind, ErrorKind::Validation));
}

#[test]
fn delete_semantics_surface_typed_not_found_and_permission_errors() {
    let not_found = mock_http::MockHttpServer::spawn(not_found_response()).unwrap();
    let not_found_client = client(&not_found.url());
    let error = not_found_client
        .containers()
        .files()
        .delete("cntr_123", "cfile_missing")
        .unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Api(ApiErrorKind::NotFound)));
    assert_eq!(error.status_code(), Some(404));
    assert_eq!(
        error.api_error().unwrap().code.as_deref(),
        Some("not_found")
    );

    let denied = mock_http::MockHttpServer::spawn(permission_response()).unwrap();
    let denied_client = client(&denied.url());
    let error = denied_client
        .containers()
        .files()
        .delete("cntr_123", "cfile_denied")
        .unwrap_err();
    assert!(matches!(
        error.kind,
        ErrorKind::Api(ApiErrorKind::PermissionDenied)
    ));
    assert_eq!(error.status_code(), Some(403));
    assert_eq!(
        error.api_error().unwrap().message,
        "Project lacks entitlement for containers"
    );
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn container_file_payload(id: &str, path: &str, source: &str) -> String {
    json!({
        "id": id,
        "object": "container.file",
        "created_at": 1_717_171_717,
        "bytes": 23,
        "container_id": "cntr_123",
        "path": path,
        "source": source
    })
    .to_string()
}

fn container_files_page_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&container_file_payload("cfile_upload", "/mnt/data/uploaded.txt", "user")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&container_file_payload("cfile_copy", "/mnt/data/copied.txt", "assistant")).unwrap()
        ],
        "has_more": true
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
            (
                String::from("x-request-id"),
                String::from("req_container_file"),
            ),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn binary_response(body: &[u8]) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/binary"),
            ),
            (
                String::from("x-request-id"),
                String::from("req_container_content"),
            ),
        ],
        body: body.to_vec(),
        ..Default::default()
    }
}

fn not_found_response() -> mock_http::ScriptedResponse {
    let body = json!({
        "error": {
            "message": "No container file with that id",
            "type": "invalid_request_error",
            "code": "not_found"
        }
    })
    .to_string();
    mock_http::ScriptedResponse {
        status_code: 404,
        reason: "Not Found",
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (
                String::from("x-request-id"),
                String::from("req_container_not_found"),
            ),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn permission_response() -> mock_http::ScriptedResponse {
    let body = json!({
        "error": {
            "message": "Project lacks entitlement for containers",
            "type": "permission_error",
            "code": "containers_unavailable"
        }
    })
    .to_string();
    mock_http::ScriptedResponse {
        status_code: 403,
        reason: "Forbidden",
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (
                String::from("x-request-id"),
                String::from("req_container_denied"),
            ),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}
