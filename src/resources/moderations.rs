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
    #[serde(default)]
    pub flagged: bool,
    #[serde(default)]
    pub categories: ModerationCategories,
    #[serde(default)]
    pub category_scores: ModerationCategoryScores,
    #[serde(default)]
    pub category_applied_input_types: ModerationCategoryAppliedInputTypes,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Category flags for one moderation result.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ModerationCategories {
    #[serde(default)]
    pub harassment: bool,
    #[serde(default, rename = "harassment/threatening")]
    pub harassment_threatening: bool,
    #[serde(default)]
    pub hate: bool,
    #[serde(default, rename = "hate/threatening")]
    pub hate_threatening: bool,
    #[serde(default)]
    pub illicit: Option<bool>,
    #[serde(default, rename = "illicit/violent")]
    pub illicit_violent: Option<bool>,
    #[serde(default, rename = "self-harm")]
    pub self_harm: bool,
    #[serde(default, rename = "self-harm/instructions")]
    pub self_harm_instructions: bool,
    #[serde(default, rename = "self-harm/intent")]
    pub self_harm_intent: bool,
    #[serde(default)]
    pub sexual: bool,
    #[serde(default, rename = "sexual/minors")]
    pub sexual_minors: bool,
    #[serde(default)]
    pub violence: bool,
    #[serde(default, rename = "violence/graphic")]
    pub violence_graphic: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Category scores for one moderation result.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ModerationCategoryScores {
    #[serde(default)]
    pub harassment: f64,
    #[serde(default, rename = "harassment/threatening")]
    pub harassment_threatening: f64,
    #[serde(default)]
    pub hate: f64,
    #[serde(default, rename = "hate/threatening")]
    pub hate_threatening: f64,
    #[serde(default)]
    pub illicit: f64,
    #[serde(default, rename = "illicit/violent")]
    pub illicit_violent: f64,
    #[serde(default, rename = "self-harm")]
    pub self_harm: f64,
    #[serde(default, rename = "self-harm/instructions")]
    pub self_harm_instructions: f64,
    #[serde(default, rename = "self-harm/intent")]
    pub self_harm_intent: f64,
    #[serde(default)]
    pub sexual: f64,
    #[serde(default, rename = "sexual/minors")]
    pub sexual_minors: f64,
    #[serde(default)]
    pub violence: f64,
    #[serde(default, rename = "violence/graphic")]
    pub violence_graphic: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Input modalities used when evaluating each moderation category.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ModerationCategoryAppliedInputTypes {
    #[serde(default)]
    pub harassment: Vec<String>,
    #[serde(default, rename = "harassment/threatening")]
    pub harassment_threatening: Vec<String>,
    #[serde(default)]
    pub hate: Vec<String>,
    #[serde(default, rename = "hate/threatening")]
    pub hate_threatening: Vec<String>,
    #[serde(default)]
    pub illicit: Vec<String>,
    #[serde(default, rename = "illicit/violent")]
    pub illicit_violent: Vec<String>,
    #[serde(default, rename = "self-harm")]
    pub self_harm: Vec<String>,
    #[serde(default, rename = "self-harm/instructions")]
    pub self_harm_instructions: Vec<String>,
    #[serde(default, rename = "self-harm/intent")]
    pub self_harm_intent: Vec<String>,
    #[serde(default)]
    pub sexual: Vec<String>,
    #[serde(default, rename = "sexual/minors")]
    pub sexual_minors: Vec<String>,
    #[serde(default)]
    pub violence: Vec<String>,
    #[serde(default, rename = "violence/graphic")]
    pub violence_graphic: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
