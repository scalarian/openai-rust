use openai_rust::{
    OpenAI,
    resources::responses::{ResponseCompactParams, ResponseCreateParams},
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAI::builder().build();

    let seed = client.responses().create(ResponseCreateParams {
        model: String::from("gpt-4.1-nano"),
        input: Some(json!(
            "In one short sentence, describe what compaction does for long conversations."
        )),
        store: Some(true),
        ..Default::default()
    })?;

    let compacted = client.responses().compact(ResponseCompactParams {
        model: String::from("gpt-4.1-nano"),
        previous_response_id: Some(seed.output().id.clone()),
        ..Default::default()
    })?;

    println!(
        "seed request id: {}",
        seed.request_id().unwrap_or("<missing>")
    );
    println!("seed response id: {}", seed.output().id);
    println!(
        "compact request id: {}",
        compacted.request_id().unwrap_or("<missing>")
    );
    println!("compaction id: {}", compacted.output().id);
    println!("compaction object: {}", compacted.output().object);
    println!(
        "compaction output items: {}",
        compacted.output().output.len()
    );
    println!(
        "compaction total tokens: {}",
        compacted.output().usage["total_tokens"]
    );

    if compacted.output().object != "response.compaction" {
        return Err(format!(
            "unexpected compaction object type: {}",
            compacted.output().object
        )
        .into());
    }
    if compacted.output().output.is_empty() {
        return Err("compaction output should not be empty".into());
    }

    Ok(())
}
