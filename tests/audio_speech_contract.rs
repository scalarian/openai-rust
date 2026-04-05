use openai_rust::OpenAI;
use serde_json::json;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn speech_preserves_binary_and_non_json_stream_modes() {
    let sse_body = concat!(
        "event: response.output_audio.delta\n",
        "data: {\"type\":\"response.output_audio.delta\",\"delta\":\"cGNt\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\"}\n\n"
    );
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        binary_response("audio/mpeg", vec![1, 2, 3, 4, 5]),
        binary_response("application/octet-stream", vec![9, 8, 7]),
        binary_response("text/event-stream", sse_body.as_bytes().to_vec()),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let mp3 = client
        .audio()
        .speech
        .create(openai_rust::resources::audio::SpeechParams {
            input: String::from("hello world"),
            model: String::from("gpt-4o-mini-tts"),
            voice: openai_rust::resources::audio::SpeechVoice::Named(String::from("alloy")),
            response_format: Some(openai_rust::resources::audio::SpeechResponseFormat::Mp3),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(mp3.output(), &vec![1, 2, 3, 4, 5]);

    let raw_audio = client
        .audio()
        .speech
        .create(openai_rust::resources::audio::SpeechParams {
            input: String::from("fast"),
            model: String::from("gpt-4o-mini-tts"),
            voice: openai_rust::resources::audio::SpeechVoice::Named(String::from("ash")),
            response_format: Some(openai_rust::resources::audio::SpeechResponseFormat::Wav),
            stream_format: Some(openai_rust::resources::audio::SpeechStreamFormat::Audio),
            speed: Some(1.25),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(raw_audio.output(), &vec![9, 8, 7]);

    let sse = client
        .audio()
        .speech
        .create(openai_rust::resources::audio::SpeechParams {
            input: String::from("stream it"),
            model: String::from("gpt-4o-mini-tts"),
            voice: openai_rust::resources::audio::SpeechVoice::Custom {
                id: String::from("voice_123"),
            },
            instructions: Some(String::from("Speak brightly")),
            response_format: Some(openai_rust::resources::audio::SpeechResponseFormat::Pcm),
            stream_format: Some(openai_rust::resources::audio::SpeechStreamFormat::Sse),
            ..Default::default()
        })
        .unwrap();
    let sse_text = String::from_utf8(sse.output().clone()).expect("SSE text bytes");
    assert!(sse_text.contains("response.output_audio.delta"));
    assert!(sse_text.contains("response.completed"));

    let requests = server.captured_requests(3).expect("captured requests");
    for request in &requests {
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/audio/speech");
        assert_eq!(
            request.headers.get("accept").map(String::as_str),
            Some("application/octet-stream")
        );
    }

    let first_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(first_body["voice"], "alloy");
    assert_eq!(first_body["response_format"], "mp3");

    let second_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(second_body["stream_format"], "audio");
    assert_eq!(second_body["speed"], json!(1.25));

    let third_body: serde_json::Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(third_body["stream_format"], "sse");
    assert_eq!(third_body["voice"]["id"], "voice_123");
    assert_eq!(third_body["instructions"], "Speak brightly");
}

fn binary_response(content_type: &str, body: Vec<u8>) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-type"), String::from(content_type)),
            (String::from("content-length"), body.len().to_string()),
        ],
        body,
        ..Default::default()
    }
}
