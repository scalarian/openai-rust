use openai_rust::resources::{
    chat::ChatCompletionCreateParams, completions::CompletionCreateParams,
    responses::ResponseCreateParams,
};
use serde_json::json;

fn main() {
    let chat = ChatCompletionCreateParams {
        model: "gpt-4.1-mini".into(),
        messages: vec![json!({"role":"user","content":"Say hello"})],
        ..Default::default()
    };

    let legacy = CompletionCreateParams {
        model: "gpt-3.5-turbo-instruct".into(),
        prompt: Some(json!("Say hello")),
        ..Default::default()
    };

    let responses = ResponseCreateParams {
        model: "gpt-4.1-mini".into(),
        input: Some(json!("Say hello")),
        ..Default::default()
    };

    println!("Compatibility chat model: {}", chat.model);
    println!("Legacy completions model: {}", legacy.model);
    println!("Preferred Responses model: {}", responses.model);
}
