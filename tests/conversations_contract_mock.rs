use openai_rust::{ErrorKind, OpenAI};
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn crud_and_path_guards() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(conversation_payload(
            "conv_create",
            json!({"topic": "onboarding", "phase": "draft"}),
        )),
        json_response(conversation_payload(
            "conv_create",
            json!({"topic": "onboarding", "phase": "draft"}),
        )),
        json_response(conversation_payload(
            "conv_create",
            json!({"topic": "onboarding", "phase": "published"}),
        )),
        json_response(
            json!({
                "id": "conv_create",
                "object": "conversation.deleted",
                "deleted": true
            })
            .to_string(),
        ),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let created = client
        .conversations()
        .create(
            openai_rust::resources::conversations::ConversationCreateParams {
                metadata: Some(json!({"topic": "onboarding", "phase": "draft"})),
                items: vec![json!({
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "Hello"}]
                })],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(created.output().id, "conv_create");
    assert_eq!(created.output().object, "conversation");
    assert_eq!(
        created.output().metadata,
        json!({"topic": "onboarding", "phase": "draft"})
    );

    let retrieved = client.conversations().retrieve("conv_create").unwrap();
    assert_eq!(retrieved.output().id, "conv_create");
    assert_eq!(retrieved.output().metadata["phase"], "draft");

    let updated = client
        .conversations()
        .update(
            "conv_create",
            openai_rust::resources::conversations::ConversationUpdateParams {
                metadata: json!({"topic": "onboarding", "phase": "published"}),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(updated.output().metadata["phase"], "published");

    let deleted = client.conversations().delete("conv_create").unwrap();
    assert_eq!(deleted.output().id, "conv_create");
    assert_eq!(deleted.output().object, "conversation.deleted");
    assert!(deleted.output().deleted);

    let requests = server.captured_requests(4).expect("captured requests");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/conversations");
    let create_body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["metadata"]["topic"], "onboarding");
    assert_eq!(create_body["items"][0]["content"][0]["text"], "Hello");

    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].path, "/v1/conversations/conv_create");

    assert_eq!(requests[2].method, "POST");
    assert_eq!(requests[2].path, "/v1/conversations/conv_create");
    let update_body: Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(
        update_body,
        json!({"metadata": {"topic": "onboarding", "phase": "published"}})
    );

    assert_eq!(requests[3].method, "DELETE");
    assert_eq!(requests[3].path, "/v1/conversations/conv_create");

    for invalid in ["", "   "] {
        let retrieve_error = client
            .conversations()
            .retrieve(invalid)
            .expect_err("blank conversation id should be rejected locally");
        assert_eq!(retrieve_error.kind, ErrorKind::Validation);

        let update_error = client
            .conversations()
            .update(
                invalid,
                openai_rust::resources::conversations::ConversationUpdateParams {
                    metadata: json!({"phase": "ignored"}),
                    ..Default::default()
                },
            )
            .expect_err("blank conversation id should be rejected locally");
        assert_eq!(update_error.kind, ErrorKind::Validation);

        let delete_error = client
            .conversations()
            .delete(invalid)
            .expect_err("blank conversation id should be rejected locally");
        assert_eq!(delete_error.kind, ErrorKind::Validation);
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

fn conversation_payload(id: &str, metadata: Value) -> String {
    json!({
        "id": id,
        "object": "conversation",
        "created_at": 1,
        "metadata": metadata
    })
    .to_string()
}
