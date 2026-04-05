use openai_rust::OpenAI;
use serde_json::json;

#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

#[test]
fn translation_preserves_file_semantics_and_typed_bodies() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(json!({"text": "hello world"}).to_string()),
        json_response(
            json!({
                "duration": 1.5,
                "language": "english",
                "text": "hello world",
                "segments": [{
                    "id": 7,
                    "avg_logprob": -0.2,
                    "compression_ratio": 0.8,
                    "end": 1.5,
                    "no_speech_prob": 0.0,
                    "seek": 0,
                    "start": 0.0,
                    "temperature": 0.0,
                    "text": "hello world",
                    "tokens": [1, 2]
                }]
            })
            .to_string(),
        ),
        text_response("1\n00:00:00,000 --> 00:00:01,000\nhello world\n"),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let audio = openai_rust::resources::audio::AudioInput::new(
        "speech.mp3",
        "audio/mpeg",
        vec![1, 2, 3, 4, 5],
    );

    let json_translation = client
        .audio()
        .translations
        .create(openai_rust::resources::audio::TranslationParams {
            file: audio.clone(),
            model: String::from("whisper-1"),
            prompt: Some(String::from("Translate naturally")),
            temperature: Some(0.4),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::Json),
            ..Default::default()
        })
        .unwrap();
    match json_translation.output() {
        openai_rust::resources::audio::TranslationResponse::Json(payload) => {
            assert_eq!(payload.text, "hello world");
        }
        other => panic!("expected json translation, got {other:?}"),
    }

    let verbose_translation = client
        .audio()
        .translations
        .create(openai_rust::resources::audio::TranslationParams {
            file: audio.clone(),
            model: String::from("whisper-1"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::VerboseJson),
            ..Default::default()
        })
        .unwrap();
    match verbose_translation.output() {
        openai_rust::resources::audio::TranslationResponse::VerboseJson(payload) => {
            assert_eq!(payload.language, "english");
            assert_eq!(payload.segments.as_ref().unwrap()[0].text, "hello world");
        }
        other => panic!("expected verbose translation, got {other:?}"),
    }

    let srt_translation = client
        .audio()
        .translations
        .create(openai_rust::resources::audio::TranslationParams {
            file: audio.clone(),
            model: String::from("whisper-1"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::Srt),
            ..Default::default()
        })
        .unwrap();
    match srt_translation.output() {
        openai_rust::resources::audio::TranslationResponse::Srt(body) => {
            assert!(body.contains("hello world"));
            assert!(body.contains("-->"));
        }
        other => panic!("expected SRT translation, got {other:?}"),
    }

    let requests = server.captured_requests(3).expect("captured requests");
    for request in &requests {
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/audio/translations");
        let multipart = multipart_support::parse_multipart(
            &request.body,
            &boundary_from_headers(&request.headers),
        )
        .unwrap();
        let file_part = multipart
            .parts
            .iter()
            .find(|part| part.name.as_deref() == Some("file"))
            .expect("file part");
        assert_eq!(file_part.filename.as_deref(), Some("speech.mp3"));
        assert_eq!(
            file_part.headers.get("content-type").map(String::as_str),
            Some("audio/mpeg")
        );
        assert_eq!(file_part.body, audio.bytes);
    }

    let first_body = multipart_support::parse_multipart(
        &requests[0].body,
        &boundary_from_headers(&requests[0].headers),
    )
    .unwrap();
    assert_text_part(&first_body, "prompt", "Translate naturally");
    assert_text_part(&first_body, "temperature", "0.4");
    assert_text_part(&first_body, "response_format", "json");

    let second_body = multipart_support::parse_multipart(
        &requests[1].body,
        &boundary_from_headers(&requests[1].headers),
    )
    .unwrap();
    assert_text_part(&second_body, "response_format", "verbose_json");

    let third_body = multipart_support::parse_multipart(
        &requests[2].body,
        &boundary_from_headers(&requests[2].headers),
    )
    .unwrap();
    assert_text_part(&third_body, "response_format", "srt");
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
