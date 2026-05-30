#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    OpenAI,
    resources::beta::{
        BetaRealtimeAudioFormat, BetaRealtimeClientSecret, BetaRealtimeClientSecretAnchor,
        BetaRealtimeClientSecretExpiresAfter, BetaRealtimeClientSecretExpiresAt,
        BetaRealtimeInputAudioNoiseReduction, BetaRealtimeInputAudioTranscription,
        BetaRealtimeInputAudioTranscriptionModel, BetaRealtimeMaxResponseOutputTokens,
        BetaRealtimeModality, BetaRealtimeNoiseReductionType, BetaRealtimeNullable,
        BetaRealtimeSessionCreateParams, BetaRealtimeSessionModel, BetaRealtimeTool,
        BetaRealtimeToolType, BetaRealtimeTracing, BetaRealtimeTranscriptionClientSecret,
        BetaRealtimeTranscriptionSessionCreateParams, BetaRealtimeTurnDetection,
        BetaRealtimeTurnDetectionType,
    },
};
use serde_json::json;

#[test]
fn beta_realtime_sessions_preserve_upstream_routes_headers_and_flexible_bodies() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(realtime_session_payload("ek_realtime")),
        json_response(realtime_session_payload("ek_transcription")),
    ])
    .unwrap();
    let client = client(&server.url());
    let realtime = client.beta().realtime();

    let session = realtime
        .sessions()
        .create(BetaRealtimeSessionCreateParams {
            model: Some(BetaRealtimeSessionModel::GptRealtime),
            modalities: Some(vec![
                BetaRealtimeModality::Text,
                BetaRealtimeModality::Audio,
            ]),
            voice: Some(String::from("verse")),
            client_secret: Some(BetaRealtimeClientSecret {
                expires_after: Some(BetaRealtimeClientSecretExpiresAfter {
                    anchor: Some(BetaRealtimeClientSecretAnchor::CreatedAt),
                    seconds: Some(120),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            input_audio_noise_reduction: Some(BetaRealtimeNullable::Value(
                BetaRealtimeInputAudioNoiseReduction {
                    noise_reduction_type: Some(BetaRealtimeNoiseReductionType::NearField),
                    ..Default::default()
                },
            )),
            max_response_output_tokens: Some(BetaRealtimeMaxResponseOutputTokens::Inf),
            tool_choice: Some(String::from("auto")),
            tools: Some(vec![BetaRealtimeTool {
                name: Some(String::from("lookup_case")),
                parameters: Some(json!({"type": "object"})),
                tool_type: Some(BetaRealtimeToolType::Function),
                ..Default::default()
            }]),
            tracing: Some(BetaRealtimeNullable::Value(BetaRealtimeTracing::Auto)),
            turn_detection: Some(BetaRealtimeNullable::Value(BetaRealtimeTurnDetection {
                turn_detection_type: Some(BetaRealtimeTurnDetectionType::ServerVad),
                threshold: Some(0.5),
                ..Default::default()
            })),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        session.output["client_secret"]["value"],
        json!("ek_realtime")
    );

    let transcription = realtime
        .transcription_sessions()
        .create(BetaRealtimeTranscriptionSessionCreateParams {
            modalities: Some(vec![BetaRealtimeModality::Text]),
            input_audio_format: Some(BetaRealtimeAudioFormat::Pcm16),
            include: Some(vec![String::from(
                "item.input_audio_transcription.logprobs",
            )]),
            input_audio_transcription: Some(BetaRealtimeNullable::Value(
                BetaRealtimeInputAudioTranscription {
                    model: Some(BetaRealtimeInputAudioTranscriptionModel::Gpt4oTranscribe),
                    language: Some(String::from("en")),
                    ..Default::default()
                },
            )),
            client_secret: Some(BetaRealtimeTranscriptionClientSecret {
                expires_at: Some(BetaRealtimeClientSecretExpiresAt {
                    anchor: Some(BetaRealtimeClientSecretAnchor::CreatedAt),
                    seconds: Some(300),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            turn_detection: Some(BetaRealtimeNullable::Null),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        transcription.output["client_secret"]["value"],
        json!("ek_transcription")
    );

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/realtime/sessions");
    assert_eq!(requests[1].path, "/v1/realtime/transcription_sessions");
    for request in &requests {
        assert_eq!(
            request.headers.get("openai-beta").map(String::as_str),
            Some("assistants=v2")
        );
    }

    let session_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(session_body["model"], json!("gpt-realtime"));
    assert_eq!(session_body["voice"], json!("verse"));
    assert_eq!(
        session_body["client_secret"]["expires_after"]["seconds"],
        json!(120)
    );
    assert_eq!(
        session_body["input_audio_noise_reduction"]["type"],
        json!("near_field")
    );
    assert_eq!(session_body["max_response_output_tokens"], json!("inf"));
    assert_eq!(session_body["tools"][0]["type"], json!("function"));
    assert_eq!(session_body["tracing"], json!("auto"));

    let transcription_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(transcription_body["input_audio_format"], json!("pcm16"));
    assert_eq!(
        transcription_body["input_audio_transcription"]["model"],
        json!("gpt-4o-transcribe")
    );
    assert_eq!(
        transcription_body["turn_detection"],
        serde_json::Value::Null
    );
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn realtime_session_payload(secret: &str) -> String {
    json!({
        "client_secret": {
            "value": secret,
            "expires_at": 1_800_000_000u64
        },
        "modalities": ["text"],
        "input_audio_format": "pcm16",
        "turn_detection": {
            "type": "server_vad",
            "threshold": 0.5
        }
    })
    .to_string()
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (
                String::from("x-request-id"),
                String::from("req_beta_realtime"),
            ),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}
