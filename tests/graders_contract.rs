#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    OpenAI,
    resources::fine_tuning::{
        FineTuningGrader, FineTuningGraderMessage, FineTuningGraderRunParams,
        FineTuningGraderValidateParams,
    },
};
use serde_json::json;

#[test]
fn graders_validate_configs_and_run_tiny_samples() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(validate_payload()),
        json_response(run_payload()),
        json_response(label_model_validate_payload()),
        json_response(label_model_run_payload()),
    ])
    .unwrap();
    let client = client(&server.url());

    let score_model = FineTuningGrader::ScoreModel {
        input: vec![
            openai_rust::resources::fine_tuning::FineTuningGraderMessage {
                role: String::from("system"),
                content: json!("Judge whether the answer matches the reference exactly."),
                message_type: Some(String::from("message")),
            },
        ],
        model: String::from("gpt-4o-mini"),
        name: String::from("judge_exactness"),
        range: Some(vec![0.0, 1.0]),
        sampling_params: Some(
            openai_rust::resources::fine_tuning::FineTuningGraderSamplingParams {
                temperature: Some(0.0),
                ..Default::default()
            },
        ),
    };

    let validated = client
        .fine_tuning()
        .alpha()
        .graders()
        .validate(FineTuningGraderValidateParams {
            grader: score_model.clone(),
        })
        .unwrap();
    assert!(matches!(
        validated.output.grader.unwrap(),
        FineTuningGrader::ScoreModel { .. }
    ));

    let run = client
        .fine_tuning()
        .alpha()
        .graders()
        .run(FineTuningGraderRunParams {
            grader: FineTuningGrader::StringCheck {
                input: String::from("{{sample.output_text}}"),
                name: String::from("exact_match"),
                operation: String::from("eq"),
                reference: String::from("sunny"),
            },
            model_sample: String::from("sunny"),
            item: Some(json!({"reference": "sunny"})),
        })
        .unwrap();

    assert_eq!(run.output.reward, 1.0);
    assert_eq!(run.output.sub_rewards.get("exact_match"), Some(&json!(1.0)));
    assert_eq!(run.output.metadata.name, "exact_match");
    assert_eq!(run.output.metadata.token_usage, Some(11));
    assert!(!run.output.metadata.errors.formula_parse_error);
    assert_eq!(
        run.output
            .model_grader_token_usage_per_model
            .get("gpt-4o-mini"),
        Some(&json!(11))
    );

    let label_model = FineTuningGrader::LabelModel {
        input: vec![FineTuningGraderMessage {
            role: String::from("system"),
            content: json!("Assign one of the provided labels to the sample output."),
            message_type: Some(String::from("message")),
        }],
        labels: vec![String::from("pass"), String::from("fail")],
        model: String::from("gpt-4o-mini"),
        name: String::from("label_weather_answer"),
        passing_labels: vec![String::from("pass")],
    };

    let label_validated = client
        .fine_tuning()
        .alpha()
        .graders()
        .validate(FineTuningGraderValidateParams {
            grader: label_model.clone(),
        })
        .unwrap();
    assert!(matches!(
        label_validated.output.grader.unwrap(),
        FineTuningGrader::LabelModel { .. }
    ));

    let label_run = client
        .fine_tuning()
        .alpha()
        .graders()
        .run(FineTuningGraderRunParams {
            grader: label_model,
            model_sample: String::from("weather forecast: sunny"),
            item: Some(json!({"reference": "sunny"})),
        })
        .unwrap();
    assert_eq!(label_run.output.reward, 1.0);
    assert_eq!(
        label_run.output.sub_rewards.get("label_weather_answer"),
        Some(&json!(1.0))
    );
    assert_eq!(label_run.output.metadata.name, "label_weather_answer");
    assert_eq!(label_run.output.metadata.grader_type(), "label_model");
    assert!(!label_run.output.metadata.errors.model_grader_parse_error);

    let requests = server.captured_requests(4).unwrap();
    assert_eq!(requests[0].path, "/v1/fine_tuning/alpha/graders/validate");
    assert_eq!(requests[1].path, "/v1/fine_tuning/alpha/graders/run");
    assert_eq!(requests[2].path, "/v1/fine_tuning/alpha/graders/validate");
    assert_eq!(requests[3].path, "/v1/fine_tuning/alpha/graders/run");

    let validate_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(validate_body["grader"]["type"], json!("score_model"));
    assert_eq!(validate_body["grader"]["model"], json!("gpt-4o-mini"));

    let run_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(run_body["grader"]["type"], json!("string_check"));
    assert_eq!(run_body["model_sample"], json!("sunny"));
    assert_eq!(run_body["item"]["reference"], json!("sunny"));

    let label_validate_body: serde_json::Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(label_validate_body["grader"]["type"], json!("label_model"));
    assert_eq!(
        label_validate_body["grader"]["labels"],
        json!(["pass", "fail"])
    );
    assert_eq!(
        label_validate_body["grader"]["passing_labels"],
        json!(["pass"])
    );

    let label_run_body: serde_json::Value = serde_json::from_slice(&requests[3].body).unwrap();
    assert_eq!(label_run_body["grader"]["type"], json!("label_model"));
    assert_eq!(
        label_run_body["model_sample"],
        json!("weather forecast: sunny")
    );
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .max_retries(0)
        .build()
}

fn validate_payload() -> String {
    json!({
        "grader": {
            "type": "score_model",
            "name": "judge_exactness",
            "model": "gpt-4o-mini",
            "input": [{"role": "system", "content": "Judge whether the answer matches the reference exactly.", "type": "message"}],
            "range": [0.0, 1.0],
            "sampling_params": {"temperature": 0.0}
        }
    })
    .to_string()
}

fn run_payload() -> String {
    json!({
        "reward": 1.0,
        "sub_rewards": {"exact_match": 1.0},
        "model_grader_token_usage_per_model": {"gpt-4o-mini": 11},
        "metadata": {
            "name": "exact_match",
            "type": "string_check",
            "sampled_model_name": "gpt-4o-mini",
            "execution_time": 0.12,
            "token_usage": 11,
            "scores": {"exact_match": 1.0},
            "errors": {
                "formula_parse_error": false,
                "invalid_variable_error": false,
                "model_grader_parse_error": false,
                "model_grader_refusal_error": false,
                "model_grader_server_error": false,
                "model_grader_server_error_details": null,
                "other_error": false,
                "python_grader_runtime_error": false,
                "python_grader_runtime_error_details": null,
                "python_grader_server_error": false,
                "python_grader_server_error_type": null,
                "sample_parse_error": false,
                "truncated_observation_error": false,
                "unresponsive_reward_error": false
            }
        }
    })
    .to_string()
}

fn label_model_validate_payload() -> String {
    json!({
        "grader": {
            "type": "label_model",
            "name": "label_weather_answer",
            "model": "gpt-4o-mini",
            "labels": ["pass", "fail"],
            "passing_labels": ["pass"],
            "input": [{"role": "system", "content": "Assign one of the provided labels to the sample output.", "type": "message"}]
        }
    })
    .to_string()
}

fn label_model_run_payload() -> String {
    json!({
        "reward": 1.0,
        "sub_rewards": {"label_weather_answer": 1.0},
        "model_grader_token_usage_per_model": {"gpt-4o-mini": 19},
        "metadata": {
            "name": "label_weather_answer",
            "type": "label_model",
            "sampled_model_name": "gpt-4o-mini",
            "execution_time": 0.18,
            "token_usage": 19,
            "scores": {"label": "pass"},
            "errors": {
                "formula_parse_error": false,
                "invalid_variable_error": false,
                "model_grader_parse_error": false,
                "model_grader_refusal_error": false,
                "model_grader_server_error": false,
                "model_grader_server_error_details": null,
                "other_error": false,
                "python_grader_runtime_error": false,
                "python_grader_runtime_error_details": null,
                "python_grader_server_error": false,
                "python_grader_server_error_type": null,
                "sample_parse_error": false,
                "truncated_observation_error": false,
                "unresponsive_reward_error": false
            }
        }
    })
    .to_string()
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
