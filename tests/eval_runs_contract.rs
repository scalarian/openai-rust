#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::evals::{
        EvalRunDataSource, EvalRunDeleteResponse, EvalRunInputMessages, EvalRunListParams,
        EvalRunOutputTextFormat, EvalRunSamplingParams, EvalRunStatus, EvalRunTextConfig,
    },
};
use serde_json::json;

#[test]
fn eval_runs_cover_routes_cancel_semantics_and_datasource_families() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(run_payload("run_resp", responses_data_source(), "queued")),
        json_response(run_payload(
            "run_comp",
            completions_data_source(),
            "in_progress",
        )),
        json_response(run_list_payload()),
        json_response(
            json!({"object": "eval.run.deleted", "deleted": true, "run_id": "run_jsonl"})
                .to_string(),
        ),
        json_response(run_payload("run_resp", responses_data_source(), "canceled")),
    ])
    .unwrap();
    let client = client(&server.url());

    let created = client.evals().runs().create(
        "eval_123",
        openai_rust::resources::evals::EvalRunCreateParams {
            data_source: EvalRunDataSource::Responses {
                source: openai_rust::resources::evals::EvalRunSource::FileContent {
                    content: vec![openai_rust::resources::evals::EvalRunSourceRow {
                        item: json!({"question": "2+2?", "expected": "4"}),
                        sample: Some(json!({"output_text": "4"})),
                    }],
                },
                input_messages: Some(EvalRunInputMessages::Template {
                    template: vec![openai_rust::resources::evals::EvalMessageTemplate {
                        role: String::from("user"),
                        content: json!("{{item.question}}"),
                        message_type: None,
                    }],
                }),
                model: Some(String::from("gpt-4o-mini")),
                sampling_params: Some(EvalRunSamplingParams {
                    max_completion_tokens: Some(32),
                    seed: Some(7),
                    temperature: Some(0.2),
                    top_p: Some(0.9),
                    text: Some(EvalRunTextConfig {
                        format: Some(EvalRunOutputTextFormat::JsonSchema(json!({
                            "name": "answer",
                            "schema": {"type": "object", "properties": {"answer": {"type": "string"}}}
                        }))),
                    }),
                    reasoning_effort: Some(String::from("low")),
                    tools: Some(json!([{"type": "function", "name": "grade"}])),
                }),
            },
            metadata: Some(json!({"suite": "advanced-platform"})),
            name: Some(String::from("responses run")),
        },
    ).unwrap();
    assert_eq!(created.output.status, EvalRunStatus::Queued);
    assert_eq!(
        created.output.report_url.as_deref(),
        Some("https://platform.openai.com/evals/reports/run_resp")
    );
    match &created.output.data_source {
        EvalRunDataSource::Responses {
            source,
            sampling_params,
            ..
        } => {
            assert!(matches!(
                source,
                openai_rust::resources::evals::EvalRunSource::FileContent { .. }
            ));
            assert_eq!(sampling_params.as_ref().unwrap().seed, Some(7));
        }
        other => panic!("expected responses datasource, got {other:?}"),
    }
    assert_eq!(created.output.result_counts.as_ref().unwrap().total, 1);
    assert_eq!(created.output.per_model_usage[0].model_name, "gpt-4o-mini");

    let retrieved = client
        .evals()
        .runs()
        .retrieve("eval_123", "run_comp")
        .unwrap();
    assert_eq!(retrieved.output.status, EvalRunStatus::InProgress);
    match &retrieved.output.data_source {
        EvalRunDataSource::Completions { source, .. } => {
            assert!(matches!(
                source,
                openai_rust::resources::evals::EvalRunSource::StoredCompletions { .. }
            ));
        }
        other => panic!("expected completions datasource, got {other:?}"),
    }

    let listed = client
        .evals()
        .runs()
        .list(
            "eval_123",
            EvalRunListParams {
                after: Some(String::from("run_000")),
                limit: Some(3),
                order: Some(openai_rust::resources::evals::EvalOrderDirection::Desc),
            },
        )
        .unwrap();
    assert_eq!(listed.output.data.len(), 3);
    assert!(listed.output.has_next_page());
    assert_eq!(listed.output.next_after(), Some("run_jsonl"));
    assert!(matches!(
        listed.output.data[2].data_source,
        EvalRunDataSource::Jsonl { .. }
    ));

    let deleted = client
        .evals()
        .runs()
        .delete("eval_123", "run_jsonl")
        .unwrap();
    assert_eq!(
        deleted.output,
        EvalRunDeleteResponse {
            object: Some(String::from("eval.run.deleted")),
            deleted: Some(true),
            run_id: Some(String::from("run_jsonl")),
            extra: Default::default(),
        }
    );

    let cancelled = client
        .evals()
        .runs()
        .cancel("eval_123", "run_resp")
        .unwrap();
    assert_eq!(cancelled.output.status, EvalRunStatus::Canceled);
    assert_eq!(
        serde_json::to_string(&cancelled.output.status).unwrap(),
        "\"canceled\""
    );

    let requests = server.captured_requests(5).unwrap();
    assert_eq!(requests[0].path, "/v1/evals/eval_123/runs");
    assert_eq!(requests[1].path, "/v1/evals/eval_123/runs/run_comp");
    assert_eq!(
        requests[2].path,
        "/v1/evals/eval_123/runs?after=run_000&limit=3&order=desc"
    );
    assert_eq!(requests[3].path, "/v1/evals/eval_123/runs/run_jsonl");
    assert_eq!(requests[4].path, "/v1/evals/eval_123/runs/run_resp");

    let create_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["data_source"]["type"], json!("responses"));
    assert_eq!(
        create_body["data_source"]["sampling_params"]["seed"],
        json!(7)
    );
    assert_eq!(
        create_body["data_source"]["sampling_params"]["text"]["format"]["type"],
        json!("json_schema")
    );
    assert_eq!(create_body["metadata"]["suite"], json!("advanced-platform"));

    let blank_eval = client.evals().runs().retrieve(" ", "run_123").unwrap_err();
    assert!(matches!(blank_eval.kind, ErrorKind::Validation));
    let blank_run = client.evals().runs().cancel("eval_123", " ").unwrap_err();
    assert!(matches!(blank_run.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn run_payload(id: &str, data_source: serde_json::Value, status: &str) -> String {
    json!({
        "id": id,
        "object": "eval.run",
        "created_at": 1_717_171_717,
        "eval_id": "eval_123",
        "name": format!("{id}-name"),
        "status": status,
        "report_url": format!("https://platform.openai.com/evals/reports/{id}"),
        "model": "gpt-4o-mini",
        "metadata": {"suite": "advanced-platform"},
        "data_source": data_source,
        "error": null,
        "result_counts": {"passed": 1, "failed": 0, "errored": 0, "total": 1},
        "per_model_usage": [{
            "model_name": "gpt-4o-mini",
            "invocation_count": 1,
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "cached_tokens": 0,
            "total_tokens": 15
        }],
        "per_testing_criteria_results": [{
            "testing_criteria": "exact_match",
            "passed": 1,
            "failed": 0
        }]
    })
    .to_string()
}

fn responses_data_source() -> serde_json::Value {
    json!({
        "type": "responses",
        "source": {
            "type": "file_content",
            "content": [{"item": {"question": "2+2?", "expected": "4"}, "sample": {"output_text": "4"}}]
        },
        "input_messages": {
            "type": "template",
            "template": [{"role": "user", "content": "{{item.question}}"}]
        },
        "model": "gpt-4o-mini",
        "sampling_params": {
            "seed": 7,
            "temperature": 0.2,
            "text": {"format": {"type": "json_schema", "json_schema": {"name": "answer", "schema": {"type": "object"}}}}
        }
    })
}

fn completions_data_source() -> serde_json::Value {
    json!({
        "type": "completions",
        "source": {
            "type": "stored_completions",
            "limit": 1,
            "metadata": {"suite": "advanced-platform"},
            "model": "gpt-4o-mini"
        },
        "input_messages": {"type": "item_reference", "item_reference": "item.input_trajectory"},
        "model": "gpt-4o-mini",
        "sampling_params": {"temperature": 0.4}
    })
}

fn jsonl_data_source() -> serde_json::Value {
    json!({
        "type": "jsonl",
        "source": {"type": "file_id", "id": "file_jsonl_123"}
    })
}

fn run_list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&run_payload("run_resp", responses_data_source(), "queued")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&run_payload("run_comp", completions_data_source(), "in_progress")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&run_payload("run_jsonl", jsonl_data_source(), "completed")).unwrap()
        ],
        "has_more": true
    }).to_string()
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}
