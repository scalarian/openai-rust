use openai_rust::{DEFAULT_BASE_URL, OpenAI};
use serde_json::Value;

fn resolve_default_host_live_base_url(client: &OpenAI, openai_base_url: Option<&str>) -> String {
    assert!(
        openai_base_url.is_none(),
        "default-host live smoke requires OPENAI_BASE_URL to be unset in-shell; run `unset OPENAI_BASE_URL` before `cargo test --test live_core_smoke -- --ignored --nocapture`"
    );

    let base_url = client
        .resolved_config()
        .expect("live smoke client should resolve configuration")
        .base_url;

    assert_eq!(
        base_url, DEFAULT_BASE_URL,
        "default-host live smoke must target the default OpenAI host"
    );

    base_url
}

#[test]
fn live_default_host_smoke_requires_openai_base_url_to_be_unset() {
    let client = OpenAI::builder().api_key("test-key").build();

    let error = std::panic::catch_unwind(|| {
        resolve_default_host_live_base_url(&client, Some("https://example.invalid/v1"))
    })
    .expect_err("explicit OPENAI_BASE_URL override should be rejected for the live smoke");

    let message = if let Some(message) = error.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = error.downcast_ref::<&str>() {
        (*message).to_string()
    } else {
        String::from("non-string panic payload")
    };

    assert!(
        message.contains("OPENAI_BASE_URL"),
        "expected panic message to mention OPENAI_BASE_URL, got: {message}"
    );
}

#[test]
fn live_default_host_smoke_resolves_the_default_openai_host() {
    let client = OpenAI::builder().api_key("test-key").build();

    assert_eq!(
        resolve_default_host_live_base_url(&client, None),
        DEFAULT_BASE_URL
    );
}

#[test]
#[ignore = "requires live OpenAI credentials"]
fn default_host_core_client_captures_real_request_id() {
    let client = OpenAI::builder().build();
    let base_url = resolve_default_host_live_base_url(
        &client,
        std::env::var("OPENAI_BASE_URL").ok().as_deref(),
    );
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

    println!("live base url: {base_url}");
    println!("live request id: {request_id}");
}
