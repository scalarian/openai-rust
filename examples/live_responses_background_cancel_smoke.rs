use openai_rust::{OpenAI, resources::responses::ResponseCreateParams};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAI::builder().build();

    let created = client.responses().create(ResponseCreateParams {
        model: String::from("gpt-4.1-nano"),
        input: Some(json!(
            "Write the numbers 1 through 400, one per line, with no commentary."
        )),
        background: Some(true),
        store: Some(true),
        ..Default::default()
    })?;

    let response_id = created.output().id.clone();
    println!(
        "background create request id: {}",
        created.request_id().unwrap_or("<missing>")
    );
    println!("background response id: {response_id}");
    println!("background create status: {:?}", created.output().status);

    let cancelled = client.responses().cancel(&response_id)?;
    let status = cancelled
        .output()
        .status
        .clone()
        .unwrap_or_else(|| String::from("<missing>"));

    println!(
        "cancel request id: {}",
        cancelled.request_id().unwrap_or("<missing>")
    );
    println!("cancel status: {status}");
    println!(
        "cancel output_text length: {}",
        cancelled.output().output_text().len()
    );

    match status.as_str() {
        "cancelled" | "completed" | "incomplete" => Ok(()),
        other => Err(format!("unexpected terminal status after cancel attempt: {other}").into()),
    }
}
