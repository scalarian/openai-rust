use openai_rust::{ApiErrorKind, ErrorKind, OpenAI};
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
            input: Some(json!("hello")),
            previous_response_id: Some("resp_prev".into()),
            conversation: Some(json!("conv_123")),
            store: Some(true),
            ..Default::default()
        })
        .unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/responses");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["model"], "gpt-4.1-nano");
    assert_eq!(body["input"], "hello");
    assert_eq!(body["previous_response_id"], "resp_prev");
    assert_eq!(body["conversation"], "conv_123");
    assert_eq!(body["store"], true);
    assert_eq!(body["stream"], false);
    assert_eq!(response.output().id, "resp_create");
    assert_eq!(
        response.output().previous_response_id.as_deref(),
        Some("resp_prev")
    );
    assert_eq!(response.output().store, Some(true));
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
    assert_eq!(response.output().object, "response.compaction");
    assert_eq!(response.output().output.len(), 2);
    assert_eq!(response.output().output[0].item_type, "message");
    assert_eq!(response.output().output[1].item_type, "compaction");
    assert_eq!(response.output().usage["total_tokens"], 15);
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
            conversation: Some(json!({"id": "conv_123"})),
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
            conversation: Some(json!("conv_123")),
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
        "created_at": 1,
        "status": "completed",
        "background": false,
        "error": null,
        "incomplete_details": null,
        "model": "gpt-4.1-nano",
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
        "tool_choice": "auto",
        "tools": [],
        "usage": {"input_tokens": 1, "output_tokens": 2, "total_tokens": 3}
    })
    .to_string()
}
