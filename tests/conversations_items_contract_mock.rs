use openai_rust::{ErrorKind, OpenAI};
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn routes_and_pagination() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(items_envelope(vec![
            message_item("item_1", "hello"),
            reasoning_item("rs_1"),
        ])),
        json_response(message_item("item_1", "hello").to_string()),
        json_response(items_envelope(vec![
            message_item("item_1", "hello"),
            message_item("item_2", "world"),
        ])),
        json_response(conversation_payload(
            "conv_123",
            json!({"phase": "after_delete"}),
        )),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let created = client
        .conversations()
        .items()
        .create(
            "conv_123",
            openai_rust::resources::conversations::ConversationItemCreateParams {
                items: vec![
                    json!({
                        "type": "message",
                        "role": "user",
                        "content": [{"type": "input_text", "text": "hello"}]
                    }),
                    json!({"type": "reasoning", "summary": []}),
                ],
                include: vec![
                    String::from("message.output_text.logprobs"),
                    String::from("reasoning.encrypted_content"),
                ],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(created.output().object, "list");
    assert_eq!(created.output().data.len(), 2);
    assert_eq!(created.output().data[0].id.as_deref(), Some("item_1"));
    assert_eq!(created.output().data[1].item_type, "reasoning");
    assert!(created.output().has_more);
    assert_eq!(created.output().next_after(), Some("rs_1"));

    let retrieved = client
        .conversations()
        .items()
        .retrieve(
            "conv_123",
            "item_1",
            openai_rust::resources::conversations::ConversationItemRetrieveParams {
                include: vec![String::from("message.output_text.logprobs")],
            },
        )
        .unwrap();
    assert_eq!(retrieved.output().id.as_deref(), Some("item_1"));
    assert_eq!(retrieved.output().item_type, "message");

    let listed = client
        .conversations()
        .items()
        .list(
            "conv_123",
            openai_rust::resources::conversations::ConversationItemListParams {
                after: Some(String::from("item_1")),
                include: vec![String::from("message.output_text.logprobs")],
                limit: Some(2),
                order: Some(String::from("asc")),
            },
        )
        .unwrap();
    assert_eq!(listed.output().data.len(), 2);
    assert_eq!(listed.output().first_id.as_deref(), Some("item_1"));
    assert_eq!(listed.output().last_id.as_deref(), Some("item_2"));
    assert!(listed.output().has_next_page());
    assert_eq!(listed.output().next_after(), Some("item_2"));

    let deleted = client
        .conversations()
        .items()
        .delete("conv_123", "item_1")
        .unwrap();
    assert_eq!(deleted.output().id, "conv_123");
    assert_eq!(deleted.output().object, "conversation");
    assert_eq!(deleted.output().metadata["phase"], "after_delete");

    let requests = server.captured_requests(4).expect("captured requests");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].path,
        "/v1/conversations/conv_123/items?include=message.output_text.logprobs&include=reasoning.encrypted_content"
    );
    let create_body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["items"][0]["content"][0]["text"], "hello");
    assert_eq!(create_body["items"][1]["type"], "reasoning");

    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].path,
        "/v1/conversations/conv_123/items/item_1?include=message.output_text.logprobs"
    );

    assert_eq!(requests[2].method, "GET");
    assert_eq!(
        requests[2].path,
        "/v1/conversations/conv_123/items?after=item_1&include=message.output_text.logprobs&limit=2&order=asc"
    );

    assert_eq!(requests[3].method, "DELETE");
    assert_eq!(requests[3].path, "/v1/conversations/conv_123/items/item_1");

    let invalid_create = client
        .conversations()
        .items()
        .create(
            "   ",
            openai_rust::resources::conversations::ConversationItemCreateParams {
                items: vec![],
                include: vec![],
                ..Default::default()
            },
        )
        .expect_err("blank conversation id should be rejected locally");
    assert_eq!(invalid_create.kind, ErrorKind::Validation);

    for invalid in ["", "   "] {
        let retrieve_error = client
            .conversations()
            .items()
            .retrieve(
                invalid,
                "item_1",
                openai_rust::resources::conversations::ConversationItemRetrieveParams::default(),
            )
            .expect_err("blank conversation id should be rejected locally");
        assert_eq!(retrieve_error.kind, ErrorKind::Validation);

        let list_error = client
            .conversations()
            .items()
            .list(
                invalid,
                openai_rust::resources::conversations::ConversationItemListParams::default(),
            )
            .expect_err("blank conversation id should be rejected locally");
        assert_eq!(list_error.kind, ErrorKind::Validation);

        let delete_error = client
            .conversations()
            .items()
            .delete(invalid, "item_1")
            .expect_err("blank conversation id should be rejected locally");
        assert_eq!(delete_error.kind, ErrorKind::Validation);
    }

    for invalid in ["", "   "] {
        let retrieve_error = client
            .conversations()
            .items()
            .retrieve(
                "conv_123",
                invalid,
                openai_rust::resources::conversations::ConversationItemRetrieveParams::default(),
            )
            .expect_err("blank item id should be rejected locally");
        assert_eq!(retrieve_error.kind, ErrorKind::Validation);

        let delete_error = client
            .conversations()
            .items()
            .delete("conv_123", invalid)
            .expect_err("blank item id should be rejected locally");
        assert_eq!(delete_error.kind, ErrorKind::Validation);
    }
}

#[test]
fn typed_known_fields_are_not_lost() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(items_envelope(vec![
            function_call_item("fc_1"),
            refusal_message_item("msg_refusal"),
        ])),
        json_response(function_call_item("fc_1").to_string()),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let listed = client
        .conversations()
        .items()
        .list("conv_123", Default::default())
        .unwrap();
    let retrieved = client
        .conversations()
        .items()
        .retrieve("conv_123", "fc_1", Default::default())
        .unwrap();

    let function_call = &listed.output().data[0];
    assert_eq!(function_call.item_type, "function_call");
    assert_eq!(function_call.name.as_deref(), Some("lookup_weather"));
    assert_eq!(
        function_call.arguments.as_deref(),
        Some(r#"{"city":"Paris"}"#)
    );
    assert_eq!(function_call.call_id.as_deref(), Some("call_123"));
    assert_eq!(function_call.status.as_deref(), Some("completed"));
    assert!(!function_call.extra.contains_key("name"));
    assert!(!function_call.extra.contains_key("arguments"));
    assert!(!function_call.extra.contains_key("call_id"));

    let refusal_message = &listed.output().data[1];
    let refusal_part = refusal_message
        .content
        .iter()
        .find(|part| part.content_type == "refusal")
        .expect("refusal content");
    assert_eq!(
        refusal_part.refusal.as_deref(),
        Some("I can't help with that")
    );
    assert!(!refusal_part.extra.contains_key("refusal"));

    let retrieved_function_call = retrieved.output();
    assert_eq!(
        retrieved_function_call.name.as_deref(),
        Some("lookup_weather")
    );
    assert_eq!(
        retrieved_function_call.arguments.as_deref(),
        Some(r#"{"city":"Paris"}"#)
    );
    assert_eq!(retrieved_function_call.call_id.as_deref(), Some("call_123"));
    assert_eq!(retrieved_function_call.status.as_deref(), Some("completed"));
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

fn items_envelope(items: Vec<Value>) -> String {
    let first_id = items
        .first()
        .and_then(|item| item.get("id"))
        .cloned()
        .unwrap_or(Value::Null);
    let last_id = items
        .last()
        .and_then(|item| item.get("id"))
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "object": "list",
        "data": items,
        "first_id": first_id,
        "last_id": last_id,
        "has_more": true
    })
    .to_string()
}

fn message_item(id: &str, text: &str) -> Value {
    json!({
        "id": id,
        "type": "message",
        "role": "user",
        "status": "completed",
        "content": [{"type": "input_text", "text": text}]
    })
}

fn reasoning_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "reasoning",
        "status": "completed",
        "summary": []
    })
}

fn function_call_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "function_call",
        "name": "lookup_weather",
        "arguments": "{\"city\":\"Paris\"}",
        "call_id": "call_123",
        "status": "completed"
    })
}

fn refusal_message_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "status": "completed",
        "content": [{"type": "refusal", "refusal": "I can't help with that"}]
    })
}

fn conversation_payload(id: &str, metadata: Value) -> String {
    json!({
        "id": id,
        "object": "conversation",
        "created_at": 1,
        "metadata": metadata
    })
    .to_string()
}
