use openai_rust::{
    DEFAULT_BASE_URL, OpenAI,
    resources::evals::{
        EvalCreateDataSourceConfig, EvalCreateParams, EvalGrader, EvalRunCreateParams,
        EvalRunDataSource, EvalRunSource, EvalRunSourceRow, EvalRunStatus,
    },
};
use serde_json::json;

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_eval_run_smoke_proves_create_cancel_and_status_request_ids() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live eval-run client should resolve configuration");
    assert_eq!(resolved.base_url, DEFAULT_BASE_URL);

    let eval = client
        .evals()
        .create(EvalCreateParams {
            data_source_config: EvalCreateDataSourceConfig::Custom {
                item_schema: json!({
                    "type": "object",
                    "properties": {
                        "question": {"type": "string"},
                        "expected": {"type": "string"},
                        "model": {"type": "string"}
                    },
                    "required": ["question", "expected", "model"]
                }),
                include_sample_schema: Some(false),
            },
            testing_criteria: vec![EvalGrader::StringCheck {
                name: String::from("exact_match"),
                input: String::from("{{sample.output_text}}"),
                operation: String::from("eq"),
                reference: String::from("{{item.expected}}"),
            }],
            metadata: Some(json!({"suite": "live-eval-run-smoke"})),
            name: Some(String::from("live eval run smoke")),
        })
        .expect("live eval create should succeed");
    let eval_id = eval.output.id.clone();

    let created = client
        .evals()
        .runs()
        .create(
            &eval_id,
            EvalRunCreateParams {
                data_source: EvalRunDataSource::Jsonl {
                    source: EvalRunSource::FileContent {
                        content: vec![EvalRunSourceRow {
                            item: json!({"question": "2+2?", "expected": "4", "model": "gpt-4.1-nano"}),
                            sample: Some(json!({"model": "gpt-4.1-nano", "output_text": "4"})),
                        }],
                    },
                },
                metadata: Some(json!({"suite": "live-eval-run-smoke"})),
                name: Some(String::from("live eval run smoke run")),
            },
        )
        .expect("live eval run create should succeed");
    let run_id = created.output.id.clone();
    assert_eq!(created.output.eval_id, eval_id);
    assert!(matches!(
        created.output.status,
        EvalRunStatus::Queued | EvalRunStatus::InProgress | EvalRunStatus::Completed
    ));

    let retrieved = client
        .evals()
        .runs()
        .retrieve(&eval_id, &run_id)
        .expect("live eval run retrieve should succeed");
    assert_eq!(retrieved.output.id, run_id);
    assert_eq!(retrieved.output.eval_id, eval_id);
    assert!(!retrieved.output.status.as_str().trim().is_empty());

    let listed = client
        .evals()
        .runs()
        .list(
            &eval_id,
            openai_rust::resources::evals::EvalRunListParams {
                after: None,
                limit: Some(20),
                order: Some(openai_rust::resources::evals::EvalOrderDirection::Desc),
            },
        )
        .expect("live eval run list should succeed");
    assert!(listed.output.data.iter().any(|run| run.id == run_id));

    let cancelled = client
        .evals()
        .runs()
        .cancel(&eval_id, &run_id)
        .expect("live eval run cancel should succeed");
    assert_eq!(cancelled.output.id, run_id);
    assert!(matches!(
        cancelled.output.status,
        EvalRunStatus::Queued
            | EvalRunStatus::InProgress
            | EvalRunStatus::Completed
            | EvalRunStatus::Canceled
            | EvalRunStatus::Failed
    ));

    println!("live eval id: {eval_id}");
    println!("live eval run id: {run_id}");
    println!(
        "live eval create request id: {}",
        eval.request_id().unwrap_or("<missing>")
    );
    println!(
        "live eval-run create request id: {}",
        created.request_id().unwrap_or("<missing>")
    );
    println!(
        "live eval-run retrieve request id: {}",
        retrieved.request_id().unwrap_or("<missing>")
    );
    println!(
        "live eval-run list request id: {}",
        listed.request_id().unwrap_or("<missing>")
    );
    println!(
        "live eval-run cancel request id: {}",
        cancelled.request_id().unwrap_or("<missing>")
    );
    println!(
        "live eval-run cancel status: {}",
        cancelled.output.status.as_str()
    );

    match client.evals().delete(&eval_id) {
        Ok(deleted) => println!(
            "live eval cleanup delete request id: {}",
            deleted.request_id().unwrap_or("<missing>")
        ),
        Err(error) => println!("live eval cleanup could not delete eval {eval_id}: {error}"),
    }
}
