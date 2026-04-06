use std::{thread, time::Duration};

use openai_rust::{
    DEFAULT_BASE_URL, OpenAI,
    resources::evals::{
        EvalCreateDataSourceConfig, EvalCreateParams, EvalGrader, EvalOutputItemListParams,
        EvalRunCreateParams, EvalRunDataSource, EvalRunSource, EvalRunSourceRow,
    },
};
use serde_json::json;

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_eval_output_items_smoke_proves_item_listing_and_inspection() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live eval-output-items client should resolve configuration");
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
            metadata: Some(json!({"suite": "live-eval-output-items-smoke"})),
            name: Some(String::from("live eval output items smoke")),
        })
        .expect("live eval create should succeed");
    let eval_id = eval.output.id.clone();

    let run = client
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
                metadata: Some(json!({"suite": "live-eval-output-items-smoke"})),
                name: Some(String::from("live eval output items smoke run")),
            },
        )
        .expect("live eval run create should succeed");
    let run_id = run.output.id.clone();

    let mut listed = client
        .evals()
        .runs()
        .output_items()
        .list(&eval_id, &run_id, EvalOutputItemListParams::default())
        .expect("live eval output-item list should succeed");
    for _ in 0..10 {
        if !listed.output.data.is_empty() {
            break;
        }
        thread::sleep(Duration::from_secs(2));
        listed = client
            .evals()
            .runs()
            .output_items()
            .list(&eval_id, &run_id, EvalOutputItemListParams::default())
            .expect("live eval output-item retry list should succeed");
    }
    assert!(
        !listed.output.data.is_empty(),
        "live eval output-item list should return at least one item"
    );
    let output_item_id = listed.output.data[0].id.clone();
    assert_eq!(listed.output.data[0].eval_id, eval_id);
    assert_eq!(listed.output.data[0].run_id, run_id);

    let retrieved = client
        .evals()
        .runs()
        .output_items()
        .retrieve(&eval_id, &run_id, &output_item_id)
        .expect("live eval output-item retrieve should succeed");
    assert_eq!(retrieved.output.id, output_item_id);
    assert_eq!(retrieved.output.eval_id, eval_id);
    assert_eq!(retrieved.output.run_id, run_id);

    println!("live eval id: {eval_id}");
    println!("live eval run id: {run_id}");
    println!("live eval output item id: {output_item_id}");
    println!(
        "live eval create request id: {}",
        eval.request_id().unwrap_or("<missing>")
    );
    println!(
        "live eval run create request id: {}",
        run.request_id().unwrap_or("<missing>")
    );
    println!(
        "live eval output-item list request id: {}",
        listed.request_id().unwrap_or("<missing>")
    );
    println!(
        "live eval output-item retrieve request id: {}",
        retrieved.request_id().unwrap_or("<missing>")
    );

    match client.evals().delete(&eval_id) {
        Ok(deleted) => println!(
            "live eval cleanup delete request id: {}",
            deleted.request_id().unwrap_or("<missing>")
        ),
        Err(error) => println!("live eval cleanup could not delete eval {eval_id}: {error}"),
    }
}
