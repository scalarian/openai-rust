use openai_rust::{ApiErrorKind, ErrorKind, OpenAI, resources::responses::ResponseCreateParams};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAI::builder().build();

    let seed = client.responses().create(ResponseCreateParams {
        model: String::from("gpt-4.1-nano"),
        input: Some(json!("Reply with exactly: continuity seed")),
        store: Some(true),
        ..Default::default()
    })?;

    let response_id = seed.output().id.clone();

    match client.responses().create(ResponseCreateParams {
        model: String::from("gpt-4.1-nano"),
        input: Some(json!(
            "This request should fail because it mixes continuity modes."
        )),
        previous_response_id: Some(response_id),
        conversation: Some(json!("conv_conflict_smoke")),
        store: Some(true),
        ..Default::default()
    }) {
        Err(error)
            if matches!(
                error.kind,
                ErrorKind::Api(ApiErrorKind::BadRequest)
                    | ErrorKind::Api(ApiErrorKind::Conflict)
                    | ErrorKind::Api(ApiErrorKind::UnprocessableEntity)
            ) =>
        {
            println!(
                "conflicting-state failure request id: {:?}",
                error.request_id()
            );
            println!("conflicting-state kind: {:?}", error.kind);
            if let Some(api_error) = error.api_error() {
                println!("conflicting-state message: {}", api_error.message);
            }
            Ok(())
        }
        Err(error) => Err(format!(
            "unexpected conflicting-state error: {error} ({:?})",
            error.kind
        )
        .into()),
        Ok(response) => Err(format!(
            "expected conflicting continuity request to fail, but got response {}",
            response.output().id
        )
        .into()),
    }
}
