use openai_rust::{
    ErrorKind, OpenAI,
    core::metadata::ResponseMetadata,
    resources::completions::{Completion, CompletionStream},
};
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn streamed_and_non_streamed_payloads_share_the_text_completion_shape() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };

    let transcript = vec![concat!(
        r#"data: {"id":"cmpl_stream","object":"text_completion","created":1,"model":"gpt-3.5-turbo-instruct","choices":[{"text":"Hel","index":0,"logprobs":null,"finish_reason":null}]}"#,
        "\n\n",
        r#"data: {"id":"cmpl_stream","object":"text_completion","created":1,"model":"gpt-3.5-turbo-instruct","choices":[{"text":"lo","index":0,"logprobs":null,"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}"#,
        "\n\n",
        "data: [DONE]\n\n"
    )];

    let mut stream =
        CompletionStream::from_sse_chunks(metadata, transcript).expect("stream should parse");

    assert!(matches!(stream.next_completion(), Some(Completion { .. })));
    assert!(matches!(stream.next_completion(), Some(Completion { .. })));
    assert!(stream.next_completion().is_none());

    let final_completion = stream.final_completion();
    assert_eq!(final_completion.object, "text_completion");
    assert_eq!(final_completion.choices.len(), 1);
    assert_eq!(final_completion.choices[0].text, "Hello");
    assert_eq!(
        final_completion.choices[0].finish_reason.as_deref(),
        Some("stop")
    );
    assert_eq!(
        final_completion.usage.as_ref().unwrap()["total_tokens"].as_i64(),
        Some(5)
    );
}

#[test]
fn stream_requires_done_or_terminal_completion() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };

    let error = CompletionStream::from_sse_chunks(
        metadata,
        [r#"data: {"id":"cmpl_stream","object":"text_completion","created":1,"model":"gpt-3.5-turbo-instruct","choices":[{"text":"partial","index":0,"logprobs":null,"finish_reason":null}]}

"#],
    )
    .expect_err("missing done marker should fail");

    assert_eq!(error.kind, ErrorKind::Parse);
}

#[test]
fn done_only_transcript_is_rejected() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };

    let error = CompletionStream::from_sse_chunks(metadata, ["data: [DONE]\n\n"])
        .expect_err("[DONE]-only transcript should fail");

    assert_eq!(error.kind, ErrorKind::Parse);
    assert!(
        error
            .message
            .contains("without any parsed completion payload"),
        "unexpected error: {}",
        error.message
    );
}

#[test]
fn stream_posts_to_legacy_completions_endpoint() {
    let body = concat!(
        "data: {\"id\":\"cmpl_stream\",\"object\":\"text_completion\",\"created\":1,\"model\":\"gpt-3.5-turbo-instruct\",\"choices\":[{\"text\":\"Hel\",\"index\":0,\"logprobs\":null,\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"cmpl_stream\",\"object\":\"text_completion\",\"created\":1,\"model\":\"gpt-3.5-turbo-instruct\",\"choices\":[{\"text\":\"lo\",\"index\":0,\"logprobs\":null,\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n"
    );
    let server = mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("text/event-stream"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body: body.as_bytes().to_vec(),
        ..Default::default()
    })
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let stream = client
        .completions()
        .stream(
            openai_rust::resources::completions::CompletionCreateParams {
                model: String::from("gpt-3.5-turbo-instruct"),
                prompt: Some(json!("Say hello")),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(stream.final_completion().choices[0].text, "Hello");

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/completions");
    let request_body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(request_body["stream"], true);
}
