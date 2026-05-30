use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use crate::{OpenAIError, error::ErrorKind};

macro_rules! realtime_string_literal_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident => $literal:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant,)+
            Unknown(String),
        }

        impl $name {
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $literal,)+
                    Self::Unknown(value) => value.as_str(),
                }
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::Unknown(String::new())
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                match value {
                    $($literal => Self::$variant,)+
                    _ => Self::Unknown(value.to_string()),
                }
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                match value.as_str() {
                    $($literal => Self::$variant,)+
                    _ => Self::Unknown(value),
                }
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }

        impl PartialEq<$name> for &str {
            fn eq(&self, other: &$name) -> bool {
                *self == other.as_str()
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Ok(Self::from(value))
            }
        }
    };
}

/// Realtime session kind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RealtimeSessionType {
    #[default]
    Realtime,
    Transcription,
}

/// Realtime output modality.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RealtimeOutputModality {
    Text,
    Audio,
}

/// Realtime max-output token limit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealtimeMaxOutputTokens {
    Tokens(u64),
    Inf,
    Unknown(String),
}

impl From<u64> for RealtimeMaxOutputTokens {
    fn from(value: u64) -> Self {
        Self::Tokens(value)
    }
}

impl From<u32> for RealtimeMaxOutputTokens {
    fn from(value: u32) -> Self {
        Self::Tokens(u64::from(value))
    }
}

impl From<usize> for RealtimeMaxOutputTokens {
    fn from(value: usize) -> Self {
        Self::Tokens(value as u64)
    }
}

impl Serialize for RealtimeMaxOutputTokens {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Tokens(tokens) => serializer.serialize_u64(*tokens),
            Self::Inf => serializer.serialize_str("inf"),
            Self::Unknown(value) => serializer.serialize_str(value),
        }
    }
}

impl<'de> Deserialize<'de> for RealtimeMaxOutputTokens {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::Number(number) => number
                .as_u64()
                .map(Self::Tokens)
                .ok_or_else(|| serde::de::Error::custom("max_output_tokens must be unsigned")),
            Value::String(value) if value == "inf" => Ok(Self::Inf),
            Value::String(value) => Ok(Self::Unknown(value)),
            _ => Err(serde::de::Error::custom(
                "max_output_tokens must be an integer or string",
            )),
        }
    }
}

/// Realtime conversation truncation strategy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealtimeTruncation {
    Auto,
    Disabled,
    RetentionRatio(Box<RealtimeTruncationRetentionRatio>),
    UnknownString(String),
    UnknownObject(Box<Value>),
}

impl From<RealtimeTruncationRetentionRatio> for RealtimeTruncation {
    fn from(value: RealtimeTruncationRetentionRatio) -> Self {
        Self::RetentionRatio(Box::new(value))
    }
}

impl Serialize for RealtimeTruncation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Disabled => serializer.serialize_str("disabled"),
            Self::RetentionRatio(value) => value.serialize(serializer),
            Self::UnknownString(value) => serializer.serialize_str(value),
            Self::UnknownObject(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for RealtimeTruncation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(value) if value == "auto" => Ok(Self::Auto),
            Value::String(value) if value == "disabled" => Ok(Self::Disabled),
            Value::String(value) => Ok(Self::UnknownString(value)),
            Value::Object(_) => match serde_json::from_value(value.clone()) {
                Ok(value) => Ok(Self::RetentionRatio(Box::new(value))),
                Err(_) => Ok(Self::UnknownObject(Box::new(value))),
            },
            _ => Err(serde::de::Error::custom(
                "truncation must be a string or object",
            )),
        }
    }
}

/// Retain a fraction of Realtime conversation tokens during truncation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeTruncationRetentionRatio {
    pub retention_ratio: Number,
    #[serde(rename = "type")]
    pub truncation_type: RealtimeTruncationRetentionRatioType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_limits: Option<RealtimeTruncationTokenLimits>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl RealtimeTruncationRetentionRatio {
    pub fn new(retention_ratio: Number) -> Self {
        Self {
            retention_ratio,
            truncation_type: RealtimeTruncationRetentionRatioType::RetentionRatio,
            token_limits: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn from_f64(retention_ratio: f64) -> Option<Self> {
        Number::from_f64(retention_ratio).map(Self::new)
    }
}

/// Realtime retention-ratio truncation object marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeTruncationRetentionRatioType {
    RetentionRatio,
}

/// Optional token limits for Realtime retention-ratio truncation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeTruncationTokenLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_instructions: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Realtime tool-choice selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealtimeToolChoice {
    Auto,
    None,
    Required,
    Function(Box<RealtimeToolChoiceFunction>),
    Mcp(Box<RealtimeToolChoiceMcp>),
    Other(Box<RealtimeToolChoiceOther>),
    UnknownString(String),
}

impl RealtimeToolChoice {
    pub fn function(name: impl Into<String>) -> Self {
        Self::Function(Box::new(RealtimeToolChoiceFunction {
            name: name.into(),
            extra: BTreeMap::new(),
        }))
    }

    pub fn mcp(server_label: impl Into<String>, name: Option<String>) -> Self {
        Self::Mcp(Box::new(RealtimeToolChoiceMcp {
            server_label: server_label.into(),
            name,
            extra: BTreeMap::new(),
        }))
    }
}

/// Force a specific Realtime function tool.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RealtimeToolChoiceFunction {
    pub name: String,
    pub extra: BTreeMap<String, Value>,
}

/// Force a specific Realtime MCP tool.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RealtimeToolChoiceMcp {
    pub server_label: String,
    pub name: Option<String>,
    pub extra: BTreeMap<String, Value>,
}

/// Forward-compatible Realtime tool-choice object.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RealtimeToolChoiceOther {
    pub tool_type: String,
    pub extra: BTreeMap<String, Value>,
}

/// Nullable Realtime config slot, used when `null` disables an active config.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RealtimeNullable<T> {
    Value(T),
    Null,
}

impl<T> From<T> for RealtimeNullable<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

/// Realtime tracing config.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealtimeTracing {
    Auto,
    Configuration(Box<RealtimeTracingConfiguration>),
    UnknownString(String),
}

impl RealtimeTracing {
    pub fn configuration(config: RealtimeTracingConfiguration) -> Self {
        Self::Configuration(Box::new(config))
    }
}

/// Granular Realtime tracing configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeTracingConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Serialize for RealtimeTracing {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Configuration(config) => config.serialize(serializer),
            Self::UnknownString(value) => serializer.serialize_str(value),
        }
    }
}

impl<'de> Deserialize<'de> for RealtimeTracing {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::String(value) if value == "auto" => Ok(Self::Auto),
            Value::String(value) => Ok(Self::UnknownString(value)),
            Value::Object(object) => serde_json::from_value(Value::Object(object))
                .map(|config| Self::Configuration(Box::new(config)))
                .map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom(
                "tracing must be a string or object",
            )),
        }
    }
}

impl Serialize for RealtimeToolChoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::None => serializer.serialize_str("none"),
            Self::Required => serializer.serialize_str("required"),
            Self::Function(function) => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("function")),
                );
                object.insert(String::from("name"), Value::String(function.name.clone()));
                object.extend(function.extra.clone());
                Value::Object(object).serialize(serializer)
            }
            Self::Mcp(mcp) => {
                let mut object = Map::new();
                object.insert(String::from("type"), Value::String(String::from("mcp")));
                object.insert(
                    String::from("server_label"),
                    Value::String(mcp.server_label.clone()),
                );
                if let Some(name) = &mcp.name {
                    object.insert(String::from("name"), Value::String(name.clone()));
                }
                object.extend(mcp.extra.clone());
                Value::Object(object).serialize(serializer)
            }
            Self::Other(other) => {
                let mut object = Map::new();
                object.insert(String::from("type"), Value::String(other.tool_type.clone()));
                object.extend(other.extra.clone());
                Value::Object(object).serialize(serializer)
            }
            Self::UnknownString(value) => serializer.serialize_str(value),
        }
    }
}

impl<'de> Deserialize<'de> for RealtimeToolChoice {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(value) => match value.as_str() {
                "auto" => Ok(Self::Auto),
                "none" => Ok(Self::None),
                "required" => Ok(Self::Required),
                _ => Ok(Self::UnknownString(value)),
            },
            Value::Object(mut object) => {
                let tool_type = remove_required_object_string(&mut object, "type")
                    .map_err(serde::de::Error::custom)?;
                match tool_type.as_str() {
                    "function" => Ok(Self::Function(Box::new(RealtimeToolChoiceFunction {
                        name: remove_required_object_string(&mut object, "name")
                            .map_err(serde::de::Error::custom)?,
                        extra: object.into_iter().collect(),
                    }))),
                    "mcp" => Ok(Self::Mcp(Box::new(RealtimeToolChoiceMcp {
                        server_label: remove_required_object_string(&mut object, "server_label")
                            .map_err(serde::de::Error::custom)?,
                        name: remove_optional_object_string(&mut object, "name")
                            .map_err(serde::de::Error::custom)?,
                        extra: object.into_iter().collect(),
                    }))),
                    _ => Ok(Self::Other(Box::new(RealtimeToolChoiceOther {
                        tool_type,
                        extra: object.into_iter().collect(),
                    }))),
                }
            }
            _ => Err(serde::de::Error::custom(
                "tool_choice must be a string or object",
            )),
        }
    }
}

fn remove_required_object_string(
    object: &mut Map<String, Value>,
    field: &str,
) -> Result<String, String> {
    object
        .remove(field)
        .and_then(|value| match value {
            Value::String(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| format!("Realtime tool choice missing string `{field}`"))
}

fn remove_optional_object_string(
    object: &mut Map<String, Value>,
    field: &str,
) -> Result<Option<String>, String> {
    match object.remove(field) {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(format!(
            "Realtime tool choice field `{field}` must be a string"
        )),
    }
}

realtime_string_literal_enum! {
    /// Additional Realtime fields that can be requested in server outputs.
    pub enum RealtimeInclude {
        ItemInputAudioTranscriptionLogprobs => "item.input_audio_transcription.logprobs",
    }
}

realtime_string_literal_enum! {
    /// Realtime conversation item content kinds.
    pub enum RealtimeConversationContentType {
        InputText => "input_text",
        InputAudio => "input_audio",
        InputImage => "input_image",
        ItemReference => "item_reference",
        Text => "text",
        Audio => "audio",
        OutputText => "output_text",
        OutputAudio => "output_audio",
    }
}

realtime_string_literal_enum! {
    /// Realtime conversation item kinds.
    pub enum RealtimeConversationItemType {
        Message => "message",
        FunctionCall => "function_call",
        FunctionCallOutput => "function_call_output",
        McpApprovalRequest => "mcp_approval_request",
        McpApprovalResponse => "mcp_approval_response",
        McpListTools => "mcp_list_tools",
        McpCall => "mcp_call",
        ItemReference => "item_reference",
    }
}

realtime_string_literal_enum! {
    /// Realtime conversation message roles.
    pub enum RealtimeConversationItemRole {
        User => "user",
        Assistant => "assistant",
        System => "system",
    }
}

realtime_string_literal_enum! {
    /// Realtime conversation item statuses.
    pub enum RealtimeConversationItemStatus {
        Completed => "completed",
        Incomplete => "incomplete",
        InProgress => "in_progress",
    }
}

/// Typed Realtime session configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeSessionConfig {
    #[serde(rename = "type")]
    pub session_type: RealtimeSessionType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<Vec<RealtimeOutputModality>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<RealtimeInclude>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<RealtimeMaxOutputTokens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<RealtimeToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracing: Option<RealtimeNullable<RealtimeTracing>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<RealtimeTruncation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_detection: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for RealtimeSessionConfig {
    fn default() -> Self {
        Self {
            session_type: RealtimeSessionType::Realtime,
            id: None,
            model: None,
            instructions: None,
            output_modalities: None,
            audio: None,
            include: None,
            max_output_tokens: None,
            parallel_tool_calls: None,
            prompt: None,
            reasoning: None,
            tool_choice: None,
            tools: None,
            tracing: None,
            truncation: None,
            turn_detection: None,
            extra: BTreeMap::new(),
        }
    }
}

/// One message content part in a Realtime conversation item.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeConversationMessageContentPart {
    #[serde(rename = "type")]
    pub part_type: RealtimeConversationContentType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl RealtimeConversationMessageContentPart {
    pub fn input_text(text: impl Into<String>) -> Self {
        Self {
            part_type: RealtimeConversationContentType::InputText,
            text: Some(text.into()),
            audio: None,
            transcript: None,
            extra: BTreeMap::new(),
        }
    }
}

/// Typed conversation item.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeConversationItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub item_type: RealtimeConversationItemType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<RealtimeConversationItemRole>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<RealtimeConversationMessageContentPart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<RealtimeConversationItemStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl RealtimeConversationItem {
    pub fn user_message(content: Vec<RealtimeConversationMessageContentPart>) -> Self {
        Self {
            item_type: RealtimeConversationItemType::Message,
            role: Some(RealtimeConversationItemRole::User),
            content,
            ..Default::default()
        }
    }
}

/// Structured Realtime error information.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeErrorInfo {
    pub message: String,
    #[serde(default, rename = "type")]
    pub error_type: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub param: Option<String>,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Create a new Realtime response with these parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeResponseCreateParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<RealtimeMaxOutputTokens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<Vec<RealtimeOutputModality>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<RealtimeToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
}

/// Typed client event helpers for the text/bootstrap path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealtimeClientEvent {
    SessionUpdate {
        event_id: Option<String>,
        session: RealtimeSessionConfig,
    },
    InputAudioBufferAppend {
        event_id: Option<String>,
        audio: String,
    },
    InputAudioBufferClear {
        event_id: Option<String>,
    },
    InputAudioBufferCommit {
        event_id: Option<String>,
    },
    ConversationItemCreate {
        event_id: Option<String>,
        previous_item_id: Option<String>,
        item: RealtimeConversationItem,
    },
    ConversationItemDelete {
        event_id: Option<String>,
        item_id: String,
    },
    ConversationItemRetrieve {
        event_id: Option<String>,
        item_id: String,
    },
    ConversationItemTruncate {
        event_id: Option<String>,
        item_id: String,
        content_index: usize,
        audio_end_ms: u64,
    },
    ResponseCreate {
        event_id: Option<String>,
        response: Option<Value>,
    },
    ResponseCancel {
        event_id: Option<String>,
        response_id: Option<String>,
    },
    OutputAudioBufferClear {
        event_id: Option<String>,
    },
}

impl RealtimeClientEvent {
    pub fn session_update(session: RealtimeSessionConfig) -> Self {
        Self::SessionUpdate {
            event_id: None,
            session,
        }
    }

    pub fn conversation_item_create(item: RealtimeConversationItem) -> Self {
        Self::ConversationItemCreate {
            event_id: None,
            previous_item_id: None,
            item,
        }
    }

    pub fn conversation_item_delete(item_id: impl Into<String>) -> Self {
        Self::ConversationItemDelete {
            event_id: None,
            item_id: item_id.into(),
        }
    }

    pub fn conversation_item_retrieve(item_id: impl Into<String>) -> Self {
        Self::ConversationItemRetrieve {
            event_id: None,
            item_id: item_id.into(),
        }
    }

    pub fn conversation_item_truncate(
        item_id: impl Into<String>,
        content_index: usize,
        audio_end_ms: u64,
    ) -> Self {
        Self::ConversationItemTruncate {
            event_id: None,
            item_id: item_id.into(),
            content_index,
            audio_end_ms,
        }
    }

    pub fn input_audio_buffer_append(audio: impl Into<String>) -> Self {
        Self::InputAudioBufferAppend {
            event_id: None,
            audio: audio.into(),
        }
    }

    pub fn input_audio_buffer_clear() -> Self {
        Self::InputAudioBufferClear { event_id: None }
    }

    pub fn input_audio_buffer_commit() -> Self {
        Self::InputAudioBufferCommit { event_id: None }
    }

    pub fn response_create(response: Option<Value>) -> Self {
        Self::ResponseCreate {
            event_id: None,
            response,
        }
    }

    pub fn response_cancel(response_id: Option<String>) -> Self {
        Self::ResponseCancel {
            event_id: None,
            response_id,
        }
    }

    pub fn output_audio_buffer_clear() -> Self {
        Self::OutputAudioBufferClear { event_id: None }
    }

    pub fn response_create_params(
        response: RealtimeResponseCreateParams,
    ) -> Result<Self, OpenAIError> {
        let response = serde_json::to_value(response).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("failed to serialize Realtime response.create event: {error}"),
            )
            .with_source(error)
        })?;
        Ok(Self::response_create(Some(response)))
    }

    pub fn with_event_id(mut self, event_id: impl Into<String>) -> Self {
        match &mut self {
            Self::SessionUpdate { event_id: slot, .. }
            | Self::InputAudioBufferAppend { event_id: slot, .. }
            | Self::InputAudioBufferClear { event_id: slot }
            | Self::InputAudioBufferCommit { event_id: slot }
            | Self::ConversationItemCreate { event_id: slot, .. }
            | Self::ConversationItemDelete { event_id: slot, .. }
            | Self::ConversationItemRetrieve { event_id: slot, .. }
            | Self::ConversationItemTruncate { event_id: slot, .. }
            | Self::ResponseCreate { event_id: slot, .. }
            | Self::ResponseCancel { event_id: slot, .. }
            | Self::OutputAudioBufferClear { event_id: slot } => *slot = Some(event_id.into()),
        }
        self
    }

    pub fn with_previous_item_id(mut self, previous_item_id: impl Into<String>) -> Self {
        if let Self::ConversationItemCreate {
            previous_item_id: slot,
            ..
        } = &mut self
        {
            *slot = Some(previous_item_id.into());
        }
        self
    }

    pub fn to_json_value(&self) -> Value {
        match self {
            Self::SessionUpdate { event_id, session } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("session.update")),
                );
                object.insert(
                    String::from("session"),
                    serde_json::to_value(session).unwrap_or(Value::Null),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                Value::Object(object)
            }
            Self::ConversationItemCreate {
                event_id,
                previous_item_id,
                item,
            } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("conversation.item.create")),
                );
                object.insert(
                    String::from("item"),
                    serde_json::to_value(item).unwrap_or(Value::Null),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                if let Some(previous_item_id) = previous_item_id {
                    object.insert(
                        String::from("previous_item_id"),
                        Value::String(previous_item_id.clone()),
                    );
                }
                Value::Object(object)
            }
            Self::ConversationItemDelete { event_id, item_id } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("conversation.item.delete")),
                );
                object.insert(String::from("item_id"), Value::String(item_id.clone()));
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                Value::Object(object)
            }
            Self::ConversationItemRetrieve { event_id, item_id } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("conversation.item.retrieve")),
                );
                object.insert(String::from("item_id"), Value::String(item_id.clone()));
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                Value::Object(object)
            }
            Self::ConversationItemTruncate {
                event_id,
                item_id,
                content_index,
                audio_end_ms,
            } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("conversation.item.truncate")),
                );
                object.insert(String::from("item_id"), Value::String(item_id.clone()));
                object.insert(String::from("content_index"), Value::from(*content_index));
                object.insert(String::from("audio_end_ms"), Value::from(*audio_end_ms));
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                Value::Object(object)
            }
            Self::ResponseCreate { event_id, response } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("response.create")),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                if let Some(response) = response {
                    object.insert(String::from("response"), response.clone());
                }
                Value::Object(object)
            }
            Self::ResponseCancel {
                event_id,
                response_id,
            } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("response.cancel")),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                if let Some(response_id) = response_id {
                    object.insert(
                        String::from("response_id"),
                        Value::String(response_id.clone()),
                    );
                }
                Value::Object(object)
            }
            Self::InputAudioBufferAppend { event_id, audio } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("input_audio_buffer.append")),
                );
                object.insert(String::from("audio"), Value::String(audio.clone()));
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                Value::Object(object)
            }
            Self::InputAudioBufferClear { event_id } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("input_audio_buffer.clear")),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                Value::Object(object)
            }
            Self::InputAudioBufferCommit { event_id } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("input_audio_buffer.commit")),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                Value::Object(object)
            }
            Self::OutputAudioBufferClear { event_id } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("output_audio_buffer.clear")),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                Value::Object(object)
            }
        }
    }
}

/// Realtime server events needed for bootstrap, text, and clean shutdown flows.
#[derive(Clone, Debug, PartialEq)]
pub enum RealtimeServerEvent {
    SessionCreated {
        event_id: String,
        session: RealtimeSessionConfig,
    },
    SessionUpdated {
        event_id: String,
        session: RealtimeSessionConfig,
    },
    ConversationCreated {
        event_id: String,
        conversation: Value,
    },
    ConversationItemAdded {
        event_id: String,
        previous_item_id: Option<String>,
        item: RealtimeConversationItem,
    },
    ConversationItemCreated {
        event_id: String,
        previous_item_id: Option<String>,
        item: RealtimeConversationItem,
    },
    ConversationItemDeleted {
        event_id: String,
        item_id: String,
    },
    ConversationItemDone {
        event_id: String,
        previous_item_id: Option<String>,
        item: RealtimeConversationItem,
    },
    ConversationItemRetrieved {
        event_id: String,
        item: RealtimeConversationItem,
    },
    ConversationItemInputAudioTranscriptionCompleted {
        event_id: String,
        item_id: String,
        content_index: usize,
        transcript: String,
        usage: Value,
        logprobs: Option<Value>,
    },
    ConversationItemInputAudioTranscriptionDelta {
        event_id: String,
        item_id: String,
        content_index: Option<usize>,
        delta: Option<String>,
        logprobs: Option<Value>,
    },
    ConversationItemInputAudioTranscriptionFailed {
        event_id: String,
        item_id: String,
        content_index: usize,
        error: Value,
    },
    ConversationItemInputAudioTranscriptionSegment {
        event_id: String,
        item_id: String,
        content_index: usize,
        id: String,
        start: f64,
        end: f64,
        speaker: String,
        text: String,
    },
    InputAudioBufferCommitted {
        event_id: String,
        item_id: String,
        previous_item_id: Option<String>,
    },
    InputAudioBufferDtmfEventReceived {
        event: String,
        received_at: u64,
    },
    InputAudioBufferTimeoutTriggered {
        event_id: String,
        item_id: String,
        audio_start_ms: u64,
        audio_end_ms: u64,
    },
    InputAudioBufferSpeechStarted {
        event_id: String,
        item_id: String,
        audio_start_ms: u64,
    },
    InputAudioBufferSpeechStopped {
        event_id: String,
        item_id: String,
        audio_end_ms: u64,
    },
    InputAudioBufferCleared {
        event_id: String,
    },
    ConversationItemTruncated {
        event_id: String,
        item_id: String,
        content_index: usize,
        audio_end_ms: u64,
    },
    OutputTextDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        delta: String,
    },
    OutputTextDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        text: String,
    },
    ResponseOutputItemAdded {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        item: RealtimeConversationItem,
    },
    ResponseOutputItemDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        item: RealtimeConversationItem,
    },
    ResponseContentPartAdded {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        part: RealtimeConversationMessageContentPart,
    },
    ResponseContentPartDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        part: RealtimeConversationMessageContentPart,
    },
    OutputAudioDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        delta: String,
    },
    OutputAudioDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
    },
    OutputAudioTranscriptDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        delta: String,
    },
    OutputAudioTranscriptDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        transcript: String,
    },
    FunctionCallArgumentsDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        delta: String,
    },
    FunctionCallArgumentsDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        arguments: String,
        name: Option<String>,
    },
    McpCallArgumentsDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        delta: String,
        obfuscation: Option<String>,
    },
    McpCallArgumentsDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        arguments: String,
    },
    McpListToolsStatus {
        event_id: String,
        event_type: String,
        item_id: String,
    },
    ResponseItemStatus {
        event_id: String,
        event_type: String,
        item_id: String,
        output_index: usize,
    },
    RateLimitsUpdated {
        event_id: String,
        rate_limits: Vec<Value>,
    },
    OutputAudioBufferStarted {
        event_id: String,
        response_id: String,
    },
    OutputAudioBufferStopped {
        event_id: String,
        response_id: String,
    },
    OutputAudioBufferCleared {
        event_id: String,
        response_id: String,
    },
    ResponseCreated {
        event_id: String,
        response: Value,
    },
    ResponseDone {
        event_id: String,
        response: Value,
    },
    Error {
        event_id: String,
        error: RealtimeErrorInfo,
    },
    Unknown {
        event_id: Option<String>,
        event_type: String,
        raw: Value,
    },
}

impl RealtimeServerEvent {
    pub fn event_type(&self) -> &str {
        match self {
            Self::SessionCreated { .. } => "session.created",
            Self::SessionUpdated { .. } => "session.updated",
            Self::ConversationCreated { .. } => "conversation.created",
            Self::ConversationItemAdded { .. } => "conversation.item.added",
            Self::ConversationItemCreated { .. } => "conversation.item.created",
            Self::ConversationItemDeleted { .. } => "conversation.item.deleted",
            Self::ConversationItemDone { .. } => "conversation.item.done",
            Self::ConversationItemRetrieved { .. } => "conversation.item.retrieved",
            Self::ConversationItemInputAudioTranscriptionCompleted { .. } => {
                "conversation.item.input_audio_transcription.completed"
            }
            Self::ConversationItemInputAudioTranscriptionDelta { .. } => {
                "conversation.item.input_audio_transcription.delta"
            }
            Self::ConversationItemInputAudioTranscriptionFailed { .. } => {
                "conversation.item.input_audio_transcription.failed"
            }
            Self::ConversationItemInputAudioTranscriptionSegment { .. } => {
                "conversation.item.input_audio_transcription.segment"
            }
            Self::InputAudioBufferCommitted { .. } => "input_audio_buffer.committed",
            Self::InputAudioBufferDtmfEventReceived { .. } => {
                "input_audio_buffer.dtmf_event_received"
            }
            Self::InputAudioBufferTimeoutTriggered { .. } => "input_audio_buffer.timeout_triggered",
            Self::InputAudioBufferSpeechStarted { .. } => "input_audio_buffer.speech_started",
            Self::InputAudioBufferSpeechStopped { .. } => "input_audio_buffer.speech_stopped",
            Self::InputAudioBufferCleared { .. } => "input_audio_buffer.cleared",
            Self::ConversationItemTruncated { .. } => "conversation.item.truncated",
            Self::OutputTextDelta { .. } => "response.output_text.delta",
            Self::OutputTextDone { .. } => "response.output_text.done",
            Self::ResponseOutputItemAdded { .. } => "response.output_item.added",
            Self::ResponseOutputItemDone { .. } => "response.output_item.done",
            Self::ResponseContentPartAdded { .. } => "response.content_part.added",
            Self::ResponseContentPartDone { .. } => "response.content_part.done",
            Self::OutputAudioDelta { .. } => "response.output_audio.delta",
            Self::OutputAudioDone { .. } => "response.output_audio.done",
            Self::OutputAudioTranscriptDelta { .. } => "response.output_audio_transcript.delta",
            Self::OutputAudioTranscriptDone { .. } => "response.output_audio_transcript.done",
            Self::FunctionCallArgumentsDelta { .. } => "response.function_call_arguments.delta",
            Self::FunctionCallArgumentsDone { .. } => "response.function_call_arguments.done",
            Self::McpCallArgumentsDelta { .. } => "response.mcp_call_arguments.delta",
            Self::McpCallArgumentsDone { .. } => "response.mcp_call_arguments.done",
            Self::McpListToolsStatus { event_type, .. } => event_type.as_str(),
            Self::ResponseItemStatus { event_type, .. } => event_type.as_str(),
            Self::RateLimitsUpdated { .. } => "rate_limits.updated",
            Self::OutputAudioBufferStarted { .. } => "output_audio_buffer.started",
            Self::OutputAudioBufferStopped { .. } => "output_audio_buffer.stopped",
            Self::OutputAudioBufferCleared { .. } => "output_audio_buffer.cleared",
            Self::ResponseCreated { .. } => "response.created",
            Self::ResponseDone { .. } => "response.done",
            Self::Error { .. } => "error",
            Self::Unknown { event_type, .. } => event_type.as_str(),
        }
    }
}

/// Decodes one typed Realtime server event from a JSON payload.
pub fn decode_server_event(value: &Value) -> Result<RealtimeServerEvent, OpenAIError> {
    let object = value.as_object().ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Parse,
            "failed to parse Realtime websocket event: expected a JSON object",
        )
    })?;
    let event_type = object.get("type").and_then(Value::as_str).ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Parse,
            "failed to parse Realtime websocket event: missing `type`",
        )
    })?;

    match event_type {
        "session.created" => Ok(RealtimeServerEvent::SessionCreated {
            event_id: required_string(object, "event_id")?,
            session: required_json(object, "session")?,
        }),
        "session.updated" => Ok(RealtimeServerEvent::SessionUpdated {
            event_id: required_string(object, "event_id")?,
            session: required_json(object, "session")?,
        }),
        "conversation.created" => Ok(RealtimeServerEvent::ConversationCreated {
            event_id: required_string(object, "event_id")?,
            conversation: object.get("conversation").cloned().unwrap_or(Value::Null),
        }),
        "conversation.item.added" => Ok(RealtimeServerEvent::ConversationItemAdded {
            event_id: required_string(object, "event_id")?,
            previous_item_id: optional_string(object, "previous_item_id"),
            item: required_json(object, "item")?,
        }),
        "conversation.item.created" => Ok(RealtimeServerEvent::ConversationItemCreated {
            event_id: required_string(object, "event_id")?,
            previous_item_id: optional_string(object, "previous_item_id"),
            item: required_json(object, "item")?,
        }),
        "conversation.item.deleted" => Ok(RealtimeServerEvent::ConversationItemDeleted {
            event_id: required_string(object, "event_id")?,
            item_id: required_string(object, "item_id")?,
        }),
        "conversation.item.done" => Ok(RealtimeServerEvent::ConversationItemDone {
            event_id: required_string(object, "event_id")?,
            previous_item_id: optional_string(object, "previous_item_id"),
            item: required_json(object, "item")?,
        }),
        "conversation.item.retrieved" => Ok(RealtimeServerEvent::ConversationItemRetrieved {
            event_id: required_string(object, "event_id")?,
            item: required_json(object, "item")?,
        }),
        "conversation.item.input_audio_transcription.completed" => Ok(
            RealtimeServerEvent::ConversationItemInputAudioTranscriptionCompleted {
                event_id: required_string(object, "event_id")?,
                item_id: required_string(object, "item_id")?,
                content_index: required_usize(object, "content_index")?,
                transcript: required_string(object, "transcript")?,
                usage: object.get("usage").cloned().unwrap_or(Value::Null),
                logprobs: object.get("logprobs").cloned(),
            },
        ),
        "conversation.item.input_audio_transcription.delta" => Ok(
            RealtimeServerEvent::ConversationItemInputAudioTranscriptionDelta {
                event_id: required_string(object, "event_id")?,
                item_id: required_string(object, "item_id")?,
                content_index: optional_usize(object, "content_index")?,
                delta: optional_string(object, "delta"),
                logprobs: object.get("logprobs").cloned(),
            },
        ),
        "conversation.item.input_audio_transcription.failed" => Ok(
            RealtimeServerEvent::ConversationItemInputAudioTranscriptionFailed {
                event_id: required_string(object, "event_id")?,
                item_id: required_string(object, "item_id")?,
                content_index: required_usize(object, "content_index")?,
                error: object.get("error").cloned().unwrap_or(Value::Null),
            },
        ),
        "conversation.item.input_audio_transcription.segment" => Ok(
            RealtimeServerEvent::ConversationItemInputAudioTranscriptionSegment {
                event_id: required_string(object, "event_id")?,
                item_id: required_string(object, "item_id")?,
                content_index: required_usize(object, "content_index")?,
                id: required_string(object, "id")?,
                start: required_f64(object, "start")?,
                end: required_f64(object, "end")?,
                speaker: required_string(object, "speaker")?,
                text: required_string(object, "text")?,
            },
        ),
        "input_audio_buffer.committed" => Ok(RealtimeServerEvent::InputAudioBufferCommitted {
            event_id: required_string(object, "event_id")?,
            item_id: required_string(object, "item_id")?,
            previous_item_id: optional_string(object, "previous_item_id"),
        }),
        "input_audio_buffer.dtmf_event_received" => {
            Ok(RealtimeServerEvent::InputAudioBufferDtmfEventReceived {
                event: required_string(object, "event")?,
                received_at: required_u64(object, "received_at")?,
            })
        }
        "input_audio_buffer.timeout_triggered" => {
            Ok(RealtimeServerEvent::InputAudioBufferTimeoutTriggered {
                event_id: required_string(object, "event_id")?,
                item_id: required_string(object, "item_id")?,
                audio_start_ms: required_u64(object, "audio_start_ms")?,
                audio_end_ms: required_u64(object, "audio_end_ms")?,
            })
        }
        "input_audio_buffer.speech_started" => {
            Ok(RealtimeServerEvent::InputAudioBufferSpeechStarted {
                event_id: required_string(object, "event_id")?,
                item_id: required_string(object, "item_id")?,
                audio_start_ms: required_u64(object, "audio_start_ms")?,
            })
        }
        "input_audio_buffer.speech_stopped" => {
            Ok(RealtimeServerEvent::InputAudioBufferSpeechStopped {
                event_id: required_string(object, "event_id")?,
                item_id: required_string(object, "item_id")?,
                audio_end_ms: required_u64(object, "audio_end_ms")?,
            })
        }
        "input_audio_buffer.cleared" => Ok(RealtimeServerEvent::InputAudioBufferCleared {
            event_id: required_string(object, "event_id")?,
        }),
        "conversation.item.truncated" => Ok(RealtimeServerEvent::ConversationItemTruncated {
            event_id: required_string(object, "event_id")?,
            item_id: required_string(object, "item_id")?,
            content_index: required_usize(object, "content_index")?,
            audio_end_ms: required_u64(object, "audio_end_ms")?,
        }),
        "response.output_text.delta" => Ok(RealtimeServerEvent::OutputTextDelta {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            delta: required_string(object, "delta")?,
        }),
        "response.output_text.done" => Ok(RealtimeServerEvent::OutputTextDone {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            text: required_string(object, "text")?,
        }),
        "response.output_item.added" => {
            let item: RealtimeConversationItem = required_json(object, "item")?;
            Ok(RealtimeServerEvent::ResponseOutputItemAdded {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: response_output_item_id(object, &item)?,
                output_index: required_usize(object, "output_index")?,
                item,
            })
        }
        "response.output_item.done" => {
            let item: RealtimeConversationItem = required_json(object, "item")?;
            Ok(RealtimeServerEvent::ResponseOutputItemDone {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: response_output_item_id(object, &item)?,
                output_index: required_usize(object, "output_index")?,
                item,
            })
        }
        "response.content_part.added" => Ok(RealtimeServerEvent::ResponseContentPartAdded {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            part: required_json(object, "part")?,
        }),
        "response.content_part.done" => Ok(RealtimeServerEvent::ResponseContentPartDone {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            part: required_json(object, "part")?,
        }),
        "response.output_audio.delta" => Ok(RealtimeServerEvent::OutputAudioDelta {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            delta: required_string(object, "delta")?,
        }),
        "response.output_audio.done" => Ok(RealtimeServerEvent::OutputAudioDone {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
        }),
        "response.output_audio_transcript.delta" => {
            Ok(RealtimeServerEvent::OutputAudioTranscriptDelta {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: required_string(object, "item_id")?,
                output_index: required_usize(object, "output_index")?,
                content_index: required_usize(object, "content_index")?,
                delta: required_string(object, "delta")?,
            })
        }
        "response.output_audio_transcript.done" => {
            Ok(RealtimeServerEvent::OutputAudioTranscriptDone {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: required_string(object, "item_id")?,
                output_index: required_usize(object, "output_index")?,
                content_index: required_usize(object, "content_index")?,
                transcript: required_string(object, "transcript")?,
            })
        }
        "response.function_call_arguments.delta" => {
            Ok(RealtimeServerEvent::FunctionCallArgumentsDelta {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: required_string(object, "item_id")?,
                output_index: required_usize(object, "output_index")?,
                delta: required_string(object, "delta")?,
            })
        }
        "response.function_call_arguments.done" => {
            Ok(RealtimeServerEvent::FunctionCallArgumentsDone {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: required_string(object, "item_id")?,
                output_index: required_usize(object, "output_index")?,
                arguments: required_string(object, "arguments")?,
                name: optional_string(object, "name"),
            })
        }
        "response.mcp_call_arguments.delta" => Ok(RealtimeServerEvent::McpCallArgumentsDelta {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            delta: required_string(object, "delta")?,
            obfuscation: optional_string(object, "obfuscation"),
        }),
        "response.mcp_call_arguments.done" => Ok(RealtimeServerEvent::McpCallArgumentsDone {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            arguments: required_string(object, "arguments")?,
        }),
        "mcp_list_tools.in_progress" | "mcp_list_tools.completed" | "mcp_list_tools.failed" => {
            Ok(RealtimeServerEvent::McpListToolsStatus {
                event_id: required_string(object, "event_id")?,
                event_type: event_type.to_string(),
                item_id: required_string(object, "item_id")?,
            })
        }
        "response.mcp_call.in_progress"
        | "response.mcp_call.completed"
        | "response.mcp_call.failed" => Ok(RealtimeServerEvent::ResponseItemStatus {
            event_id: required_string(object, "event_id")?,
            event_type: event_type.to_string(),
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
        }),
        "rate_limits.updated" => Ok(RealtimeServerEvent::RateLimitsUpdated {
            event_id: required_string(object, "event_id")?,
            rate_limits: required_json(object, "rate_limits")?,
        }),
        "output_audio_buffer.started" => Ok(RealtimeServerEvent::OutputAudioBufferStarted {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
        }),
        "output_audio_buffer.stopped" => Ok(RealtimeServerEvent::OutputAudioBufferStopped {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
        }),
        "output_audio_buffer.cleared" => Ok(RealtimeServerEvent::OutputAudioBufferCleared {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
        }),
        "response.created" => Ok(RealtimeServerEvent::ResponseCreated {
            event_id: required_string(object, "event_id")?,
            response: object.get("response").cloned().unwrap_or(Value::Null),
        }),
        "response.done" => Ok(RealtimeServerEvent::ResponseDone {
            event_id: required_string(object, "event_id")?,
            response: object.get("response").cloned().unwrap_or(Value::Null),
        }),
        "error" => Ok(RealtimeServerEvent::Error {
            event_id: required_string(object, "event_id")?,
            error: required_json(object, "error")?,
        }),
        _ => Ok(RealtimeServerEvent::Unknown {
            event_id: optional_string(object, "event_id"),
            event_type: event_type.to_string(),
            raw: value.clone(),
        }),
    }
}

/// Parses and decodes one typed Realtime server event from text.
pub fn decode_server_event_text(text: &str) -> Result<RealtimeServerEvent, OpenAIError> {
    let value = serde_json::from_str::<Value>(text).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Realtime websocket event JSON: {error}"),
        )
        .with_source(error)
    })?;
    decode_server_event(&value)
}

fn required_json<T>(object: &Map<String, Value>, key: &str) -> Result<T, OpenAIError>
where
    T: for<'de> Deserialize<'de>,
{
    let value = object.get(key).cloned().ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Realtime websocket event: missing `{key}`"),
        )
    })?;
    serde_json::from_value(value).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Realtime websocket event field `{key}`: {error}"),
        )
        .with_source(error)
    })
}

fn response_output_item_id(
    object: &Map<String, Value>,
    item: &RealtimeConversationItem,
) -> Result<String, OpenAIError> {
    optional_string(object, "item_id")
        .or_else(|| item.id.clone())
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                "failed to parse Realtime websocket event: missing `item_id`",
            )
        })
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String, OpenAIError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to parse Realtime websocket event: missing `{key}`"),
            )
        })
}

fn optional_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_string)
}

fn required_usize(object: &Map<String, Value>, key: &str) -> Result<usize, OpenAIError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to parse Realtime websocket event: missing `{key}`"),
            )
        })
}

fn optional_usize(object: &Map<String, Value>, key: &str) -> Result<Option<usize>, OpenAIError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .map(Some)
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to parse Realtime websocket event field `{key}` as usize"),
            )
        })
}

fn required_u64(object: &Map<String, Value>, key: &str) -> Result<u64, OpenAIError> {
    object.get(key).and_then(Value::as_u64).ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Realtime websocket event: missing `{key}`"),
        )
    })
}

fn required_f64(object: &Map<String, Value>, key: &str) -> Result<f64, OpenAIError> {
    object.get(key).and_then(Value::as_f64).ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Realtime websocket event: missing `{key}`"),
        )
    })
}
