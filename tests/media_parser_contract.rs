use serde_json::json;

#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    OpenAI,
    resources::{
        audio::{
            AudioInput, AudioResponseFormat, SpeechParams, SpeechResponseFormat, SpeechVoice,
            TranscriptionParams, TranslationParams,
        },
        images::{ImageGenerateParams, ImageGenerationStreamEvent},
    },
};

#[test]
fn shared_media_parser_selects_json_text_binary_and_sse_paths() {
    let image_stream = concat!(
        "event: image_generation.partial_image\n",
        "data: {\"type\":\"image_generation.partial_image\",\"partial_image_index\":0,\"b64_json\":\"aW1n\",\"created_at\":1717171717,\"background\":\"transparent\",\"output_format\":\"png\",\"quality\":\"high\",\"size\":\"1024x1024\"}\n\n",
        "event: image_generation.completed\n",
        "data: {\"type\":\"image_generation.completed\",\"b64_json\":\"aW1nLWZpbmFs\",\"created_at\":1717171718,\"background\":\"transparent\",\"output_format\":\"png\",\"quality\":\"high\",\"size\":\"1024x1024\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2,\"input_tokens_details\":{\"text_tokens\":1,\"image_tokens\":0}}}\n\n"
    );

    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(json!({"text": "transcribed", "usage": {"total_tokens": 9}})),
        text_response("translated text"),
        binary_response("audio/mpeg", vec![1, 2, 3, 4]),
        sse_response(image_stream),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let transcription = client
        .audio()
        .transcriptions
        .create(TranscriptionParams {
            file: AudioInput::new("clip.wav", "audio/wav", vec![1, 2, 3]),
            model: "gpt-4o-mini-transcribe".into(),
            response_format: Some(AudioResponseFormat::Json),
            ..Default::default()
        })
        .unwrap();
    assert!(matches!(
        transcription.output(),
        openai_rust::resources::audio::TranscriptionResponse::Json(payload)
            if payload.text == "transcribed"
    ));

    let translation = client
        .audio()
        .translations
        .create(TranslationParams {
            file: AudioInput::new("clip.wav", "audio/wav", vec![1, 2, 3]),
            model: "gpt-4o-mini-transcribe".into(),
            response_format: Some(AudioResponseFormat::Text),
            ..Default::default()
        })
        .unwrap();
    assert!(matches!(
        translation.output(),
        openai_rust::resources::audio::TranslationResponse::Text(text) if text == "translated text"
    ));

    let speech = client
        .audio()
        .speech
        .create(SpeechParams {
            input: "Say hi".into(),
            model: "gpt-4o-mini-tts".into(),
            voice: SpeechVoice::Named("alloy".into()),
            response_format: Some(SpeechResponseFormat::Mp3),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(speech.output(), &vec![1, 2, 3, 4]);

    let mut stream = client
        .images()
        .generate_stream(ImageGenerateParams {
            prompt: "draw a square".into(),
            stream: Some(true),
            ..Default::default()
        })
        .unwrap();
    let first = stream.next_event().expect("partial image event");
    let second = stream.next_event().expect("completed image event");
    assert!(matches!(first, ImageGenerationStreamEvent::PartialImage(_)));
    assert!(matches!(second, ImageGenerationStreamEvent::Completed(_)));
}

fn json_response(body: serde_json::Value) -> mock_http::ScriptedResponse {
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
