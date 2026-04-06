#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ApiErrorKind, ErrorKind, OpenAI,
    resources::fine_tuning::{
        FineTuningCheckpointListParams, FineTuningCheckpointPermissionCreateParams,
        FineTuningCheckpointPermissionListParams, FineTuningJobCreateParams,
        FineTuningJobListParams, FineTuningJobStatus,
    },
};
use serde_json::json;

#[test]
fn job_lifecycle_checkpoint_listing_and_permission_admin_semantics() {
    let success_server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(job_payload("ftjob_123", "queued")),
        json_response(job_payload("ftjob_123", "running")),
        json_response(job_list_payload()),
        json_response(job_payload("ftjob_123", "cancelled")),
        json_response(checkpoint_list_payload()),
        json_response(permission_create_payload()),
        json_response(permission_list_payload()),
        json_response(permission_delete_payload()),
    ])
    .unwrap();
    let sdk = client(&success_server.url(), "sk-test");

    let created = sdk
        .fine_tuning()
        .jobs()
        .create(FineTuningJobCreateParams {
            model: String::from("gpt-4o-mini"),
            training_file: String::from("file-train"),
            validation_file: Some(String::from("file-valid")),
            suffix: Some(String::from("weather-mini")),
            seed: Some(7),
            metadata: Some(json!({"suite": "fine-tuning"})),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(created.output.status, FineTuningJobStatus::Queued);

    let retrieved = sdk.fine_tuning().jobs().retrieve("ftjob_123").unwrap();
    assert_eq!(retrieved.output.status, FineTuningJobStatus::Running);
    assert_eq!(retrieved.output.training_file, "file-train");

    let listed = sdk
        .fine_tuning()
        .jobs()
        .list(FineTuningJobListParams {
            after: Some(String::from("ftjob_000")),
            limit: Some(2),
            metadata: Some(
                [(String::from("suite"), String::from("fine-tuning"))]
                    .into_iter()
                    .collect(),
            ),
        })
        .unwrap();
    assert_eq!(listed.output.data.len(), 2);
    assert_eq!(listed.output.next_after(), Some("ftjob_124"));

    let cancelled = sdk.fine_tuning().jobs().cancel("ftjob_123").unwrap();
    assert_eq!(cancelled.output.status, FineTuningJobStatus::Cancelled);

    let checkpoints = sdk
        .fine_tuning()
        .jobs()
        .checkpoints()
        .list(
            "ftjob_123",
            FineTuningCheckpointListParams {
                after: Some(String::from("ckpt_000")),
                limit: Some(1),
            },
        )
        .unwrap();
    assert_eq!(checkpoints.output.data[0].id, "ckpt_123");
    assert_eq!(
        checkpoints.output.data[0].fine_tuned_model_checkpoint,
        "ft:gpt-4o-mini:org:weather:checkpoint"
    );
    assert_eq!(checkpoints.output.data[0].metrics.train_loss, Some(0.42));
    assert_eq!(
        checkpoints.output.data[0]
            .metrics
            .extra
            .get("custom_metric"),
        Some(&json!(0.99))
    );

    let created_permissions = sdk
        .fine_tuning()
        .checkpoints()
        .permissions()
        .create(
            "ft:gpt-4o-mini:org:weather:checkpoint",
            FineTuningCheckpointPermissionCreateParams {
                project_ids: vec![String::from("proj_123"), String::from("proj_456")],
            },
        )
        .unwrap();
    assert_eq!(created_permissions.output.data.len(), 2);
    assert_eq!(created_permissions.output.data[0].project_id, "proj_123");

    let listed_permissions = sdk
        .fine_tuning()
        .checkpoints()
        .permissions()
        .list(
            "ft:gpt-4o-mini:org:weather:checkpoint",
            FineTuningCheckpointPermissionListParams {
                after: Some(String::from("perm_000")),
                limit: Some(2),
                order: Some(String::from("descending")),
                project_id: Some(String::from("proj_123")),
            },
        )
        .unwrap();
    assert_eq!(listed_permissions.output.data.len(), 2);
    assert_eq!(listed_permissions.output.next_after(), Some("perm_456"));

    let deleted_permission = sdk
        .fine_tuning()
        .checkpoints()
        .permissions()
        .delete("ft:gpt-4o-mini:org:weather:checkpoint", "perm_123")
        .unwrap();
    assert!(deleted_permission.output.deleted);

    let requests = success_server.captured_requests(8).unwrap();
    assert_eq!(requests[0].path, "/v1/fine_tuning/jobs");
    assert_eq!(requests[1].path, "/v1/fine_tuning/jobs/ftjob_123");
    assert_eq!(
        requests[2].path,
        "/v1/fine_tuning/jobs?after=ftjob_000&limit=2&metadata%5Bsuite%5D=fine-tuning"
    );
    assert_eq!(requests[3].path, "/v1/fine_tuning/jobs/ftjob_123/cancel");
    assert_eq!(
        requests[4].path,
        "/v1/fine_tuning/jobs/ftjob_123/checkpoints?after=ckpt_000&limit=1"
    );
    assert_eq!(
        requests[5].path,
        "/v1/fine_tuning/checkpoints/ft:gpt-4o-mini:org:weather:checkpoint/permissions"
    );
    assert_eq!(
        requests[6].path,
        "/v1/fine_tuning/checkpoints/ft:gpt-4o-mini:org:weather:checkpoint/permissions?after=perm_000&limit=2&order=descending&project_id=proj_123"
    );
    assert_eq!(
        requests[7].path,
        "/v1/fine_tuning/checkpoints/ft:gpt-4o-mini:org:weather:checkpoint/permissions/perm_123"
    );

    let create_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["model"], json!("gpt-4o-mini"));
    assert_eq!(create_body["training_file"], json!("file-train"));
    assert_eq!(create_body["validation_file"], json!("file-valid"));
    assert_eq!(create_body["suffix"], json!("weather-mini"));
    assert_eq!(create_body["seed"], json!(7));
    assert_eq!(create_body["metadata"]["suite"], json!("fine-tuning"));

    let permission_create_body: serde_json::Value =
        serde_json::from_slice(&requests[5].body).unwrap();
    assert_eq!(
        permission_create_body["project_ids"],
        json!(["proj_123", "proj_456"])
    );

    let blank_id = sdk.fine_tuning().jobs().retrieve(" ").unwrap_err();
    assert!(matches!(blank_id.kind, ErrorKind::Validation));

    let denied_server = mock_http::MockHttpServer::spawn(error_response(
        403,
        "Forbidden",
        json!({
            "error": {
                "message": "Admin API key required",
                "type": "invalid_request_error",
                "code": "permission_denied",
                "param": "api_key"
            }
        })
        .to_string(),
    ))
    .unwrap();
    let denied_client = client(&denied_server.url(), "sk-project-key");
    let denied = denied_client
        .fine_tuning()
        .checkpoints()
        .permissions()
        .create(
            "ft:gpt-4o-mini:org:weather:checkpoint",
            FineTuningCheckpointPermissionCreateParams {
                project_ids: vec![String::from("proj_123")],
            },
        )
        .expect_err("non-admin keys should surface a permission error");
    assert_eq!(denied.kind, ErrorKind::Api(ApiErrorKind::PermissionDenied));
    assert_eq!(
        denied.api_error().and_then(|error| error.code.as_deref()),
        Some("permission_denied")
    );
}

fn client(base_url: &str, api_key: &str) -> OpenAI {
    OpenAI::builder()
        .api_key(api_key)
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
        "result_files": ["file_result"],
        "seed": 7,
        "status": status,
        "trained_tokens": null,
        "training_file": "file-train",
        "validation_file": "file-valid",
        "estimated_finish": 1_717_181_717,
        "metadata": {"suite": "fine-tuning"}
    })
    .to_string()
}

fn job_list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&job_payload("ftjob_123", "queued")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&job_payload("ftjob_124", "running")).unwrap()
        ],
        "has_more": true
    })
    .to_string()
}

fn checkpoint_list_payload() -> String {
    json!({
        "object": "list",
        "data": [{
            "id": "ckpt_123",
            "object": "fine_tuning.job.checkpoint",
            "created_at": 1_717_171_900,
            "fine_tuned_model_checkpoint": "ft:gpt-4o-mini:org:weather:checkpoint",
            "fine_tuning_job_id": "ftjob_123",
            "step_number": 4,
            "metrics": {
                "train_loss": 0.42,
                "valid_mean_token_accuracy": 0.91,
                "custom_metric": 0.99
            }
        }],
        "has_more": false
    })
    .to_string()
}

fn permission_create_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {"id": "perm_123", "object": "checkpoint.permission", "created_at": 1_717_171_930, "project_id": "proj_123"},
            {"id": "perm_456", "object": "checkpoint.permission", "created_at": 1_717_171_931, "project_id": "proj_456"}
        ],
        "has_more": false
    })
    .to_string()
}

fn permission_list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {"id": "perm_123", "object": "checkpoint.permission", "created_at": 1_717_171_930, "project_id": "proj_123"},
            {"id": "perm_456", "object": "checkpoint.permission", "created_at": 1_717_171_931, "project_id": "proj_456"}
        ],
        "has_more": true,
        "first_id": "perm_123",
        "last_id": "perm_456"
    })
    .to_string()
}

fn permission_delete_payload() -> String {
    json!({
        "id": "perm_123",
        "object": "checkpoint.permission",
        "deleted": true
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

fn error_response(
    status_code: u16,
    reason: &'static str,
    body: String,
) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        status_code,
        reason,
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
