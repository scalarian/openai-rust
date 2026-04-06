use openai_rust::resources::embeddings::{EmbeddingCreateParams, EmbeddingEncodingFormat};
use serde_json::json;

fn main() {
    let params = EmbeddingCreateParams {
        model: "text-embedding-3-small".into(),
        input: json!(["rust sdk", "responses"]),
        dimensions: Some(256),
        encoding_format: Some(EmbeddingEncodingFormat::Float),
        user: Some("docs-example".into()),
        ..Default::default()
    };

    println!("Embedding model: {}", params.model);
    println!("Requested dimensions: {:?}", params.dimensions);
    println!("Explicit encoding: {:?}", params.encoding_format);
}
