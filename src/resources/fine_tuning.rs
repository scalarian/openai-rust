use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize, Serializer, de::Error as _};
use serde_json::{Map, Value};

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    resources::files::{encode_path_id, validate_path_id},
};

/// Top-level fine-tuning API family.
#[derive(Clone, Debug)]
pub struct FineTuning {
    runtime: Arc<ClientRuntime>,
}

impl FineTuning {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns the fine-tuning jobs surface.
    pub fn jobs(&self) -> FineTuningJobs {
        FineTuningJobs::new(self.runtime.clone())
    }

    /// Returns the top-level checkpoint helpers.
    pub fn checkpoints(&self) -> FineTuningCheckpoints {
        FineTuningCheckpoints::new(self.runtime.clone())
    }

    /// Returns the alpha fine-tuning surface.
    pub fn alpha(&self) -> FineTuningAlpha {
        FineTuningAlpha::new(self.runtime.clone())
    }
}

/// Fine-tuning jobs API.
#[derive(Clone, Debug)]
pub struct FineTuningJobs {
    runtime: Arc<ClientRuntime>,
}

impl FineTuningJobs {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns nested checkpoint-listing helpers scoped to jobs.
    pub fn checkpoints(&self) -> FineTuningJobCheckpoints {
        FineTuningJobCheckpoints::new(self.runtime.clone())
    }

    /// Creates a fine-tuning job.
    pub fn create(
        &self,
        params: FineTuningJobCreateParams,
    ) -> Result<ApiResponse<FineTuningJob>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/fine_tuning/jobs",
            &params,
            RequestOptions::default(),
        )
    }

    /// Retrieves a fine-tuning job by id.
    pub fn retrieve(&self, job_id: &str) -> Result<ApiResponse<FineTuningJob>, OpenAIError> {
        let job_id = encode_path_id(validate_path_id("job_id", job_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/fine_tuning/jobs/{job_id}"),
            RequestOptions::default(),
        )
    }

    /// Lists fine-tuning jobs with cursor pagination and metadata filters.
    pub fn list(
        &self,
        params: FineTuningJobListParams,
    ) -> Result<ApiResponse<FineTuningJobPage>, OpenAIError> {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(metadata) = params.metadata {
            for (key, value) in metadata {
                serializer.append_pair(&format!("metadata[{key}]"), &value);
            }
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            String::from("/fine_tuning/jobs")
        } else {
            format!("/fine_tuning/jobs?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Cancels a fine-tuning job.
    pub fn cancel(&self, job_id: &str) -> Result<ApiResponse<FineTuningJob>, OpenAIError> {
        let job_id = encode_path_id(validate_path_id("job_id", job_id)?);
        self.runtime.execute_json(
            "POST",
            format!("/fine_tuning/jobs/{job_id}/cancel"),
            RequestOptions::default(),
        )
    }

    /// Lists fine-tuning job events.
    pub fn list_events(
        &self,
        job_id: &str,
        params: FineTuningJobEventListParams,
    ) -> Result<ApiResponse<FineTuningJobEventPage>, OpenAIError> {
        let job_id = encode_path_id(validate_path_id("job_id", job_id)?);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            format!("/fine_tuning/jobs/{job_id}/events")
        } else {
            format!("/fine_tuning/jobs/{job_id}/events?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Pauses a fine-tuning job.
    pub fn pause(&self, job_id: &str) -> Result<ApiResponse<FineTuningJob>, OpenAIError> {
        let job_id = encode_path_id(validate_path_id("job_id", job_id)?);
        self.runtime.execute_json(
            "POST",
            format!("/fine_tuning/jobs/{job_id}/pause"),
            RequestOptions::default(),
        )
    }

    /// Resumes a paused fine-tuning job.
    pub fn resume(&self, job_id: &str) -> Result<ApiResponse<FineTuningJob>, OpenAIError> {
        let job_id = encode_path_id(validate_path_id("job_id", job_id)?);
        self.runtime.execute_json(
            "POST",
            format!("/fine_tuning/jobs/{job_id}/resume"),
            RequestOptions::default(),
        )
    }
}

/// Fine-tuning job checkpoints scoped to a job id.
#[derive(Clone, Debug)]
pub struct FineTuningJobCheckpoints {
    runtime: Arc<ClientRuntime>,
}

impl FineTuningJobCheckpoints {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Lists checkpoints for a fine-tuning job.
    pub fn list(
        &self,
        job_id: &str,
        params: FineTuningCheckpointListParams,
    ) -> Result<ApiResponse<FineTuningCheckpointPage>, OpenAIError> {
        let job_id = encode_path_id(validate_path_id("job_id", job_id)?);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            format!("/fine_tuning/jobs/{job_id}/checkpoints")
        } else {
            format!("/fine_tuning/jobs/{job_id}/checkpoints?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }
}

/// Top-level checkpoint helpers.
#[derive(Clone, Debug)]
pub struct FineTuningCheckpoints {
    runtime: Arc<ClientRuntime>,
}

impl FineTuningCheckpoints {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns checkpoint permission operations.
    pub fn permissions(&self) -> FineTuningCheckpointPermissions {
        FineTuningCheckpointPermissions::new(self.runtime.clone())
    }
}

/// Top-level alpha fine-tuning helpers.
#[derive(Clone, Debug)]
pub struct FineTuningAlpha {
    runtime: Arc<ClientRuntime>,
}

impl FineTuningAlpha {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns alpha grader operations.
    pub fn graders(&self) -> FineTuningGraders {
        FineTuningGraders::new(self.runtime.clone())
    }
}

/// Checkpoint permission API.
#[derive(Clone, Debug)]
pub struct FineTuningCheckpointPermissions {
    runtime: Arc<ClientRuntime>,
}

impl FineTuningCheckpointPermissions {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates checkpoint permissions for one or more projects.
    pub fn create(
        &self,
        checkpoint_id: &str,
        params: FineTuningCheckpointPermissionCreateParams,
    ) -> Result<ApiResponse<FineTuningCheckpointPermissionPage>, OpenAIError> {
        let checkpoint_id = encode_path_id(validate_path_id("checkpoint_id", checkpoint_id)?);
        self.runtime.execute_json_with_body(
            "POST",
            format!("/fine_tuning/checkpoints/{checkpoint_id}/permissions"),
            &params,
            RequestOptions::default(),
        )
    }

    /// Lists permissions for a fine-tuned checkpoint.
    pub fn list(
        &self,
        checkpoint_id: &str,
        params: FineTuningCheckpointPermissionListParams,
    ) -> Result<ApiResponse<FineTuningCheckpointPermissionPage>, OpenAIError> {
        let checkpoint_id = encode_path_id(validate_path_id("checkpoint_id", checkpoint_id)?);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", &order);
        }
        if let Some(project_id) = params.project_id {
            serializer.append_pair("project_id", &project_id);
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            format!("/fine_tuning/checkpoints/{checkpoint_id}/permissions")
        } else {
            format!("/fine_tuning/checkpoints/{checkpoint_id}/permissions?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Deletes a single checkpoint permission.
    pub fn delete(
        &self,
        checkpoint_id: &str,
        permission_id: &str,
    ) -> Result<ApiResponse<FineTuningCheckpointPermissionDeleteResponse>, OpenAIError> {
        let checkpoint_id = encode_path_id(validate_path_id("checkpoint_id", checkpoint_id)?);
        let permission_id = encode_path_id(validate_path_id("permission_id", permission_id)?);
        self.runtime.execute_json(
            "DELETE",
            format!("/fine_tuning/checkpoints/{checkpoint_id}/permissions/{permission_id}"),
            RequestOptions::default(),
        )
    }
}

/// Alpha graders API.
#[derive(Clone, Debug)]
pub struct FineTuningGraders {
    runtime: Arc<ClientRuntime>,
}

impl FineTuningGraders {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Validates a grader configuration.
    pub fn validate(
        &self,
        params: FineTuningGraderValidateParams,
    ) -> Result<ApiResponse<FineTuningGraderValidateResponse>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/fine_tuning/alpha/graders/validate",
            &params,
            RequestOptions::default(),
        )
    }

    /// Runs a grader against a tiny sample.
    pub fn run(
        &self,
        params: FineTuningGraderRunParams,
    ) -> Result<ApiResponse<FineTuningGraderRunResponse>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/fine_tuning/alpha/graders/run",
            &params,
            RequestOptions::default(),
        )
    }
}

/// Create-job request parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FineTuningJobCreateParams {
    pub model: String,
    pub training_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperparameters: Option<FineTuningSupervisedHyperparameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<FineTuningMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_file: Option<String>,
}

/// Supports `auto` or a numeric override.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum AutoOrNumber {
    #[default]
    Auto,
    Number(u64),
}

impl Serialize for AutoOrNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Number(value) => serializer.serialize_u64(*value),
        }
    }
}

impl<'de> Deserialize<'de> for AutoOrNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(value) if value == "auto" => Ok(Self::Auto),
            Value::Number(value) => value
                .as_u64()
                .map(Self::Number)
                .ok_or_else(|| D::Error::custom("expected unsigned integer for hyperparameter")),
            other => Err(D::Error::custom(format!(
                "expected `auto` or number for hyperparameter, got {other}"
            ))),
        }
    }
}

impl Serialize for FineTuningMethod {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = Map::new();
        match self {
            Self::Supervised(config) => {
                object.insert(
                    String::from("type"),
                    Value::String(String::from("supervised")),
                );
                object.insert(
                    String::from("supervised"),
                    serde_json::to_value(config.supervised.clone())
                        .map_err(serde::ser::Error::custom)?,
                );
            }
            Self::Dpo(config) => {
                object.insert(String::from("type"), Value::String(String::from("dpo")));
                object.insert(
                    String::from("dpo"),
                    serde_json::to_value(config.dpo.clone()).map_err(serde::ser::Error::custom)?,
                );
            }
            Self::Reinforcement(config) => {
                object.insert(
                    String::from("type"),
                    Value::String(String::from("reinforcement")),
                );
                object.insert(
                    String::from("reinforcement"),
                    serde_json::to_value(config.reinforcement.clone())
                        .map_err(serde::ser::Error::custom)?,
                );
            }
        }
        Value::Object(object).serialize(serializer)
    }
}

/// Fine-tuning method variants.
#[derive(Clone, Debug, PartialEq)]
pub enum FineTuningMethod {
    Supervised(FineTuningMethodConfig),
    Dpo(FineTuningMethodConfig),
    Reinforcement(FineTuningMethodConfig),
}

/// Shared method-config envelope.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FineTuningMethodConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supervised: Option<FineTuningSupervisedMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpo: Option<FineTuningDpoMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reinforcement: Option<FineTuningReinforcementMethod>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FineTuningSupervisedMethod {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperparameters: Option<FineTuningSupervisedHyperparameters>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FineTuningDpoMethod {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperparameters: Option<FineTuningDpoHyperparameters>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FineTuningReinforcementMethod {
    pub grader: FineTuningGrader,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperparameters: Option<FineTuningReinforcementHyperparameters>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FineTuningSupervisedHyperparameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub learning_rate_multiplier: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_epochs: Option<AutoOrNumber>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FineTuningDpoHyperparameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beta: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub learning_rate_multiplier: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_epochs: Option<AutoOrNumber>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FineTuningReinforcementHyperparameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_multiplier: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_interval: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_samples: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub learning_rate_multiplier: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_epochs: Option<AutoOrNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

/// Job-list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FineTuningJobListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub metadata: Option<BTreeMap<String, String>>,
}

/// Job-events query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FineTuningJobEventListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
}

/// Checkpoint-list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FineTuningCheckpointListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
}

/// Create-checkpoint-permission request body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FineTuningCheckpointPermissionCreateParams {
    pub project_ids: Vec<String>,
}

/// Checkpoint-permission list parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FineTuningCheckpointPermissionListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<String>,
    pub project_id: Option<String>,
}

/// Fine-tuning job resource.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningJob {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub error: Option<FineTuningJobError>,
    #[serde(default)]
    pub fine_tuned_model: Option<String>,
    #[serde(default)]
    pub finished_at: Option<u64>,
    #[serde(default)]
    pub hyperparameters: Option<FineTuningSupervisedHyperparameters>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub organization_id: String,
    #[serde(default)]
    pub result_files: Vec<String>,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub status: FineTuningJobStatus,
    #[serde(default)]
    pub trained_tokens: Option<u64>,
    #[serde(default)]
    pub training_file: String,
    #[serde(default)]
    pub validation_file: Option<String>,
    #[serde(default)]
    pub estimated_finish: Option<u64>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub method: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Fine-tuning job status preserving known and additive values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FineTuningJobStatus {
    ValidatingFiles,
    Queued,
    Running,
    Paused,
    Succeeded,
    Failed,
    Cancelled,
    Unknown(String),
}

impl FineTuningJobStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::ValidatingFiles => "validating_files",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl Default for FineTuningJobStatus {
    fn default() -> Self {
        Self::Unknown(String::from("unknown"))
    }
}

impl<'de> Deserialize<'de> for FineTuningJobStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "validating_files" => Self::ValidatingFiles,
            "queued" => Self::Queued,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown(value),
        })
    }
}

/// Fine-tuning job error payload.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningJobError {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub param: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page of fine-tuning jobs.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningJobPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<FineTuningJob>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FineTuningJobPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|job| job.id.as_str())
        } else {
            None
        }
    }
}

/// Fine-tuning job event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningJobEvent {
    pub id: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub level: FineTuningJobEventLevel,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Option<Value>,
    #[serde(rename = "type", default)]
    pub event_type: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FineTuningJobEvent {
    pub fn event_type(&self) -> Option<&str> {
        self.event_type
            .as_deref()
            .or_else(|| self.extra.get("type").and_then(Value::as_str))
    }
}

/// Fine-tuning job event level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FineTuningJobEventLevel {
    Info,
    Warn,
    Error,
    Unknown(String),
}

impl Default for FineTuningJobEventLevel {
    fn default() -> Self {
        Self::Unknown(String::from("unknown"))
    }
}

impl<'de> Deserialize<'de> for FineTuningJobEventLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "info" => Self::Info,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Unknown(value),
        })
    }
}

/// Cursor page of fine-tuning events.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningJobEventPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<FineTuningJobEvent>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Fine-tuning job checkpoint resource.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningCheckpoint {
    pub id: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub fine_tuned_model_checkpoint: String,
    #[serde(default)]
    pub fine_tuning_job_id: String,
    #[serde(default)]
    pub metrics: FineTuningCheckpointMetrics,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub step_number: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Checkpoint metrics preserving additive keys.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningCheckpointMetrics {
    #[serde(default)]
    pub full_valid_loss: Option<f64>,
    #[serde(default)]
    pub full_valid_mean_token_accuracy: Option<f64>,
    #[serde(default)]
    pub step: Option<f64>,
    #[serde(default)]
    pub train_loss: Option<f64>,
    #[serde(default)]
    pub train_mean_token_accuracy: Option<f64>,
    #[serde(default)]
    pub valid_loss: Option<f64>,
    #[serde(default)]
    pub valid_mean_token_accuracy: Option<f64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page of checkpoints.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningCheckpointPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<FineTuningCheckpoint>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FineTuningCheckpointPage {
    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|checkpoint| checkpoint.id.as_str())
        } else {
            None
        }
    }
}

/// Checkpoint permission item.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningCheckpointPermission {
    pub id: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Permission page used by create/list.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningCheckpointPermissionPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<FineTuningCheckpointPermission>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FineTuningCheckpointPermissionPage {
    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|permission| permission.id.as_str())
        } else {
            None
        }
    }
}

/// Delete-permission response.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningCheckpointPermissionDeleteResponse {
    pub id: String,
    #[serde(default)]
    pub object: String,
    pub deleted: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Fine-tuning grader definitions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FineTuningGrader {
    StringCheck {
        input: String,
        name: String,
        operation: String,
        reference: String,
    },
    TextSimilarity {
        evaluation_metric: String,
        input: String,
        name: String,
        reference: String,
    },
    Python {
        name: String,
        source: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        image_tag: Option<String>,
    },
    ScoreModel {
        input: Vec<FineTuningGraderMessage>,
        model: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Vec<f64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sampling_params: Option<FineTuningGraderSamplingParams>,
    },
    Multi {
        calculate_output: String,
        graders: Box<FineTuningGrader>,
        name: String,
    },
}

/// Grader input message for model-based graders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FineTuningGraderMessage {
    pub role: String,
    pub content: Value,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
}

/// Sampling params for score-model graders.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FineTuningGraderSamplingParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completions_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
}

/// Validate-grader request body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FineTuningGraderValidateParams {
    pub grader: FineTuningGrader,
}

/// Run-grader request body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FineTuningGraderRunParams {
    pub grader: FineTuningGrader,
    pub model_sample: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<Value>,
}

/// Validate-grader response.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningGraderValidateResponse {
    #[serde(default)]
    pub grader: Option<FineTuningGrader>,
}

/// Run-grader response.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningGraderRunResponse {
    #[serde(default)]
    pub metadata: FineTuningGraderRunMetadata,
    #[serde(default)]
    pub model_grader_token_usage_per_model: BTreeMap<String, Value>,
    pub reward: f64,
    #[serde(default)]
    pub sub_rewards: BTreeMap<String, Value>,
}

/// Run-grader metadata.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningGraderRunMetadata {
    #[serde(default)]
    pub errors: FineTuningGraderRunErrors,
    #[serde(default)]
    pub execution_time: f64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sampled_model_name: Option<String>,
    #[serde(default)]
    pub scores: BTreeMap<String, Value>,
    #[serde(default)]
    pub token_usage: Option<u64>,
    #[serde(rename = "type", default)]
    pub type_field: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FineTuningGraderRunMetadata {
    pub fn grader_type(&self) -> &str {
        if self.type_field.is_empty() {
            self.extra
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
        } else {
            self.type_field.as_str()
        }
    }
}

/// Structured grader run error flags.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FineTuningGraderRunErrors {
    #[serde(default)]
    pub formula_parse_error: bool,
    #[serde(default)]
    pub invalid_variable_error: bool,
    #[serde(default)]
    pub model_grader_parse_error: bool,
    #[serde(default)]
    pub model_grader_refusal_error: bool,
    #[serde(default)]
    pub model_grader_server_error: bool,
    #[serde(default)]
    pub model_grader_server_error_details: Option<String>,
    #[serde(default)]
    pub other_error: bool,
    #[serde(default)]
    pub python_grader_runtime_error: bool,
    #[serde(default)]
    pub python_grader_runtime_error_details: Option<String>,
    #[serde(default)]
    pub python_grader_server_error: bool,
    #[serde(default)]
    pub python_grader_server_error_type: Option<String>,
    #[serde(default)]
    pub sample_parse_error: bool,
    #[serde(default)]
    pub truncated_observation_error: bool,
    #[serde(default)]
    pub unresponsive_reward_error: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
