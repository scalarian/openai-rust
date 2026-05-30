use std::collections::BTreeMap;

use openai_rust::{
    ApiErrorKind, ErrorKind, OpenAI,
    resources::responses::{
        ResponseCodeInterpreterTool, ResponseComputerAction, ResponseConversation,
        ResponseConversationObject, ResponseFormatTextConfig, ResponsePrompt, ResponseReasoning,
        ResponseTool, ResponseToolChoice, ResponseWebSearchPreviewTool,
    },
};
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn create_populates_output_text_helper() {
    let server = mock_http::MockHttpServer::spawn(json_response(response_payload(
        "resp_create",
        Some(true),
        Some("resp_prev"),
        Some(json!("conv_123")),
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            background: Some(true),
            context_management: vec![json!({"type": "auto"})],
            include: vec![String::from("message.output_text.logprobs")],
            input: Some(json!("hello")),
            instructions: Some(String::from("Be concise.")),
            max_output_tokens: Some(512),
            max_tool_calls: Some(4),
            metadata: Some(json!({"trace": "resp_create"})),
            parallel_tool_calls: Some(false),
            previous_response_id: Some("resp_prev".into()),
            conversation: Some(ResponseConversation::Id(String::from("conv_123"))),
            prompt: Some(ResponsePrompt {
                id: String::from("pmpt_123"),
                variables: Some(BTreeMap::from([(String::from("topic"), json!("Rust"))])),
                ..Default::default()
            }),
            prompt_cache_key: Some(String::from("cache-key")),
            prompt_cache_retention: Some(String::from("24h")),
            reasoning: Some(ResponseReasoning {
                effort: Some(String::from("low")),
                ..Default::default()
            }),
            safety_identifier: Some(String::from("user_hash")),
            service_tier: Some(String::from("priority")),
            store: Some(true),
            stream: Some(false),
            stream_options: Some(json!({"include_usage": true})),
            temperature: Some(0.2),
            tool_choice: Some(ResponseToolChoice::Function {
                name: String::from("lookup_weather"),
            }),
            top_logprobs: Some(2),
            top_p: Some(0.8),
            truncation: Some(String::from("auto")),
            user: Some(String::from("legacy-user")),
            tools: vec![
                ResponseTool::WebSearchPreview(ResponseWebSearchPreviewTool {
                    search_content_types: Vec::new(),
                    search_context_size: Some(String::from("low")),
                    user_location: None,
                    extra: BTreeMap::new(),
                }),
                ResponseTool::CodeInterpreter(ResponseCodeInterpreterTool {
                    container: json!("auto"),
                    extra: BTreeMap::new(),
                }),
            ],
            ..Default::default()
        })
        .unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/responses");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["model"], "gpt-4.1-nano");
    assert_eq!(body["background"], true);
    assert_eq!(body["context_management"][0]["type"], "auto");
    assert_eq!(body["include"], json!(["message.output_text.logprobs"]));
    assert_eq!(body["input"], "hello");
    assert_eq!(body["instructions"], "Be concise.");
    assert_eq!(body["max_output_tokens"], 512);
    assert_eq!(body["max_tool_calls"], 4);
    assert_eq!(body["metadata"]["trace"], "resp_create");
    assert_eq!(body["parallel_tool_calls"], false);
    assert_eq!(body["previous_response_id"], "resp_prev");
    assert_eq!(body["conversation"], "conv_123");
    assert_eq!(body["prompt"]["id"], "pmpt_123");
    assert_eq!(body["prompt"]["variables"]["topic"], "Rust");
    assert_eq!(body["prompt_cache_key"], "cache-key");
    assert_eq!(body["prompt_cache_retention"], "24h");
    assert_eq!(body["reasoning"]["effort"], "low");
    assert_eq!(body["safety_identifier"], "user_hash");
    assert_eq!(body["service_tier"], "priority");
    assert_eq!(body["store"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["temperature"], 0.2);
    assert_eq!(body["tool_choice"]["type"], "function");
    assert_eq!(body["tool_choice"]["name"], "lookup_weather");
    assert_eq!(body["stream"], false);
    assert_eq!(body["top_logprobs"], 2);
    assert_eq!(body["top_p"], 0.8);
    assert_eq!(body["truncation"], "auto");
    assert_eq!(body["user"], "legacy-user");
    assert_eq!(body["tools"][0]["type"], "web_search_preview");
    assert_eq!(body["tools"][0]["search_context_size"], "low");
    assert_eq!(body["tools"][1]["type"], "code_interpreter");
    assert_eq!(body["tools"][1]["container"], "auto");
    assert_eq!(response.output().id, "resp_create");
    assert_eq!(response.output().object, "response");
    assert_eq!(response.output().created_at, 1.25);
    assert_eq!(response.output().status.as_deref(), Some("completed"));
    assert_eq!(response.output().model.as_deref(), Some("gpt-4.1-nano"));
    assert_eq!(
        response.output().instructions,
        Some(json!("Server instructions"))
    );
    assert_eq!(response.output().parallel_tool_calls, Some(true));
    assert_eq!(
        response.output().previous_response_id.as_deref(),
        Some("resp_prev")
    );
    assert_eq!(
        response.output().conversation,
        Some(ResponseConversation::Id(String::from("conv_123")))
    );
    assert_eq!(response.output().store, Some(true));
    assert_eq!(response.output().background, Some(false));
    assert_eq!(response.output().completed_at, Some(2.5));
    assert_eq!(response.output().max_output_tokens, Some(512));
    assert_eq!(response.output().max_tool_calls, Some(4));
    let prompt = response.output().prompt.as_ref().unwrap();
    assert_eq!(prompt.id, "pmpt_response");
    assert_eq!(prompt.variables.as_ref().unwrap()["topic"], json!("Rust"));
    assert_eq!(
        response.output().prompt_cache_key.as_deref(),
        Some("response-cache-key")
    );
    assert_eq!(
        response.output().prompt_cache_retention.as_deref(),
        Some("24h")
    );
    assert_eq!(
        response
            .output()
            .reasoning
            .as_ref()
            .and_then(|reasoning| reasoning.effort.as_deref()),
        Some("low")
    );
    assert_eq!(
        response.output().safety_identifier.as_deref(),
        Some("response_user_hash")
    );
    assert_eq!(response.output().service_tier.as_deref(), Some("priority"));
    assert_eq!(response.output().temperature, Some(0.2));
    assert_eq!(
        response
            .output()
            .text
            .as_ref()
            .and_then(|text| text.format.as_ref()),
        Some(&ResponseFormatTextConfig::Text)
    );
    assert_eq!(
        response.output().tool_choice,
        Some(ResponseToolChoice::Auto)
    );
    assert_eq!(
        response.output().tools,
        vec![ResponseTool::WebSearchPreview(
            ResponseWebSearchPreviewTool {
                search_content_types: Vec::new(),
                search_context_size: None,
                user_location: None,
                extra: BTreeMap::new(),
            }
        )]
    );
    let mcp_list = response
        .output()
        .output
        .iter()
        .find(|item| item.item_type == "mcp_list_tools")
        .expect("mcp list tools item");
    assert_eq!(mcp_list.server_label.as_deref(), Some("deepwiki"));
    assert_eq!(mcp_list.tools[0].name, "search_docs");
    assert_eq!(mcp_list.tools[0].input_schema["type"], "object");
    assert_eq!(
        mcp_list.tools[0].annotations.as_ref().unwrap()["readOnlyHint"],
        true
    );
    assert_eq!(
        mcp_list.tools[0].description.as_deref(),
        Some("Search docs")
    );
    let computer_call = response
        .output()
        .output
        .iter()
        .find(|item| item.item_type == "computer_call")
        .expect("computer call item");
    assert_eq!(computer_call.call_id.as_deref(), Some("call_computer"));
    assert_eq!(
        computer_call.pending_safety_checks[0].id,
        "safety_pending_1"
    );
    assert_eq!(
        computer_call.pending_safety_checks[0].code.as_deref(),
        Some("unsafe_browser")
    );
    assert_eq!(
        computer_call.pending_safety_checks[0].message.as_deref(),
        Some("Browser confirmation required")
    );
    assert!(matches!(
        computer_call.action.as_ref(),
        Some(ResponseComputerAction::Click(action))
            if action.button == "left" && action.x == 10 && action.y == 20
    ));
    assert!(matches!(
        computer_call.actions.as_ref().unwrap().as_slice(),
        [
            ResponseComputerAction::Keypress(_),
            ResponseComputerAction::Type(_),
            ResponseComputerAction::Wait
        ]
    ));
    let computer_output = response
        .output()
        .output
        .iter()
        .find(|item| item.item_type == "computer_call_output")
        .expect("computer output item");
    assert_eq!(
        computer_output.acknowledged_safety_checks[0].id,
        "safety_ack_1"
    );
    assert_eq!(
        computer_output.acknowledged_safety_checks[0]
            .message
            .as_deref(),
        Some("Acknowledged")
    );
    assert_eq!(response.output().top_logprobs, Some(2));
    assert_eq!(response.output().top_p, Some(0.8));
    assert_eq!(response.output().truncation.as_deref(), Some("auto"));
    assert_eq!(response.output().user.as_deref(), Some("legacy-user"));
    assert_eq!(
        response.output().metadata,
        Some(json!({"trace": "response_payload"}))
    );
    let usage = response.output().usage.as_ref().unwrap();
    assert_eq!(usage.input_tokens, Some(1));
    assert_eq!(usage.output_tokens, Some(2));
    assert_eq!(usage.total_tokens, Some(3));
    assert_eq!(
        usage.input_tokens_details.as_ref().unwrap().cached_tokens,
        Some(1)
    );
    assert_eq!(
        usage
            .output_tokens_details
            .as_ref()
            .unwrap()
            .reasoning_tokens,
        Some(1)
    );
    assert_eq!(response.output().output_text(), "Hello world!");
}

#[test]
fn retrieve_round_trips_output_text_and_query() {
    let server = mock_http::MockHttpServer::spawn(json_response(response_payload(
        "resp_store",
        Some(true),
        None,
        None,
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .responses()
        .retrieve(
            "resp_store",
            openai_rust::resources::responses::ResponseRetrieveParams {
                include: vec!["message.output_text.logprobs".into()],
                include_obfuscation: Some(true),
                starting_after: Some(7),
                stream: Some(false),
            },
        )
        .unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.path,
        "/v1/responses/resp_store?include=message.output_text.logprobs&include_obfuscation=true&starting_after=7&stream=false"
    );
    assert_eq!(response.output().id, "resp_store");
    assert_eq!(response.output().output_text(), "Hello world!");

    let error = client
        .responses()
        .retrieve("   ", Default::default())
        .expect_err("blank response id should be rejected locally");
    assert_eq!(error.kind, ErrorKind::Validation);
}

#[test]
fn delete_returns_unit() {
    let server = mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse {
        headers: vec![(String::from("content-length"), String::from("0"))],
        ..Default::default()
    })
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    client.responses().delete("resp_delete").unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.path, "/v1/responses/resp_delete");
    assert_eq!(
        request.headers.get("accept").map(String::as_str),
        Some("*/*")
    );

    let error = client
        .responses()
        .delete("")
        .expect_err("blank response id should be rejected locally");
    assert_eq!(error.kind, ErrorKind::Validation);
}

#[test]
fn cancel_posts_to_background_endpoint() {
    let server = mock_http::MockHttpServer::spawn(json_response(response_payload(
        "resp_bg",
        Some(true),
        None,
        None,
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client.responses().cancel("resp_bg").unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/responses/resp_bg/cancel");
    assert_eq!(response.output().id, "resp_bg");
    assert_eq!(response.output().output_text(), "Hello world!");

    let error = client
        .responses()
        .cancel("   ")
        .expect_err("blank response id should be rejected locally");
    assert_eq!(error.kind, ErrorKind::Validation);
}

#[test]
fn tool_and_refusal_fields_round_trip() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(response_payload_with_tool_and_refusal("resp_create")),
        json_response(response_payload_with_tool_and_refusal("resp_retrieve")),
        json_response(response_payload_with_tool_and_refusal("resp_cancel")),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let created = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            input: Some(json!("hello")),
            ..Default::default()
        })
        .unwrap();
    let retrieved = client
        .responses()
        .retrieve("resp_retrieve", Default::default())
        .unwrap();
    let cancelled = client.responses().cancel("resp_cancel").unwrap();

    for response in [created.output(), retrieved.output(), cancelled.output()] {
        let function_call = response
            .output
            .iter()
            .find(|item| item.item_type == "function_call")
            .expect("function_call item");
        assert_eq!(function_call.name.as_deref(), Some("lookup_weather"));
        assert_eq!(
            function_call.arguments.as_deref(),
            Some(r#"{"city":"Paris"}"#)
        );
        assert_eq!(
            function_call.arguments_json,
            Some(json!(r#"{"city":"Paris"}"#))
        );
        assert_eq!(function_call.call_id.as_deref(), Some("call_123"));
        assert_eq!(function_call.status.as_deref(), Some("completed"));
        assert_eq!(function_call.namespace.as_deref(), Some("weather"));
        assert_eq!(function_call.created_by.as_deref(), Some("assistant"));

        let text_message = response
            .output
            .iter()
            .find(|item| item.id.as_deref() == Some("msg_text"))
            .expect("text message");
        assert_eq!(text_message.status.as_deref(), Some("completed"));
        assert_eq!(text_message.phase.as_deref(), Some("final_answer"));
        assert_eq!(
            text_message.content[0].annotations,
            vec![json!({
                "type": "url_citation",
                "start_index": 0,
                "end_index": 5,
                "title": "Weather",
                "url": "https://example.com/weather"
            })]
        );
        assert_eq!(
            text_message.content[0].logprobs,
            Some(vec![json!({
                "token": "Hello",
                "bytes": [72, 101, 108, 108, 111],
                "logprob": -0.01,
                "top_logprobs": []
            })])
        );

        let reasoning = response
            .output
            .iter()
            .find(|item| item.item_type == "reasoning")
            .expect("reasoning item");
        assert_eq!(
            reasoning.summary,
            vec![json!({"type": "summary_text", "text": "Checked weather"})]
        );
        assert_eq!(
            reasoning.encrypted_content.as_deref(),
            Some("enc_reasoning")
        );
        assert_eq!(reasoning.content[0].content_type, "reasoning_text");

        let tool_search = response
            .output
            .iter()
            .find(|item| item.item_type == "tool_search_call")
            .expect("tool_search_call item");
        assert_eq!(tool_search.execution.as_deref(), Some("server"));
        assert_eq!(
            tool_search.arguments_json,
            Some(json!({"query": "weather"}))
        );
        assert_eq!(
            tool_search.arguments.as_deref(),
            Some(r#"{"query":"weather"}"#)
        );

        let file_search = response
            .output
            .iter()
            .find(|item| item.item_type == "file_search_call")
            .expect("file_search_call item");
        assert_eq!(file_search.queries, vec![String::from("docs")]);
        assert_eq!(
            file_search.results,
            Some(vec![
                json!({"file_id": "file_1", "filename": "guide.md", "score": 0.9})
            ])
        );

        let code_call = response
            .output
            .iter()
            .find(|item| item.item_type == "code_interpreter_call")
            .expect("code_interpreter_call item");
        assert_eq!(code_call.container_id.as_deref(), Some("cntr_123"));
        assert_eq!(
            code_call.outputs,
            Some(vec![json!({"type": "logs", "logs": "ok"})])
        );

        let mcp_call = response
            .output
            .iter()
            .find(|item| item.item_type == "mcp_call")
            .expect("mcp_call item");
        assert_eq!(mcp_call.server_label.as_deref(), Some("weather_mcp"));
        assert_eq!(
            mcp_call.approval_request_id.as_deref(),
            Some("approval_123")
        );
        assert_eq!(mcp_call.output, Some(json!("sunny")));

        let image_call = response
            .output
            .iter()
            .find(|item| item.item_type == "image_generation_call")
            .expect("image_generation_call item");
        assert_eq!(image_call.result.as_deref(), Some("aW1n"));

        let refusal_message = response
            .output
            .iter()
            .find(|item| item.id.as_deref() == Some("msg_refusal"))
            .expect("refusal message");
        let refusal_part = refusal_message
            .content
            .iter()
            .find(|part| part.content_type == "refusal")
            .expect("refusal content");
        assert_eq!(
            refusal_part.refusal.as_deref(),
            Some("I can't help with that")
        );
        assert_eq!(response.refusal_text(), Some("I can't help with that"));
        assert_eq!(response.output_text(), "Hello world!");
    }
}

#[test]
fn compact_returns_compaction_object() {
    let body = json!({
        "id": "cmp_123",
        "object": "response.compaction",
        "created_at": 1,
        "output": [
            {
                "id": "msg_user",
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "Original prompt"}
                ]
            },
            {
                "id": "cmp_item",
                "type": "compaction",
                "summary": "Summarized context"
            }
        ],
        "usage": {
            "input_tokens": 12,
            "output_tokens": 3,
            "total_tokens": 15
        }
    });
    let server = mock_http::MockHttpServer::spawn(json_value_response(body)).unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .responses()
        .compact(openai_rust::resources::responses::ResponseCompactParams {
            model: "gpt-4.1-nano".into(),
            input: Some(json!("follow-up")),
            previous_response_id: Some("resp_prev".into()),
            prompt_cache_key: Some(String::from("compact-cache")),
            prompt_cache_retention: Some(String::from("in_memory")),
            service_tier: Some(String::from("flex")),
            ..Default::default()
        })
        .unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/responses/compact");
    let request_body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(request_body["model"], "gpt-4.1-nano");
    assert_eq!(request_body["input"], "follow-up");
    assert_eq!(request_body["previous_response_id"], "resp_prev");
    assert_eq!(request_body["prompt_cache_key"], "compact-cache");
    assert_eq!(request_body["prompt_cache_retention"], "in_memory");
    assert_eq!(request_body["service_tier"], "flex");
    assert_eq!(response.output().object, "response.compaction");
    assert_eq!(response.output().output.len(), 2);
    assert_eq!(response.output().output[0].item_type, "message");
    assert_eq!(response.output().output[1].item_type, "compaction");
    assert_eq!(
        response.output().usage.as_ref().unwrap().total_tokens,
        Some(15)
    );
}

#[test]
fn continuity_fields_round_trip() {
    let server = mock_http::MockHttpServer::spawn(json_response(response_payload(
        "resp_conflict",
        Some(true),
        Some("resp_prev"),
        Some(json!({"id": "conv_123"})),
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            previous_response_id: Some("resp_prev".into()),
            conversation: Some(ResponseConversation::Object(ResponseConversationObject {
                id: String::from("conv_123"),
                extra: BTreeMap::new(),
            })),
            ..Default::default()
        })
        .unwrap();

    let request = server.captured_request().expect("captured request");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["previous_response_id"], "resp_prev");
    assert_eq!(body["conversation"], json!({"id": "conv_123"}));
}

#[test]
fn store_flag_pass_through() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(response_payload("resp_stored", Some(true), None, None)),
        json_response(response_payload("resp_ephemeral", Some(false), None, None)),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let stored = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            store: Some(true),
            ..Default::default()
        })
        .unwrap();
    let ephemeral = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            store: Some(false),
            ..Default::default()
        })
        .unwrap();

    let requests = server.captured_requests(2).expect("captured requests");
    let first: Value = serde_json::from_slice(&requests[0].body).unwrap();
    let second: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(first["store"], true);
    assert_eq!(second["store"], false);
    assert_eq!(stored.output().store, Some(true));
    assert_eq!(ephemeral.output().store, Some(false));
}

#[test]
fn conflicting_state_api_failures_surface_cleanly() {
    let body = br#"{"error":{"message":"previous_response_id cannot be used with conversation","type":"invalid_request_error","code":"conflict_state"}}"#.to_vec();
    let server = mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse {
        status_code: 400,
        reason: "Bad Request",
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("x-request-id"),
                String::from("req_conflict_state"),
            ),
        ],
        body,
        ..Default::default()
    })
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let error = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            previous_response_id: Some("resp_prev".into()),
            conversation: Some(ResponseConversation::Id(String::from("conv_123"))),
            ..Default::default()
        })
        .expect_err("conflicting continuity modes should surface API failure");

    assert_eq!(error.kind, ErrorKind::Api(ApiErrorKind::BadRequest));
    assert_eq!(error.request_id(), Some("req_conflict_state"));
    assert_eq!(
        error.api_error().unwrap().code.as_deref(),
        Some("conflict_state")
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

fn json_value_response(body: Value) -> mock_http::ScriptedResponse {
    json_response(body.to_string())
}

fn response_payload(
    id: &str,
    store: Option<bool>,
    previous_response_id: Option<&str>,
    conversation: Option<Value>,
) -> String {
    json!({
        "id": id,
        "object": "response",
        "created_at": 1.25,
        "status": "completed",
        "background": false,
        "completed_at": 2.5,
        "error": null,
        "incomplete_details": null,
        "instructions": "Server instructions",
        "metadata": {"trace": "response_payload"},
        "model": "gpt-4.1-nano",
        "max_output_tokens": 512,
        "max_tool_calls": 4,
        "output": [
            {
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "output_text", "text": "Hello "},
                    {"type": "refusal", "text": "ignored"}
                ]
            },
            {
                "id": "reasoning_1",
                "type": "reasoning",
                "summary": []
            },
            {
                "id": "mcp_tools_1",
                "type": "mcp_list_tools",
                "server_label": "deepwiki",
                "tools": [{
                    "name": "search_docs",
                    "input_schema": {"type": "object"},
                    "annotations": {"readOnlyHint": true},
                    "description": "Search docs"
                }]
            },
            {
                "id": "computer_1",
                "type": "computer_call",
                "call_id": "call_computer",
                "status": "completed",
                "action": {"type": "click", "button": "left", "x": 10, "y": 20},
                "actions": [
                    {"type": "keypress", "keys": ["CTRL", "L"]},
                    {"type": "type", "text": "openai.com"},
                    {"type": "wait"}
                ],
                "pending_safety_checks": [{
                    "id": "safety_pending_1",
                    "code": "unsafe_browser",
                    "message": "Browser confirmation required"
                }]
            },
            {
                "id": "computer_output_1",
                "type": "computer_call_output",
                "call_id": "call_computer",
                "status": "completed",
                "output": {"type": "computer_screenshot", "image_url": "data:image/png;base64,AA=="},
                "acknowledged_safety_checks": [{
                    "id": "safety_ack_1",
                    "code": "unsafe_browser",
                    "message": "Acknowledged"
                }]
            },
            {
                "id": "msg_2",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "output_text", "text": "world!"}
                ]
            }
        ],
        "parallel_tool_calls": true,
        "previous_response_id": previous_response_id,
        "conversation": conversation,
        "store": store,
        "prompt": {"id": "pmpt_response", "variables": {"topic": "Rust"}},
        "prompt_cache_key": "response-cache-key",
        "prompt_cache_retention": "24h",
        "reasoning": {"effort": "low"},
        "safety_identifier": "response_user_hash",
        "service_tier": "priority",
        "temperature": 0.2,
        "text": {"format": {"type": "text"}},
        "tool_choice": "auto",
        "tools": [{"type": "web_search_preview"}],
        "top_logprobs": 2,
        "top_p": 0.8,
        "truncation": "auto",
        "user": "legacy-user",
        "usage": {
            "input_tokens": 1,
            "input_tokens_details": {"cached_tokens": 1},
            "output_tokens": 2,
            "output_tokens_details": {"reasoning_tokens": 1},
            "total_tokens": 3
        }
    })
    .to_string()
}

fn response_payload_with_tool_and_refusal(id: &str) -> String {
    json!({
        "id": id,
        "object": "response",
        "created_at": 1,
        "status": "completed",
        "background": false,
        "error": null,
        "incomplete_details": null,
        "model": "gpt-4.1-nano",
        "output": [
            {
                "id": "msg_text",
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "phase": "final_answer",
                "content": [
                    {
                        "type": "output_text",
                        "text": "Hello ",
                        "annotations": [{
                            "type": "url_citation",
                            "start_index": 0,
                            "end_index": 5,
                            "title": "Weather",
                            "url": "https://example.com/weather"
                        }],
                        "logprobs": [{
                            "token": "Hello",
                            "bytes": [72, 101, 108, 108, 111],
                            "logprob": -0.01,
                            "top_logprobs": []
                        }]
                    },
                    {"type": "output_text", "text": "world!"}
                ]
            },
            {
                "id": "reasoning_1",
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "Checked weather"}],
                "content": [{"type": "reasoning_text", "text": "Need weather lookup"}],
                "encrypted_content": "enc_reasoning",
                "status": "completed"
            },
            {
                "id": "fc_123",
                "type": "function_call",
                "name": "lookup_weather",
                "arguments": "{\"city\":\"Paris\"}",
                "call_id": "call_123",
                "status": "completed",
                "namespace": "weather",
                "created_by": "assistant"
            },
            {
                "id": "fs_123",
                "type": "file_search_call",
                "queries": ["docs"],
                "status": "completed",
                "results": [{"file_id": "file_1", "filename": "guide.md", "score": 0.9}]
            },
            {
                "id": "ci_123",
                "type": "code_interpreter_call",
                "container_id": "cntr_123",
                "code": "print('ok')",
                "outputs": [{"type": "logs", "logs": "ok"}],
                "status": "completed"
            },
            {
                "id": "mcp_123",
                "type": "mcp_call",
                "arguments": "{\"city\":\"Paris\"}",
                "name": "weather",
                "server_label": "weather_mcp",
                "approval_request_id": "approval_123",
                "output": "sunny",
                "status": "completed"
            },
            {
                "id": "tool_search_123",
                "type": "tool_search_call",
                "arguments": {"query": "weather"},
                "execution": "server",
                "status": "completed"
            },
            {
                "id": "image_123",
                "type": "image_generation_call",
                "result": "aW1n",
                "status": "completed"
            },
            {
                "id": "msg_refusal",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "refusal", "refusal": "I can't help with that"}
                ]
            }
        ],
        "usage": {"input_tokens": 1, "output_tokens": 2, "total_tokens": 3}
    })
    .to_string()
}
