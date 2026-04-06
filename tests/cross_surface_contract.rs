#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/cross_surface.rs"]
mod cross_surface;

use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use openai_rust::{
    ErrorKind, OpenAI,
    core::metadata::ResponseMetadata,
    error::ApiErrorKind,
    realtime::{
        RealtimeAuth, RealtimeConnectOptions, RealtimeServerEvent, RealtimeSessionConfig,
        RealtimeSessionTTL,
    },
    resources::{
        chat::ChatCompletionCreateParams,
        files::{FileCreateParams, FilePurpose, FileUpload},
        multimodal::{ResponseInputMessage, ResponseInputPart},
        responses::{
            ResponseCreateParams, ResponseFormatTextConfig, ResponseFormatTextJSONSchemaConfig,
            ResponseParseParams, ResponseTextConfig,
        },
        webhooks::{WebhookEvent, WebhookHeaders},
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener as TokioTcpListener,
    sync::oneshot,
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        Message,
        handshake::server::{Request as WsRequest, Response as WsResponse},
    },
};

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct Scorecard {
    winner: String,
    score: u32,
}

static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

#[test]
fn env_loaded_client_reaches_multiple_subsystems_without_per_endpoint_reconfiguration() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(response_payload(
            "resp_env",
            r#"{"winner":"Dodgers","score":4}"#,
        ))
        .with_request_id("req_resp_env"),
        json_response(chat_completion_payload("chatcmpl_env", "compat hello"))
            .with_request_id("req_chat_env"),
        octet_stream_response(b"binary-cross-surface".to_vec()).with_request_id("req_file_env"),
    ])
    .unwrap();

    with_env(
        &[
            ("OPENAI_API_KEY", Some("sk-env-cross")),
            ("OPENAI_BASE_URL", Some(server.url().as_str())),
            ("OPENAI_ORG_ID", Some("org_env_cross")),
            ("OPENAI_PROJECT_ID", Some("proj_env_cross")),
        ],
        || {
            let client = OpenAI::new();

            let response = client
                .responses()
                .create(ResponseCreateParams {
                    model: String::from("gpt-4.1-mini"),
                    input: Some(json!("Say hi")),
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(response.output.id, "resp_env");

            let chat = client
                .chat()
                .completions()
                .create(ChatCompletionCreateParams {
                    model: String::from("gpt-4.1-mini"),
                    messages: vec![json!({"role": "user", "content": "Say hi"})],
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(chat.output.id, "chatcmpl_env");

            let binary = client.files().content("file_env_asset").unwrap();
            assert_eq!(binary.output, b"binary-cross-surface");

            let requests = server.captured_requests(3).expect("captured requests");
            assert_eq!(requests[0].path, "/v1/responses");
            assert_eq!(requests[1].path, "/v1/chat/completions");
            assert_eq!(requests[2].path, "/v1/files/file_env_asset/content");

            for request in requests {
                assert_eq!(
                    request.headers.get("authorization").map(String::as_str),
                    Some("Bearer sk-env-cross")
                );
                assert_eq!(
                    request
                        .headers
                        .get("openai-organization")
                        .map(String::as_str),
                    Some("org_env_cross")
                );
                assert_eq!(
                    request.headers.get("openai-project").map(String::as_str),
                    Some("proj_env_cross")
                );
            }
        },
    );
}

#[test]
fn request_ids_and_typed_errors_stay_consistent_across_result_forms() {
    let object_server = mock_http::MockHttpServer::spawn(
        json_response(response_payload(
            "resp_object",
            r#"{"winner":"Dodgers","score":4}"#,
        ))
        .with_request_id("req_object"),
    )
    .unwrap();
    let object_client = client(&object_server.url());
    let object_response = object_client
        .responses()
        .create(ResponseCreateParams {
            model: String::from("gpt-4.1-mini"),
            input: Some(json!("score update")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(object_response.request_id(), Some("req_object"));
    assert_eq!(object_response.status_code(), 200);

    let bytes_server = mock_http::MockHttpServer::spawn(
        octet_stream_response(b"exact-byte-stream".to_vec()).with_request_id("req_bytes"),
    )
    .unwrap();
    let bytes_client = client(&bytes_server.url());
    let bytes_response = bytes_client.files().content("file_bytes").unwrap();
    assert_eq!(bytes_response.request_id(), Some("req_bytes"));
    assert_eq!(bytes_response.output, b"exact-byte-stream");

    let error_server = mock_http::MockHttpServer::spawn(api_error_response(
        429,
        json!({
            "error": {
                "message": "too many requests",
                "type": "rate_limit_error",
                "code": "rate_limited",
                "param": null
            }
        }),
        "req_error",
    ))
    .unwrap();
    let error_client = client(&error_server.url());
    let error = error_client
        .batches()
        .retrieve("batch_rate_limited")
        .expect_err("rate limit should surface as typed API error");
    assert_eq!(error.kind, ErrorKind::Api(ApiErrorKind::RateLimit));
    assert_eq!(error.status_code(), Some(429));
    assert_eq!(error.request_id(), Some("req_error"));
    assert_eq!(error.header("content-type"), Some("application/json"));

    let stream_server = IncrementalSseServer::spawn(
        concat!(
            "event: response.created\n",
            "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Hel\"}\n\n",
        ),
        concat!(
            "event: response.output_text.done\n",
            "data: {\"output_index\":0,\"content_index\":0,\"text\":\"Hello\"}\n\n",
            "event: response.completed\n",
            "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello\"}]}],\"usage\":{}}\n\n",
            "data: [DONE]\n\n"
        ),
        Duration::from_millis(300),
        "req_stream",
    )
    .unwrap();
    let stream_client = client(&stream_server.url());
    let mut stream = stream_client
        .responses()
        .stream(ResponseCreateParams {
            model: String::from("gpt-4.1-mini"),
            input: Some(json!("Say hello")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(stream.metadata().request_id(), Some("req_stream"));
    assert!(stream.next_event().is_some());
    assert!(stream.next_event().is_some());
    let final_response = stream.final_response().unwrap();
    assert_eq!(final_response.id, "resp_stream");
    assert_eq!(final_response.output_text(), "Hello");
}

#[test]
fn structured_streaming_converges_on_the_same_final_typed_result_as_non_stream_parse() {
    let parse_server = mock_http::MockHttpServer::spawn(json_response(response_payload(
        "resp_parse",
        r#"{"winner":"Dodgers","score":4}"#,
    )))
    .unwrap();
    let client = client(&parse_server.url());
    let parsed = client
        .responses()
        .parse::<Scorecard>(parse_params())
        .expect("non-stream structured parse");
    assert_eq!(
        parsed.output().output_parsed(),
        Some(&Scorecard {
            winner: String::from("Dodgers"),
            score: 4,
        })
    );

    let metadata = ResponseMetadata {
        status_code: 200,
        request_id: Some(String::from("req_structured_stream")),
        ..Default::default()
    };
    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_streamed\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"{\\\"winner\\\":\\\"Dodgers\\\",\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"\\\"score\\\":4}\"}\n\n",
        "event: response.output_text.done\n",
        "data: {\"output_index\":0,\"content_index\":0,\"text\":\"{\\\"winner\\\":\\\"Dodgers\\\",\\\"score\\\":4}\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_streamed\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"{\\\"winner\\\":\\\"Dodgers\\\",\\\"score\\\":4}\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );
    let mut stream =
        openai_rust::resources::responses::ResponseStream::from_sse_chunks(metadata, [transcript])
            .unwrap();
    let streamed = stream
        .parse_final::<Scorecard>(parse_text_config(), &[])
        .expect("streamed structured parse");
    assert_eq!(
        streamed.output_parsed(),
        Some(&Scorecard {
            winner: String::from("Dodgers"),
            score: 4,
        })
    );
}

#[test]
fn file_ids_flow_directly_into_input_file_without_manual_identifier_rewriting() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(file_object_payload("file_doc_123")).with_request_id("req_file_create"),
        json_response(response_payload("resp_file_chain", "ingested file"))
            .with_request_id("req_response_create"),
    ])
    .unwrap();
    let client = client(&server.url());

    let file = client
        .files()
        .create(FileCreateParams {
            file: FileUpload::new("brief.txt", "text/plain", b"brief".to_vec()),
            purpose: FilePurpose::UserData,
            expires_after: None,
        })
        .unwrap();

    let response = client
        .responses()
        .create(
            ResponseCreateParams {
                model: String::from("gpt-4.1-mini"),
                ..Default::default()
            }
            .with_serialized_input(vec![ResponseInputMessage::user(vec![
                ResponseInputPart::input_text("Summarize the uploaded brief"),
                ResponseInputPart::input_file_id(file.output.id.clone()),
            ])])
            .unwrap(),
        )
        .unwrap();
    assert_eq!(response.output.id, "resp_file_chain");

    let requests = server.captured_requests(2).unwrap();
    let request_body: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(
        request_body["input"][0]["content"][1]["type"],
        Value::String(String::from("input_file"))
    );
    assert_eq!(
        request_body["input"][0]["content"][1]["file_id"],
        Value::String(String::from("file_doc_123"))
    );
}

#[test]
fn rest_configuration_bootstraps_realtime_without_extra_glue() {
    let server = mock_http::MockHttpServer::spawn(
        json_response(json!({
            "client_secret": {
                "value": "ek_cross_surface_secret",
                "expires_at": 4_102_444_800_i64
            },
            "session": {
                "type": "realtime",
                "model": "gpt-realtime-mini"
            }
        }))
        .with_request_id("req_realtime_secret"),
    )
    .unwrap();
    let client = with_env_result(
        &[
            ("OPENAI_API_KEY", Some("sk-env-realtime")),
            ("OPENAI_BASE_URL", Some(server.url().as_str())),
            ("OPENAI_ORG_ID", Some("org_realtime")),
            ("OPENAI_PROJECT_ID", Some("proj_realtime")),
        ],
        OpenAI::new,
    );

    let secret = client
        .realtime()
        .client_secrets()
        .create(openai_rust::realtime::RealtimeClientSecretCreateParams {
            expires_after: Some(RealtimeSessionTTL {
                anchor: String::from("created_at"),
                seconds: 60,
            }),
            session: Some(RealtimeSessionConfig {
                model: Some(String::from("gpt-realtime-mini")),
                ..Default::default()
            }),
        })
        .unwrap();
    assert_eq!(secret.request_id(), Some("req_realtime_secret"));

    let target = client
        .realtime()
        .prepare_ws_target(RealtimeConnectOptions {
            model: Some(String::from("gpt-realtime-mini")),
            auth: Some(RealtimeAuth::client_secret(
                secret.output.client_secret.value.clone(),
            )),
            ..Default::default()
        })
        .unwrap();

    let rest_request = server
        .captured_request()
        .expect("captured realtime request");
    assert_eq!(
        rest_request
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer sk-env-realtime")
    );
    assert_eq!(
        rest_request
            .headers
            .get("openai-organization")
            .map(String::as_str),
        Some("org_realtime")
    );
    assert_eq!(
        rest_request
            .headers
            .get("openai-project")
            .map(String::as_str),
        Some("proj_realtime")
    );
    assert_eq!(
        target.headers.get("authorization").map(String::as_str),
        Some("Bearer ek_cross_surface_secret")
    );
    assert_eq!(
        target
            .headers
            .get("openai-organization")
            .map(String::as_str),
        Some("org_realtime")
    );
    assert_eq!(
        target.headers.get("openai-project").map(String::as_str),
        Some("proj_realtime")
    );
    assert_eq!(
        target.url,
        "ws://".to_string()
            + server.url().trim_start_matches("http://")
            + "/realtime?model=gpt-realtime-mini"
    );
}

#[test]
fn verified_webhook_fixtures_preserve_event_and_resource_identifiers_for_routing() {
    let client = OpenAI::builder().webhook_secret("test-secret").build();
    let response_body = br#"{"id":"evt_response_completed","created_at":1,"type":"response.completed","data":{"id":"resp_123"}}"#;
    let batch_body = br#"{"id":"evt_batch_completed","created_at":2,"type":"batch.completed","data":{"id":"batch_456"}}"#;

    let response_headers =
        signed_headers("test-secret", "wh_response", now_seconds(), response_body);
    let batch_headers = signed_headers("test-secret", "wh_batch", now_seconds(), batch_body);

    let response_event = client
        .webhooks()
        .unwrap(response_body, &response_headers)
        .unwrap();
    let batch_event = client
        .webhooks()
        .unwrap(batch_body, &batch_headers)
        .unwrap();

    assert_eq!(response_event.event_type(), "response.completed");
    assert_eq!(response_event.event_id(), "evt_response_completed");
    assert_eq!(response_event.resource_id(), "resp_123");

    assert_eq!(batch_event.event_type(), "batch.completed");
    assert_eq!(batch_event.event_id(), "evt_batch_completed");
    assert_eq!(batch_event.resource_id(), "batch_456");

    assert!(matches!(response_event, WebhookEvent::ResponseCompleted(_)));
    assert!(matches!(batch_event, WebhookEvent::BatchCompleted(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_cross_surface_report_matches_publish_ready_equivalence_baseline() {
    let report = mock_publish_ready_equivalence_report().await;
    assert_eq!(report, cross_surface::expected_publish_ready_equivalence_baseline());
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("serialize mock cross-surface report")
    );
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .max_retries(0)
        .build()
}

fn parse_params() -> ResponseParseParams {
    ResponseParseParams {
        model: String::from("gpt-4.1-mini"),
        input: Some(json!("who won?")),
        text: parse_text_config(),
        ..Default::default()
    }
}

fn parse_text_config() -> Option<ResponseTextConfig> {
    Some(ResponseTextConfig {
        format: Some(ResponseFormatTextConfig::JsonSchema(
            ResponseFormatTextJSONSchemaConfig {
                name: String::from("scorecard"),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "winner": {"type": "string"},
                        "score": {"type": "integer"}
                    },
                    "required": ["winner", "score"],
                    "additionalProperties": false
                }),
                description: None,
                strict: Some(true),
            },
        )),
        verbosity: None,
    })
}

fn response_payload(id: &str, output_text: &str) -> Value {
    let parsed_json: Value =
        serde_json::from_str(output_text).unwrap_or_else(|_| json!(output_text));
    json!({
        "id": id,
        "object": "response",
        "created_at": 1,
        "status": "completed",
        "output": [
            {
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "content": [
                    {
                        "type": "output_text",
                        "text": output_text
                    }
                ]
            },
            {
                "id": "fc_1",
                "type": "function_call",
                "name": "lookup_box_score",
                "arguments": "{\"game_id\":7}"
            }
        ],
        "usage": {},
        "text": {
            "format": {
                "type": "json_schema",
                "name": "scorecard",
                "schema": {
                    "type": "object"
                }
            }
        },
        "output_parsed": parsed_json
    })
}

fn chat_completion_payload(id: &str, content: &str) -> Value {
    json!({
        "id": id,
        "object": "chat.completion",
        "created": 1,
        "model": "gpt-4.1-mini",
        "choices": [
            {
                "index": 0,
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": content
                }
            }
        ],
        "usage": {
            "prompt_tokens": 1,
            "completion_tokens": 1,
            "total_tokens": 2
        }
    })
}

fn file_object_payload(id: &str) -> Value {
    file_object_payload_with_status(id, "processed")
}

fn file_object_payload_with_status(id: &str, status: &str) -> Value {
    json!({
        "id": id,
        "object": "file",
        "bytes": 5,
        "created_at": 1,
        "filename": "brief.txt",
        "purpose": "user_data",
        "status": status
    })
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

fn api_error_response(
    status_code: u16,
    body: Value,
    request_id: &str,
) -> mock_http::ScriptedResponse {
    let mut response = json_response(body);
    response.status_code = status_code;
    response.reason = "Too Many Requests";
    response
        .headers
        .push((String::from("x-request-id"), String::from(request_id)));
    response
}

fn octet_stream_response(body: Vec<u8>) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/octet-stream"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body,
        ..Default::default()
    }
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

struct IncrementalSseServer {
    addr: std::net::SocketAddr,
    worker: Option<thread::JoinHandle<()>>,
}

impl IncrementalSseServer {
    fn spawn(
        first_chunk: &'static str,
        second_chunk: &'static str,
        delay: Duration,
        request_id: &'static str,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let worker = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = read_request_headers(&mut stream);
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nx-request-id: {request_id}\r\nconnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                );
                let _ = stream.write_all(first_chunk.as_bytes());
                let _ = stream.flush();
                thread::sleep(delay);
                let _ = stream.write_all(second_chunk.as_bytes());
                let _ = stream.flush();
                let _ = stream.shutdown(Shutdown::Both);
            }
        });
        Ok(Self {
            addr,
            worker: Some(worker),
        })
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for IncrementalSseServer {
    fn drop(&mut self) {
        let _ = TcpStream::connect(self.addr);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_request_headers(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0_u8; 1024];
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    Ok(())
}

fn signed_headers(secret: &str, webhook_id: &str, timestamp: i64, body: &[u8]) -> WebhookHeaders {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{webhook_id}.{timestamp}.").as_bytes());
    mac.update(body);
    let signature = STANDARD.encode(mac.finalize().into_bytes());
    WebhookHeaders::from_pairs([
        ("webhook-id", webhook_id.to_string()),
        ("webhook-timestamp", timestamp.to_string()),
        ("webhook-signature", format!("v1,{signature}")),
    ])
}

fn now_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

async fn mock_publish_ready_equivalence_report() -> cross_surface::NormalizedCrossSurfaceReport {
    let surface_server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(response_payload(
            "resp_publish_ready",
            r#"{"winner":"Dodgers","score":4}"#,
        ))
        .with_request_id("req_publish_ready_response"),
        json_response(chat_completion_payload("chatcmpl_publish_ready", "hi"))
            .with_request_id("req_publish_ready_chat"),
        json_response(file_object_payload_with_status(
            "file_publish_ready",
            "uploaded",
        ))
        .with_request_id("req_publish_ready_file"),
    ])
    .expect("mock surface server");

    let (response, chat, file, surface_requests) = with_env_result(
        &[
            ("OPENAI_API_KEY", Some("sk-env-publish-ready")),
            ("OPENAI_BASE_URL", Some(surface_server.url().as_str())),
            ("OPENAI_ORG_ID", Some("org_publish_ready")),
            ("OPENAI_PROJECT_ID", Some("proj_publish_ready")),
        ],
        || {
            let client = OpenAI::new();

            let response = client
                .responses()
                .create(ResponseCreateParams {
                    model: String::from("gpt-4.1-mini"),
                    input: Some(json!("Reply with exactly hi.")),
                    ..Default::default()
                })
                .expect("mock responses.create");
            let chat = client
                .chat()
                .completions()
                .create(ChatCompletionCreateParams {
                    model: String::from("gpt-4.1-mini"),
                    messages: vec![json!({
                        "role": "user",
                        "content": "Reply with exactly hi."
                    })],
                    ..Default::default()
                })
                .expect("mock chat.completions.create");
            let file = client
                .files()
                .create(FileCreateParams {
                    file: FileUpload::new(
                        "cross-surface-live.txt",
                        "text/plain",
                        b"cross-surface live smoke".to_vec(),
                    ),
                    purpose: FilePurpose::UserData,
                    expires_after: None,
                })
                .expect("mock files.create");
            let requests = surface_server
                .captured_requests(3)
                .expect("captured mock surface requests");
            (response, chat, file, requests)
        },
    );

    assert_eq!(surface_requests[0].path, "/v1/responses");
    assert_eq!(surface_requests[1].path, "/v1/chat/completions");
    assert_eq!(surface_requests[2].path, "/v1/files");
    for request in &surface_requests {
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer sk-env-publish-ready")
        );
        assert_eq!(
            request
                .headers
                .get("openai-organization")
                .map(String::as_str),
            Some("org_publish_ready")
        );
        assert_eq!(
            request.headers.get("openai-project").map(String::as_str),
            Some("proj_publish_ready")
        );
    }

    let realtime_trace = mock_realtime_bootstrap_trace().await;

    cross_surface::NormalizedCrossSurfaceReport {
        entries: vec![
            cross_surface::normalized_entry(
                "responses.create",
                "success",
                metadata_shape(response.request_id()),
                "completed",
                Vec::<String>::new(),
            ),
            cross_surface::normalized_entry(
                "chat.completions.create",
                "success",
                metadata_shape(chat.request_id()),
                normalize_chat_finish_reason(
                    chat.output.choices[0].finish_reason.as_deref(),
                ),
                Vec::<String>::new(),
            ),
            cross_surface::normalized_entry(
                "files.create",
                "success",
                metadata_shape(file.request_id()),
                normalize_file_status(file.output.status.as_ref()),
                Vec::<String>::new(),
            ),
            cross_surface::normalized_entry(
                "realtime.client_secrets.create + ws bootstrap",
                "success",
                metadata_shape(Some(realtime_trace.secret_request_id.as_str())),
                "session_created",
                realtime_trace.event_ordering,
            ),
        ],
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rest_configuration_bootstraps_realtime_through_websocket_hop() {
    let trace = mock_realtime_bootstrap_trace().await;
    assert_eq!(trace.rest_request.path, "/v1/realtime/client_secrets");
    assert_eq!(
        trace
            .rest_request
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer sk-env-realtime")
    );
    assert_eq!(
        trace
            .rest_request
            .headers
            .get("openai-organization")
            .map(String::as_str),
        Some("org_realtime")
    );
    assert_eq!(
        trace
            .rest_request
            .headers
            .get("openai-project")
            .map(String::as_str),
        Some("proj_realtime")
    );
    assert_eq!(
        trace.ws_handshake.path,
        "/v1/realtime?model=gpt-realtime-mini"
    );
    assert_eq!(
        trace
            .ws_handshake
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer ek_cross_surface_secret")
    );
    assert_eq!(
        trace
            .ws_handshake
            .headers
            .get("openai-organization")
            .map(String::as_str),
        Some("org_realtime")
    );
    assert_eq!(
        trace
            .ws_handshake
            .headers
            .get("openai-project")
            .map(String::as_str),
        Some("proj_realtime")
    );
    assert_eq!(
        trace.event_ordering,
        vec![
            String::from("rest.client_secrets.create"),
            String::from("ws.session.created"),
            String::from("ws.close"),
        ]
    );
}

async fn mock_realtime_bootstrap_trace() -> MockRealtimeBootstrapTrace {
    let harness = MockRealtimeBootstrapHarness::spawn()
        .await
        .expect("spawn realtime bootstrap harness");
    let client = with_env_result(
        &[
            ("OPENAI_API_KEY", Some("sk-env-realtime")),
            ("OPENAI_BASE_URL", Some(harness.base_url.as_str())),
            ("OPENAI_ORG_ID", Some("org_realtime")),
            ("OPENAI_PROJECT_ID", Some("proj_realtime")),
        ],
        OpenAI::new,
    );

    let secret = client
        .realtime()
        .client_secrets()
        .create(openai_rust::realtime::RealtimeClientSecretCreateParams {
            expires_after: Some(RealtimeSessionTTL {
                anchor: String::from("created_at"),
                seconds: 60,
            }),
            session: Some(RealtimeSessionConfig {
                model: Some(String::from("gpt-realtime-mini")),
                ..Default::default()
            }),
        })
        .expect("mock realtime client secret should succeed");
    assert_eq!(secret.request_id(), Some("req_realtime_secret"));

    let mut connection = client
        .realtime()
        .connect(RealtimeConnectOptions {
            model: Some(String::from("gpt-realtime-mini")),
            auth: Some(RealtimeAuth::client_secret(
                secret.output.client_secret.value.clone(),
            )),
            ..Default::default()
        })
        .await
        .expect("mock realtime websocket should connect");

    let bootstrap = connection
        .next_event()
        .await
        .expect("expected mock realtime bootstrap event")
        .expect("mock realtime bootstrap should decode");
    assert!(matches!(
        bootstrap,
        RealtimeServerEvent::SessionCreated { .. }
    ));
    connection
        .close()
        .await
        .expect("mock realtime close should succeed");
    assert!(connection.next_event().await.is_none());

    harness.finish(String::from("req_realtime_secret")).await
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CapturedWsHandshake {
    path: String,
    headers: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MockRealtimeBootstrapTrace {
    rest_request: mock_http::CapturedRequest,
    ws_handshake: CapturedWsHandshake,
    secret_request_id: String,
    event_ordering: Vec<String>,
}

struct MockRealtimeBootstrapHarness {
    base_url: String,
    trace_rx: oneshot::Receiver<Result<MockRealtimeBootstrapTrace, String>>,
    worker: tokio::task::JoinHandle<()>,
}

impl MockRealtimeBootstrapHarness {
    async fn spawn() -> Result<Self, String> {
        let listener = TokioTcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|error| error.to_string())?;
        let addr = listener.local_addr().map_err(|error| error.to_string())?;
        let (trace_tx, trace_rx) = oneshot::channel();
        let worker = tokio::spawn(async move {
            let trace = async {
                let (mut rest_stream, _) = listener.accept().await.map_err(|error| error.to_string())?;
                let rest_request = read_async_http_request(&mut rest_stream).await?;
                let secret_response = serde_json::to_vec(&json!({
                    "client_secret": {
                        "value": "ek_cross_surface_secret",
                        "expires_at": 4_102_444_800_i64
                    },
                    "session": {
                        "type": "realtime",
                        "model": "gpt-realtime-mini"
                    }
                }))
                .map_err(|error| error.to_string())?;
                write_async_http_response(
                    &mut rest_stream,
                    "200 OK",
                    vec![
                        (String::from("content-type"), String::from("application/json")),
                        (String::from("content-length"), secret_response.len().to_string()),
                        (
                            String::from("x-request-id"),
                            String::from("req_realtime_secret"),
                        ),
                    ],
                    &secret_response,
                )
                .await?;

                let captured_ws = std::sync::Arc::new(Mutex::new(None::<CapturedWsHandshake>));
                let captured_ws_for_accept = captured_ws.clone();
                let (ws_stream, _) = listener.accept().await.map_err(|error| error.to_string())?;
                let mut socket = accept_hdr_async(ws_stream, move |request: &WsRequest, response: WsResponse| {
                    let mut headers = BTreeMap::new();
                    for (name, value) in request.headers() {
                        headers.insert(
                            name.as_str().to_ascii_lowercase(),
                            value.to_str().unwrap_or_default().to_string(),
                        );
                    }
                    *captured_ws_for_accept.lock().unwrap() = Some(CapturedWsHandshake {
                        path: request
                            .uri()
                            .path_and_query()
                            .map(|value| value.as_str().to_string())
                            .unwrap_or_else(|| request.uri().path().to_string()),
                        headers,
                    });
                    Ok(response)
                })
                .await
                .map_err(|error| error.to_string())?;

                socket
                    .send(Message::Text(
                        json!({
                            "type": "session.created",
                            "event_id": "evt_cross_surface_created",
                            "session": {
                                "id": "sess_cross_surface",
                                "type": "realtime",
                                "model": "gpt-realtime-mini",
                                "output_modalities": ["text"]
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .await
                    .map_err(|error| error.to_string())?;

                let mut observed_close = false;
                while let Some(message) = futures_util::StreamExt::next(&mut socket).await {
                    match message.map_err(|error| error.to_string())? {
                        Message::Close(_) => {
                            observed_close = true;
                            break;
                        }
                        Message::Ping(payload) => {
                            futures_util::SinkExt::send(&mut socket, Message::Pong(payload))
                                .await
                                .map_err(|error| error.to_string())?;
                        }
                        _ => {}
                    }
                }

                let ws_handshake = captured_ws
                    .lock()
                    .unwrap()
                    .clone()
                    .ok_or_else(|| String::from("missing captured websocket handshake"))?;
                Ok(MockRealtimeBootstrapTrace {
                    rest_request,
                    ws_handshake,
                    secret_request_id: String::from("req_realtime_secret"),
                    event_ordering: if observed_close {
                        vec![
                            String::from("rest.client_secrets.create"),
                            String::from("ws.session.created"),
                            String::from("ws.close"),
                        ]
                    } else {
                        vec![
                            String::from("rest.client_secrets.create"),
                            String::from("ws.session.created"),
                        ]
                    },
                })
            }
            .await;

            let _ = trace_tx.send(trace);
        });

        Ok(Self {
            base_url: format!("http://{addr}/v1"),
            trace_rx,
            worker,
        })
    }

    async fn finish(self, secret_request_id: String) -> MockRealtimeBootstrapTrace {
        let mut trace = self
            .trace_rx
            .await
            .expect("realtime bootstrap trace sender") 
            .expect("realtime bootstrap trace");
        trace.secret_request_id = secret_request_id;
        let _ = self.worker.await;
        trace
    }
}

async fn read_async_http_request(
    stream: &mut tokio::net::TcpStream,
) -> Result<mock_http::CapturedRequest, String> {
    let mut buffer = Vec::new();
    let mut header_end = None;
    loop {
        let mut chunk = [0_u8; 1024];
        let bytes_read = stream
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
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
        let bytes_read = stream
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
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
        received_after: Duration::ZERO,
    })
}

async fn write_async_http_response(
    stream: &mut tokio::net::TcpStream,
    status: &str,
    headers: Vec<(String, String)>,
    body: &[u8],
) -> Result<(), String> {
    let mut response = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    response.push_str("connection: close\r\n\r\n");
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    stream
        .write_all(body)
        .await
        .map_err(|error| error.to_string())?;
    stream.flush().await.map_err(|error| error.to_string())
}

fn metadata_shape(request_id: Option<&str>) -> &'static str {
    if request_id.is_some() {
        "request_id:present"
    } else {
        "request_id:missing"
    }
}

fn normalize_chat_finish_reason(finish_reason: Option<&str>) -> &'static str {
    match finish_reason {
        Some("stop") => "completed",
        Some(_) => "non_stop_terminal",
        None => "missing_finish_reason",
    }
}

fn normalize_file_status(status: Option<&openai_rust::resources::files::FileStatus>) -> &'static str {
    match status {
        Some(openai_rust::resources::files::FileStatus::Uploaded)
        | Some(openai_rust::resources::files::FileStatus::Processed) => "ready_or_processing",
        Some(openai_rust::resources::files::FileStatus::Error) => "error",
        Some(openai_rust::resources::files::FileStatus::Deleted) => "deleted",
        Some(openai_rust::resources::files::FileStatus::Unknown) => "unknown",
        None => "missing_status",
    }
}

fn with_env(vars: &[(&str, Option<&str>)], test: impl FnOnce()) {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let saved = vars
        .iter()
        .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
        .collect::<Vec<_>>();
    for (key, value) in vars {
        unsafe {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
    test();
    restore_env(saved);
}

fn with_env_result<T>(vars: &[(&str, Option<&str>)], test: impl FnOnce() -> T) -> T {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let saved = vars
        .iter()
        .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
        .collect::<Vec<_>>();
    for (key, value) in vars {
        unsafe {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
    let result = test();
    restore_env(saved);
    result
}

fn restore_env(saved: Vec<(String, Option<String>)>) {
    for (key, value) in saved {
        unsafe {
            match value {
                Some(value) => std::env::set_var(&key, value),
                None => std::env::remove_var(&key),
            }
        }
    }
}
