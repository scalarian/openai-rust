use openai_rust::{
    OpenAI,
    resources::responses::{ResponseCompactParams, ResponseCreateParams, ResponseInput},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAI::builder().build();

    let seed = client.responses().create(ResponseCreateParams {
        model: String::from("gpt-4.1-nano"),
        input: Some(ResponseInput::text(
            "In one short sentence, describe what compaction does for long conversations.",
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
        compacted
            .output()
            .usage
            .as_ref()
            .and_then(|usage| usage.total_tokens)
            .map(|tokens| tokens.to_string())
            .unwrap_or_else(|| String::from("<missing>"))
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
