use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    resources::files::{encode_path_id, validate_path_id},
};

/// Top-level evals API family.
#[derive(Clone, Debug)]
pub struct Evals {
    runtime: Arc<ClientRuntime>,
}

impl Evals {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns eval-run operations scoped under the evals family.
    pub fn runs(&self) -> EvalRuns {
        EvalRuns::new(self.runtime.clone())
    }

    /// Creates an evaluation.
    pub fn create(&self, params: EvalCreateParams) -> Result<ApiResponse<Eval>, OpenAIError> {
        self.runtime
            .execute_json_with_body("POST", "/evals", &params, RequestOptions::default())
    }

    /// Retrieves an evaluation by id.
    pub fn retrieve(&self, eval_id: &str) -> Result<ApiResponse<Eval>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/evals/{eval_id}"),
            RequestOptions::default(),
        )
    }

    /// Updates an evaluation's mutable fields.
    pub fn update(
        &self,
        eval_id: &str,
        params: EvalUpdateParams,
    ) -> Result<ApiResponse<Eval>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        self.runtime.execute_json_with_body(
            "POST",
            format!("/evals/{eval_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    /// Lists evaluations with cursor pagination semantics.
    pub fn list(&self, params: EvalListParams) -> Result<ApiResponse<EvalPage>, OpenAIError> {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", order.as_str());
        }
        if let Some(order_by) = params.order_by {
            serializer.append_pair("order_by", order_by.as_str());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            String::from("/evals")
        } else {
            format!("/evals?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Deletes an evaluation.
    pub fn delete(&self, eval_id: &str) -> Result<ApiResponse<EvalDeleteResponse>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        self.runtime.execute_json(
            "DELETE",
            format!("/evals/{eval_id}"),
            RequestOptions::default(),
        )
    }
}

/// Eval runs API.
#[derive(Clone, Debug)]
pub struct EvalRuns {
    runtime: Arc<ClientRuntime>,
}

impl EvalRuns {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns eval run output-item inspection helpers.
    pub fn output_items(&self) -> EvalOutputItems {
        EvalOutputItems::new(self.runtime.clone())
    }

    /// Creates an eval run for the given evaluation.
    pub fn create(
        &self,
        eval_id: &str,
        params: EvalRunCreateParams,
    ) -> Result<ApiResponse<EvalRun>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        self.runtime.execute_json_with_body(
            "POST",
            format!("/evals/{eval_id}/runs"),
            &params,
            RequestOptions::default(),
        )
    }

    /// Retrieves an eval run.
    pub fn retrieve(
        &self,
        eval_id: &str,
        run_id: &str,
    ) -> Result<ApiResponse<EvalRun>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        let run_id = encode_path_id(validate_path_id("run_id", run_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/evals/{eval_id}/runs/{run_id}"),
            RequestOptions::default(),
        )
    }

    /// Lists eval runs for an evaluation.
    pub fn list(
        &self,
        eval_id: &str,
        params: EvalRunListParams,
    ) -> Result<ApiResponse<EvalRunPage>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", order.as_str());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            format!("/evals/{eval_id}/runs")
        } else {
            format!("/evals/{eval_id}/runs?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Deletes an eval run.
    pub fn delete(
        &self,
        eval_id: &str,
        run_id: &str,
    ) -> Result<ApiResponse<EvalRunDeleteResponse>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        let run_id = encode_path_id(validate_path_id("run_id", run_id)?);
        self.runtime.execute_json(
            "DELETE",
            format!("/evals/{eval_id}/runs/{run_id}"),
            RequestOptions::default(),
        )
    }

    /// Cancels an eval run using the documented non-suffixed POST route.
    pub fn cancel(&self, eval_id: &str, run_id: &str) -> Result<ApiResponse<EvalRun>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        let run_id = encode_path_id(validate_path_id("run_id", run_id)?);
        self.runtime.execute_json(
            "POST",
            format!("/evals/{eval_id}/runs/{run_id}"),
            RequestOptions::default(),
        )
    }
}

/// Eval run output-item inspection API.
#[derive(Clone, Debug)]
pub struct EvalOutputItems {
    runtime: Arc<ClientRuntime>,
}

impl EvalOutputItems {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Lists output items for an eval run.
    pub fn list(
        &self,
        eval_id: &str,
        run_id: &str,
        params: EvalOutputItemListParams,
    ) -> Result<ApiResponse<EvalOutputItemPage>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        let run_id = encode_path_id(validate_path_id("run_id", run_id)?);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", order.as_str());
        }
        if let Some(status) = params.status {
            serializer.append_pair("status", status.as_str());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            format!("/evals/{eval_id}/runs/{run_id}/output_items")
        } else {
            format!("/evals/{eval_id}/runs/{run_id}/output_items?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Retrieves a single output item.
    pub fn retrieve(
        &self,
        eval_id: &str,
        run_id: &str,
        output_item_id: &str,
    ) -> Result<ApiResponse<EvalOutputItem>, OpenAIError> {
        let eval_id = encode_path_id(validate_path_id("eval_id", eval_id)?);
        let run_id = encode_path_id(validate_path_id("run_id", run_id)?);
        let output_item_id = encode_path_id(validate_path_id("output_item_id", output_item_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/evals/{eval_id}/runs/{run_id}/output_items/{output_item_id}"),
            RequestOptions::default(),
        )
    }
}

/// Eval creation parameters.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EvalCreateParams {
    pub data_source_config: EvalCreateDataSourceConfig,
    pub testing_criteria: Vec<EvalGrader>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Datasource configuration accepted when creating an eval.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalCreateDataSourceConfig {
    Custom {
        item_schema: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        include_sample_schema: Option<bool>,
    },
    Logs {
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<Value>,
    },
    StoredCompletions {
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<Value>,
    },
}

/// Datasource configuration returned by eval CRUD endpoints.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalDataSourceConfig {
    Custom {
        schema: Value,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    Logs {
        schema: Value,
        #[serde(default)]
        metadata: Option<Value>,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    StoredCompletions {
        schema: Value,
        #[serde(default)]
        metadata: Option<Value>,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
}

/// A message/template item reused by eval graders and runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalMessageTemplate {
    pub role: String,
    pub content: Value,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
}

/// Eval grader definitions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalGrader {
    LabelModel {
        input: Vec<EvalMessageTemplate>,
        labels: Vec<String>,
        model: String,
        name: String,
        passing_labels: Vec<String>,
    },
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
        pass_threshold: f64,
    },
    Python {
        name: String,
        source: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        image_tag: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pass_threshold: Option<f64>,
    },
    ScoreModel {
        input: Vec<EvalMessageTemplate>,
        model: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pass_threshold: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Vec<f64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sampling_params: Option<Value>,
    },
}

/// Update-eval body.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct EvalUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Eval list parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EvalListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<EvalOrderDirection>,
    pub order_by: Option<EvalOrderBy>,
}

/// Shared order direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvalOrderDirection {
    Asc,
    Desc,
}

impl EvalOrderDirection {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Eval list ordering field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvalOrderBy {
    CreatedAt,
    UpdatedAt,
}

impl EvalOrderBy {
    pub fn as_str(&self) -> &str {
        match self {
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
        }
    }
}

/// Eval resource.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Eval {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    pub data_source_config: EvalDataSourceConfig,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub testing_criteria: Vec<EvalGrader>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page of evals.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<Eval>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl EvalPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|item| item.id.as_str())
        } else {
            None
        }
    }
}

/// Eval deletion response.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalDeleteResponse {
    #[serde(default)]
    pub object: String,
    pub deleted: bool,
    pub eval_id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Run creation parameters.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EvalRunCreateParams {
    pub data_source: EvalRunDataSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Datasource families supported by eval runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalRunDataSource {
    Jsonl {
        source: EvalRunSource,
    },
    Completions {
        source: EvalRunSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        input_messages: Option<EvalRunInputMessages>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sampling_params: Option<EvalRunSamplingParams>,
    },
    Responses {
        source: EvalRunSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        input_messages: Option<EvalRunInputMessages>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sampling_params: Option<EvalRunSamplingParams>,
    },
}

/// Row item for inline datasource content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalRunSourceRow {
    pub item: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample: Option<Value>,
}

/// Source selectors used by eval-run datasources.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalRunSource {
    FileContent {
        content: Vec<EvalRunSourceRow>,
    },
    FileId {
        id: String,
    },
    StoredCompletions {
        #[serde(skip_serializing_if = "Option::is_none")]
        created_after: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        created_before: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },
    Responses {
        #[serde(skip_serializing_if = "Option::is_none")]
        created_after: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        created_before: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        instructions_search: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_effort: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        temperature: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tools: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        top_p: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        users: Option<Vec<String>>,
    },
}

/// Input message shaping for model-sampled eval runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalRunInputMessages {
    Template { template: Vec<EvalMessageTemplate> },
    ItemReference { item_reference: String },
}

/// Run sampling configuration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EvalRunSamplingParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<EvalRunTextConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
}

/// Text output sampling configuration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EvalRunTextConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<EvalRunOutputTextFormat>,
}

/// Output text formatting modes for run sampling.
#[derive(Clone, Debug, PartialEq)]
pub enum EvalRunOutputTextFormat {
    Text,
    JsonObject,
    JsonSchema(Value),
    Unknown(Value),
}

impl Serialize for EvalRunOutputTextFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self {
            Self::Text => serde_json::json!({"type": "text"}),
            Self::JsonObject => serde_json::json!({"type": "json_object"}),
            Self::JsonSchema(schema) => {
                serde_json::json!({"type": "json_schema", "json_schema": schema})
            }
            Self::Unknown(raw) => raw.clone(),
        };
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EvalRunOutputTextFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let type_name = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        Ok(match type_name {
            "text" => Self::Text,
            "json_object" => Self::JsonObject,
            "json_schema" => {
                Self::JsonSchema(value.get("json_schema").cloned().unwrap_or(Value::Null))
            }
            _ => Self::Unknown(value),
        })
    }
}

/// Eval run status values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvalRunStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
    Canceled,
    Unknown(String),
}

impl EvalRunStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Queued => "queued",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl Serialize for EvalRunStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl Default for EvalRunStatus {
    fn default() -> Self {
        Self::Unknown(String::from("unknown"))
    }
}

impl<'de> Deserialize<'de> for EvalRunStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "queued" => Self::Queued,
            "in_progress" | "running" => Self::InProgress,
            "completed" | "succeeded" => Self::Completed,
            "failed" => Self::Failed,
            "canceled" | "cancelled" => Self::Canceled,
            _ => Self::Unknown(value),
        })
    }
}

/// Eval API error payload surfaced by runs and output items.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalApiError {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Per-model usage stats for an eval run.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalRunPerModelUsage {
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub invocation_count: u64,
    #[serde(default)]
    pub model_name: String,
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Per-testing-criteria result summary for an eval run.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalRunTestingCriteriaResult {
    #[serde(default)]
    pub failed: u64,
    #[serde(default)]
    pub passed: u64,
    #[serde(default)]
    pub testing_criteria: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Aggregate counts for an eval run.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalRunResultCounts {
    #[serde(default)]
    pub errored: u64,
    #[serde(default)]
    pub failed: u64,
    #[serde(default)]
    pub passed: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Eval run resource.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct EvalRun {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    pub data_source: EvalRunDataSource,
    #[serde(default)]
    pub error: Option<EvalApiError>,
    #[serde(default)]
    pub eval_id: String,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub per_model_usage: Vec<EvalRunPerModelUsage>,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub per_testing_criteria_results: Vec<EvalRunTestingCriteriaResult>,
    #[serde(default)]
    pub report_url: Option<String>,
    #[serde(default)]
    pub result_counts: Option<EvalRunResultCounts>,
    #[serde(default)]
    pub status: EvalRunStatus,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Eval-run list params.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EvalRunListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<EvalOrderDirection>,
}

/// Cursor page of eval runs.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalRunPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<EvalRun>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl EvalRunPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|item| item.id.as_str())
        } else {
            None
        }
    }
}

/// Eval-run delete response.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalRunDeleteResponse {
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn deserialize_null_default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<Vec<T>>::deserialize(deserializer).map(Option::unwrap_or_default)
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}

/// Output item list params.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EvalOutputItemListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<EvalOrderDirection>,
    pub status: Option<EvalOutputItemStatus>,
}

/// Output item status used for filtering and inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvalOutputItemStatus {
    Pass,
    Fail,
    Unknown(String),
}

impl EvalOutputItemStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl Default for EvalOutputItemStatus {
    fn default() -> Self {
        Self::Unknown(String::from("unknown"))
    }
}

impl<'de> Deserialize<'de> for EvalOutputItemStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "pass" | "passed" => Self::Pass,
            "fail" | "failed" => Self::Fail,
            _ => Self::Unknown(value),
        })
    }
}

/// Single grader result attached to an output item.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalOutputItemResult {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub sample: Option<Value>,
    #[serde(rename = "type", default)]
    pub type_field: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl EvalOutputItemResult {
    pub fn grader_type(&self) -> Option<&str> {
        self.type_field
            .as_deref()
            .or_else(|| self.extra.get("type").and_then(Value::as_str))
    }
}

/// Input/output message stored on an output-item sample.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalOutputItemMessage {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Usage details attached to an output-item sample.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalOutputItemUsage {
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Sample provenance attached to an output item.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalOutputItemSample {
    #[serde(default)]
    pub error: Option<EvalApiError>,
    #[serde(default)]
    pub finish_reason: String,
    #[serde(default)]
    pub input: Vec<EvalOutputItemMessage>,
    #[serde(default)]
    pub max_completion_tokens: u64,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub output: Vec<EvalOutputItemMessage>,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub temperature: f64,
    #[serde(default)]
    pub top_p: f64,
    #[serde(default)]
    pub usage: EvalOutputItemUsage,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Eval output item resource.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalOutputItem {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub datasource_item: Value,
    #[serde(default)]
    pub datasource_item_id: u64,
    #[serde(default)]
    pub eval_id: String,
    #[serde(default)]
    pub results: Vec<EvalOutputItemResult>,
    #[serde(default)]
    pub run_id: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub sample: EvalOutputItemSample,
    #[serde(default)]
    pub status: EvalOutputItemStatus,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page of eval output items.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EvalOutputItemPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<EvalOutputItem>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl EvalOutputItemPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|item| item.id.as_str())
        } else {
            None
        }
    }
}
