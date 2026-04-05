use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
};

/// Moderations API family.
#[derive(Clone, Debug)]
pub struct Moderations {
    runtime: Arc<ClientRuntime>,
}

impl Moderations {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a moderation request while preserving per-input result correspondence.
    pub fn create(
        &self,
        params: ModerationCreateParams,
    ) -> Result<ApiResponse<ModerationCreateResponse>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/moderations",
            &params,
            RequestOptions::default(),
        )
    }
}

/// Moderation create parameters accepting text or multimodal inputs.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ModerationCreateParams {
    pub input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed moderations response.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ModerationCreateResponse {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub results: Vec<ModerationResult>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// One moderation result, corresponding to one input item.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ModerationResult {
    pub flagged: bool,
    #[serde(default)]
    pub categories: BTreeMap<String, bool>,
    #[serde(default)]
    pub category_scores: BTreeMap<String, f64>,
    #[serde(default)]
    pub category_applied_input_types: BTreeMap<String, Vec<String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
