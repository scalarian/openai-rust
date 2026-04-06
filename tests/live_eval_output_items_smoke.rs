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
    assert_live_output_item_fields(&listed.output.data[0]);

    let retrieved = client
        .evals()
        .runs()
        .output_items()
        .retrieve(&eval_id, &run_id, &output_item_id)
        .expect("live eval output-item retrieve should succeed");
    assert_eq!(retrieved.output.id, output_item_id);
    assert_eq!(retrieved.output.eval_id, eval_id);
    assert_eq!(retrieved.output.run_id, run_id);
    assert_live_output_item_fields(&retrieved.output);

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

fn assert_live_output_item_fields(item: &openai_rust::resources::evals::EvalOutputItem) {
    if let Some(result) = item.results.first() {
        assert!(
            !result.name.trim().is_empty(),
            "live eval output-item grader results should expose a representative non-null name"
        );
    }

    let sample = &item.sample;
    let has_provenance = !sample.model.trim().is_empty()
        || !sample.finish_reason.trim().is_empty()
        || !sample.input.is_empty()
        || !sample.output.is_empty()
        || sample.error.is_some()
        || !sample.extra.is_empty();
    if has_provenance {
        assert!(
            !sample.model.trim().is_empty() || !sample.finish_reason.trim().is_empty(),
            "live eval output-item sample provenance should expose a representative non-null model or finish_reason when present"
        );
    }

    let usage = &sample.usage;
    let has_usage = usage.total_tokens > 0
        || usage.prompt_tokens > 0
        || usage.completion_tokens > 0
        || usage.cached_tokens > 0
        || !usage.extra.is_empty();
    if has_usage {
        assert!(
            usage.total_tokens >= usage.prompt_tokens,
            "live eval output-item sample usage should keep total_tokens >= prompt_tokens when usage is present"
        );
        assert!(
            usage.total_tokens >= usage.completion_tokens,
            "live eval output-item sample usage should keep total_tokens >= completion_tokens when usage is present"
        );
    }
}
