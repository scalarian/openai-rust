use openai_rust::OpenAI;
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

#[test]
fn images_generate_edit_and_variation_preserve_typed_and_multipart_contracts() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(generate_payload()),
        json_response(edit_payload()),
        json_response(variation_payload()),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let generated = client
        .images()
        .generate(openai_rust::resources::images::ImageGenerateParams {
            prompt: String::from("A stained glass lighthouse"),
            model: Some(String::from("gpt-image-1")),
            background: Some(String::from("transparent")),
            moderation: Some(String::from("low")),
            n: Some(1),
            output_compression: Some(80),
            output_format: Some(String::from("png")),
            partial_images: Some(1),
            quality: Some(String::from("high")),
            size: Some(String::from("1024x1024")),
            user: Some(String::from("user-123")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(generated.output().created, 1_717_171_717);
    assert_eq!(generated.output().data.len(), 2);
    assert_eq!(
        generated.output().data[0].b64_json.as_deref(),
        Some("Zmlyc3Q=")
    );
    assert_eq!(
        generated.output().data[1].url.as_deref(),
        Some("https://cdn.example.com/final.png")
    );
    assert_eq!(generated.output().usage.as_ref().unwrap().total_tokens, 42);

    let first = openai_rust::resources::images::ImageInput::new(
        "frame-1.png",
        "image/png",
        vec![0, 1, 2, 3],
    );
    let second = openai_rust::resources::images::ImageInput::new(
        "frame-2.png",
        "image/png",
        vec![4, 5, 6, 7],
    );
    let mask =
        openai_rust::resources::images::ImageInput::new("mask.png", "image/png", vec![9, 8, 7, 6]);

    let edited = client
        .images()
        .edit(openai_rust::resources::images::ImageEditParams {
            images: vec![first.clone(), second.clone()],
            prompt: String::from("Make it brighter"),
            mask: Some(mask.clone()),
            background: Some(String::from("transparent")),
            input_fidelity: Some(String::from("high")),
            model: Some(String::from("gpt-image-1")),
            n: Some(1),
            output_compression: Some(55),
            output_format: Some(String::from("png")),
            partial_images: Some(2),
            quality: Some(String::from("high")),
            size: Some(String::from("1024x1024")),
            user: Some(String::from("user-456")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(edited.output().created, 1_818_181_818);
    assert_eq!(
        edited.output().data[0].b64_json.as_deref(),
        Some("ZWRpdGVk")
    );

    let variation = client
        .images()
        .create_variation(openai_rust::resources::images::ImageVariationParams {
            image: second.clone(),
            model: Some(String::from("dall-e-2")),
            n: Some(2),
            response_format: Some(String::from("b64_json")),
            size: Some(String::from("1024x1024")),
            user: Some(String::from("user-789")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(variation.output().created, 1_919_191_919);
    assert_eq!(
        variation.output().data[0].b64_json.as_deref(),
        Some("dmFyaWF0aW9u")
    );

    let requests = server.captured_requests(3).expect("captured requests");

    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/images/generations");
    let generation_body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(generation_body["prompt"], "A stained glass lighthouse");
    assert_eq!(generation_body["model"], "gpt-image-1");
    assert_eq!(generation_body["partial_images"], 1);
    assert_eq!(generation_body["output_format"], "png");

    assert_eq!(requests[1].method, "POST");
    assert_eq!(requests[1].path, "/v1/images/edits");
    let edit_boundary = boundary_from_headers(&requests[1].headers);
    let edit_body = multipart_support::parse_multipart(&requests[1].body, &edit_boundary).unwrap();
    let image_parts: Vec<_> = edit_body
        .parts
        .iter()
        .filter(|part| part.name.as_deref() == Some("image"))
        .collect();
    assert_eq!(image_parts.len(), 2);
    assert_eq!(image_parts[0].filename.as_deref(), Some("frame-1.png"));
    assert_eq!(
        image_parts[0]
            .headers
            .get("content-type")
            .map(String::as_str),
        Some("image/png")
    );
    assert_eq!(image_parts[0].body, first.bytes);
    assert_eq!(image_parts[1].filename.as_deref(), Some("frame-2.png"));
    assert_eq!(image_parts[1].body, second.bytes);
    let mask_part = edit_body
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some("mask"))
        .expect("mask part");
    assert_eq!(mask_part.filename.as_deref(), Some("mask.png"));
    assert_eq!(
        mask_part.headers.get("content-type").map(String::as_str),
        Some("image/png")
    );
    assert_eq!(mask_part.body, mask.bytes);
    assert_text_part(&edit_body, "prompt", "Make it brighter");
    assert_text_part(&edit_body, "background", "transparent");
    assert_text_part(&edit_body, "input_fidelity", "high");
    assert_text_part(&edit_body, "partial_images", "2");
    assert_text_part(&edit_body, "output_format", "png");
    assert_text_part(&edit_body, "quality", "high");

    assert_eq!(requests[2].method, "POST");
    assert_eq!(requests[2].path, "/v1/images/variations");
    let variation_boundary = boundary_from_headers(&requests[2].headers);
    let variation_body =
        multipart_support::parse_multipart(&requests[2].body, &variation_boundary).unwrap();
    assert_eq!(variation_body.parts.len(), 6);
    let variation_image_parts: Vec<_> = variation_body
        .parts
        .iter()
        .filter(|part| part.name.as_deref() == Some("image"))
        .collect();
    assert_eq!(variation_image_parts.len(), 1);
    assert_eq!(
        variation_image_parts[0].filename.as_deref(),
        Some("frame-2.png")
    );
    assert_eq!(
        variation_image_parts[0]
            .headers
            .get("content-type")
            .map(String::as_str),
        Some("image/png")
    );
    assert_eq!(variation_image_parts[0].body, second.bytes);
    assert_text_part(&variation_body, "model", "dall-e-2");
    assert_text_part(&variation_body, "n", "2");
    assert_text_part(&variation_body, "response_format", "b64_json");
    assert_text_part(&variation_body, "size", "1024x1024");
    assert_text_part(&variation_body, "user", "user-789");
}

fn boundary_from_headers(headers: &std::collections::BTreeMap<String, String>) -> String {
    headers["content-type"]
        .split("boundary=")
        .nth(1)
        .expect("multipart boundary")
        .trim_matches('"')
        .to_string()
}

fn assert_text_part(multipart: &multipart_support::ParsedMultipart, name: &str, value: &str) {
    let part = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("missing multipart text part `{name}`"));
    assert_eq!(std::str::from_utf8(&part.body).unwrap(), value);
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

fn generate_payload() -> String {
    json!({
        "created": 1717171717_i64,
        "data": [
            {"b64_json": "Zmlyc3Q=", "revised_prompt": "A stained glass lighthouse at sunset"},
            {"url": "https://cdn.example.com/final.png"}
        ],
        "usage": {
            "input_tokens": 10,
            "input_tokens_details": {"text_tokens": 6, "image_tokens": 4},
            "output_tokens": 32,
            "total_tokens": 42
        }
    })
    .to_string()
}

fn edit_payload() -> String {
    json!({
        "created": 1818181818_i64,
        "data": [{"b64_json": "ZWRpdGVk"}],
        "usage": {
            "input_tokens": 12,
            "input_tokens_details": {"text_tokens": 7, "image_tokens": 5},
            "output_tokens": 28,
            "total_tokens": 40
        }
    })
    .to_string()
}

fn variation_payload() -> String {
    json!({
        "created": 1919191919_i64,
        "data": [{"b64_json": "dmFyaWF0aW9u"}]
    })
    .to_string()
}
