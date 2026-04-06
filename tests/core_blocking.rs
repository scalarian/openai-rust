#![cfg(feature = "blocking")]

mod support;

use openai_rust::{
    OpenAI,
    blocking::OpenAI as BlockingOpenAI,
    resources::{
        responses::{ResponseCreateParams, ResponseStreamEvent},
        uploads::{ChunkedUploadSource, UploadChunkedParams, UploadPurpose},
    },
};
use serde_json::json;

#[test]
fn blocking_responses_create_matches_async_results_and_metadata() {
    let async_server =
        support::mock_http::MockHttpServer::spawn(json_response(response_payload("resp_async")))
            .unwrap();
    let blocking_server =
        support::mock_http::MockHttpServer::spawn(json_response(response_payload("resp_blocking")))
            .unwrap();

    let async_client = async_client(&async_server.url());
    let blocking_client = blocking_client(&blocking_server.url());
    let params = ResponseCreateParams {
        model: String::from("gpt-4.1-mini"),
        input: Some(json!("say hi")),
        ..Default::default()
    };

    let async_response = async_client.responses().create(params.clone()).unwrap();
    let blocking_response = blocking_client.responses().create(params).unwrap();

    assert_eq!(async_response.output.output_text(), "Hello world!");
    assert_eq!(blocking_response.output.output_text(), "Hello world!");
    assert_eq!(
        async_response.output.output_text(),
        blocking_response.output.output_text()
    );
    assert_eq!(
        async_response.metadata.status_code,
        blocking_response.metadata.status_code
    );
    assert_eq!(
        async_response.metadata.request_id,
        blocking_response.metadata.request_id
    );
    assert_eq!(async_response.header("x-custom-meta"), Some("present"));
    assert_eq!(
        async_response.header("x-custom-meta"),
        blocking_response.header("x-custom-meta")
    );

    let async_request = async_server.captured_request().unwrap();
    let blocking_request = blocking_server.captured_request().unwrap();
    assert_eq!(async_request.method, blocking_request.method);
    assert_eq!(async_request.path, blocking_request.path);
    assert_eq!(async_request.body, blocking_request.body);
}

#[test]
fn blocking_stream_consumption_matches_async_terminal_output() {
    let async_server =
        support::mock_http::MockHttpServer::spawn(sse_response(text_stream())).unwrap();
    let blocking_server =
        support::mock_http::MockHttpServer::spawn(sse_response(text_stream())).unwrap();

    let async_client = async_client(&async_server.url());
    let blocking_client = blocking_client(&blocking_server.url());
    let params = ResponseCreateParams {
        model: String::from("gpt-4.1-mini"),
        input: Some(json!("say hi")),
        ..Default::default()
    };

    let mut async_stream = async_client.responses().stream(params.clone()).unwrap();
    let mut blocking_stream = blocking_client.responses().stream(params).unwrap();

    assert!(matches!(
        async_stream.next_event(),
        Some(ResponseStreamEvent::Created { .. })
    ));
    assert!(matches!(
        blocking_stream.next_event(),
        Some(ResponseStreamEvent::Created { .. })
    ));
    assert!(matches!(
        async_stream.next_event(),
        Some(ResponseStreamEvent::OutputTextDelta { ref delta, .. }) if delta == "Hello"
    ));
    assert!(matches!(
        blocking_stream.next_event(),
        Some(ResponseStreamEvent::OutputTextDelta { ref delta, .. }) if delta == "Hello"
    ));

    let async_final = async_stream.final_response().unwrap().clone();
    let blocking_final = blocking_stream.final_response().unwrap().clone();
    assert_eq!(async_final.output_text(), blocking_final.output_text());
    assert_eq!(async_stream.metadata(), blocking_stream.metadata());
}

#[test]
fn blocking_chunked_upload_matches_async_request_sequence() {
    let async_server = support::mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(upload_payload("upload_async", "pending")),
        json_response(part_payload("part-1")),
        json_response(part_payload("part-2")),
        json_response(completed_payload(
            "upload_async",
            "memory.bin",
            vec!["part-1", "part-2"],
        )),
    ])
    .unwrap();
    let blocking_server = support::mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(upload_payload("upload_blocking", "pending")),
        json_response(part_payload("part-1")),
        json_response(part_payload("part-2")),
        json_response(completed_payload(
            "upload_blocking",
            "memory.bin",
            vec!["part-1", "part-2"],
        )),
    ])
    .unwrap();

    let params = UploadChunkedParams {
        source: ChunkedUploadSource::InMemory {
            bytes: b"abcdef".to_vec(),
            filename: Some(String::from("memory.bin")),
            byte_length: Some(6),
        },
        mime_type: String::from("application/octet-stream"),
        purpose: UploadPurpose::Assistants,
        part_size: Some(3),
        md5: Some(String::from("memory-md5")),
    };

    let async_response = async_client(&async_server.url())
        .uploads()
        .upload_file_chunked(params.clone())
        .unwrap();
    let blocking_response = blocking_client(&blocking_server.url())
        .uploads()
        .upload_file_chunked(params)
        .unwrap();

    assert_eq!(
        async_response.output.status,
        blocking_response.output.status
    );
    assert_eq!(
        async_response.metadata.status_code,
        blocking_response.metadata.status_code
    );
    let async_requests = async_server.captured_requests(4).unwrap();
    let blocking_requests = blocking_server.captured_requests(4).unwrap();

    assert_eq!(async_requests[0].method, blocking_requests[0].method);
    assert_eq!(async_requests[0].path, blocking_requests[0].path);
    assert_eq!(async_requests[0].body, blocking_requests[0].body);

    assert!(async_requests[1].path.ends_with("/parts"));
    assert!(blocking_requests[1].path.ends_with("/parts"));
    assert_eq!(multipart_data(&async_requests[1]), b"abc");
    assert_eq!(multipart_data(&blocking_requests[1]), b"abc");
    assert_eq!(multipart_data(&async_requests[2]), b"def");
    assert_eq!(multipart_data(&blocking_requests[2]), b"def");

    assert!(async_requests[3].path.ends_with("/complete"));
    assert!(blocking_requests[3].path.ends_with("/complete"));
    assert_eq!(async_requests[3].body, blocking_requests[3].body);
}

fn multipart_data(request: &support::mock_http::CapturedRequest) -> Vec<u8> {
    let content_type = request.headers.get("content-type").unwrap();
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    let multipart = support::multipart::parse_multipart(&request.body, boundary).unwrap();
    multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some("data"))
        .unwrap()
        .body
        .clone()
}

fn async_client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .max_retries(0)
        .build()
}

fn blocking_client(base_url: &str) -> BlockingOpenAI {
    BlockingOpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .max_retries(0)
        .build()
}

fn json_response(body: String) -> support::mock_http::ScriptedResponse {
    support::mock_http::ScriptedResponse {
        status_code: 200,
        reason: "OK",
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
            (String::from("x-request-id"), String::from("req_blocking")),
            (String::from("x-custom-meta"), String::from("present")),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn sse_response(body: String) -> support::mock_http::ScriptedResponse {
    support::mock_http::ScriptedResponse {
        status_code: 200,
        reason: "OK",
        headers: vec![
            (
                String::from("content-type"),
                String::from("text/event-stream"),
            ),
            (String::from("content-length"), body.len().to_string()),
            (String::from("x-request-id"), String::from("req_stream")),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn response_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "response",
        "created_at": 1,
        "status": "completed",
        "output": [{
            "id": "msg_1",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "output_text", "text": "Hello "},
                {"type": "output_text", "text": "world!"}
            ]
        }],
        "usage": {}
    })
    .to_string()
}

fn text_stream() -> String {
    concat!(
        r#"event: response.created
"#,
        r#"data: {"id":"resp_stream","object":"response","created_at":1,"status":"in_progress","output":[{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"output_text","text":""}]}],"usage":{},"sequence_number":1}

"#,
        r#"event: response.output_text.delta
"#,
        r#"data: {"output_index":0,"content_index":0,"delta":"Hello","sequence_number":2}

"#,
        r#"event: response.output_text.done
"#,
        r#"data: {"output_index":0,"content_index":0,"text":"Hello world","sequence_number":3}

"#,
        r#"event: response.completed
"#,
        r#"data: {"id":"resp_stream","object":"response","created_at":1,"status":"completed","output":[{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello world"}]}],"usage":{},"sequence_number":4}

"#,
        r#"data: [DONE]

"#
    )
    .to_string()
}

fn upload_payload(id: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "upload",
        "bytes": 6,
        "created_at": 1,
        "expires_at": 2,
        "filename": "memory.bin",
        "purpose": "assistants",
        "status": status
    })
    .to_string()
}

fn part_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "upload.part",
        "created_at": 1,
        "upload_id": "upload"
    })
    .to_string()
}

fn completed_payload(id: &str, filename: &str, part_ids: Vec<&str>) -> String {
    json!({
        "id": id,
        "object": "upload",
        "bytes": 6,
        "created_at": 1,
        "expires_at": 2,
        "filename": filename,
        "purpose": "assistants",
        "status": "completed",
        "part_ids": part_ids
    })
    .to_string()
}
