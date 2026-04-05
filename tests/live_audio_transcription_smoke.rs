use openai_rust::{DEFAULT_BASE_URL, OpenAI};

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_audio_transcription_smoke_captures_request_id() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live transcription client should resolve configuration");
    assert_eq!(resolved.base_url, DEFAULT_BASE_URL);

    let speech = client
        .audio()
        .speech
        .create(openai_rust::resources::audio::SpeechParams {
            input: String::from("hello from openai rust"),
            model: String::from("gpt-4o-mini-tts"),
            voice: openai_rust::resources::audio::SpeechVoice::Named(String::from("alloy")),
            response_format: Some(openai_rust::resources::audio::SpeechResponseFormat::Wav),
            ..Default::default()
        })
        .expect("live speech generation should succeed");
    assert!(
        !speech.output().is_empty(),
        "live speech bytes should not be empty"
    );

    let response = client
        .audio()
        .transcriptions
        .create(openai_rust::resources::audio::TranscriptionParams {
            file: openai_rust::resources::audio::AudioInput::new(
                "tiny.wav",
                "audio/wav",
                speech.output().clone(),
            ),
            model: String::from("gpt-4o-mini-transcribe"),
            response_format: Some(openai_rust::resources::audio::AudioResponseFormat::Json),
            ..Default::default()
        })
        .expect("live transcription request should succeed");

    let request_id = response
        .request_id()
        .expect("live transcription response should expose a request id");
    assert!(!request_id.trim().is_empty());

    match response.output() {
        openai_rust::resources::audio::TranscriptionResponse::Json(payload) => {
            assert!(!payload.text.trim().is_empty());
            println!("live transcription text: {}", payload.text);
        }
        other => panic!("expected json transcription response, got {other:?}"),
    }

    println!("live transcription request id: {request_id}");
}
