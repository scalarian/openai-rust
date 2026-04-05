use openai_rust::{
    ErrorKind, OpenAI,
    core::metadata::ResponseMetadata,
    resources::responses::{
        ResponseCreateParams, ResponseRetrieveParams, ResponseStream, ResponseStreamEvent,
        ResponseStreamTerminal,
    },
};
use serde_json::json;

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
    assert!(matches!(
        stream.terminal_state(),
        Some(ResponseStreamTerminal::Completed(_))
    ));
    let final_response = stream.final_response().expect("completed response");
    assert_eq!(final_response.output_text(), "Hello world");

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
                starting_after: Some(2),
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
    assert!(request.path.contains("starting_after=2"));
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
    let refusal = ResponseStream::from_sse_chunks(metadata.clone(), vec![refusal_stream])
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
    let failed =
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
        "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"},{\"type\":\"reasoning_text\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Hello\"}\n\n",
        "event: response.reasoning_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":1,\"delta\":\"Thinking...\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\" world\"}\n\n",
        "event: response.output_text.done\n",
        "data: {\"output_index\":0,\"content_index\":0,\"text\":\"Hello world\"}\n\n",
        "event: response.reasoning_text.done\n",
        "data: {\"output_index\":0,\"content_index\":1,\"text\":\"Thinking...\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello world\"},{\"type\":\"reasoning_text\",\"text\":\"Thinking...\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    )
    .to_string()
}
