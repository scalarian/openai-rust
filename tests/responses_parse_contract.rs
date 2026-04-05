use openai_rust::{
    ErrorKind, OpenAI,
    resources::responses::{
        FunctionTool, ResponseFormatTextConfig, ResponseFormatTextJSONSchemaConfig,
        ResponseParseParams, ResponseTextConfig,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct Scorecard {
    winner: String,
    score: u32,
}

#[test]
fn parse_returns_typed_output_and_strict_tool_arguments() {
    let server = mock_http::MockHttpServer::spawn(json_response(parsed_response_payload(
        r#"{"winner":"Dodgers","score":4}"#,
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .responses()
        .parse::<Scorecard>(ResponseParseParams {
            model: "gpt-4.1-nano".into(),
            input: Some(json!("who won?")),
            text: Some(ResponseTextConfig {
                format: Some(ResponseFormatTextConfig::JsonSchema(
                    ResponseFormatTextJSONSchemaConfig {
                        name: "scorecard".into(),
                        schema: json!({
                            "type": "object",
                            "properties": {
                                "winner": {"type": "string"},
                                "score": {"type": "integer"}
                            },
                            "required": ["winner", "score"],
                            "additionalProperties": false
                        }),
                        strict: Some(true),
                        description: None,
                    },
                )),
                verbosity: None,
            }),
            tools: vec![FunctionTool {
                name: "lookup_box_score".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "game_id": {"type": "integer"}
                    },
                    "required": ["game_id"],
                    "additionalProperties": false
                }),
                strict: Some(true),
                description: Some("Look up the game".into()),
                defer_loading: None,
            }],
            ..Default::default()
        })
        .unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/responses");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["stream"], false);
    assert_eq!(body["text"]["format"]["type"], "json_schema");
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["strict"], true);

    assert_eq!(
        response.output().output_parsed(),
        Some(&Scorecard {
            winner: "Dodgers".into(),
            score: 4
        })
    );
    let function_call = response
        .output()
        .output
        .iter()
        .find(|item| item.item_type == "function_call")
        .expect("function call");
    assert_eq!(
        function_call.parsed_arguments.as_ref().unwrap()["game_id"],
        json!(7)
    );
}

#[test]
fn parse_surfaces_explicit_refusal_and_parse_failures() {
    let refusal_server =
        mock_http::MockHttpServer::spawn(json_response(refusal_response_payload())).unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(refusal_server.url())
        .max_retries(0)
        .build();

    let refusal = client
        .responses()
        .parse::<Scorecard>(parse_params())
        .expect_err("refusal should fail explicitly");
    assert_eq!(refusal.kind, ErrorKind::Parse);
    assert!(refusal.message.contains("refusal"));

    let parse_server = mock_http::MockHttpServer::spawn(json_response(parsed_response_payload(
        r#"{"winner":"Dodgers""#,
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(parse_server.url())
        .max_retries(0)
        .build();

    let parse_error = client
        .responses()
        .parse::<Scorecard>(parse_params())
        .expect_err("invalid structured output should fail explicitly");
    assert_eq!(parse_error.kind, ErrorKind::Parse);
    assert!(parse_error.message.contains("structured output"));
}

fn parse_params() -> ResponseParseParams {
    ResponseParseParams {
        model: "gpt-4.1-nano".into(),
        input: Some(json!("who won?")),
        text: Some(ResponseTextConfig {
            format: Some(ResponseFormatTextConfig::JsonSchema(
                ResponseFormatTextJSONSchemaConfig {
                    name: "scorecard".into(),
                    schema: json!({
                        "type": "object",
                        "properties": {
                            "winner": {"type": "string"},
                            "score": {"type": "integer"}
                        },
                        "required": ["winner", "score"],
                        "additionalProperties": false
                    }),
                    strict: Some(true),
                    description: None,
                },
            )),
            verbosity: None,
        }),
        ..Default::default()
    }
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

fn parsed_response_payload(output_text: &str) -> String {
    json!({
        "id": "resp_parse",
        "object": "response",
        "created_at": 1,
        "status": "completed",
        "output": [
            {
                "id": "fc_1",
                "type": "function_call",
                "call_id": "call_1",
                "name": "lookup_box_score",
                "arguments": "{\"game_id\":7}",
                "status": "completed"
            },
            {
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "output_text", "text": output_text}
                ]
            }
        ],
        "usage": {"input_tokens": 4, "output_tokens": 6, "total_tokens": 10}
    })
    .to_string()
}

fn refusal_response_payload() -> String {
    json!({
        "id": "resp_refusal",
        "object": "response",
        "created_at": 1,
        "status": "completed",
        "output": [
            {
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "refusal", "text": "I can't help with that."}
                ]
            }
        ],
        "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}
    })
    .to_string()
}
