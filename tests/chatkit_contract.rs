#[path = "support/mock_http.rs"]
mod mock_http;

use std::collections::BTreeMap;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::beta::{
        ChatKitAutomaticThreadTitlingParam, ChatKitConfigurationParam, ChatKitFileUploadParam,
        ChatKitHistoryParam, ChatKitOrder, ChatKitSessionCreateParams,
        ChatKitSessionExpiresAfterParam, ChatKitSessionExpiryAnchor, ChatKitSessionRateLimitsParam,
        ChatKitSessionStatus, ChatKitSessionWorkflowParam, ChatKitStateValue,
        ChatKitThreadItemListParams, ChatKitThreadListParams, ChatKitThreadStatus,
        ChatKitWorkflowTracingParam,
    },
};
use serde_json::json;

#[test]
fn chatkit_beta_sessions_and_threads_preserve_routes_headers_and_shapes() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(session_payload("sess_created", "active")),
        json_response(session_payload("sess_created", "cancelled")),
        json_response(thread_payload("thread_123")),
        json_response(thread_page_payload()),
        json_response(delete_payload("thread_123")),
        json_response(thread_items_payload()),
    ])
    .unwrap();
    let client = client(&server.url());
    let chatkit = client.beta().chatkit();

    let created = chatkit
        .sessions()
        .create(ChatKitSessionCreateParams {
            user: String::from("end-user-123"),
            workflow: ChatKitSessionWorkflowParam {
                id: String::from("workflow_support"),
                state_variables: Some(BTreeMap::from([
                    (
                        String::from("tier"),
                        ChatKitStateValue::String(String::from("pro")),
                    ),
                    (String::from("entitled"), ChatKitStateValue::Bool(true)),
                ])),
                tracing: Some(ChatKitWorkflowTracingParam {
                    enabled: Some(false),
                }),
                version: Some(String::from("2026-05-30")),
            },
            chatkit_configuration: Some(ChatKitConfigurationParam {
                automatic_thread_titling: Some(ChatKitAutomaticThreadTitlingParam {
                    enabled: Some(true),
                }),
                file_upload: Some(ChatKitFileUploadParam {
                    enabled: Some(true),
                    max_file_size: Some(64),
                    max_files: Some(3),
                }),
                history: Some(ChatKitHistoryParam {
                    enabled: Some(true),
                    recent_threads: Some(8),
                }),
            }),
            expires_after: Some(ChatKitSessionExpiresAfterParam {
                anchor: ChatKitSessionExpiryAnchor::CreatedAt,
                seconds: 900,
            }),
            rate_limits: Some(ChatKitSessionRateLimitsParam {
                max_requests_per_1_minute: Some(12),
            }),
        })
        .unwrap();
    assert_eq!(created.output.id, "sess_created");
    assert_eq!(created.output.status, ChatKitSessionStatus::Active);
    assert_eq!(created.output.rate_limits.max_requests_per_1_minute, 12);
    assert_eq!(
        created.output.workflow.version.as_deref(),
        Some("2026-05-30")
    );
    assert!(created.output.workflow.tracing.enabled);
    assert_eq!(
        created.output.chatkit_configuration.file_upload.max_files,
        Some(3)
    );

    let cancelled = chatkit.sessions().cancel("sess_created").unwrap();
    assert_eq!(cancelled.output.status, ChatKitSessionStatus::Cancelled);

    let thread = chatkit.threads().retrieve("thread_123").unwrap();
    assert!(matches!(thread.output.status, ChatKitThreadStatus::Active));

    let threads = chatkit
        .threads()
        .list(ChatKitThreadListParams {
            after: Some(String::from("item_after")),
            before: Some(String::from("item_before")),
            limit: Some(2),
            order: Some(ChatKitOrder::Desc),
            user: Some(String::from("end-user-123")),
        })
        .unwrap();
    assert_eq!(threads.output.data.len(), 1);
    assert!(threads.output.has_next_page());
    assert_eq!(threads.output.next_after(), Some("thread_123"));

    let deleted = chatkit.threads().delete("thread_123").unwrap();
    assert!(deleted.output.deleted);

    let items = chatkit
        .threads()
        .list_items(
            "thread_123",
            ChatKitThreadItemListParams {
                after: Some(String::from("item_1")),
                before: None,
                limit: Some(3),
                order: Some(ChatKitOrder::Asc),
            },
        )
        .unwrap();
    assert_eq!(items.output.data[0]["type"], json!("chatkit.user_message"));

    let requests = server.captured_requests(6).unwrap();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/chatkit/sessions");
    assert_eq!(requests[1].path, "/v1/chatkit/sessions/sess_created/cancel");
    assert_eq!(requests[2].path, "/v1/chatkit/threads/thread_123");
    assert_eq!(
        requests[3].path,
        "/v1/chatkit/threads?after=item_after&before=item_before&limit=2&order=desc&user=end-user-123"
    );
    assert_eq!(requests[4].method, "DELETE");
    assert_eq!(requests[4].path, "/v1/chatkit/threads/thread_123");
    assert_eq!(
        requests[5].path,
        "/v1/chatkit/threads/thread_123/items?after=item_1&limit=3&order=asc"
    );
    for request in &requests {
        assert_eq!(
            request.headers.get("openai-beta").map(String::as_str),
            Some("chatkit_beta=v1")
        );
    }

    let create_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["user"], json!("end-user-123"));
    assert_eq!(create_body["workflow"]["id"], json!("workflow_support"));
    assert_eq!(create_body["workflow"]["tracing"]["enabled"], json!(false));
    assert_eq!(create_body["expires_after"]["anchor"], json!("created_at"));
    assert_eq!(
        create_body["rate_limits"]["max_requests_per_1_minute"],
        json!(12)
    );
    assert_eq!(
        create_body["chatkit_configuration"]["file_upload"]["max_files"],
        json!(3)
    );

    let blank_session = chatkit.sessions().cancel(" ").unwrap_err();
    assert!(matches!(blank_session.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn session_payload(id: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "chatkit.session",
        "client_secret": "ck_secret_test",
        "expires_at": 1_800_000_000u64,
        "max_requests_per_1_minute": 12,
        "status": status,
        "user": "end-user-123",
        "workflow": {
            "id": "workflow_support",
            "version": "2026-05-30",
            "state_variables": {"tier": "pro", "entitled": true},
            "tracing": {"enabled": true}
        },
        "chatkit_configuration": {
            "automatic_thread_titling": {"enabled": true},
            "file_upload": {"enabled": true, "max_file_size": 64, "max_files": 3},
            "history": {"enabled": true, "recent_threads": 8}
        },
        "rate_limits": {"max_requests_per_1_minute": 12}
    })
    .to_string()
}

fn thread_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "chatkit.thread",
        "created_at": 1_717_171_717u64,
        "status": {"type": "active"},
        "title": "Support thread",
        "user": "end-user-123"
    })
    .to_string()
}

fn thread_page_payload() -> String {
    json!({
        "object": "list",
        "data": [serde_json::from_str::<serde_json::Value>(&thread_payload("thread_123")).unwrap()],
        "first_id": "thread_123",
        "last_id": "thread_123",
        "has_more": true
    })
    .to_string()
}

fn delete_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "chatkit.thread.deleted",
        "deleted": true
    })
    .to_string()
}

fn thread_items_payload() -> String {
    json!({
        "object": "list",
        "data": [{
            "id": "item_1",
            "object": "chatkit.thread_item",
            "created_at": 1_717_171_818u64,
            "thread_id": "thread_123",
            "type": "chatkit.user_message",
            "content": [{"type": "input_text", "text": "hello"}]
        }],
        "first_id": "item_1",
        "last_id": "item_1",
        "has_more": false
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
            (String::from("x-request-id"), String::from("req_chatkit")),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}
