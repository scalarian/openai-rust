use openai_rust::{ErrorKind, OpenAI};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn compatibility_surface_supports_create_and_stored_completion_crud() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(chat_completion_payload("chatcmpl_store", "stored hello")),
        json_response(chat_completion_payload("chatcmpl_store", "stored hello")),
        json_response(chat_completion_payload("chatcmpl_store", "stored hello")),
        json_response(list_payload()),
        json_response(delete_payload("chatcmpl_store")),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let created = client
        .chat()
        .completions()
        .create(openai_rust::resources::chat::ChatCompletionCreateParams {
            model: String::from("gpt-4.1-mini"),
            messages: vec![json!({
                "role": "user",
                "content": "Say hello"
            })],
            store: Some(true),
            metadata: Some(json!({"tenant": "acme"})),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(created.output().id, "chatcmpl_store");
    assert_eq!(
        created.output().choices[0].message.content.as_deref(),
        Some("stored hello")
    );

    let retrieved = client
        .chat()
        .completions()
        .retrieve("chatcmpl_store")
        .unwrap();
    assert_eq!(retrieved.output().id, "chatcmpl_store");

    let updated = client
        .chat()
        .completions()
        .update(
            "chatcmpl_store",
            openai_rust::resources::chat::StoredChatCompletionUpdateParams {
                metadata: json!({"tenant": "acme", "phase": "updated"}),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        updated.output().choices[0].message.content.as_deref(),
        Some("stored hello")
    );

    let mut metadata = BTreeMap::new();
    metadata.insert(String::from("tenant"), String::from("acme"));
    let listed = client
        .chat()
        .completions()
        .list(
            openai_rust::resources::chat::StoredChatCompletionsListParams {
                after: Some(String::from("chatcmpl_prev")),
                limit: Some(1),
                model: Some(String::from("gpt-4.1-mini")),
                order: Some(String::from("desc")),
                metadata,
            },
        )
        .unwrap();
    assert_eq!(listed.output().data.len(), 1);
    assert!(listed.output().has_next_page());
    assert_eq!(listed.output().next_after(), Some("chatcmpl_store"));

    let deleted = client
        .chat()
        .completions()
        .delete("chatcmpl_store")
        .unwrap();
    assert_eq!(deleted.output().id, "chatcmpl_store");
    assert!(deleted.output().deleted);

    let requests = server.captured_requests(5).expect("captured requests");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/chat/completions");
    let create_body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["store"], Value::Bool(true));
    assert_eq!(create_body["metadata"]["tenant"], "acme");
    assert_eq!(create_body["messages"][0]["content"], "Say hello");

    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].path, "/v1/chat/completions/chatcmpl_store");

    assert_eq!(requests[2].method, "POST");
    assert_eq!(requests[2].path, "/v1/chat/completions/chatcmpl_store");
    let update_body: Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(
        update_body,
        json!({"metadata": {"tenant": "acme", "phase": "updated"}})
    );

    assert_eq!(requests[3].method, "GET");
    assert_eq!(
        requests[3].path,
        "/v1/chat/completions?after=chatcmpl_prev&limit=1&metadata[tenant]=acme&model=gpt-4.1-mini&order=desc"
    );

    assert_eq!(requests[4].method, "DELETE");
    assert_eq!(requests[4].path, "/v1/chat/completions/chatcmpl_store");

    let blank_id = client
        .chat()
        .completions()
        .retrieve("   ")
        .expect_err("blank completion id should be rejected locally");
    assert_eq!(blank_id.kind, ErrorKind::Validation);

    let blank_update = client
        .chat()
        .completions()
        .update(
            "",
            openai_rust::resources::chat::StoredChatCompletionUpdateParams {
                metadata: json!({}),
                ..Default::default()
            },
        )
        .expect_err("blank completion id should be rejected locally");
    assert_eq!(blank_update.kind, ErrorKind::Validation);

    let blank_delete = client
        .chat()
        .completions()
        .delete(" ")
        .expect_err("blank completion id should be rejected locally");
    assert_eq!(blank_delete.kind, ErrorKind::Validation);
}

#[test]
fn stored_chat_retrieve_accepts_nullable_tool_calls() {
    let body = json!({
        "id": "chatcmpl_store",
        "object": "chat.completion",
        "created": 1,
        "model": "gpt-4.1-mini",
        "choices": [
            {
                "index": 0,
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": "stored hello",
                    "tool_calls": null
                }
            }
        ]
    })
    .to_string();
    let server = mock_http::MockHttpServer::spawn_sequence(vec![json_response(body)]).unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let retrieved = client
        .chat()
        .completions()
        .retrieve("chatcmpl_store")
        .expect("stored chat completion should deserialize when tool_calls is null");

    assert!(retrieved.output().choices[0].message.tool_calls.is_empty());
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

fn chat_completion_payload(id: &str, text: &str) -> String {
    json!({
        "id": id,
        "object": "chat.completion",
        "created": 1,
        "model": "gpt-4.1-mini",
        "choices": [
            {
                "index": 0,
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": text
                }
            }
        ],
        "usage": {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5}
    })
    .to_string()
}

fn list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "id": "chatcmpl_store",
                "object": "chat.completion",
                "created": 1,
                "model": "gpt-4.1-mini",
                "choices": [
                    {
                        "index": 0,
                        "finish_reason": "stop",
                        "message": {
                            "role": "assistant",
                            "content": "stored hello"
                        }
                    }
                ]
            }
        ],
        "first_id": "chatcmpl_store",
        "last_id": "chatcmpl_store",
        "has_more": true
    })
    .to_string()
}

fn delete_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "chat.completion.deleted",
        "deleted": true
    })
    .to_string()
}
