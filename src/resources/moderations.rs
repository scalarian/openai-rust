use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
};

/// Moderation input modality and input-item discriminator literals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModerationInputType {
    Text,
    Image,
    ImageUrl,
    Unknown(String),
}

impl ModerationInputType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::ImageUrl => "image_url",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl From<&str> for ModerationInputType {
    fn from(value: &str) -> Self {
        match value {
            "text" => Self::Text,
            "image" => Self::Image,
            "image_url" => Self::ImageUrl,
            _ => Self::Unknown(value.to_string()),
        }
    }
}

impl From<String> for ModerationInputType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "text" => Self::Text,
            "image" => Self::Image,
            "image_url" => Self::ImageUrl,
            _ => Self::Unknown(value),
        }
    }
}

impl AsRef<str> for ModerationInputType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for ModerationInputType {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl std::fmt::Display for ModerationInputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<&str> for ModerationInputType {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<ModerationInputType> for &str {
    fn eq(&self, other: &ModerationInputType) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<String> for ModerationInputType {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<ModerationInputType> for String {
    fn eq(&self, other: &ModerationInputType) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Serialize for ModerationInputType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ModerationInputType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from(value))
    }
}

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
    pub input: ModerationInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Text or multimodal moderation input.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ModerationInput {
    Text(String),
    Texts(Vec<String>),
    Items(Vec<ModerationInputItem>),
}

impl ModerationInput {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn texts(texts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::Texts(texts.into_iter().map(Into::into).collect())
    }

    pub fn items(items: Vec<ModerationInputItem>) -> Self {
        Self::Items(items)
    }
}

impl Default for ModerationInput {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl From<String> for ModerationInput {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for ModerationInput {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<String>> for ModerationInput {
    fn from(value: Vec<String>) -> Self {
        Self::Texts(value)
    }
}

impl From<Vec<&str>> for ModerationInput {
    fn from(value: Vec<&str>) -> Self {
        Self::Texts(value.into_iter().map(str::to_string).collect())
    }
}

impl From<Vec<ModerationInputItem>> for ModerationInput {
    fn from(value: Vec<ModerationInputItem>) -> Self {
        Self::Items(value)
    }
}

/// One multimodal moderation input item.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ModerationInputItem {
    Text(ModerationTextInput),
    ImageUrl(ModerationImageUrlInput),
}

impl ModerationInputItem {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(ModerationTextInput::new(text))
    }

    pub fn image_url(url: impl Into<String>) -> Self {
        Self::ImageUrl(ModerationImageUrlInput::url(url))
    }
}

/// Text input item for multimodal moderation requests.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModerationTextInput {
    #[serde(rename = "type")]
    pub input_type: ModerationInputType,
    pub text: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ModerationTextInput {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            input_type: ModerationInputType::Text,
            text: text.into(),
            extra: BTreeMap::new(),
        }
    }
}

/// Image URL input item for multimodal moderation requests.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModerationImageUrlInput {
    #[serde(rename = "type")]
    pub input_type: ModerationInputType,
    pub image_url: ModerationImageUrl,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ModerationImageUrlInput {
    pub fn url(url: impl Into<String>) -> Self {
        Self {
            input_type: ModerationInputType::ImageUrl,
            image_url: ModerationImageUrl {
                url: url.into(),
                extra: BTreeMap::new(),
            },
            extra: BTreeMap::new(),
        }
    }
}

/// Image URL wrapper for moderation image inputs.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModerationImageUrl {
    pub url: String,
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
    pub harassment: Vec<ModerationInputType>,
    #[serde(default, rename = "harassment/threatening")]
    pub harassment_threatening: Vec<ModerationInputType>,
    #[serde(default)]
    pub hate: Vec<ModerationInputType>,
    #[serde(default, rename = "hate/threatening")]
    pub hate_threatening: Vec<ModerationInputType>,
    #[serde(default)]
    pub illicit: Vec<ModerationInputType>,
    #[serde(default, rename = "illicit/violent")]
    pub illicit_violent: Vec<ModerationInputType>,
    #[serde(default, rename = "self-harm")]
    pub self_harm: Vec<ModerationInputType>,
    #[serde(default, rename = "self-harm/instructions")]
    pub self_harm_instructions: Vec<ModerationInputType>,
    #[serde(default, rename = "self-harm/intent")]
    pub self_harm_intent: Vec<ModerationInputType>,
    #[serde(default)]
    pub sexual: Vec<ModerationInputType>,
    #[serde(default, rename = "sexual/minors")]
    pub sexual_minors: Vec<ModerationInputType>,
    #[serde(default)]
    pub violence: Vec<ModerationInputType>,
    #[serde(default, rename = "violence/graphic")]
    pub violence_graphic: Vec<ModerationInputType>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
