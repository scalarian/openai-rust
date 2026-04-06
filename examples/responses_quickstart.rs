use openai_rust::OpenAI;
use openai_rust::resources::responses::ResponseCreateParams;
use serde_json::json;

fn main() {
    let client = OpenAI::builder().build();
    let params = ResponseCreateParams {
        model: "gpt-4.1-mini".into(),
        input: Some(json!("Say hello from Rust.")),
        ..Default::default()
    };

    println!(
        "Prepared a Responses quickstart for model `{}`.",
        params.model
    );
    println!(
        "Env-based client config captured. Explicit API key set? {}",
        client.config().api_key.is_some()
    );
    println!("Next step: export OPENAI_API_KEY and call client.responses().create(params).");
}
