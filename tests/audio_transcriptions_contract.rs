use openai_rust::OpenAI;
use serde_json::json;

#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

#[test]
fn transcription_dispatches_typed_formats_and_preserves_multipart_semantics() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(
            json!({
                "text": "hello there",
                "usage": {
                    "type": "tokens",
                    "input_tokens": 10,
                    "output_tokens": 4,
                    "total_tokens": 14,
                    "input_token_details": {"audio_tokens": 7, "text_tokens": 3}
                },
                "logprobs": [{"token": "hello", "logprob": -0.1, "bytes": [104, 101, 108, 108, 111]}]
            })
            .to_string(),
        ),
        json_response(
            json!({
                "duration": 1.25,
                "language": "en",
                "text": "hello there",
                "segments": [{
                    "id": 1,
                    "avg_logprob": -0.2,
                    "compression_ratio": 0.9,
                    "end": 1.25,
                    "no_speech_prob": 0.01,
                    "seek": 0,
                    "start": 0.0,
                    "temperature": 0.0,
                    "text": "hello there",
                    "tokens": [1, 2, 3]
                }],
                "words": [{"word": "hello", "start": 0.0, "end": 0.4}],
                "usage": {"type": "duration", "seconds": 1.25}
            })
            .to_string(),
        ),
        text_response("WEBVTT\n\n00:00.000 --> 00:01.000\nhello there\n"),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let audio =
        openai_rust::resources::audio::AudioInput::new("clip.wav", "audio/wav", tiny_wav_bytes());

    let json_transcription = client
        .audio()
        .transcriptions
        .create(openai_rust::resources::audio::TranscriptionParams {
            file: audio.clone(),
            model: String::from("gpt-4o-transcribe"),
            chunking_strategy: Some(
                openai_rust::resources::audio::TranscriptionChunkingStrategy::Auto,
            ),
            include: vec![openai_rust::resources::audio::TranscriptionInclude::Logprobs],
            known_speaker_names: vec![String::from("agent"), String::from("customer")],
            known_speaker_references: vec![
                String::from("data:audio/wav;base64,QUJD"),
                String::from("data:audio/wav;base64,REVG"),
            ],
            language: Some(String::from("en")),
            prompt: Some(String::from("Warm greeting")),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::Json),
            temperature: Some(0.0),
            timestamp_granularities: vec![
                openai_rust::resources::audio::TranscriptionTimestampGranularity::Word,
            ],
            ..Default::default()
        })
        .unwrap();
    match json_transcription.output() {
        openai_rust::resources::audio::TranscriptionResponse::Json(payload) => {
            assert_eq!(payload.text, "hello there");
            assert_eq!(payload.usage.as_ref().unwrap().total_tokens(), Some(14));
            assert_eq!(payload.logprobs[0].token.as_deref(), Some("hello"));
        }
        other => panic!("expected json transcription, got {other:?}"),
    }

    let verbose = client
        .audio()
        .transcriptions
        .create(openai_rust::resources::audio::TranscriptionParams {
            file: audio.clone(),
            model: String::from("whisper-1"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::VerboseJson),
            timestamp_granularities: vec![
                openai_rust::resources::audio::TranscriptionTimestampGranularity::Word,
                openai_rust::resources::audio::TranscriptionTimestampGranularity::Segment,
            ],
            ..Default::default()
        })
        .unwrap();
    match verbose.output() {
        openai_rust::resources::audio::TranscriptionResponse::VerboseJson(payload) => {
            assert_eq!(payload.text, "hello there");
            assert_eq!(payload.segments.as_ref().unwrap()[0].text, "hello there");
            assert_eq!(payload.words.as_ref().unwrap()[0].word, "hello");
            assert_eq!(payload.usage.as_ref().unwrap().seconds(), Some(1.25));
        }
        other => panic!("expected verbose transcription, got {other:?}"),
    }

    let vtt = client
        .audio()
        .transcriptions
        .create(openai_rust::resources::audio::TranscriptionParams {
            file: audio.clone(),
            model: String::from("whisper-1"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::Vtt),
            ..Default::default()
        })
        .unwrap();
    match vtt.output() {
        openai_rust::resources::audio::TranscriptionResponse::Vtt(body) => {
            assert!(body.contains("WEBVTT"));
            assert!(body.contains("hello there"));
        }
        other => panic!("expected VTT transcription, got {other:?}"),
    }

    let requests = server.captured_requests(3).expect("captured requests");
    for request in &requests {
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/audio/transcriptions");
        let boundary = boundary_from_headers(&request.headers);
        let multipart = multipart_support::parse_multipart(&request.body, &boundary).unwrap();
        let file_part = multipart
            .parts
            .iter()
            .find(|part| part.name.as_deref() == Some("file"))
            .expect("file part");
        assert_eq!(file_part.filename.as_deref(), Some("clip.wav"));
        assert_eq!(
            file_part.headers.get("content-type").map(String::as_str),
            Some("audio/wav")
        );
        assert_eq!(file_part.body, audio.bytes);
    }

    let first_body = multipart_support::parse_multipart(
        &requests[0].body,
        &boundary_from_headers(&requests[0].headers),
    )
    .unwrap();
    assert_text_part(&first_body, "model", "gpt-4o-transcribe");
    assert_text_part(&first_body, "chunking_strategy", "auto");
    assert_repeated_text_parts(&first_body, "include", &["logprobs"]);
    assert_repeated_text_parts(&first_body, "known_speaker_names", &["agent", "customer"]);
    assert_repeated_text_parts(
        &first_body,
        "known_speaker_references",
        &["data:audio/wav;base64,QUJD", "data:audio/wav;base64,REVG"],
    );
    assert_text_part(&first_body, "language", "en");
    assert_text_part(&first_body, "prompt", "Warm greeting");
    assert_text_part(&first_body, "response_format", "json");
    assert_text_part(&first_body, "temperature", "0");
    assert_repeated_text_parts(&first_body, "timestamp_granularities", &["word"]);

    let second_body = multipart_support::parse_multipart(
        &requests[1].body,
        &boundary_from_headers(&requests[1].headers),
    )
    .unwrap();
    assert_text_part(&second_body, "response_format", "verbose_json");
    assert_repeated_text_parts(
        &second_body,
        "timestamp_granularities",
        &["word", "segment"],
    );

    let third_body = multipart_support::parse_multipart(
        &requests[2].body,
        &boundary_from_headers(&requests[2].headers),
    )
    .unwrap();
    assert_text_part(&third_body, "response_format", "vtt");
}

#[test]
fn streaming_transcription_assembles_deltas_segments_and_usage() {
    let body = concat!(
        "event: transcript.text.delta\n",
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"hello \",\"segment_id\":\"seg_1\"}\n\n",
        "event: transcript.text.segment\n",
        "data: {\"type\":\"transcript.text.segment\",\"id\":\"seg_1\",\"speaker\":\"agent\",\"start\":0.0,\"end\":0.6,\"text\":\"hello\"}\n\n",
        "event: transcript.text.delta\n",
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"there\",\"segment_id\":\"seg_1\"}\n\n",
        "event: transcript.text.done\n",
        "data: {\"type\":\"transcript.text.done\",\"text\":\"hello there\",\"usage\":{\"type\":\"tokens\",\"input_tokens\":11,\"output_tokens\":2,\"total_tokens\":13,\"input_token_details\":{\"audio_tokens\":11}}}\n\n",
        "data: [DONE]\n\n"
    );

    let server = mock_http::MockHttpServer::spawn(sse_response(body)).unwrap();
    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let mut stream = client
        .audio()
        .transcriptions
        .stream(openai_rust::resources::audio::TranscriptionParams {
            file: openai_rust::resources::audio::AudioInput::new(
                "clip.wav",
                "audio/wav",
                tiny_wav_bytes(),
            ),
            model: String::from("gpt-4o-transcribe-diarize"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::DiarizedJson),
            ..Default::default()
        })
        .unwrap();

    match stream.next_event().expect("delta") {
        openai_rust::resources::audio::TranscriptionStreamEvent::TextDelta(event) => {
            assert_eq!(event.delta, "hello ");
            assert_eq!(event.segment_id.as_deref(), Some("seg_1"));
        }
        other => panic!("expected text delta event, got {other:?}"),
    }
    match stream.next_event().expect("segment") {
        openai_rust::resources::audio::TranscriptionStreamEvent::TextSegment(event) => {
            assert_eq!(event.id, "seg_1");
            assert_eq!(event.speaker, "agent");
        }
        other => panic!("expected text segment event, got {other:?}"),
    }
    match stream.next_event().expect("second delta") {
        openai_rust::resources::audio::TranscriptionStreamEvent::TextDelta(event) => {
            assert_eq!(event.delta, "there");
        }
        other => panic!("expected second text delta event, got {other:?}"),
    }
    match stream.next_event().expect("done") {
        openai_rust::resources::audio::TranscriptionStreamEvent::TextDone(event) => {
            assert_eq!(event.text, "hello there");
            assert_eq!(event.usage.as_ref().unwrap().total_tokens(), Some(13));
        }
        other => panic!("expected text done event, got {other:?}"),
    }
    assert!(stream.next_event().is_none());
    assert_eq!(stream.final_text().unwrap(), "hello there");
    assert_eq!(stream.final_usage().unwrap().total_tokens(), Some(13));
    assert_eq!(stream.segments()[0].speaker, "agent");
}

#[test]
fn streaming_transcription_rejects_eof_truncated_transcripts_without_done_event() {
    let body = concat!(
        "event: transcript.text.delta\n",
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"hello \",\"segment_id\":\"seg_1\"}\n\n",
        "event: transcript.text.segment\n",
        "data: {\"type\":\"transcript.text.segment\",\"id\":\"seg_1\",\"speaker\":\"agent\",\"start\":0.0,\"end\":0.6,\"text\":\"hello\"}\n\n",
        "data: [DONE]\n\n"
    );

    let server = mock_http::MockHttpServer::spawn(sse_response(body)).unwrap();
    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let error = client
        .audio()
        .transcriptions
        .stream(openai_rust::resources::audio::TranscriptionParams {
            file: openai_rust::resources::audio::AudioInput::new(
                "clip.wav",
                "audio/wav",
                tiny_wav_bytes(),
            ),
            model: String::from("gpt-4o-transcribe-diarize"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::DiarizedJson),
            ..Default::default()
        })
        .expect_err("truncated streams should fail before surfacing success");

    assert!(
        error
            .to_string()
            .contains("terminal transcript.text.done event"),
        "unexpected error: {error}"
    );
}

#[test]
fn streaming_transcription_rejects_eof_truncated_transcripts_without_done_or_done_marker() {
    let body = concat!(
        "event: transcript.text.delta\n",
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"hello \",\"segment_id\":\"seg_1\"}\n\n",
        "event: transcript.text.segment\n",
        "data: {\"type\":\"transcript.text.segment\",\"id\":\"seg_1\",\"speaker\":\"agent\",\"start\":0.0,\"end\":0.6,\"text\":\"hello\"}\n\n"
    );

    let server = mock_http::MockHttpServer::spawn(sse_response(body)).unwrap();
    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let error = client
        .audio()
        .transcriptions
        .stream(openai_rust::resources::audio::TranscriptionParams {
            file: openai_rust::resources::audio::AudioInput::new(
                "clip.wav",
                "audio/wav",
                tiny_wav_bytes(),
            ),
            model: String::from("gpt-4o-transcribe-diarize"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::DiarizedJson),
            ..Default::default()
        })
        .expect_err("EOF-truncated streams should fail even without [DONE]");

    assert!(
        error
            .to_string()
            .contains("terminal transcript.text.done event"),
        "unexpected error: {error}"
    );
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

fn text_response(body: &str) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("text/plain; charset=utf-8"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body: body.as_bytes().to_vec(),
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

fn boundary_from_headers(headers: &std::collections::BTreeMap<String, String>) -> String {
    headers["content-type"]
        .split("boundary=")
        .nth(1)
        .expect("multipart boundary")
        .trim_matches('"')
        .to_string()
}

fn assert_text_part(multipart: &multipart_support::ParsedMultipart, name: &str, value: &str) {
    let part = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("missing multipart text part `{name}`"));
    assert_eq!(std::str::from_utf8(&part.body).unwrap(), value);
}

fn assert_repeated_text_parts(
    multipart: &multipart_support::ParsedMultipart,
    name: &str,
    values: &[&str],
) {
    let observed: Vec<_> = multipart
        .parts
        .iter()
        .filter(|part| part.name.as_deref() == Some(name))
        .map(|part| std::str::from_utf8(&part.body).unwrap().to_string())
        .collect();
    let expected: Vec<_> = values.iter().map(|value| value.to_string()).collect();
    assert_eq!(observed, expected);
}

fn tiny_wav_bytes() -> Vec<u8> {
    const SAMPLE_RATE: u32 = 16_000;
    const SAMPLES: usize = 160;
    let mut pcm = Vec::with_capacity(SAMPLES * 2);
    for i in 0..SAMPLES {
        let t = i as f32 / SAMPLE_RATE as f32;
        let sample = (t * 440.0 * std::f32::consts::TAU).sin();
        let value = (sample * i16::MAX as f32 * 0.2) as i16;
        pcm.extend_from_slice(&value.to_le_bytes());
    }

    let data_len = pcm.len() as u32;
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&pcm);
    wav
}
