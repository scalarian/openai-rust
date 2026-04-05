use openai_rust::{
    OpenAI,
    resources::responses::{
        FunctionTool, ResponseFormatTextConfig, ResponseInputTokensCountParams, ResponseTextConfig,
    },
};
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn input_tokens_count_forwards_modalities_and_tools() {
    let server = mock_http::MockHttpServer::spawn(json_response(
        json!({
            "object": "response.input_tokens",
            "input_tokens": 123,
            "input_tokens_details": {
                "cached_tokens": 11
            }
        })
        .to_string(),
    ))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .responses()
        .input_tokens()
        .count(ResponseInputTokensCountParams {
            model: Some("gpt-4.1-nano".into()),
            input: Some(json!([
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "describe this"},
                        {"type": "input_image", "image_url": "https://example.com/cat.png"}
                    ]
                }
            ])),
            instructions: Some("Be brief".into()),
            parallel_tool_calls: Some(true),
            text: Some(ResponseTextConfig {
                format: Some(ResponseFormatTextConfig::Text),
                verbosity: Some("low".into()),
            }),
            tools: vec![FunctionTool {
                name: "lookup_weather".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {"city": {"type": "string"}},
                    "required": ["city"]
                }),
                strict: Some(true),
                description: Some("Weather lookup".into()),
                defer_loading: None,
            }],
            truncation: Some("auto".into()),
            ..Default::default()
        })
        .unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/responses/input_tokens");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["model"], "gpt-4.1-nano");
    assert_eq!(body["instructions"], "Be brief");
    assert_eq!(body["parallel_tool_calls"], true);
    assert_eq!(body["input"][0]["content"][1]["type"], "input_image");
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["strict"], true);
    assert_eq!(body["text"]["verbosity"], "low");
    assert_eq!(body["truncation"], "auto");

    assert_eq!(response.output().object, "response.input_tokens");
    assert_eq!(response.output().input_tokens, 123);
    assert_eq!(
        response.output().extra["input_tokens_details"]["cached_tokens"],
        json!(11)
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
