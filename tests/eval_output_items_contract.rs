#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::evals::{EvalOutputItemListParams, EvalOutputItemStatus},
};
use serde_json::json;

#[test]
fn eval_output_items_preserve_grader_results_sample_provenance_and_usage_detail() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(output_item_list_payload()),
        json_response(output_item_payload("out_123", "pass")),
    ])
    .unwrap();
    let client = client(&server.url());

    let listed = client
        .evals()
        .runs()
        .output_items()
        .list(
            "eval_123",
            "run_123",
            EvalOutputItemListParams {
                after: Some(String::from("out_000")),
                limit: Some(1),
                order: Some(openai_rust::resources::evals::EvalOrderDirection::Desc),
                status: Some(EvalOutputItemStatus::Pass),
            },
        )
        .unwrap();
    assert_eq!(listed.output.data.len(), 1);
    assert!(listed.output.has_next_page());
    assert_eq!(listed.output.next_after(), Some("out_123"));
    assert_eq!(listed.output.data[0].results[0].name, "exact_match");
    assert_eq!(
        listed.output.data[0].results[0].extra["reason"],
        json!("matched expected answer")
    );
    assert_eq!(listed.output.data[0].sample.usage.prompt_tokens, 11);
    assert_eq!(
        listed.output.data[0].sample.usage.extra["completion_tokens_details"]["reasoning_tokens"],
        json!(2)
    );
    assert_eq!(
        listed.output.data[0].sample.output[0].role.as_deref(),
        Some("assistant")
    );
    assert_eq!(
        listed.output.data[0].sample.output[0]
            .content
            .as_ref()
            .unwrap(),
        &json!("4")
    );

    let retrieved = client
        .evals()
        .runs()
        .output_items()
        .retrieve("eval_123", "run_123", "out_123")
        .unwrap();
    assert_eq!(retrieved.output.datasource_item_id, 42);
    assert_eq!(retrieved.output.datasource_item["question"], json!("2+2?"));
    assert_eq!(
        retrieved.output.sample.input[0].content.as_ref().unwrap(),
        &json!("2+2?")
    );
    assert_eq!(
        retrieved
            .output
            .sample
            .error
            .as_ref()
            .unwrap()
            .code
            .as_deref(),
        Some("grader_timeout")
    );
    assert_eq!(
        retrieved.output.results[1].sample.as_ref().unwrap()["rubric"],
        json!("lenient")
    );

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(
        requests[0].path,
        "/v1/evals/eval_123/runs/run_123/output_items?after=out_000&limit=1&order=desc&status=pass"
    );
    assert_eq!(
        requests[1].path,
        "/v1/evals/eval_123/runs/run_123/output_items/out_123"
    );

    let blank_eval = client
        .evals()
        .runs()
        .output_items()
        .list(" ", "run_123", EvalOutputItemListParams::default())
        .unwrap_err();
    assert!(matches!(blank_eval.kind, ErrorKind::Validation));
    let blank_output_item = client
        .evals()
        .runs()
        .output_items()
        .retrieve("eval_123", "run_123", " ")
        .unwrap_err();
    assert!(matches!(blank_output_item.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn output_item_payload(id: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "eval.run.output_item",
        "created_at": 1_717_171_818,
        "eval_id": "eval_123",
        "run_id": "run_123",
        "datasource_item_id": 42,
        "datasource_item": {"question": "2+2?", "expected": "4"},
        "status": status,
        "results": [
            {
                "name": "exact_match",
                "passed": true,
                "score": 1.0,
                "type": "string_check",
                "reason": "matched expected answer"
            },
            {
                "name": "judge",
                "passed": true,
                "score": 0.91,
                "type": "score_model",
                "sample": {"rubric": "lenient"},
                "model": "gpt-4o-mini"
            }
        ],
        "sample": {
            "model": "gpt-4o-mini",
            "seed": 7,
            "temperature": 0.2,
            "top_p": 0.9,
            "max_completion_tokens": 32,
            "finish_reason": "stop",
            "error": {"code": "grader_timeout", "message": "grader retried"},
            "input": [{"role": "user", "content": "2+2?"}],
            "output": [{"role": "assistant", "content": "4"}],
            "usage": {
                "prompt_tokens": 11,
                "completion_tokens": 5,
                "cached_tokens": 0,
                "total_tokens": 16,
                "completion_tokens_details": {"reasoning_tokens": 2}
            }
        }
    })
    .to_string()
}

fn output_item_list_payload() -> String {
    json!({
        "object": "list",
        "data": [serde_json::from_str::<serde_json::Value>(&output_item_payload("out_123", "pass")).unwrap()],
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
