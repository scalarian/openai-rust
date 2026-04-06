#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::fine_tuning::{
        AutoOrNumber, FineTuningGrader, FineTuningJobCreateParams, FineTuningJobEventLevel,
        FineTuningJobEventListParams, FineTuningMethod, FineTuningMethodConfig,
        FineTuningReinforcementHyperparameters, FineTuningSupervisedHyperparameters,
    },
};
use serde_json::json;

#[test]
fn method_variants_and_job_controls_remain_distinct() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(job_payload("ftjob_supervised", "queued")),
        json_response(job_payload("ftjob_reinforcement", "queued")),
        json_response(events_payload()),
        json_response(job_payload("ftjob_pause", "paused")),
        json_response(job_payload("ftjob_resume", "running")),
    ])
    .unwrap();
    let client = client(&server.url());

    let supervised = client
        .fine_tuning()
        .jobs()
        .create(FineTuningJobCreateParams {
            model: String::from("gpt-4o-mini"),
            training_file: String::from("file-supervised"),
            method: Some(FineTuningMethod::Supervised(FineTuningMethodConfig {
                supervised: Some(
                    openai_rust::resources::fine_tuning::FineTuningSupervisedMethod {
                        hyperparameters: Some(FineTuningSupervisedHyperparameters {
                            batch_size: Some(AutoOrNumber::Number(2)),
                            n_epochs: Some(AutoOrNumber::Auto),
                            learning_rate_multiplier: Some(AutoOrNumber::Number(1)),
                        }),
                    },
                ),
                ..Default::default()
            })),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(supervised.output.id, "ftjob_supervised");

    let reinforcement = client
        .fine_tuning()
        .jobs()
        .create(FineTuningJobCreateParams {
            model: String::from("gpt-4o-mini"),
            training_file: String::from("file-reinforcement"),
            method: Some(FineTuningMethod::Reinforcement(FineTuningMethodConfig {
                reinforcement: Some(
                    openai_rust::resources::fine_tuning::FineTuningReinforcementMethod {
                        grader: FineTuningGrader::StringCheck {
                            input: String::from("{{sample.output_text}}"),
                            name: String::from("exact_match"),
                            operation: String::from("eq"),
                            reference: String::from("sunny"),
                        },
                        hyperparameters: Some(FineTuningReinforcementHyperparameters {
                            eval_interval: Some(AutoOrNumber::Number(5)),
                            reasoning_effort: Some(String::from("low")),
                            ..Default::default()
                        }),
                    },
                ),
                ..Default::default()
            })),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(reinforcement.output.id, "ftjob_reinforcement");

    let events = client
        .fine_tuning()
        .jobs()
        .list_events(
            "ftjob_reinforcement",
            FineTuningJobEventListParams {
                after: Some(String::from("evt_000")),
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(events.output.data.len(), 2);
    assert_eq!(events.output.data[0].level, FineTuningJobEventLevel::Info);
    assert_eq!(events.output.data[1].message, "job metrics available");

    let paused = client
        .fine_tuning()
        .jobs()
        .pause("ftjob_reinforcement")
        .unwrap();
    assert_eq!(paused.output.status.as_str(), "paused");

    let resumed = client
        .fine_tuning()
        .jobs()
        .resume("ftjob_reinforcement")
        .unwrap();
    assert_eq!(resumed.output.status.as_str(), "running");

    let requests = server.captured_requests(5).unwrap();
    assert_eq!(requests[0].path, "/v1/fine_tuning/jobs");
    assert_eq!(requests[1].path, "/v1/fine_tuning/jobs");
    assert_eq!(
        requests[2].path,
        "/v1/fine_tuning/jobs/ftjob_reinforcement/events?after=evt_000&limit=2"
    );
    assert_eq!(
        requests[3].path,
        "/v1/fine_tuning/jobs/ftjob_reinforcement/pause"
    );
    assert_eq!(
        requests[4].path,
        "/v1/fine_tuning/jobs/ftjob_reinforcement/resume"
    );

    let supervised_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(supervised_body["method"]["type"], json!("supervised"));
    assert_eq!(
        supervised_body["method"]["supervised"]["hyperparameters"]["batch_size"],
        json!(2)
    );
    assert_eq!(
        supervised_body["method"]["supervised"]["hyperparameters"]["n_epochs"],
        json!("auto")
    );

    let reinforcement_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(reinforcement_body["method"]["type"], json!("reinforcement"));
    assert_eq!(
        reinforcement_body["method"]["reinforcement"]["grader"]["type"],
        json!("string_check")
    );
    assert_eq!(
        reinforcement_body["method"]["reinforcement"]["hyperparameters"]["eval_interval"],
        json!(5)
    );
    assert_eq!(
        reinforcement_body["method"]["reinforcement"]["hyperparameters"]["reasoning_effort"],
        json!("low")
    );

    let blank = client
        .fine_tuning()
        .jobs()
        .list_events(" ", FineTuningJobEventListParams::default())
        .unwrap_err();
    assert!(matches!(blank.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .max_retries(0)
        .build()
}

fn job_payload(id: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "fine_tuning.job",
        "created_at": 1_717_171_717,
        "error": null,
        "fine_tuned_model": null,
        "finished_at": null,
        "hyperparameters": {"n_epochs": "auto"},
        "model": "gpt-4o-mini",
        "organization_id": "org_123",
        "result_files": [],
        "seed": 7,
        "status": status,
        "trained_tokens": null,
        "training_file": "file-train",
        "validation_file": null
    })
    .to_string()
}

fn events_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "id": "evt_123",
                "object": "fine_tuning.job.event",
                "created_at": 1_717_171_800,
                "level": "info",
                "message": "job queued",
                "type": "message"
            },
            {
                "id": "evt_124",
                "object": "fine_tuning.job.event",
                "created_at": 1_717_171_801,
                "level": "warn",
                "message": "job metrics available",
                "type": "metrics",
                "data": {"train_loss": 0.42}
            }
        ],
        "has_more": false
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
