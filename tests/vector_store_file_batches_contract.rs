#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

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
fn upload_and_poll_uploads_new_files_concurrently() {
    let server = ConcurrentUploadServer::spawn().unwrap();
    let base_url = server.url().to_string();
    let client = client(&base_url);

    let response = client
        .vector_stores()
        .file_batches()
        .upload_and_poll(
            "vs_123",
            VectorStoreFileBatchUploadAndPollParams {
                files: vec![
                    FileUpload::new("first.txt", "text/plain", b"first upload".to_vec()),
                    FileUpload::new("second.txt", "text/plain", b"second upload".to_vec()),
                ],
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

    let requests = server.captured_requests(5).unwrap();
    assert_eq!(requests[0].path, "/v1/files");
    assert_eq!(requests[1].path, "/v1/files");
    assert_eq!(requests[2].path, "/v1/vector_stores/vs_123/file_batches");
    assert!(
        requests[1].received_after < Duration::from_millis(60),
        "expected second upload request to start before the delayed first upload completed, got {:?}",
        requests
    );

    let batch_body: serde_json::Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(
        batch_body["file_ids"],
        json!(["file_existing", "file_first", "file_second"])
    );
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
    let base_url = server.url().to_string();
    let client = client(&base_url);

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

#[test]
fn upload_and_poll_waits_for_all_upload_workers_before_returning_error() {
    let server = MixedUploadFailureServer::spawn().unwrap();
    let client = client(server.url());
    let started = Instant::now();

    let error = client
        .vector_stores()
        .file_batches()
        .upload_and_poll(
            "vs_123",
            VectorStoreFileBatchUploadAndPollParams {
                files: vec![
                    FileUpload::new("fail.txt", "text/plain", b"fail upload".to_vec()),
                    FileUpload::new("slow.txt", "text/plain", b"slow upload".to_vec()),
                ],
                file_ids: vec![String::from("file_existing")],
            },
            VectorStoreFileBatchPollOptions {
                poll_interval: None,
                max_wait: Duration::from_secs(1),
            },
        )
        .unwrap_err();
    let elapsed = started.elapsed();

    assert!(!matches!(error.kind, ErrorKind::Validation));
    assert!(
        elapsed >= Duration::from_millis(120),
        "expected helper to wait for delayed upload worker before returning error, got {:?}",
        elapsed
    );

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(requests[0].path, "/v1/files");
    assert_eq!(requests[1].path, "/v1/files");
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

#[derive(Debug)]
struct ConcurrentUploadServer {
    url: String,
    captured: mpsc::Receiver<mock_http::CapturedRequest>,
    worker: Option<thread::JoinHandle<()>>,
}

#[derive(Debug)]
struct MixedUploadFailureServer {
    url: String,
    captured: mpsc::Receiver<mock_http::CapturedRequest>,
    worker: Option<thread::JoinHandle<()>>,
}

impl MixedUploadFailureServer {
    fn spawn() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let url = format!("http://{}", listener.local_addr()?);
        let (captured_tx, captured_rx) = mpsc::channel();
        let started_at = Instant::now();
        let worker = thread::spawn(move || {
            let mut handles = Vec::new();
            while handles.len() < 2 {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let captured_tx = captured_tx.clone();
                        handles.push(thread::spawn(move || {
                            handle_mixed_upload_failure_connection(stream, started_at, captured_tx)
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
            drop(captured_tx);
            for handle in handles {
                let _ = handle.join();
            }
        });

        Ok(Self {
            url,
            captured: captured_rx,
            worker: Some(worker),
        })
    }

    fn url(&self) -> &str {
        &self.url
    }

    fn captured_requests(&self, count: usize) -> Option<Vec<mock_http::CapturedRequest>> {
        let mut requests = Vec::with_capacity(count);
        for _ in 0..count {
            requests.push(self.captured.recv_timeout(Duration::from_secs(2)).ok()?);
        }
        Some(requests)
    }
}

impl Drop for MixedUploadFailureServer {
    fn drop(&mut self) {
        let _ = TcpStream::connect(self.url.trim_start_matches("http://"));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl ConcurrentUploadServer {
    fn spawn() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let url = format!("http://{}", listener.local_addr()?);
        let (captured_tx, captured_rx) = mpsc::channel();
        let started_at = Instant::now();
        let poll_count = Arc::new(Mutex::new(0_u8));
        let worker = thread::spawn({
            let poll_count = Arc::clone(&poll_count);
            move || {
                let mut handles = Vec::new();
                while handles.len() < 5 {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let captured_tx = captured_tx.clone();
                            let poll_count = Arc::clone(&poll_count);
                            handles.push(thread::spawn(move || {
                                handle_concurrent_upload_connection(
                                    stream,
                                    started_at,
                                    captured_tx,
                                    poll_count,
                                )
                            }));
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
                drop(captured_tx);
                for handle in handles {
                    let _ = handle.join();
                }
            }
        });

        Ok(Self {
            url,
            captured: captured_rx,
            worker: Some(worker),
        })
    }

    fn url(&self) -> &str {
        &self.url
    }

    fn captured_requests(&self, count: usize) -> Option<Vec<mock_http::CapturedRequest>> {
        let mut requests = Vec::with_capacity(count);
        for _ in 0..count {
            requests.push(self.captured.recv_timeout(Duration::from_secs(2)).ok()?);
        }
        Some(requests)
    }
}

impl Drop for ConcurrentUploadServer {
    fn drop(&mut self) {
        let _ = TcpStream::connect(self.url.trim_start_matches("http://"));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn handle_concurrent_upload_connection(
    mut stream: TcpStream,
    started_at: Instant,
    captured_tx: mpsc::Sender<mock_http::CapturedRequest>,
    poll_count: Arc<Mutex<u8>>,
) {
    let request = read_request(&mut stream, started_at).unwrap();
    let _ = captured_tx.send(request.clone());

    let (body, extra_headers, delay) = match request.path.as_str() {
        "/v1/files" => {
            let body_text = String::from_utf8_lossy(&request.body);
            if body_text.contains("filename=\"first.txt\"") {
                (
                    file_payload("file_first"),
                    vec![
                        (
                            String::from("content-length"),
                            file_payload("file_first").len().to_string(),
                        ),
                        (
                            String::from("content-type"),
                            String::from("application/json"),
                        ),
                    ],
                    Duration::from_millis(120),
                )
            } else {
                (
                    file_payload("file_second"),
                    vec![
                        (
                            String::from("content-length"),
                            file_payload("file_second").len().to_string(),
                        ),
                        (
                            String::from("content-type"),
                            String::from("application/json"),
                        ),
                    ],
                    Duration::ZERO,
                )
            }
        }
        "/v1/vector_stores/vs_123/file_batches" => {
            let body = vector_store_file_batch_payload(
                "vsfb_upload",
                "in_progress",
                counts(2, 0, 0, 0, 2),
            );
            (
                body.clone(),
                vec![
                    (String::from("content-length"), body.len().to_string()),
                    (
                        String::from("content-type"),
                        String::from("application/json"),
                    ),
                ],
                Duration::ZERO,
            )
        }
        "/v1/vector_stores/vs_123/file_batches/vsfb_upload" => {
            let mut poll_count = poll_count.lock().unwrap();
            let body = if *poll_count == 0 {
                *poll_count += 1;
                vector_store_file_batch_payload("vsfb_upload", "in_progress", counts(2, 0, 0, 0, 2))
            } else {
                vector_store_file_batch_payload("vsfb_upload", "completed", counts(0, 2, 0, 0, 2))
            };
            let mut headers = vec![
                (String::from("content-length"), body.len().to_string()),
                (
                    String::from("content-type"),
                    String::from("application/json"),
                ),
            ];
            if *poll_count == 1 {
                headers.push((String::from("openai-poll-after-ms"), String::from("10")));
            }
            (body, headers, Duration::ZERO)
        }
        _ => return,
    };

    if !delay.is_zero() {
        thread::sleep(delay);
    }
    let _ = write_http_response(&mut stream, body.as_bytes(), &extra_headers);
}

fn handle_mixed_upload_failure_connection(
    mut stream: TcpStream,
    started_at: Instant,
    captured_tx: mpsc::Sender<mock_http::CapturedRequest>,
) {
    let request = read_request(&mut stream, started_at).unwrap();
    let _ = captured_tx.send(request.clone());

    if request.path != "/v1/files" {
        return;
    }

    let body_text = String::from_utf8_lossy(&request.body);
    if body_text.contains("filename=\"slow.txt\"") {
        thread::sleep(Duration::from_millis(150));
        let body = file_payload("file_slow");
        let headers = vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
        ];
        let _ = write_http_response(&mut stream, body.as_bytes(), &headers);
        return;
    }

    let body = json!({
        "error": {
            "message": "upload failed",
            "type": "invalid_request_error"
        }
    })
    .to_string();
    let headers = vec![
        (String::from("content-length"), body.len().to_string()),
        (
            String::from("content-type"),
            String::from("application/json"),
        ),
    ];
    let _ =
        write_http_response_with_status(&mut stream, "400 Bad Request", body.as_bytes(), &headers);
}

fn read_request(
    stream: &mut TcpStream,
    started_at: Instant,
) -> std::io::Result<mock_http::CapturedRequest> {
    let mut buffer = Vec::new();
    let mut header_end = None;
    loop {
        let mut chunk = [0_u8; 1024];
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            header_end = Some(position);
            break;
        }
    }

    let Some(header_end) = header_end else {
        return Ok(mock_http::CapturedRequest::default());
    };
    let body_start = header_end + 4;
    let header_text = String::from_utf8_lossy(&buffer[..body_start]);
    let mut lines = header_text.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut headers = BTreeMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    while buffer.len().saturating_sub(body_start) < content_length {
        let mut chunk = [0_u8; 1024];
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
    }

    Ok(mock_http::CapturedRequest {
        method,
        path,
        headers,
        body: buffer[body_start..].to_vec(),
        received_after: started_at.elapsed(),
    })
}

fn write_http_response(
    stream: &mut TcpStream,
    body: &[u8],
    headers: &[(String, String)],
) -> std::io::Result<()> {
    write_http_response_with_status(stream, "200 OK", body, headers)
}

fn write_http_response_with_status(
    stream: &mut TcpStream,
    status: &str,
    body: &[u8],
    headers: &[(String, String)],
) -> std::io::Result<()> {
    let mut response_bytes = format!("HTTP/1.1 {status}\r\n").into_bytes();
    for (name, value) in headers {
        response_bytes.extend_from_slice(format!("{}: {}\r\n", name, value).as_bytes());
    }
    response_bytes.extend_from_slice(b"connection: close\r\n\r\n");
    response_bytes.extend_from_slice(body);
    stream.write_all(&response_bytes)?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)?;
    Ok(())
}
