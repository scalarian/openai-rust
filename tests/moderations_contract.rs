use openai_rust::OpenAI;
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn moderations_preserve_text_and_multimodal_input_correspondence() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(text_results_payload()),
        json_response(multimodal_results_payload()),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let text_response = client
        .moderations()
        .create(
            openai_rust::resources::moderations::ModerationCreateParams {
                model: Some(String::from("omni-moderation-latest")),
                input: json!(["first text", "second text"]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(text_response.output().results.len(), 2);
    assert!(!text_response.output().results[0].flagged);
    assert!(text_response.output().results[1].flagged);
    assert_eq!(
        text_response.output().results[0].category_applied_input_types["violence"],
        vec![String::from("text")]
    );

    let multimodal_input = json!([
        {
            "type": "input_text",
            "text": "describe this image"
        },
        {
            "type": "input_image",
            "image_url": "https://example.com/cat.png"
        }
    ]);
    let multimodal_response = client
        .moderations()
        .create(
            openai_rust::resources::moderations::ModerationCreateParams {
                model: Some(String::from("omni-moderation-latest")),
                input: multimodal_input.clone(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(multimodal_response.output().results.len(), 2);
    assert_eq!(
        multimodal_response.output().results[0].category_applied_input_types["self-harm"],
        vec![String::from("text")]
    );
    assert_eq!(
        multimodal_response.output().results[1].category_applied_input_types["violence"],
        vec![String::from("image")]
    );

    let requests = server.captured_requests(2).expect("captured requests");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/moderations");
    let text_body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(text_body["input"], json!(["first text", "second text"]));
    assert_eq!(text_body["model"], "omni-moderation-latest");

    let multimodal_body: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(multimodal_body["input"], multimodal_input);
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

fn text_results_payload() -> String {
    json!({
        "id": "modr_text",
        "model": "omni-moderation-latest",
        "results": [
            {
                "flagged": false,
                "categories": {"violence": false, "self-harm": false},
                "category_scores": {"violence": 0.01, "self-harm": 0.0},
                "category_applied_input_types": {
                    "violence": ["text"],
                    "self-harm": ["text"]
                }
            },
            {
                "flagged": true,
                "categories": {"violence": true, "self-harm": false},
                "category_scores": {"violence": 0.91, "self-harm": 0.0},
                "category_applied_input_types": {
                    "violence": ["text"],
                    "self-harm": ["text"]
                }
            }
        ]
    })
    .to_string()
}

fn multimodal_results_payload() -> String {
    json!({
        "id": "modr_multi",
        "model": "omni-moderation-latest",
        "results": [
            {
                "flagged": false,
                "categories": {"self-harm": false},
                "category_scores": {"self-harm": 0.0},
                "category_applied_input_types": {
                    "self-harm": ["text"]
                }
            },
            {
                "flagged": true,
                "categories": {"violence": true},
                "category_scores": {"violence": 0.72},
                "category_applied_input_types": {
                    "violence": ["image"]
                }
            }
        ]
    })
    .to_string()
}
