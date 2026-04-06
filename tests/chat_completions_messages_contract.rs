use openai_rust::{ErrorKind, OpenAI};
use serde_json::json;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn stored_messages_list_preserves_cursor_ordering_and_termination() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(first_page_payload()),
        json_response(second_page_payload()),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let first_page = client
        .chat()
        .completions()
        .messages()
        .list(
            "chatcmpl_store",
            openai_rust::resources::chat::StoredChatCompletionMessagesListParams {
                after: Some(String::from("msg 0/seed?cursor=true")),
                limit: Some(2),
                order: Some(String::from("asc&stable")),
            },
        )
        .unwrap();

    let first_request = server.captured_request().expect("captured first request");
    assert_eq!(first_request.method, "GET");
    assert_eq!(
        first_request.path,
        "/v1/chat/completions/chatcmpl_store/messages?after=msg%200%2Fseed%3Fcursor%3Dtrue&limit=2&order=asc%26stable"
    );
    assert_eq!(first_page.output().data.len(), 2);
    assert_eq!(first_page.output().data[0].id.as_deref(), Some("msg_1"));
    assert_eq!(first_page.output().next_after(), Some("msg_2"));
    assert!(first_page.output().has_next_page());

    let second_page = client
        .chat()
        .completions()
        .messages()
        .list(
            "chatcmpl_store",
            openai_rust::resources::chat::StoredChatCompletionMessagesListParams {
                after: first_page.output().next_after().map(str::to_string),
                limit: Some(2),
                order: Some(String::from("desc?final=false")),
            },
        )
        .unwrap();

    let second_request = server.captured_request().expect("captured second request");
    assert_eq!(
        second_request.path,
        "/v1/chat/completions/chatcmpl_store/messages?after=msg_2&limit=2&order=desc%3Ffinal%3Dfalse"
    );
    assert_eq!(second_page.output().data.len(), 0);
    assert_eq!(second_page.output().first_id, None);
    assert_eq!(second_page.output().last_id, None);
    assert_eq!(second_page.output().next_after(), None);
    assert!(!second_page.output().has_next_page());

    let error = client
        .chat()
        .completions()
        .messages()
        .list("   ", Default::default())
        .expect_err("blank completion id should be rejected locally");
    assert_eq!(error.kind, ErrorKind::Validation);
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

fn first_page_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "id": "msg_1",
                "object": "chat.completion.message",
                "role": "user",
                "content": "First"
            },
            {
                "id": "msg_2",
                "object": "chat.completion.message",
                "role": "assistant",
                "content": "Second"
            }
        ],
        "first_id": "msg_1",
        "last_id": "msg_2",
        "has_more": true
    })
    .to_string()
}

fn second_page_payload() -> String {
    json!({
        "object": "list",
        "data": [],
        "has_more": false
    })
    .to_string()
}
