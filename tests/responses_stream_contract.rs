use openai_rust::{
    ErrorKind, OpenAI,
    core::metadata::ResponseMetadata,
    resources::responses::{
        ResponseCreateParams, ResponseFormatTextConfig, ResponseFormatTextJSONSchemaConfig,
        ResponseRetrieveParams, ResponseStream, ResponseStreamEvent, ResponseStreamTerminal,
        ResponseTextConfig,
    },
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

mod support;

#[test]
fn text_and_reasoning_accumulate() {
    let server =
        support::mock_http::MockHttpServer::spawn(sse_response(text_and_reasoning_stream()))
            .expect("mock server");
    let client = OpenAI::builder()
        .api_key("sk-test")
        .base_url(server.url())
        .build();

    let mut stream = client
        .responses()
        .stream(ResponseCreateParams {
            model: String::from("gpt-4.1-mini"),
            input: Some(json!("say hi")),
            ..Default::default()
        })
        .expect("stream should start");

    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::Created { .. })
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::OutputTextDelta { ref delta, .. }) if delta == "Hello"
    ));
    assert_eq!(stream.current_response().unwrap().output_text(), "Hello");
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::ReasoningTextDelta { ref delta, .. }) if delta == "Thinking..."
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::OutputTextDelta { ref delta, .. }) if delta == " world"
    ));
    assert_eq!(
        stream.current_response().unwrap().output_text(),
        "Hello world"
    );
    let final_output_text = stream
        .final_response()
        .expect("completed response")
        .output_text()
        .to_string();
    assert!(matches!(
        stream.terminal_state(),
        Some(ResponseStreamTerminal::Completed(_))
    ));
    assert_eq!(final_output_text, "Hello world");

    let request = server.captured_request().expect("captured request");
    let body: serde_json::Value = serde_json::from_slice(&request.body).expect("json body");
    assert_eq!(body.get("stream"), Some(&serde_json::Value::Bool(true)));
}

#[test]
fn background_resume_skips_seen_events() {
    let server =
        support::mock_http::MockHttpServer::spawn(sse_response(text_and_reasoning_stream()))
            .expect("mock server");
    let client = OpenAI::builder()
        .api_key("sk-test")
        .base_url(server.url())
        .build();

    let mut stream = client
        .responses()
        .resume_stream(
            "resp_stream",
            ResponseRetrieveParams {
                starting_after: Some(3),
                ..Default::default()
            },
        )
        .expect("resume should succeed");

    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::OutputTextDelta { ref delta, .. }) if delta == " world"
    ));
    assert_eq!(
        stream.final_response().unwrap().output_text(),
        "Hello world"
    );

    let request = server.captured_request().expect("captured request");
    assert!(request.path.contains("stream=true"));
    assert!(request.path.contains("starting_after=3"));
}

#[tokio::test(flavor = "current_thread")]
async fn async_consumers_can_abort_without_hanging() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let mut stream = ResponseStream::from_sse_chunks(metadata, vec![text_and_reasoning_stream()])
        .expect("stream transcript");

    assert!(stream.next_event_async().await.is_some());
    stream.abort();
    assert!(stream.next_event_async().await.is_none());

    let error = stream
        .final_response()
        .expect_err("aborted stream should not finalize");
    assert_eq!(error.kind, ErrorKind::Transport);
}

#[test]
fn incremental_delivery_and_abort() {
    let server = IncrementalSseServer::spawn(
        concat!(
            "event: response.created\n",
            "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"}]}],\"usage\":{},\"sequence_number\":1}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Hello\",\"sequence_number\":2}\n\n",
        ),
        concat!(
            "event: response.output_text.delta\n",
            "data: {\"output_index\":0,\"content_index\":0,\"delta\":\" world\",\"sequence_number\":3}\n\n",
            "event: response.completed\n",
            "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello world\"}]}],\"usage\":{},\"sequence_number\":4}\n\n",
            "data: [DONE]\n\n"
        ),
        Duration::from_millis(400),
    )
    .expect("incremental server");
    let client = OpenAI::builder()
        .api_key("sk-test")
        .base_url(server.url())
        .build();

    let started = Instant::now();
    let mut stream = client
        .responses()
        .stream(ResponseCreateParams {
            model: String::from("gpt-4.1-mini"),
            input: Some(json!("say hi")),
            ..Default::default()
        })
        .expect("stream should start");

    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::Created { .. })
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::OutputTextDelta { ref delta, .. }) if delta == "Hello"
    ));
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "expected first delta before delayed tail chunk, got {:?}",
        started.elapsed()
    );

    stream.abort();
    assert!(stream.next_event().is_none());
    let error = stream
        .final_response()
        .expect_err("aborted stream should not finalize");
    assert_eq!(error.kind, ErrorKind::Transport);
}

#[test]
fn unknown_events_and_invalid_ordering_are_deterministic() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
        "event: response.future.added\n",
        "data: {\"unexpected\":true}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Hello\",\"extra_field\":true}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );

    let mut stream = ResponseStream::from_sse_chunks(metadata.clone(), vec![transcript])
        .expect("unknown events should be tolerated");
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::Created { .. })
    ));
    assert!(
        matches!(stream.next_event(), Some(ResponseStreamEvent::Unknown { ref event, .. }) if event == "response.future.added")
    );
    assert_eq!(stream.final_response().unwrap().output_text(), "Hello");

    let invalid = concat!(
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"oops\"}\n\n"
    );
    let error =
        ResponseStream::from_sse_chunks(metadata, vec![invalid]).expect_err("ordering error");
    assert_eq!(error.kind, ErrorKind::Validation);
}

#[test]
fn newer_response_stream_events_are_typed_and_update_snapshots() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let transcript = concat!(
        "event: response.queued\n",
        "data: {\"id\":\"resp_new_events\",\"object\":\"response\",\"created_at\":1,\"status\":\"queued\",\"output\":[],\"usage\":{},\"sequence_number\":1}\n\n",
        "event: response.in_progress\n",
        "data: {\"id\":\"resp_new_events\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"rs_1\",\"type\":\"reasoning\",\"summary\":[]},{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\",\"annotations\":[]}]},{\"id\":\"img_1\",\"type\":\"image_generation_call\",\"status\":\"in_progress\"}],\"usage\":{},\"sequence_number\":2}\n\n",
        "event: response.reasoning_summary_part.added\n",
        "data: {\"item_id\":\"rs_1\",\"output_index\":0,\"summary_index\":0,\"part\":{\"type\":\"summary_text\",\"text\":\"\"},\"sequence_number\":3}\n\n",
        "event: response.reasoning_summary_text.delta\n",
        "data: {\"item_id\":\"rs_1\",\"output_index\":0,\"summary_index\":0,\"delta\":\"Plan\",\"sequence_number\":4}\n\n",
        "event: response.reasoning_summary_text.done\n",
        "data: {\"item_id\":\"rs_1\",\"output_index\":0,\"summary_index\":0,\"text\":\"Plan done\",\"sequence_number\":5}\n\n",
        "event: response.reasoning_summary_part.done\n",
        "data: {\"item_id\":\"rs_1\",\"output_index\":0,\"summary_index\":0,\"part\":{\"type\":\"summary_text\",\"text\":\"Plan done\"},\"sequence_number\":6}\n\n",
        "event: response.output_text.annotation.added\n",
        "data: {\"item_id\":\"msg_1\",\"output_index\":1,\"content_index\":0,\"annotation_index\":0,\"annotation\":{\"type\":\"url_citation\",\"url\":\"https://example.com\"},\"sequence_number\":7}\n\n",
        "event: response.audio.delta\n",
        "data: {\"delta\":\"QUJD\",\"sequence_number\":8}\n\n",
        "event: response.audio.done\n",
        "data: {\"sequence_number\":9}\n\n",
        "event: response.audio.transcript.delta\n",
        "data: {\"delta\":\"hello\",\"sequence_number\":10}\n\n",
        "event: response.audio.transcript.done\n",
        "data: {\"sequence_number\":11}\n\n",
        "event: response.image_generation_call.in_progress\n",
        "data: {\"item_id\":\"img_1\",\"output_index\":2,\"sequence_number\":12}\n\n",
        "event: response.image_generation_call.generating\n",
        "data: {\"item_id\":\"img_1\",\"output_index\":2,\"sequence_number\":13}\n\n",
        "event: response.image_generation_call.partial_image\n",
        "data: {\"item_id\":\"img_1\",\"output_index\":2,\"partial_image_b64\":\"aW1n\",\"partial_image_index\":0,\"sequence_number\":14}\n\n",
        "event: response.image_generation_call.completed\n",
        "data: {\"item_id\":\"img_1\",\"output_index\":2,\"sequence_number\":15}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_new_events\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"rs_1\",\"type\":\"reasoning\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"Plan done\"}]},{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\",\"annotations\":[{\"type\":\"url_citation\",\"url\":\"https://example.com\"}]}]},{\"id\":\"img_1\",\"type\":\"image_generation_call\",\"status\":\"completed\"}],\"usage\":{},\"sequence_number\":16}\n\n",
        "data: [DONE]\n\n"
    );

    let mut stream = ResponseStream::from_sse_chunks(metadata, vec![transcript]).expect("stream");

    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::Queued { ref response })
            if response.status.as_deref() == Some("queued")
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::InProgress { ref response })
            if response.status.as_deref() == Some("in_progress")
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::ReasoningSummaryPartAdded {
            output_index: 0,
            summary_index: 0,
            ..
        })
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::ReasoningSummaryTextDelta { ref delta, .. })
            if delta == "Plan"
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::ReasoningSummaryTextDone { ref text, .. })
            if text == "Plan done"
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::ReasoningSummaryPartDone {
            output_index: 0,
            summary_index: 0,
            ..
        })
    ));
    match stream.next_event() {
        Some(ResponseStreamEvent::OutputTextAnnotationAdded {
            annotation,
            annotation_index: 0,
            ..
        }) => assert_eq!(annotation["url"], "https://example.com"),
        other => panic!("unexpected annotation event: {other:?}"),
    }
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::AudioDelta { ref delta }) if delta == "QUJD"
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::AudioDone)
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::AudioTranscriptDelta { ref delta }) if delta == "hello"
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::AudioTranscriptDone)
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::ImageGenerationCallInProgress {
            output_index: 2,
            ..
        })
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::ImageGenerationCallGenerating {
            output_index: 2,
            ..
        })
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::ImageGenerationCallPartialImage {
            ref partial_image_b64,
            partial_image_index: 0,
            ..
        }) if partial_image_b64 == "aW1n"
    ));
    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::ImageGenerationCallCompleted {
            output_index: 2,
            ..
        })
    ));

    let snapshot = stream
        .current_response()
        .expect("snapshot after new events");
    assert_eq!(
        snapshot.output[0].summary,
        vec![json!({"type": "summary_text", "text": "Plan done"})]
    );
    assert_eq!(
        snapshot.output[1].content[0].annotations,
        vec![json!({"type": "url_citation", "url": "https://example.com"})]
    );
    assert_eq!(snapshot.output[2].status.as_deref(), Some("completed"));

    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::Completed { .. })
    ));
    assert_eq!(
        stream.final_response().unwrap().status.as_deref(),
        Some("completed")
    );
}

#[test]
fn terminal_failure_and_refusal_states_remain_explicit() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let refusal_stream = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_refusal\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"refusal\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
        "event: response.refusal.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"I can't comply\"}\n\n",
        "event: response.refusal.done\n",
        "data: {\"output_index\":0,\"content_index\":0,\"text\":\"I can't comply\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_refusal\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"refusal\",\"text\":\"I can't comply\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );
    let mut refusal = ResponseStream::from_sse_chunks(metadata.clone(), vec![refusal_stream])
        .expect("refusal transcript");
    let refusal_response = refusal
        .final_response()
        .expect("completed refusal response");
    assert_eq!(refusal_response.output_text(), "");
    assert_eq!(refusal_response.refusal_text(), Some("I can't comply"));

    let failed_stream = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_failed\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[],\"usage\":{}}\n\n",
        "event: response.failed\n",
        "data: {\"id\":\"resp_failed\",\"object\":\"response\",\"created_at\":1,\"status\":\"failed\",\"output\":[],\"error\":{\"message\":\"boom\"},\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );
    let mut failed =
        ResponseStream::from_sse_chunks(metadata, vec![failed_stream]).expect("failed transcript");
    assert!(
        matches!(failed.terminal_state(), Some(ResponseStreamTerminal::Failed(response)) if response.status.as_deref() == Some("failed"))
    );
    let error = failed
        .final_response()
        .expect_err("failed stream must stay explicit");
    assert_eq!(
        error.kind,
        ErrorKind::Api(openai_rust::ApiErrorKind::Server)
    );
}

#[test]
fn response_error_event_surfaces_api_error_payload() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let error_stream = concat!(
        "event: error\n",
        "data: {\"message\":\"stream exploded\",\"code\":\"bad_stream\",\"param\":\"input\",\"sequence_number\":1}\n\n",
        "data: [DONE]\n\n"
    );

    let error = ResponseStream::from_sse_chunks(metadata, vec![error_stream])
        .expect_err("response.error should fail the stream explicitly");
    assert_eq!(
        error.kind,
        ErrorKind::Api(openai_rust::ApiErrorKind::BadRequest)
    );
    let api_error = error.api_error().expect("api error payload");
    assert_eq!(api_error.message, "stream exploded");
    assert_eq!(api_error.code.as_deref(), Some("bad_stream"));
    assert_eq!(api_error.param.as_deref(), Some("input"));
}

#[test]
fn terminal_incomplete_state_remains_explicit() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let incomplete_stream = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_incomplete\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"{\\\"city\\\":\\\"Paris\\\"\"}\n\n",
        "event: response.incomplete\n",
        "data: {\"id\":\"resp_incomplete\",\"object\":\"response\",\"created_at\":1,\"status\":\"incomplete\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"{\\\"city\\\":\\\"Paris\\\"\"}]}],\"incomplete_details\":{\"reason\":\"max_output_tokens\"},\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );

    let mut incomplete = ResponseStream::from_sse_chunks(metadata, vec![incomplete_stream])
        .expect("incomplete transcript");
    assert!(matches!(
        incomplete.terminal_state(),
        Some(ResponseStreamTerminal::Incomplete(response))
            if response.status.as_deref() == Some("incomplete")
                && response
                    .incomplete_details
                    .as_ref()
                    .and_then(|details| details.reason.as_deref())
                    == Some("max_output_tokens")
    ));

    let response_error = incomplete
        .final_response()
        .expect_err("incomplete streams must not become a successful response");
    assert_eq!(response_error.kind, ErrorKind::Parse);

    let parse_error = incomplete
        .parse_final::<serde_json::Value>(
            Some(ResponseTextConfig {
                format: Some(ResponseFormatTextConfig::JsonSchema(
                    ResponseFormatTextJSONSchemaConfig {
                        name: String::from("partial_city"),
                        schema: json!({"type": "object"}),
                        description: None,
                        strict: Some(true),
                    },
                )),
                verbosity: None,
            }),
            &[],
        )
        .expect_err("parsed helpers must not synthesize success for incomplete streams");
    assert_eq!(parse_error.kind, ErrorKind::Parse);
}

fn sse_response(body: impl Into<Vec<u8>>) -> support::mock_http::ScriptedResponse {
    let body = body.into();
    support::mock_http::ScriptedResponse {
        status_code: 200,
        reason: "OK",
        headers: vec![
            (
                String::from("content-type"),
                String::from("text/event-stream"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body,
        ..Default::default()
    }
}

fn text_and_reasoning_stream() -> String {
    concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"},{\"type\":\"reasoning_text\",\"text\":\"\"}]}],\"usage\":{},\"sequence_number\":1}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Hello\",\"sequence_number\":2}\n\n",
        "event: response.reasoning_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":1,\"delta\":\"Thinking...\",\"sequence_number\":3}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\" world\",\"sequence_number\":4}\n\n",
        "event: response.output_text.done\n",
        "data: {\"output_index\":0,\"content_index\":0,\"text\":\"Hello world\",\"sequence_number\":5}\n\n",
        "event: response.reasoning_text.done\n",
        "data: {\"output_index\":0,\"content_index\":1,\"text\":\"Thinking...\",\"sequence_number\":6}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello world\"},{\"type\":\"reasoning_text\",\"text\":\"Thinking...\"}]}],\"usage\":{},\"sequence_number\":7}\n\n",
        "data: [DONE]\n\n"
    )
    .to_string()
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
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let worker = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = read_request_headers(&mut stream);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
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
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(());
        }
    }
}
