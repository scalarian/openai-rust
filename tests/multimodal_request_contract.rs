use serde_json::Value;

#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    OpenAI,
    resources::{
        chat::ChatCompletionCreateParams,
        multimodal::{
            ChatCompletionContentPart, ChatCompletionMessage, ImageDetail, InputAudioData,
            InputAudioFormat, ResponseInputMessage, ResponseInputPart,
        },
        responses::ResponseCreateParams,
    },
};

#[test]
fn shared_multimodal_request_builders_preserve_ordered_text_image_audio_parts() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(serde_json::json!({
            "id": "resp_mm",
            "object": "response",
            "created_at": 1,
            "output": [],
            "usage": {}
        })),
        json_response(serde_json::json!({
            "id": "chatcmpl_mm",
            "object": "chat.completion",
            "created": 1,
            "choices": [
                {
                    "index": 0,
                    "finish_reason": "stop",
                    "message": {"role": "assistant", "content": "ok"}
                }
            ]
        })),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    client
        .responses()
        .create(
            ResponseCreateParams {
                model: "gpt-4.1-mini".into(),
                ..Default::default()
            }
            .with_serialized_input(vec![ResponseInputMessage::user(vec![
                ResponseInputPart::input_text("Describe the clip"),
                ResponseInputPart::input_image_url(
                    "data:image/png;base64,AAAA",
                    Some(ImageDetail::High),
                ),
                ResponseInputPart::input_audio(InputAudioData {
                    data: "UklGRg==".into(),
                    format: InputAudioFormat::Mp3,
                }),
            ])])
            .expect("serialize response multimodal input"),
        )
        .unwrap();

    client
        .chat()
        .completions()
        .create(
            ChatCompletionCreateParams {
                model: "gpt-4.1-mini".into(),
                ..Default::default()
            }
            .with_serialized_messages(vec![ChatCompletionMessage::user_parts(vec![
                ChatCompletionContentPart::text("Describe the clip"),
                ChatCompletionContentPart::image_url(
                    "data:image/png;base64,AAAA",
                    Some(ImageDetail::Low),
                ),
                ChatCompletionContentPart::input_audio(InputAudioData {
                    data: "UklGRg==".into(),
                    format: InputAudioFormat::Wav,
                }),
            ])])
            .expect("serialize chat multimodal message"),
        )
        .unwrap();

    let requests = server.captured_requests(2).expect("captured requests");

    let response_body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    let response_parts = response_body["input"][0]["content"].as_array().unwrap();
    assert_eq!(response_parts[0]["type"], "input_text");
    assert_eq!(response_parts[0]["text"], "Describe the clip");
    assert_eq!(response_parts[1]["type"], "input_image");
    assert_eq!(response_parts[1]["image_url"], "data:image/png;base64,AAAA");
    assert_eq!(response_parts[1]["detail"], "high");
    assert_eq!(response_parts[2]["type"], "input_audio");
    assert_eq!(response_parts[2]["input_audio"]["data"], "UklGRg==");
    assert_eq!(response_parts[2]["input_audio"]["format"], "mp3");

    let chat_body: Value = serde_json::from_slice(&requests[1].body).unwrap();
    let chat_parts = chat_body["messages"][0]["content"].as_array().unwrap();
    assert_eq!(chat_parts[0]["type"], "text");
    assert_eq!(chat_parts[0]["text"], "Describe the clip");
    assert_eq!(chat_parts[1]["type"], "image_url");
    assert_eq!(
        chat_parts[1]["image_url"]["url"],
        "data:image/png;base64,AAAA"
    );
    assert_eq!(chat_parts[1]["image_url"]["detail"], "low");
    assert_eq!(chat_parts[2]["type"], "input_audio");
    assert_eq!(chat_parts[2]["input_audio"]["data"], "UklGRg==");
    assert_eq!(chat_parts[2]["input_audio"]["format"], "wav");
}

#[test]
fn responses_multimodal_request_preserves_text_image_and_file_field_names() {
    let server = mock_http::MockHttpServer::spawn(json_response(serde_json::json!({
        "id": "resp_file",
        "object": "response",
        "created_at": 1,
        "output": [],
        "usage": {}
    })))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    client
        .responses()
        .create(
            ResponseCreateParams {
                model: "gpt-4.1-mini".into(),
                ..Default::default()
            }
            .with_serialized_input(vec![ResponseInputMessage::user(vec![
                ResponseInputPart::input_text("Compare these inputs"),
                ResponseInputPart::input_image_file("file-image-123", Some(ImageDetail::Auto)),
                ResponseInputPart::input_file_id("file-doc-123"),
                ResponseInputPart::input_file_url("https://example.com/brief.txt"),
                ResponseInputPart::input_file_data("ZmlsZSBjb250ZW50cw==", "brief.txt"),
            ])])
            .expect("serialize responses multimodal file input"),
        )
        .unwrap();

    let request = server.captured_request().expect("captured request");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    let parts = body["input"][0]["content"].as_array().unwrap();

    assert_eq!(parts[0]["type"], "input_text");
    assert_eq!(parts[1]["type"], "input_image");
    assert_eq!(parts[1]["file_id"], "file-image-123");
    assert_eq!(parts[1]["detail"], "auto");

    assert_eq!(parts[2]["type"], "input_file");
    assert_eq!(parts[2]["file_id"], "file-doc-123");
    assert!(parts[2].get("image_url").is_none());

    assert_eq!(parts[3]["type"], "input_file");
    assert_eq!(parts[3]["file_url"], "https://example.com/brief.txt");

    assert_eq!(parts[4]["type"], "input_file");
    assert_eq!(parts[4]["file_data"], "ZmlsZSBjb250ZW50cw==");
    assert_eq!(parts[4]["filename"], "brief.txt");
}

fn json_response(body: Value) -> mock_http::ScriptedResponse {
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
