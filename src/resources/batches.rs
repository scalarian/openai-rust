use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    resources::files::{encode_path_id, validate_path_id},
};

/// Top-level batches API family.
#[derive(Clone, Debug)]
pub struct Batches {
    runtime: Arc<ClientRuntime>,
}

impl Batches {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates and executes a batch from an uploaded input file.
    pub fn create(&self, params: BatchCreateParams) -> Result<ApiResponse<Batch>, OpenAIError> {
        self.runtime
            .execute_json_with_body("POST", "/batches", &params, RequestOptions::default())
    }

    /// Retrieves one batch by id.
    pub fn retrieve(&self, batch_id: &str) -> Result<ApiResponse<Batch>, OpenAIError> {
        let batch_id = encode_path_id(validate_path_id("batch_id", batch_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/batches/{batch_id}"),
            RequestOptions::default(),
        )
    }

    /// Lists organization batches with cursor pagination controls.
    pub fn list(&self, params: BatchListParams) -> Result<ApiResponse<BatchPage>, OpenAIError> {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            String::from("/batches")
        } else {
            format!("/batches?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Cancels an in-progress batch and returns the current lifecycle resource.
    pub fn cancel(&self, batch_id: &str) -> Result<ApiResponse<Batch>, OpenAIError> {
        let batch_id = encode_path_id(validate_path_id("batch_id", batch_id)?);
        self.runtime.execute_json(
            "POST",
            format!("/batches/{batch_id}/cancel"),
            RequestOptions::default(),
        )
    }
}

/// Create-batch request body.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BatchCreateParams {
    pub completion_window: BatchCompletionWindow,
    pub endpoint: BatchEndpoint,
    pub input_file_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_expires_after: Option<BatchOutputExpiresAfter>,
}

/// Supported batch completion windows.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BatchCompletionWindow {
    #[serde(rename = "24h")]
    Hours24,
}

/// Supported batch endpoints.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BatchEndpoint {
    #[serde(rename = "/v1/responses")]
    Responses,
    #[serde(rename = "/v1/chat/completions")]
    ChatCompletions,
    #[serde(rename = "/v1/embeddings")]
    Embeddings,
    #[serde(rename = "/v1/completions")]
    Completions,
    #[serde(rename = "/v1/moderations")]
    Moderations,
    #[serde(rename = "/v1/images/generations")]
    ImagesGenerations,
    #[serde(rename = "/v1/images/edits")]
    ImagesEdits,
    #[serde(rename = "/v1/videos")]
    Videos,
}

/// Output/error file expiration policy for a batch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchOutputExpiresAfter {
    pub anchor: String,
    pub seconds: u64,
}

/// List-batches query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BatchListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
}

/// Typed batch resource.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Batch {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub completion_window: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub input_file_id: String,
    pub status: BatchStatus,
    #[serde(default)]
    pub cancelled_at: Option<u64>,
    #[serde(default)]
    pub cancelling_at: Option<u64>,
    #[serde(default)]
    pub completed_at: Option<u64>,
    #[serde(default)]
    pub error_file_id: Option<String>,
    #[serde(default)]
    pub errors: Option<BatchErrors>,
    #[serde(default)]
    pub expired_at: Option<u64>,
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub failed_at: Option<u64>,
    #[serde(default)]
    pub finalizing_at: Option<u64>,
    #[serde(default)]
    pub in_progress_at: Option<u64>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub output_file_id: Option<String>,
    #[serde(default)]
    pub request_counts: Option<BatchRequestCounts>,
    #[serde(default)]
    pub usage: Option<BatchUsage>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Batch lifecycle status.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    Validating,
    Failed,
    InProgress,
    Finalizing,
    Completed,
    Expired,
    Cancelling,
    Cancelled,
    #[default]
    #[serde(other)]
    Unknown,
}

/// Error list returned on some batch failures.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct BatchErrors {
    #[serde(default)]
    pub data: Vec<BatchError>,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// One batch error entry.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct BatchError {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub line: Option<u64>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub param: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Request-count summary for a batch.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct BatchRequestCounts {
    #[serde(default)]
    pub completed: u64,
    #[serde(default)]
    pub failed: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Optional usage details attached to newer batches.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct BatchUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub input_tokens_details: Option<BatchInputTokensDetails>,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub output_tokens_details: Option<BatchOutputTokensDetails>,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Input-token usage breakdown.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct BatchInputTokensDetails {
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Output-token usage breakdown.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct BatchOutputTokensDetails {
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page for listing batches.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct BatchPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<Batch>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl BatchPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|batch| batch.id.as_str())
        } else {
            None
        }
    }
}
