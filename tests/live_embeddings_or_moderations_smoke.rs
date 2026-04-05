use openai_rust::OpenAI;
use serde_json::json;

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_embeddings_smoke_captures_request_id() {
    let client = OpenAI::builder().build();

    let response = client
        .embeddings()
        .create(openai_rust::resources::embeddings::EmbeddingCreateParams {
            model: String::from("text-embedding-3-small"),
            input: json!("hello from openai-rust"),
            ..Default::default()
        })
        .expect("live embeddings request should succeed");

    let request_id = response
        .request_id()
        .expect("live embeddings response should expose a request id");
    assert!(!request_id.trim().is_empty());
    assert_eq!(response.output().data.len(), 1);
    assert_eq!(response.output().data[0].index, 0);
    assert!(
        response.output().data[0]
            .embedding
            .as_float_slice()
            .is_some_and(|embedding| !embedding.is_empty()),
        "default embeddings transport behavior should decode into float vectors"
    );

    println!("live embeddings request id: {request_id}");
    println!(
        "live embedding dimensions: {}",
        response.output().data[0]
            .embedding
            .as_float_slice()
            .map(|embedding| embedding.len())
            .unwrap_or_default()
    );
}
