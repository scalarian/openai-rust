use openai_rust::{ApiErrorKind, ErrorKind, OpenAI, resources::responses::ResponseCreateParams};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAI::builder().build();

    let stored = client.responses().create(ResponseCreateParams {
        model: String::from("gpt-4.1-nano"),
        input: Some(json!("Reply with exactly: stored response smoke")),
        store: Some(true),
        ..Default::default()
    })?;
    let stored_id = stored.output().id.clone();
    let stored_request_id = stored.request_id().unwrap_or("<missing>");

    let retrieved = client
        .responses()
        .retrieve(&stored_id, Default::default())?;

    println!("stored create request id: {stored_request_id}");
    println!("stored response id: {stored_id}");
    println!(
        "stored retrieve request id: {}",
        retrieved.request_id().unwrap_or("<missing>")
    );
    println!("stored output_text: {}", retrieved.output().output_text());

    let unstored = client.responses().create(ResponseCreateParams {
        model: String::from("gpt-4.1-nano"),
        input: Some(json!("Reply with exactly: ephemeral response smoke")),
        store: Some(false),
        ..Default::default()
    })?;
    let unstored_id = unstored.output().id.clone();

    match client
        .responses()
        .retrieve(&unstored_id, Default::default())
    {
        Err(error)
            if matches!(
                error.kind,
                ErrorKind::Api(ApiErrorKind::NotFound)
                    | ErrorKind::Api(ApiErrorKind::PermissionDenied)
            ) =>
        {
            println!(
                "unstored retrieve failed as expected: kind={:?} request_id={:?}",
                error.kind,
                error.request_id()
            );
        }
        Ok(response) => {
            return Err(format!(
                "expected unstored retrieve to fail, but it succeeded for {} with request id {:?}",
                response.output().id,
                response.request_id()
            )
            .into());
        }
        Err(error) => {
            return Err(format!(
                "unexpected unstored retrieve error: {error} ({:?})",
                error.kind
            )
            .into());
        }
    }

    Ok(())
}
