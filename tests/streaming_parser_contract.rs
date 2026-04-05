use openai_rust::helpers::sse::{SseFrame, SseParser};

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
