use openai_rust::{DEFAULT_BASE_URL, OpenAI};

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_evals_crud_smoke_captures_request_ids() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live evals client should resolve configuration");
    assert_eq!(resolved.base_url, DEFAULT_BASE_URL);

    let created = client
        .evals()
        .create(openai_rust::resources::evals::EvalCreateParams {
            data_source_config: openai_rust::resources::evals::EvalCreateDataSourceConfig::Custom {
                item_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "question": {"type": "string"},
                        "expected": {"type": "string"}
                    },
                    "required": ["question", "expected"]
                }),
                include_sample_schema: Some(true),
            },
            testing_criteria: vec![openai_rust::resources::evals::EvalGrader::StringCheck {
                name: String::from("exact_match"),
                input: String::from("{{sample.output_text}}"),
                operation: String::from("eq"),
                reference: String::from("{{item.expected}}"),
            }],
            metadata: Some(serde_json::json!({"suite": "live-evals-smoke"})),
            name: Some(String::from("live evals smoke")),
        })
        .expect("live eval create should succeed");
    let eval_id = created.output.id.clone();
    let create_request_id = created
        .request_id()
        .expect("live eval create should expose a request id");
    assert!(!create_request_id.trim().is_empty());

    let updated = client
        .evals()
        .update(
            &eval_id,
            openai_rust::resources::evals::EvalUpdateParams {
                metadata: Some(
                    serde_json::json!({"suite": "live-evals-smoke", "phase": "updated"}),
                ),
                name: Some(String::from("live evals smoke updated")),
            },
        )
        .expect("live eval update should succeed");
    assert_eq!(updated.output.id, eval_id);
    let update_request_id = updated
        .request_id()
        .expect("live eval update should expose a request id");

    let retrieved = client
        .evals()
        .retrieve(&eval_id)
        .expect("live eval retrieve should succeed");
    assert_eq!(retrieved.output.id, eval_id);
    assert_eq!(retrieved.output.name, "live evals smoke updated");

    let deleted = client
        .evals()
        .delete(&eval_id)
        .expect("live eval delete should succeed");
    assert!(deleted.output.deleted);

    println!("live eval id: {eval_id}");
    println!("live eval create request id: {create_request_id}");
    println!("live eval update request id: {update_request_id}");
    println!(
        "live eval retrieve request id: {}",
        retrieved.request_id().unwrap_or("<missing>")
    );
    println!(
        "live eval delete request id: {}",
        deleted.request_id().unwrap_or("<missing>")
    );
}
