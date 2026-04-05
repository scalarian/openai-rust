use openai_rust::OpenAI;
use serde_json::Value;

#[test]
#[ignore = "requires live OpenAI credentials"]
fn default_host_core_client_captures_real_request_id() {
    let client = OpenAI::builder().build();
    let response = client
        .execute_json::<Value>("GET", "/models", Default::default())
        .expect("live default-host request should succeed");

    let request_id = response
        .request_id()
        .expect("live response should expose a non-empty request id");
    assert!(!request_id.trim().is_empty());
    assert!(
        response.output()["data"].is_array(),
        "expected GET /models to return a list payload"
    );

    println!("live request id: {request_id}");
}
