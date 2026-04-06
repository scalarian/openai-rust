#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::evals::{
        EvalDataSourceConfig, EvalDeleteResponse, EvalGrader, EvalListParams, EvalOrderBy,
        EvalOrderDirection, EvalUpdateParams,
    },
};
use serde_json::json;

#[test]
fn evals_crud_preserves_schema_bearing_datasource_and_testing_criteria_contracts() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(eval_payload(
            "eval_custom",
            "Custom eval",
            json!({
                "type": "custom",
                "schema": {
                    "type": "object",
                    "properties": {
                        "question": {"type": "string"},
                        "expected": {"type": "string"}
                    },
                    "required": ["question", "expected"]
                }
            }),
            criteria_payload(),
        )),
        json_response(eval_payload(
            "eval_logs",
            "Logs eval",
            json!({
                "type": "logs",
                "schema": {
                    "type": "object",
                    "properties": {
                        "item": {"type": "object"},
                        "sample": {"type": "object"}
                    }
                },
                "metadata": {"usecase": "chatbot"}
            }),
            criteria_payload(),
        )),
        json_response(eval_payload(
            "eval_sc",
            "Stored completions",
            json!({
                "type": "stored_completions",
                "schema": {"type": "object", "properties": {"prompt": {"type": "string"}}},
                "metadata": {"source": "archive"}
            }),
            criteria_payload(),
        )),
        json_response(eval_list_payload()),
        json_response(
            json!({
                "object": "eval.deleted",
                "deleted": true,
                "eval_id": "eval_sc"
            })
            .to_string(),
        ),
    ])
    .unwrap();
    let client = client(&server.url());

    let created = client
        .evals()
        .create(openai_rust::resources::evals::EvalCreateParams {
            data_source_config: openai_rust::resources::evals::EvalCreateDataSourceConfig::Custom {
                item_schema: json!({
                    "type": "object",
                    "properties": {
                        "question": {"type": "string"},
                        "expected": {"type": "string"}
                    },
                    "required": ["question", "expected"]
                }),
                include_sample_schema: Some(true),
            },
            testing_criteria: vec![
                EvalGrader::StringCheck {
                    name: String::from("exact_match"),
                    input: String::from("{{sample.output_text}}"),
                    operation: String::from("eq"),
                    reference: String::from("{{item.expected}}"),
                },
                EvalGrader::ScoreModel {
                    name: String::from("judge"),
                    model: String::from("gpt-4o-mini"),
                    input: vec![openai_rust::resources::evals::EvalMessageTemplate {
                        role: String::from("user"),
                        content: json!("Grade {{sample.output_text}} against {{item.expected}}"),
                        message_type: None,
                    }],
                    pass_threshold: Some(0.8),
                    range: Some(vec![0.0, 1.0]),
                    sampling_params: Some(json!({"temperature": 0.1})),
                },
            ],
            metadata: Some(json!({"suite": "advanced-platform"})),
            name: Some(String::from("Custom eval")),
        })
        .unwrap();
    assert_eq!(created.output.id, "eval_custom");
    match &created.output.data_source_config {
        EvalDataSourceConfig::Custom { schema, .. } => {
            assert_eq!(schema["properties"]["question"]["type"], json!("string"));
        }
        other => panic!("expected custom datasource, got {other:?}"),
    }
    assert!(matches!(
        created.output.testing_criteria[0],
        EvalGrader::StringCheck { .. }
    ));

    let retrieved = client.evals().retrieve("eval_logs").unwrap();
    match &retrieved.output.data_source_config {
        EvalDataSourceConfig::Logs {
            metadata, schema, ..
        } => {
            assert_eq!(metadata.as_ref().unwrap()["usecase"], json!("chatbot"));
            assert_eq!(schema["properties"]["sample"]["type"], json!("object"));
        }
        other => panic!("expected logs datasource, got {other:?}"),
    }

    let updated = client
        .evals()
        .update(
            "eval_sc",
            EvalUpdateParams {
                metadata: Some(json!({"source": "archive", "env": "prod"})),
                name: Some(String::from("Stored completions")),
            },
        )
        .unwrap();
    match &updated.output.data_source_config {
        EvalDataSourceConfig::StoredCompletions { metadata, .. } => {
            assert_eq!(metadata.as_ref().unwrap()["source"], json!("archive"));
        }
        other => panic!("expected stored completions datasource, got {other:?}"),
    }

    let listed = client
        .evals()
        .list(EvalListParams {
            after: Some(String::from("eval_000")),
            limit: Some(2),
            order: Some(EvalOrderDirection::Asc),
            order_by: Some(EvalOrderBy::UpdatedAt),
        })
        .unwrap();
    assert_eq!(listed.output.data.len(), 3);
    assert!(listed.output.has_next_page());
    assert_eq!(listed.output.next_after(), Some("eval_sc"));
    assert!(matches!(
        listed.output.data[2].data_source_config,
        EvalDataSourceConfig::StoredCompletions { .. }
    ));

    let deleted = client.evals().delete("eval_sc").unwrap();
    assert_eq!(
        deleted.output,
        EvalDeleteResponse {
            object: String::from("eval.deleted"),
            deleted: true,
            eval_id: String::from("eval_sc"),
            extra: Default::default(),
        }
    );

    let requests = server.captured_requests(5).unwrap();
    assert_eq!(requests[0].path, "/v1/evals");
    assert_eq!(requests[1].path, "/v1/evals/eval_logs");
    assert_eq!(requests[2].path, "/v1/evals/eval_sc");
    assert_eq!(
        requests[3].path,
        "/v1/evals?after=eval_000&limit=2&order=asc&order_by=updated_at"
    );
    assert_eq!(requests[4].path, "/v1/evals/eval_sc");

    let create_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["data_source_config"]["type"], json!("custom"));
    assert_eq!(
        create_body["data_source_config"]["include_sample_schema"],
        json!(true)
    );
    assert_eq!(
        create_body["testing_criteria"][1]["pass_threshold"],
        json!(0.8)
    );
    assert_eq!(
        create_body["testing_criteria"][1]["sampling_params"]["temperature"],
        json!(0.1)
    );

    let update_body: serde_json::Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(update_body["metadata"]["env"], json!("prod"));
    assert_eq!(update_body["name"], json!("Stored completions"));

    let blank_id = client.evals().retrieve(" ").unwrap_err();
    assert!(matches!(blank_id.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn eval_payload(
    id: &str,
    name: &str,
    data_source_config: serde_json::Value,
    testing_criteria: serde_json::Value,
) -> String {
    json!({
        "id": id,
        "object": "eval",
        "created_at": 1_717_171_717,
        "name": name,
        "metadata": {"suite": "advanced-platform"},
        "data_source_config": data_source_config,
        "testing_criteria": testing_criteria,
    })
    .to_string()
}

fn criteria_payload() -> serde_json::Value {
    json!([
        {
            "type": "string_check",
            "name": "exact_match",
            "input": "{{sample.output_text}}",
            "operation": "eq",
            "reference": "{{item.expected}}"
        },
        {
            "type": "score_model",
            "name": "judge",
            "model": "gpt-4o-mini",
            "input": [{"role": "user", "content": "Grade {{sample.output_text}} against {{item.expected}}"}],
            "range": [0.0, 1.0],
            "pass_threshold": 0.8,
            "sampling_params": {"temperature": 0.1}
        }
    ])
}

fn eval_list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&eval_payload("eval_custom", "Custom eval", json!({"type": "custom", "schema": {"type": "object"}}), criteria_payload())).unwrap(),
            serde_json::from_str::<serde_json::Value>(&eval_payload("eval_logs", "Logs eval", json!({"type": "logs", "schema": {"type": "object"}, "metadata": {"usecase": "chatbot"}}), criteria_payload())).unwrap(),
            serde_json::from_str::<serde_json::Value>(&eval_payload("eval_sc", "Stored completions", json!({"type": "stored_completions", "schema": {"type": "object"}, "metadata": {"source": "archive"}}), criteria_payload())).unwrap()
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
