use openai_rust::OpenAI;
use serde_json::json;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn diarized_transcription_and_streaming_segments_preserve_speaker_metadata() {
    let stream_body = concat!(
        "event: transcript.text.segment\n",
        "data: {\"type\":\"transcript.text.segment\",\"id\":\"seg_agent\",\"speaker\":\"agent\",\"start\":0.0,\"end\":0.7,\"text\":\"Hello\"}\n\n",
        "event: transcript.text.delta\n",
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"Hello\",\"segment_id\":\"seg_agent\"}\n\n",
        "event: transcript.text.segment\n",
        "data: {\"type\":\"transcript.text.segment\",\"id\":\"seg_customer\",\"speaker\":\"customer\",\"start\":0.7,\"end\":1.3,\"text\":\"Hi\"}\n\n",
        "event: transcript.text.done\n",
        "data: {\"type\":\"transcript.text.done\",\"text\":\"Hello Hi\",\"usage\":{\"type\":\"tokens\",\"input_tokens\":6,\"output_tokens\":2,\"total_tokens\":8}}\n\n",
        "data: [DONE]\n\n"
    );
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(
            json!({
                "duration": 1.3,
                "task": "transcribe",
                "text": "Hello Hi",
                "segments": [
                    {"id": "seg_agent", "speaker": "agent", "start": 0.0, "end": 0.7, "text": "Hello", "type": "transcript.text.segment"},
                    {"id": "seg_customer", "speaker": "customer", "start": 0.7, "end": 1.3, "text": "Hi", "type": "transcript.text.segment"}
                ],
                "usage": {"type": "tokens", "input_tokens": 6, "output_tokens": 2, "total_tokens": 8}
            })
            .to_string(),
        ),
        sse_response(stream_body),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let diarized = client
        .audio()
        .transcriptions
        .create(openai_rust::resources::audio::TranscriptionParams {
            file: openai_rust::resources::audio::AudioInput::new(
                "dialog.wav",
                "audio/wav",
                vec![1, 2, 3],
            ),
            model: String::from("gpt-4o-transcribe-diarize"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::DiarizedJson),
            known_speaker_names: vec![String::from("agent"), String::from("customer")],
            known_speaker_references: vec![
                String::from("data:audio/wav;base64,QUJD"),
                String::from("data:audio/wav;base64,REVG"),
            ],
            ..Default::default()
        })
        .unwrap();

    match diarized.output() {
        openai_rust::resources::audio::TranscriptionResponse::DiarizedJson(payload) => {
            assert_eq!(payload.text, "Hello Hi");
            assert_eq!(payload.segments[0].speaker, "agent");
            assert_eq!(payload.segments[1].speaker, "customer");
            assert_eq!(payload.usage.as_ref().unwrap().total_tokens(), Some(8));
        }
        other => panic!("expected diarized transcription, got {other:?}"),
    }

    let mut stream = client
        .audio()
        .transcriptions
        .stream(openai_rust::resources::audio::TranscriptionParams {
            file: openai_rust::resources::audio::AudioInput::new(
                "dialog.wav",
                "audio/wav",
                vec![1, 2, 3],
            ),
            model: String::from("gpt-4o-transcribe-diarize"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::DiarizedJson),
            ..Default::default()
        })
        .unwrap();

    match stream.next_event().expect("first segment") {
        openai_rust::resources::audio::TranscriptionStreamEvent::TextSegment(segment) => {
            assert_eq!(segment.id, "seg_agent");
            assert_eq!(segment.speaker, "agent");
        }
        other => panic!("expected first segment event, got {other:?}"),
    }
    let _ = stream.next_event().expect("delta");
    match stream.next_event().expect("second segment") {
        openai_rust::resources::audio::TranscriptionStreamEvent::TextSegment(segment) => {
            assert_eq!(segment.id, "seg_customer");
            assert_eq!(segment.speaker, "customer");
        }
        other => panic!("expected second segment event, got {other:?}"),
    }
    let _ = stream.next_event().expect("done");
    assert_eq!(stream.segments().len(), 2);
    assert_eq!(stream.segments()[0].speaker, "agent");
    assert_eq!(stream.segments()[1].speaker, "customer");
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn sse_response(body: &str) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("text/event-stream"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body: body.as_bytes().to_vec(),
        ..Default::default()
    }
}
