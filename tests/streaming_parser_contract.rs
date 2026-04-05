use openai_rust::{
    OpenAI,
    helpers::sse::{SseFrame, SseParser},
    resources::responses::{ResponseRetrieveParams, ResponseStreamEvent},
};

mod support;

#[test]
fn fragmented_sse_frames_reassemble_into_same_logical_events() {
    let transcript = support::sse::SseTranscript::from_events(vec![
        support::sse::SseEvent::named("response.created").json(r#"{\"id\":\"resp_123\"}"#),
        support::sse::SseEvent::named("response.output_text.delta")
            .json(r#"{\"delta\":\"Hello\"}"#)
            .json(r#"{\"delta\":\"world\"}"#),
    ]);
    let encoded = format!(": comment\nignored: field\n{}", transcript.encode());
    let fragments = support::sse::SseTranscript::from_events(vec![]);
    let chunks = {
        let mut parser_input = encoded.into_bytes();
        let mut chunked = Vec::new();
        for size in [1usize, 2, 5, 3, 8, 13] {
            if parser_input.is_empty() {
                break;
            }
            let take = size.min(parser_input.len());
            chunked.push(parser_input.drain(..take).collect::<Vec<u8>>());
        }
        if !parser_input.is_empty() {
            chunked.push(parser_input);
        }
        chunked
    };

    let mut parser = SseParser::default();
    let mut frames = Vec::new();
    for chunk in &chunks {
        frames.extend(parser.push(chunk).expect("chunk should parse"));
    }
    frames.extend(parser.finish().expect("finish should flush"));

    assert_eq!(
        frames,
        vec![
            SseFrame {
                event: Some(String::from("response.created")),
                data: String::from(r#"{\"id\":\"resp_123\"}"#),
            },
            SseFrame {
                event: Some(String::from("response.output_text.delta")),
                data: String::from("{\\\"delta\\\":\\\"Hello\\\"}\n{\\\"delta\\\":\\\"world\\\"}"),
            },
        ]
    );
    let _ = fragments;
}

#[test]
fn crlf_and_eof_boundaries_do_not_corrupt_sse_parsing() {
    let mut parser = SseParser::default();
    let payload = concat!(
        "event: response.created\r\n",
        "data: {\"id\":\"resp_123\"}\r\n",
        "\r\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_123\"}\r"
    );

    let mut frames = parser
        .push(payload.as_bytes())
        .expect("payload should parse");
    frames.extend(parser.finish().expect("finish should flush eof frame"));

    assert_eq!(
        frames,
        vec![
            SseFrame {
                event: Some(String::from("response.created")),
                data: String::from("{\"id\":\"resp_123\"}"),
            },
            SseFrame {
                event: Some(String::from("response.completed")),
                data: String::from("{\"id\":\"resp_123\"}"),
            },
        ]
    );
}

#[test]
fn resume_uses_server_sequence_numbers() {
    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"}]}],\"usage\":{},\"sequence_number\":10}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Hello\",\"sequence_number\":11}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\" world\",\"sequence_number\":12}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_stream\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello world\"}]}],\"usage\":{},\"sequence_number\":13}\n\n",
        "data: [DONE]\n\n"
    );
    let server =
        support::mock_http::MockHttpServer::spawn(sse_response(transcript)).expect("mock server");
    let client = OpenAI::builder()
        .api_key("sk-test")
        .base_url(server.url())
        .build();

    let mut stream = client
        .responses()
        .resume_stream(
            "resp_stream",
            ResponseRetrieveParams {
                starting_after: Some(10),
                ..Default::default()
            },
        )
        .expect("resume should succeed");

    assert!(matches!(
        stream.next_event(),
        Some(ResponseStreamEvent::OutputTextDelta { ref delta, .. }) if delta == "Hello"
    ));
    assert_eq!(
        stream
            .final_response()
            .expect("completed response")
            .output_text(),
        "Hello world"
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
