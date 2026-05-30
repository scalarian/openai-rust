use openai_rust::resources::{
    chat::{ChatCompletionCreateParams, ChatCompletionMessageParam},
    completions::{CompletionCreateParams, CompletionPrompt},
    responses::{ResponseCreateParams, ResponseInput},
};

fn main() {
    let chat = ChatCompletionCreateParams {
        model: "gpt-5.5".into(),
        messages: vec![ChatCompletionMessageParam::user("Say hello")],
        ..Default::default()
    };

    let legacy = CompletionCreateParams {
        model: "gpt-3.5-turbo-instruct".into(),
        prompt: Some(CompletionPrompt::from("Say hello")),
        ..Default::default()
    };

    let responses = ResponseCreateParams {
        model: "gpt-5.5".into(),
        input: Some(ResponseInput::text("Say hello")),
        ..Default::default()
    };

    println!("Compatibility chat model: {}", chat.model);
    println!("Legacy completions model: {}", legacy.model);
    println!("Preferred Responses model: {}", responses.model);
}
