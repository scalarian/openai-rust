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
            audio: Some(json!({"voice": "alloy", "format": "wav"})),
            frequency_penalty: Some(0.1),
            function_call: Some(json!("auto")),
            functions: Some(vec![json!({
                "name": "legacy_lookup",
                "parameters": {"type": "object"}
            })]),
            logit_bias: Some(json!({"42": -1})),
            logprobs: Some(true),
            max_completion_tokens: Some(128),
            max_tokens: Some(256),
            store: Some(true),
            metadata: Some(json!({"tenant": "acme"})),
            modalities: Some(vec![String::from("text"), String::from("audio")]),
            n: Some(1),
            parallel_tool_calls: Some(true),
            prediction: Some(json!({"type": "content", "content": "stored hello"})),
            presence_penalty: Some(0.2),
            prompt_cache_key: Some(String::from("chat-cache")),
            prompt_cache_retention: Some(String::from("24h")),
            reasoning_effort: Some(String::from("low")),
            response_format: Some(json!({"type": "json_object"})),
            safety_identifier: Some(String::from("user_hash")),
            seed: Some(7),
            service_tier: Some(String::from("priority")),
            stop: Some(json!(["END"])),
            stream: Some(false),
            stream_options: Some(json!({"include_usage": true})),
            temperature: Some(0.3),
            tool_choice: Some(json!("auto")),
            tools: Some(vec![json!({
                "type": "function",
                "function": {"name": "lookup", "parameters": {"type": "object"}}
            })]),
            top_logprobs: Some(2),
            top_p: Some(0.9),
            user: Some(String::from("legacy-user")),
            verbosity: Some(String::from("medium")),
            web_search_options: Some(json!({"search_context_size": "low"})),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(created.output().id, "chatcmpl_store");
    assert_eq!(
        created.output().choices[0].message.content.as_deref(),
        Some("stored hello")
    );
    assert_eq!(created.output().service_tier.as_deref(), Some("priority"));
    assert_eq!(
        created.output().system_fingerprint.as_deref(),
        Some("fp_chat_123")
    );
    assert_eq!(created.output().usage.as_ref().unwrap().total_tokens, 5);
    assert_eq!(
        created
            .output()
            .usage
            .as_ref()
            .unwrap()
            .completion_tokens_details
            .as_ref()
            .unwrap()
            .reasoning_tokens,
        Some(1)
    );
    assert_eq!(
        created.output().choices[0]
            .logprobs
            .as_ref()
            .unwrap()
            .content[0]
            .token,
        "stored"
    );
    assert_eq!(
        created.output().choices[0].message.annotations[0]
            .url_citation
            .as_ref()
            .unwrap()
            .url,
        "https://example.com/source"
    );
    assert_eq!(
        created.output().choices[0]
            .message
            .audio
            .as_ref()
            .unwrap()
            .transcript,
        "stored hello"
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
    metadata.insert(String::from("tenant/id"), String::from("acme & west?x=y"));
    let listed = client
        .chat()
        .completions()
        .list(
            openai_rust::resources::chat::StoredChatCompletionsListParams {
                after: Some(String::from("chatcmpl_prev")),
                limit: Some(1),
                model: Some(String::from("gpt-4.1-mini/compat?preview=true")),
                order: Some(String::from("desc+later")),
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
    assert_eq!(create_body["audio"]["voice"], "alloy");
    assert_eq!(create_body["frequency_penalty"], 0.1);
    assert_eq!(create_body["function_call"], "auto");
    assert_eq!(create_body["functions"][0]["name"], "legacy_lookup");
    assert_eq!(create_body["logit_bias"]["42"], -1);
    assert_eq!(create_body["logprobs"], true);
    assert_eq!(create_body["max_completion_tokens"], 128);
    assert_eq!(create_body["max_tokens"], 256);
    assert_eq!(create_body["store"], Value::Bool(true));
    assert_eq!(create_body["metadata"]["tenant"], "acme");
    assert_eq!(create_body["messages"][0]["content"], "Say hello");
    assert_eq!(create_body["modalities"], json!(["text", "audio"]));
    assert_eq!(create_body["n"], 1);
    assert_eq!(create_body["parallel_tool_calls"], true);
    assert_eq!(create_body["prediction"]["content"], "stored hello");
    assert_eq!(create_body["presence_penalty"], 0.2);
    assert_eq!(create_body["prompt_cache_key"], "chat-cache");
    assert_eq!(create_body["prompt_cache_retention"], "24h");
    assert_eq!(create_body["reasoning_effort"], "low");
    assert_eq!(create_body["response_format"]["type"], "json_object");
    assert_eq!(create_body["safety_identifier"], "user_hash");
    assert_eq!(create_body["seed"], 7);
    assert_eq!(create_body["service_tier"], "priority");
    assert_eq!(create_body["stop"], json!(["END"]));
    assert_eq!(create_body["stream_options"]["include_usage"], true);
    assert_eq!(create_body["temperature"], 0.3);
    assert_eq!(create_body["tool_choice"], "auto");
    assert_eq!(create_body["tools"][0]["function"]["name"], "lookup");
    assert_eq!(create_body["top_logprobs"], 2);
    assert_eq!(create_body["top_p"], 0.9);
    assert_eq!(create_body["user"], "legacy-user");
    assert_eq!(create_body["verbosity"], "medium");
    assert_eq!(
        create_body["web_search_options"]["search_context_size"],
        "low"
    );

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
        "/v1/chat/completions?after=chatcmpl_prev&limit=1&metadata%5Btenant%2Fid%5D=acme%20%26%20west%3Fx%3Dy&model=gpt-4.1-mini%2Fcompat%3Fpreview%3Dtrue&order=desc%2Blater"
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
                "logprobs": {
                    "content": [{
                        "token": "stored",
                        "bytes": [115, 116, 111, 114, 101, 100],
                        "logprob": -0.1,
                        "top_logprobs": [{
                            "token": "stored",
                            "bytes": [115, 116, 111, 114, 101, 100],
                            "logprob": -0.1
                        }]
                    }],
                    "refusal": []
                },
                "message": {
                    "role": "assistant",
                    "content": text,
                    "annotations": [{
                        "type": "url_citation",
                        "url_citation": {
                            "start_index": 0,
                            "end_index": 6,
                            "title": "source",
                            "url": "https://example.com/source"
                        }
                    }],
                    "audio": {
                        "id": "audio_123",
                        "data": "UklGRg==",
                        "expires_at": 1_717_171_999,
                        "transcript": text
                    }
                }
            }
        ],
        "service_tier": "priority",
        "system_fingerprint": "fp_chat_123",
        "usage": {
            "prompt_tokens": 3,
            "completion_tokens": 2,
            "total_tokens": 5,
            "completion_tokens_details": {
                "reasoning_tokens": 1,
                "audio_tokens": 0,
                "accepted_prediction_tokens": 0,
                "rejected_prediction_tokens": 0
            },
            "prompt_tokens_details": {
                "audio_tokens": 0,
                "cached_tokens": 1
            }
        }
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
