use openai_rust::{ErrorKind, OpenAI};
use serde_json::json;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn input_items_list_exposes_typed_items_and_cursor_termination() {
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
        .responses()
        .input_items()
        .list(
            "resp_123",
            openai_rust::resources::responses::ResponseInputItemsListParams {
                after: Some("item_0".into()),
                include: vec!["message.input_image.image_url".into()],
                limit: Some(2),
                order: Some("asc".into()),
            },
        )
        .unwrap();

    let request = server.captured_request().expect("captured first request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.path,
        "/v1/responses/resp_123/input_items?after=item_0&include=message.input_image.image_url&limit=2&order=asc"
    );
    assert_eq!(first_page.output().data.len(), 2);
    assert_eq!(first_page.output().data[0].id.as_deref(), Some("item_1"));
    assert_eq!(
        first_page.output().data[0].content[0].content_type,
        "input_text"
    );
    assert_eq!(first_page.output().first_id.as_deref(), Some("item_1"));
    assert_eq!(first_page.output().last_id.as_deref(), Some("item_2"));
    assert_eq!(first_page.output().next_after(), Some("item_2"));

    let second_page = client
        .responses()
        .input_items()
        .list(
            "resp_123",
            openai_rust::resources::responses::ResponseInputItemsListParams {
                after: first_page.output().next_after().map(str::to_string),
                limit: Some(2),
                order: Some("asc".into()),
                ..Default::default()
            },
        )
        .unwrap();

    let request = server.captured_request().expect("captured second request");
    assert_eq!(
        request.path,
        "/v1/responses/resp_123/input_items?after=item_2&limit=2&order=asc"
    );
    assert_eq!(second_page.output().data.len(), 1);
    assert_eq!(second_page.output().data[0].id.as_deref(), Some("item_3"));
    assert_eq!(second_page.output().next_after(), None);
    assert!(!second_page.output().has_next_page());

    let error = client
        .responses()
        .input_items()
        .list("   ", Default::default())
        .expect_err("blank response id should be rejected locally");
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
                "id": "item_1",
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "Describe this image"}
                ]
            },
            {
                "id": "item_2",
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_image", "image_url": "https://example.com/cat.png"}
                ]
            }
        ],
        "first_id": "item_1",
        "last_id": "item_2",
        "has_more": true
    })
    .to_string()
}

fn second_page_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "id": "item_3",
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "Thank you"}
                ]
            }
        ],
        "first_id": "item_3",
        "last_id": "item_3",
        "has_more": false
    })
    .to_string()
}
