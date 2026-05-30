use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::Duration,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use serde_json::Value;
use tokio::runtime::Builder;
use tokio::sync::watch;

use crate::{
    OpenAIError,
    core::{
        metadata::ResponseMetadata, request::RequestOptions, runtime::ClientRuntime,
        transport::execute_text_stream,
    },
    error::ErrorKind,
    helpers::sse::{SseFrame, SseParser},
    resources::{
        common::{
            ListOrder, PromptCacheRetention, ReasoningEffort, SearchContextSize, ServiceTier,
            Verbosity,
        },
        multimodal::{ChatImageDetail, InputAudioFormat},
    },
};

macro_rules! chat_string_literal_enum {
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

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Ok(match value.as_str() {
                    $($literal => Self::$variant,)+
                    _ => Self::Unknown(value),
                })
            }
        }
    };
}

/// Chat namespace for compatibility surfaces.
#[derive(Clone, Debug)]
pub struct Chat {
    runtime: Arc<ClientRuntime>,
}

impl Chat {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns the Chat Completions compatibility surface.
    pub fn completions(&self) -> ChatCompletions {
        ChatCompletions::new(self.runtime.clone())
    }
}

/// Stored and streamed Chat Completions compatibility surface.
#[derive(Clone, Debug)]
pub struct ChatCompletions {
    runtime: Arc<ClientRuntime>,
}

impl ChatCompletions {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns the stored-message listing helper.
    pub fn messages(&self) -> StoredChatCompletionMessages {
        StoredChatCompletionMessages::new(self.runtime.clone())
    }

    /// Creates a non-streamed chat completion.
    pub fn create(
        &self,
        params: ChatCompletionCreateParams,
    ) -> Result<crate::core::response::ApiResponse<ChatCompletion>, OpenAIError> {
        let body = params.into_request_body(false);
        self.runtime.execute_json_with_body(
            "POST",
            "/chat/completions",
            &body,
            RequestOptions::default(),
        )
    }

    /// Creates a non-streamed chat completion and parses structured output/tool arguments.
    pub fn parse<T>(
        &self,
        params: ChatCompletionCreateParams,
    ) -> Result<crate::core::response::ApiResponse<ParsedChatCompletion<T>>, OpenAIError>
    where
        T: DeserializeOwned,
    {
        let response_format = params.response_format.clone();
        let tools = params.tools.clone().unwrap_or_default();
        let response = self.create(params)?;
        let parsed =
            parse_chat_completion_output(response.output, response_format.as_ref(), &tools)?;
        Ok(crate::core::response::ApiResponse {
            output: parsed,
            metadata: response.metadata,
        })
    }

    /// Creates a streamed chat completion and accumulates the final message snapshot.
    pub fn stream(
        &self,
        params: ChatCompletionCreateParams,
    ) -> Result<ChatCompletionStream, OpenAIError> {
        let response_format = params.response_format.clone();
        let tools = params.tools.clone().unwrap_or_default();
        let body = params.into_request_body(true);
        let request = self
            .runtime
            .prepare_json_request("POST", "/chat/completions", &body)?;
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;

        ChatCompletionStream::start_live(request, options, response_format, tools)
    }

    /// Retrieves a stored chat completion by id.
    pub fn retrieve(
        &self,
        completion_id: &str,
    ) -> Result<crate::core::response::ApiResponse<ChatCompletion>, OpenAIError> {
        let completion_id = validate_path_id("completion_id", completion_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/chat/completions/{completion_id}"),
            RequestOptions::default(),
        )
    }

    /// Updates stored chat-completion metadata.
    pub fn update(
        &self,
        completion_id: &str,
        params: StoredChatCompletionUpdateParams,
    ) -> Result<crate::core::response::ApiResponse<ChatCompletion>, OpenAIError> {
        let completion_id = validate_path_id("completion_id", completion_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/chat/completions/{completion_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    /// Lists stored chat completions with cursor/filter semantics.
    pub fn list(
        &self,
        params: StoredChatCompletionsListParams,
    ) -> Result<crate::core::response::ApiResponse<StoredChatCompletionsPage>, OpenAIError> {
        let path = append_query("/chat/completions", params.to_query_pairs());
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Deletes a stored chat completion.
    pub fn delete(
        &self,
        completion_id: &str,
    ) -> Result<crate::core::response::ApiResponse<DeletedChatCompletion>, OpenAIError> {
        let completion_id = validate_path_id("completion_id", completion_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/chat/completions/{completion_id}"),
            RequestOptions::default(),
        )
    }
}

/// Stored-message helper surface under chat completions.
#[derive(Clone, Debug)]
pub struct StoredChatCompletionMessages {
    runtime: Arc<ClientRuntime>,
}

impl StoredChatCompletionMessages {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Lists stored messages for a chat completion with cursor semantics.
    pub fn list(
        &self,
        completion_id: &str,
        params: StoredChatCompletionMessagesListParams,
    ) -> Result<crate::core::response::ApiResponse<StoredChatCompletionMessagesPage>, OpenAIError>
    {
        let completion_id = validate_path_id("completion_id", completion_id)?;
        let path = append_query(
            &format!("/chat/completions/{completion_id}/messages"),
            params.to_query_pairs(),
        );
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }
}

/// Request body for chat-completion creation.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ChatCompletionCreateParams {
    pub model: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub messages: Vec<ChatCompletionMessageParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<ChatCompletionAudioParams>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<ChatCompletionFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<ChatCompletionFunction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<BTreeMap<String, i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<ChatCompletionModality>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prediction: Option<ChatCompletionPredictionContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<PromptCacheRetention>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ChatCompletionResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<ChatStop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<ChatCompletionStreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ChatCompletionToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatCompletionTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<Verbosity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search_options: Option<ChatWebSearchOptions>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionCreateParams {
    pub fn with_serialized_messages<T>(mut self, messages: T) -> Result<Self, OpenAIError>
    where
        T: Serialize,
    {
        let value = serialize_json_value("chat.completions.messages", messages)?;
        let Value::Array(messages) = value else {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "chat.completions.messages must serialize to a JSON array",
            ));
        };
        self.messages = messages
            .into_iter()
            .map(ChatCompletionMessageParam::Json)
            .collect();
        Ok(self)
    }

    fn into_request_body(self, stream: bool) -> Value {
        let mut value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        if let Value::Object(ref mut object) = value {
            object.insert(String::from("stream"), Value::Bool(stream));
        }
        value
    }
}

/// Message parameter accepted by chat-completion creation.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionMessageParam {
    Developer(ChatCompletionDeveloperMessageParam),
    System(ChatCompletionSystemMessageParam),
    User(ChatCompletionUserMessageParam),
    Assistant(ChatCompletionAssistantMessageParam),
    Tool(ChatCompletionToolMessageParam),
    Function(ChatCompletionFunctionMessageParam),
    Json(Value),
}

impl ChatCompletionMessageParam {
    pub fn user(content: impl Into<ChatCompletionUserMessageContent>) -> Self {
        Self::User(ChatCompletionUserMessageParam::new(content))
    }

    pub fn system(content: impl Into<ChatCompletionTextMessageContent>) -> Self {
        Self::System(ChatCompletionSystemMessageParam::new(content))
    }

    pub fn developer(content: impl Into<ChatCompletionTextMessageContent>) -> Self {
        Self::Developer(ChatCompletionDeveloperMessageParam::new(content))
    }

    pub fn assistant(content: impl Into<ChatCompletionAssistantMessageContent>) -> Self {
        Self::Assistant(ChatCompletionAssistantMessageParam::new(Some(
            content.into(),
        )))
    }

    pub fn tool(
        tool_call_id: impl Into<String>,
        content: impl Into<ChatCompletionTextMessageContent>,
    ) -> Self {
        Self::Tool(ChatCompletionToolMessageParam::new(tool_call_id, content))
    }

    pub fn function(name: impl Into<String>, content: Option<String>) -> Self {
        Self::Function(ChatCompletionFunctionMessageParam::new(name, content))
    }
}

impl From<ChatCompletionDeveloperMessageParam> for ChatCompletionMessageParam {
    fn from(value: ChatCompletionDeveloperMessageParam) -> Self {
        Self::Developer(value)
    }
}

impl From<ChatCompletionSystemMessageParam> for ChatCompletionMessageParam {
    fn from(value: ChatCompletionSystemMessageParam) -> Self {
        Self::System(value)
    }
}

impl From<ChatCompletionUserMessageParam> for ChatCompletionMessageParam {
    fn from(value: ChatCompletionUserMessageParam) -> Self {
        Self::User(value)
    }
}

impl From<ChatCompletionAssistantMessageParam> for ChatCompletionMessageParam {
    fn from(value: ChatCompletionAssistantMessageParam) -> Self {
        Self::Assistant(value)
    }
}

impl From<ChatCompletionToolMessageParam> for ChatCompletionMessageParam {
    fn from(value: ChatCompletionToolMessageParam) -> Self {
        Self::Tool(value)
    }
}

impl From<ChatCompletionFunctionMessageParam> for ChatCompletionMessageParam {
    fn from(value: ChatCompletionFunctionMessageParam) -> Self {
        Self::Function(value)
    }
}

impl From<Value> for ChatCompletionMessageParam {
    fn from(value: Value) -> Self {
        Self::Json(value)
    }
}

/// Chat message role literal used in chat completion responses and stream deltas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatCompletionRole {
    Developer,
    System,
    User,
    Assistant,
    Tool,
    Function,
    Unknown(String),
}

impl ChatCompletionRole {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Developer => "developer",
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
            Self::Function => "function",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl AsRef<str> for ChatCompletionRole {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for ChatCompletionRole {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl std::fmt::Display for ChatCompletionRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ChatCompletionRole {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ChatCompletionRole {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "developer" => Self::Developer,
            "system" => Self::System,
            "user" => Self::User,
            "assistant" => Self::Assistant,
            "tool" => Self::Tool,
            "function" => Self::Function,
            _ => Self::Unknown(value),
        })
    }
}

/// Reason a chat completion choice stopped generating.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatCompletionFinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    FunctionCall,
    Unknown(String),
}

impl ChatCompletionFinishReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Stop => "stop",
            Self::Length => "length",
            Self::ToolCalls => "tool_calls",
            Self::ContentFilter => "content_filter",
            Self::FunctionCall => "function_call",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl AsRef<str> for ChatCompletionFinishReason {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for ChatCompletionFinishReason {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl std::fmt::Display for ChatCompletionFinishReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ChatCompletionFinishReason {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ChatCompletionFinishReason {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "stop" => Self::Stop,
            "length" => Self::Length,
            "tool_calls" => Self::ToolCalls,
            "content_filter" => Self::ContentFilter,
            "function_call" => Self::FunctionCall,
            _ => Self::Unknown(value),
        })
    }
}

chat_string_literal_enum! {
    /// Chat message content-part discriminator.
    pub enum ChatCompletionContentPartType {
        Text => "text",
        ImageUrl => "image_url",
        InputAudio => "input_audio",
        File => "file",
        Refusal => "refusal",
    }
}

chat_string_literal_enum! {
    /// Predicted-content discriminator used by chat completions.
    pub enum ChatCompletionPredictionContentType {
        Content => "content",
    }
}

chat_string_literal_enum! {
    /// Chat tool discriminator.
    pub enum ChatCompletionToolType {
        Function => "function",
        Custom => "custom",
    }
}

chat_string_literal_enum! {
    /// Chat allowed-tool choice discriminator.
    pub enum ChatCompletionAllowedToolChoiceType {
        AllowedTools => "allowed_tools",
    }
}

chat_string_literal_enum! {
    /// Mode used by a constrained chat tool choice.
    pub enum ChatCompletionAllowedToolsMode {
        Auto => "auto",
        Required => "required",
    }
}

chat_string_literal_enum! {
    /// Chat web-search user-location discriminator.
    pub enum ChatWebSearchUserLocationType {
        Approximate => "approximate",
    }
}

chat_string_literal_enum! {
    /// Chat custom-tool format discriminator.
    pub enum ChatCompletionCustomToolFormatType {
        Grammar => "grammar",
    }
}

chat_string_literal_enum! {
    /// Grammar syntax accepted by chat custom tools.
    pub enum ChatCompletionCustomToolGrammarSyntax {
        Lark => "lark",
        Regex => "regex",
    }
}

/// Developer-role chat message parameter.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionDeveloperMessageParam {
    pub content: ChatCompletionTextMessageContent,
    #[serde(rename = "role")]
    pub role: ChatCompletionRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionDeveloperMessageParam {
    pub fn new(content: impl Into<ChatCompletionTextMessageContent>) -> Self {
        Self {
            content: content.into(),
            role: ChatCompletionRole::Developer,
            name: None,
            extra: BTreeMap::new(),
        }
    }
}

/// System-role chat message parameter.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionSystemMessageParam {
    pub content: ChatCompletionTextMessageContent,
    #[serde(rename = "role")]
    pub role: ChatCompletionRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionSystemMessageParam {
    pub fn new(content: impl Into<ChatCompletionTextMessageContent>) -> Self {
        Self {
            content: content.into(),
            role: ChatCompletionRole::System,
            name: None,
            extra: BTreeMap::new(),
        }
    }
}

/// User-role chat message parameter.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionUserMessageParam {
    pub content: ChatCompletionUserMessageContent,
    #[serde(rename = "role")]
    pub role: ChatCompletionRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionUserMessageParam {
    pub fn new(content: impl Into<ChatCompletionUserMessageContent>) -> Self {
        Self {
            content: content.into(),
            role: ChatCompletionRole::User,
            name: None,
            extra: BTreeMap::new(),
        }
    }
}

/// Assistant-role chat message parameter.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionAssistantMessageParam {
    #[serde(rename = "role")]
    pub role: ChatCompletionRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<ChatCompletionAssistantAudioParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ChatCompletionAssistantMessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<ChatCompletionAssistantFunctionCallParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatCompletionMessageToolCallParam>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionAssistantMessageParam {
    pub fn new(content: Option<ChatCompletionAssistantMessageContent>) -> Self {
        Self {
            role: ChatCompletionRole::Assistant,
            audio: None,
            content,
            function_call: None,
            name: None,
            refusal: None,
            tool_calls: None,
            extra: BTreeMap::new(),
        }
    }
}

/// Tool-role chat message parameter.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionToolMessageParam {
    pub content: ChatCompletionTextMessageContent,
    #[serde(rename = "role")]
    pub role: ChatCompletionRole,
    pub tool_call_id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionToolMessageParam {
    pub fn new(
        tool_call_id: impl Into<String>,
        content: impl Into<ChatCompletionTextMessageContent>,
    ) -> Self {
        Self {
            content: content.into(),
            role: ChatCompletionRole::Tool,
            tool_call_id: tool_call_id.into(),
            extra: BTreeMap::new(),
        }
    }
}

/// Legacy function-role chat message parameter.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionFunctionMessageParam {
    pub content: Option<String>,
    pub name: String,
    #[serde(rename = "role")]
    pub role: ChatCompletionRole,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionFunctionMessageParam {
    pub fn new(name: impl Into<String>, content: Option<String>) -> Self {
        Self {
            content,
            name: name.into(),
            role: ChatCompletionRole::Function,
            extra: BTreeMap::new(),
        }
    }
}

/// Text-only message content used by developer, system, and tool messages.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionTextMessageContent {
    Text(String),
    Parts(Vec<ChatCompletionContentPartText>),
}

impl From<String> for ChatCompletionTextMessageContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for ChatCompletionTextMessageContent {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<ChatCompletionContentPartText>> for ChatCompletionTextMessageContent {
    fn from(value: Vec<ChatCompletionContentPartText>) -> Self {
        Self::Parts(value)
    }
}

/// User message content accepted by chat completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionUserMessageContent {
    Text(String),
    Parts(Vec<ChatCompletionContentPartParam>),
}

impl From<String> for ChatCompletionUserMessageContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for ChatCompletionUserMessageContent {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<ChatCompletionContentPartParam>> for ChatCompletionUserMessageContent {
    fn from(value: Vec<ChatCompletionContentPartParam>) -> Self {
        Self::Parts(value)
    }
}

/// Assistant message content accepted by chat completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionAssistantMessageContent {
    Text(String),
    Parts(Vec<ChatCompletionAssistantContentPartParam>),
}

impl From<String> for ChatCompletionAssistantMessageContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for ChatCompletionAssistantMessageContent {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<ChatCompletionAssistantContentPartParam>> for ChatCompletionAssistantMessageContent {
    fn from(value: Vec<ChatCompletionAssistantContentPartParam>) -> Self {
        Self::Parts(value)
    }
}

/// User content part accepted by chat completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionContentPartParam {
    Text(ChatCompletionContentPartText),
    Image(ChatCompletionContentPartImage),
    InputAudio(ChatCompletionContentPartInputAudio),
    File(ChatCompletionContentPartFile),
}

impl From<ChatCompletionContentPartText> for ChatCompletionContentPartParam {
    fn from(value: ChatCompletionContentPartText) -> Self {
        Self::Text(value)
    }
}

impl From<ChatCompletionContentPartImage> for ChatCompletionContentPartParam {
    fn from(value: ChatCompletionContentPartImage) -> Self {
        Self::Image(value)
    }
}

impl From<ChatCompletionContentPartInputAudio> for ChatCompletionContentPartParam {
    fn from(value: ChatCompletionContentPartInputAudio) -> Self {
        Self::InputAudio(value)
    }
}

impl From<ChatCompletionContentPartFile> for ChatCompletionContentPartParam {
    fn from(value: ChatCompletionContentPartFile) -> Self {
        Self::File(value)
    }
}

/// Assistant content part accepted by chat completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionAssistantContentPartParam {
    Text(ChatCompletionContentPartText),
    Refusal(ChatCompletionContentPartRefusal),
}

impl From<ChatCompletionContentPartText> for ChatCompletionAssistantContentPartParam {
    fn from(value: ChatCompletionContentPartText) -> Self {
        Self::Text(value)
    }
}

impl From<ChatCompletionContentPartRefusal> for ChatCompletionAssistantContentPartParam {
    fn from(value: ChatCompletionContentPartRefusal) -> Self {
        Self::Refusal(value)
    }
}

/// Image content part accepted in user chat messages.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionContentPartImage {
    pub image_url: ChatCompletionImageUrlParam,
    #[serde(rename = "type")]
    pub content_type: ChatCompletionContentPartType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionContentPartImage {
    pub fn url(url: impl Into<String>, detail: Option<ChatImageDetail>) -> Self {
        Self {
            image_url: ChatCompletionImageUrlParam {
                url: url.into(),
                detail,
                extra: BTreeMap::new(),
            },
            content_type: ChatCompletionContentPartType::ImageUrl,
            extra: BTreeMap::new(),
        }
    }
}

/// Chat-completions output modalities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatCompletionModality {
    Text,
    Audio,
}

impl ChatCompletionModality {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Audio => "audio",
        }
    }
}

impl AsRef<str> for ChatCompletionModality {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Image URL descriptor for chat message content.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionImageUrlParam {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ChatImageDetail>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Input-audio content part accepted in user chat messages.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionContentPartInputAudio {
    pub input_audio: ChatCompletionInputAudioParam,
    #[serde(rename = "type")]
    pub content_type: ChatCompletionContentPartType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionContentPartInputAudio {
    pub fn new(input_audio: ChatCompletionInputAudioParam) -> Self {
        Self {
            input_audio,
            content_type: ChatCompletionContentPartType::InputAudio,
            extra: BTreeMap::new(),
        }
    }
}

/// Encoded input audio embedded in a chat user message.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionInputAudioParam {
    pub data: String,
    pub format: InputAudioFormat,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// File content part accepted in user chat messages.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionContentPartFile {
    pub file: ChatCompletionContentPartFileValue,
    #[serde(rename = "type")]
    pub content_type: ChatCompletionContentPartType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionContentPartFile {
    pub fn new(file: ChatCompletionContentPartFileValue) -> Self {
        Self {
            file,
            content_type: ChatCompletionContentPartType::File,
            extra: BTreeMap::new(),
        }
    }
}

/// File value nested in a chat file content part.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionContentPartFileValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Refusal content part accepted in assistant chat messages.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionContentPartRefusal {
    pub refusal: String,
    #[serde(rename = "type")]
    pub content_type: ChatCompletionContentPartType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionContentPartRefusal {
    pub fn new(refusal: impl Into<String>) -> Self {
        Self {
            refusal: refusal.into(),
            content_type: ChatCompletionContentPartType::Refusal,
            extra: BTreeMap::new(),
        }
    }
}

/// Previous assistant audio response reference.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionAssistantAudioParam {
    pub id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Deprecated assistant function-call payload.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionAssistantFunctionCallParam {
    pub arguments: String,
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Tool-call parameter nested in assistant messages.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionMessageToolCallParam {
    Function(ChatCompletionMessageFunctionToolCallParam),
    Custom(ChatCompletionMessageCustomToolCallParam),
}

impl From<ChatCompletionMessageFunctionToolCallParam> for ChatCompletionMessageToolCallParam {
    fn from(value: ChatCompletionMessageFunctionToolCallParam) -> Self {
        Self::Function(value)
    }
}

impl From<ChatCompletionMessageCustomToolCallParam> for ChatCompletionMessageToolCallParam {
    fn from(value: ChatCompletionMessageCustomToolCallParam) -> Self {
        Self::Custom(value)
    }
}

/// Function tool-call parameter nested in assistant messages.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionMessageFunctionToolCallParam {
    pub id: String,
    pub function: ChatCompletionMessageFunctionToolCallFunctionParam,
    #[serde(rename = "type")]
    pub tool_type: ChatCompletionToolType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionMessageFunctionToolCallParam {
    pub fn new(
        id: impl Into<String>,
        function: ChatCompletionMessageFunctionToolCallFunctionParam,
    ) -> Self {
        Self {
            id: id.into(),
            function,
            tool_type: ChatCompletionToolType::Function,
            extra: BTreeMap::new(),
        }
    }
}

/// Function payload nested in an assistant tool-call parameter.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionMessageFunctionToolCallFunctionParam {
    pub arguments: String,
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Custom tool-call parameter nested in assistant messages.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionMessageCustomToolCallParam {
    pub id: String,
    pub custom: ChatCompletionMessageCustomToolCallCustomParam,
    #[serde(rename = "type")]
    pub tool_type: ChatCompletionToolType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionMessageCustomToolCallParam {
    pub fn new(
        id: impl Into<String>,
        custom: ChatCompletionMessageCustomToolCallCustomParam,
    ) -> Self {
        Self {
            id: id.into(),
            custom,
            tool_type: ChatCompletionToolType::Custom,
            extra: BTreeMap::new(),
        }
    }
}

/// Custom payload nested in an assistant tool-call parameter.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionMessageCustomToolCallCustomParam {
    pub input: String,
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Parameters for audio output from chat completions.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionAudioParams {
    pub format: ChatCompletionAudioFormat,
    pub voice: ChatCompletionVoice,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Output audio format for chat completions.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatCompletionAudioFormat {
    #[default]
    Wav,
    Aac,
    Mp3,
    Flac,
    Opus,
    Pcm16,
}

impl ChatCompletionAudioFormat {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Aac => "aac",
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Opus => "opus",
            Self::Pcm16 => "pcm16",
        }
    }
}

impl AsRef<str> for ChatCompletionAudioFormat {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Built-in voice name or custom voice object.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionVoice {
    Name(String),
    Custom(ChatCompletionVoiceId),
}

impl Default for ChatCompletionVoice {
    fn default() -> Self {
        Self::Name(String::new())
    }
}

impl From<String> for ChatCompletionVoice {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}

impl From<&str> for ChatCompletionVoice {
    fn from(value: &str) -> Self {
        Self::Name(value.to_string())
    }
}

/// Custom voice reference for chat completion audio.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionVoiceId {
    pub id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Streaming options for chat completions.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionStreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_obfuscation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Stop sequence configuration for chat completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatStop {
    String(String),
    Strings(Vec<String>),
}

impl From<String> for ChatStop {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for ChatStop {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<Vec<String>> for ChatStop {
    fn from(value: Vec<String>) -> Self {
        Self::Strings(value)
    }
}

/// Static predicted output content for faster matching completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionPredictionContent {
    pub content: ChatCompletionPredictionContentValue,
    #[serde(rename = "type")]
    pub content_type: ChatCompletionPredictionContentType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionPredictionContent {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: ChatCompletionPredictionContentValue::Text(content.into()),
            content_type: ChatCompletionPredictionContentType::Content,
            extra: BTreeMap::new(),
        }
    }

    pub fn parts(content: Vec<ChatCompletionContentPartText>) -> Self {
        Self {
            content: ChatCompletionPredictionContentValue::TextParts(content),
            content_type: ChatCompletionPredictionContentType::Content,
            extra: BTreeMap::new(),
        }
    }
}

/// Prediction content value accepted by chat completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionPredictionContentValue {
    Text(String),
    TextParts(Vec<ChatCompletionContentPartText>),
}

impl From<String> for ChatCompletionPredictionContentValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for ChatCompletionPredictionContentValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<ChatCompletionContentPartText>> for ChatCompletionPredictionContentValue {
    fn from(value: Vec<ChatCompletionContentPartText>) -> Self {
        Self::TextParts(value)
    }
}

/// Text content part accepted in chat prediction content.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionContentPartText {
    #[serde(rename = "type")]
    pub content_type: ChatCompletionContentPartType,
    pub text: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionContentPartText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            content_type: ChatCompletionContentPartType::Text,
            text: text.into(),
            extra: BTreeMap::new(),
        }
    }
}

/// Response format configuration for chat completions.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatCompletionResponseFormat {
    Text,
    JsonObject,
    JsonSchema(ChatCompletionResponseFormatJsonSchema),
}

impl Serialize for ChatCompletionResponseFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Text => {
                #[derive(Serialize)]
                struct TextFormat<'a> {
                    #[serde(rename = "type")]
                    format_type: &'a str,
                }

                TextFormat {
                    format_type: "text",
                }
                .serialize(serializer)
            }
            Self::JsonObject => {
                #[derive(Serialize)]
                struct JsonObjectFormat<'a> {
                    #[serde(rename = "type")]
                    format_type: &'a str,
                }

                JsonObjectFormat {
                    format_type: "json_object",
                }
                .serialize(serializer)
            }
            Self::JsonSchema(json_schema) => {
                #[derive(Serialize)]
                struct JsonSchemaFormat<'a> {
                    #[serde(rename = "type")]
                    format_type: &'a str,
                    json_schema: &'a ChatCompletionResponseFormatJsonSchema,
                }

                JsonSchemaFormat {
                    format_type: "json_schema",
                    json_schema,
                }
                .serialize(serializer)
            }
        }
    }
}

/// JSON-schema response format payload for chat completions.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionResponseFormatJsonSchema {
    pub name: String,
    pub schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Web search options for chat completions.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatWebSearchOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<SearchContextSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<ChatWebSearchUserLocation>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Approximate user location wrapper for chat web search.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatWebSearchUserLocation {
    pub approximate: ChatWebSearchUserLocationApproximate,
    #[serde(rename = "type")]
    pub location_type: ChatWebSearchUserLocationType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatWebSearchUserLocation {
    pub fn approximate(approximate: ChatWebSearchUserLocationApproximate) -> Self {
        Self {
            approximate,
            location_type: ChatWebSearchUserLocationType::Approximate,
            extra: BTreeMap::new(),
        }
    }
}

/// Approximate location values for chat web search.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatWebSearchUserLocationApproximate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Deprecated function-call selector for legacy chat function definitions.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatCompletionFunctionCall {
    None,
    Auto,
    Named(ChatCompletionFunctionCallOption),
}

impl Serialize for ChatCompletionFunctionCall {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::None => serializer.serialize_str("none"),
            Self::Auto => serializer.serialize_str("auto"),
            Self::Named(option) => option.serialize(serializer),
        }
    }
}

/// Named legacy function-call selector.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionFunctionCallOption {
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Deprecated legacy function definition for chat completions.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Tool-choice selector for chat completions.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatCompletionToolChoice {
    None,
    Auto,
    Required,
    Function(ChatCompletionNamedToolChoiceFunction),
    Custom(ChatCompletionNamedToolChoiceCustom),
    AllowedTools(ChatCompletionAllowedToolChoice),
}

impl Serialize for ChatCompletionToolChoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::None => serializer.serialize_str("none"),
            Self::Auto => serializer.serialize_str("auto"),
            Self::Required => serializer.serialize_str("required"),
            Self::Function(function) => {
                #[derive(Serialize)]
                struct FunctionChoice<'a> {
                    #[serde(rename = "type")]
                    choice_type: &'a str,
                    function: &'a ChatCompletionNamedToolChoiceFunction,
                }

                FunctionChoice {
                    choice_type: "function",
                    function,
                }
                .serialize(serializer)
            }
            Self::Custom(custom) => {
                #[derive(Serialize)]
                struct CustomChoice<'a> {
                    #[serde(rename = "type")]
                    choice_type: &'a str,
                    custom: &'a ChatCompletionNamedToolChoiceCustom,
                }

                CustomChoice {
                    choice_type: "custom",
                    custom,
                }
                .serialize(serializer)
            }
            Self::AllowedTools(allowed_tools) => allowed_tools.serialize(serializer),
        }
    }
}

/// Function name used by a named chat tool choice.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionNamedToolChoiceFunction {
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Custom tool name used by a named chat tool choice.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionNamedToolChoiceCustom {
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Allowed-tool choice wrapper for chat completions.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionAllowedToolChoice {
    pub allowed_tools: ChatCompletionAllowedTools,
    #[serde(rename = "type")]
    pub choice_type: ChatCompletionAllowedToolChoiceType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionAllowedToolChoice {
    pub fn new(allowed_tools: ChatCompletionAllowedTools) -> Self {
        Self {
            allowed_tools,
            choice_type: ChatCompletionAllowedToolChoiceType::AllowedTools,
            extra: BTreeMap::new(),
        }
    }
}

/// Set of tools allowed by a constrained chat tool choice.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionAllowedTools {
    pub mode: ChatCompletionAllowedToolsMode,
    pub tools: Vec<BTreeMap<String, Value>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Tool definition accepted by chat completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionTool {
    Function(ChatCompletionFunctionTool),
    Custom(ChatCompletionCustomTool),
}

impl From<ChatCompletionFunctionTool> for ChatCompletionTool {
    fn from(value: ChatCompletionFunctionTool) -> Self {
        Self::Function(value)
    }
}

impl From<ChatCompletionCustomTool> for ChatCompletionTool {
    fn from(value: ChatCompletionCustomTool) -> Self {
        Self::Custom(value)
    }
}

/// Function tool definition for chat completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionFunctionTool {
    pub function: ChatCompletionFunctionDefinition,
    #[serde(rename = "type")]
    pub tool_type: ChatCompletionToolType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionFunctionTool {
    pub fn new(function: ChatCompletionFunctionDefinition) -> Self {
        Self {
            function,
            tool_type: ChatCompletionToolType::Function,
            extra: BTreeMap::new(),
        }
    }
}

/// Function definition nested in a chat tool.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionFunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Custom tool definition for chat completions.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionCustomTool {
    pub custom: ChatCompletionCustomToolConfig,
    #[serde(rename = "type")]
    pub tool_type: ChatCompletionToolType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionCustomTool {
    pub fn new(custom: ChatCompletionCustomToolConfig) -> Self {
        Self {
            custom,
            tool_type: ChatCompletionToolType::Custom,
            extra: BTreeMap::new(),
        }
    }
}

/// Custom tool properties for chat completions.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionCustomToolConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<ChatCompletionCustomToolFormat>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Input format for chat custom tools.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatCompletionCustomToolFormat {
    Text,
    Grammar(ChatCompletionCustomToolGrammarFormat),
}

impl Serialize for ChatCompletionCustomToolFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Text => {
                #[derive(Serialize)]
                struct TextFormat<'a> {
                    #[serde(rename = "type")]
                    format_type: &'a str,
                }

                TextFormat {
                    format_type: "text",
                }
                .serialize(serializer)
            }
            Self::Grammar(grammar) => grammar.serialize(serializer),
        }
    }
}

/// Grammar input format for chat custom tools.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionCustomToolGrammarFormat {
    pub grammar: ChatCompletionCustomToolGrammar,
    #[serde(rename = "type")]
    pub format_type: ChatCompletionCustomToolFormatType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionCustomToolGrammarFormat {
    pub fn new(grammar: ChatCompletionCustomToolGrammar) -> Self {
        Self {
            grammar,
            format_type: ChatCompletionCustomToolFormatType::Grammar,
            extra: BTreeMap::new(),
        }
    }
}

/// Grammar definition for chat custom tools.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatCompletionCustomToolGrammar {
    pub definition: String,
    pub syntax: ChatCompletionCustomToolGrammarSyntax,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Metadata-only stored chat-completion update body.
#[derive(Clone, Debug, Default, Serialize)]
pub struct StoredChatCompletionUpdateParams {
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Query parameters for stored chat-completion listing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoredChatCompletionsListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
    pub model: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl StoredChatCompletionsListParams {
    fn to_query_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        if let Some(after) = &self.after {
            pairs.push((String::from("after"), after.clone()));
        }
        if let Some(limit) = self.limit {
            pairs.push((String::from("limit"), limit.to_string()));
        }
        for (key, value) in &self.metadata {
            pairs.push((format!("metadata[{key}]"), value.clone()));
        }
        if let Some(model) = &self.model {
            pairs.push((String::from("model"), model.clone()));
        }
        if let Some(order) = &self.order {
            pairs.push((String::from("order"), order.as_str().to_string()));
        }
        pairs
    }
}

/// Query parameters for stored chat-completion message listing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoredChatCompletionMessagesListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl StoredChatCompletionMessagesListParams {
    fn to_query_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        if let Some(after) = &self.after {
            pairs.push((String::from("after"), after.clone()));
        }
        if let Some(limit) = self.limit {
            pairs.push((String::from("limit"), limit.to_string()));
        }
        if let Some(order) = &self.order {
            pairs.push((String::from("order"), order.as_str().to_string()));
        }
        pairs
    }
}

/// Typed chat completion object.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatCompletion {
    pub id: String,
    pub object: String,
    pub created: i64,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub choices: Vec<ChatCompletionChoice>,
    #[serde(default)]
    pub service_tier: Option<ServiceTier>,
    #[serde(default)]
    pub system_fingerprint: Option<String>,
    #[serde(default)]
    pub usage: Option<ChatCompletionUsage>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed choice on a chat completion.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatCompletionChoice {
    pub index: usize,
    #[serde(default)]
    pub finish_reason: Option<ChatCompletionFinishReason>,
    pub message: ChatCompletionMessage,
    #[serde(default)]
    pub logprobs: Option<ChatCompletionChoiceLogprobs>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed assistant/user/system message for chat completions.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionMessage {
    #[serde(default)]
    pub role: Option<ChatCompletionRole>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub refusal: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub annotations: Vec<ChatCompletionAnnotation>,
    #[serde(default)]
    pub audio: Option<ChatCompletionAudio>,
    #[serde(default)]
    pub function_call: Option<LegacyFunctionCall>,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub tool_calls: Vec<ChatCompletionMessageToolCall>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// URL citation annotation attached to a chat-completion message.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionAnnotation {
    #[serde(default, rename = "type")]
    pub annotation_type: Option<String>,
    #[serde(default)]
    pub url_citation: Option<ChatCompletionUrlCitation>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// URL-citation details for a chat-completion annotation.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionUrlCitation {
    #[serde(default)]
    pub end_index: i64,
    #[serde(default)]
    pub start_index: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Audio response metadata for chat completions.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionAudio {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub expires_at: i64,
    #[serde(default)]
    pub transcript: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Legacy `function_call` object retained for compatibility.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct LegacyFunctionCall {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Indexed tool-call record on a compatibility message.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionMessageToolCall {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(rename = "type", default)]
    pub tool_type: Option<ChatCompletionToolType>,
    #[serde(default)]
    pub function: ToolCallFunction,
    #[serde(default)]
    pub custom: Option<ChatCompletionMessageCustomToolCallCustom>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Function payload inside a tool-call record.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ToolCallFunction {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Custom tool payload inside a tool-call record.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionMessageCustomToolCallCustom {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub input: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Chat completion returned by `chat.completions.parse`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedChatCompletion<T> {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: Option<String>,
    pub choices: Vec<ParsedChatCompletionChoice<T>>,
    pub service_tier: Option<ServiceTier>,
    pub system_fingerprint: Option<String>,
    pub usage: Option<ChatCompletionUsage>,
    pub extra: BTreeMap<String, Value>,
}

impl<T> ParsedChatCompletion<T> {
    pub fn first_parsed(&self) -> Option<&T> {
        self.choices
            .first()
            .and_then(|choice| choice.message.parsed.as_ref())
    }
}

/// Parsed choice on a chat completion.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedChatCompletionChoice<T> {
    pub index: usize,
    pub finish_reason: Option<ChatCompletionFinishReason>,
    pub message: ParsedChatCompletionMessage<T>,
    pub logprobs: Option<ChatCompletionChoiceLogprobs>,
    pub extra: BTreeMap<String, Value>,
}

/// Parsed assistant message for chat completions.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedChatCompletionMessage<T> {
    pub role: Option<ChatCompletionRole>,
    pub content: Option<String>,
    pub refusal: Option<String>,
    pub parsed: Option<T>,
    pub annotations: Vec<ChatCompletionAnnotation>,
    pub audio: Option<ChatCompletionAudio>,
    pub function_call: Option<LegacyFunctionCall>,
    pub tool_calls: Vec<ParsedChatCompletionMessageToolCall>,
    pub extra: BTreeMap<String, Value>,
}

impl<T> ParsedChatCompletionMessage<T> {
    pub fn parsed(&self) -> Option<&T> {
        self.parsed.as_ref()
    }
}

/// Parsed tool-call record on a compatibility message.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedChatCompletionMessageToolCall {
    pub id: Option<String>,
    pub index: Option<usize>,
    pub tool_type: Option<ChatCompletionToolType>,
    pub function: ParsedToolCallFunction,
    pub custom: Option<ChatCompletionMessageCustomToolCallCustom>,
    pub extra: BTreeMap<String, Value>,
}

/// Function payload with parsed strict arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedToolCallFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
    pub parsed_arguments: Option<Value>,
    pub extra: BTreeMap<String, Value>,
}

fn parse_chat_completion_output<T>(
    completion: ChatCompletion,
    response_format: Option<&ChatCompletionResponseFormat>,
    tools: &[ChatCompletionTool],
) -> Result<ParsedChatCompletion<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    let parse_content = matches!(
        response_format,
        Some(
            ChatCompletionResponseFormat::JsonObject | ChatCompletionResponseFormat::JsonSchema(_)
        )
    );
    let strict_tool_names = strict_chat_function_tool_names(tools);

    let choices = completion
        .choices
        .into_iter()
        .map(|choice| parse_chat_completion_choice(choice, parse_content, &strict_tool_names))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ParsedChatCompletion {
        id: completion.id,
        object: completion.object,
        created: completion.created,
        model: completion.model,
        choices,
        service_tier: completion.service_tier,
        system_fingerprint: completion.system_fingerprint,
        usage: completion.usage,
        extra: completion.extra,
    })
}

fn parse_chat_completion_choice<T>(
    choice: ChatCompletionChoice,
    parse_content: bool,
    strict_tool_names: &BTreeSet<String>,
) -> Result<ParsedChatCompletionChoice<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    if matches!(
        choice.finish_reason,
        Some(ChatCompletionFinishReason::Length)
    ) {
        return Err(OpenAIError::new(
            ErrorKind::Parse,
            "chat completion ended because it reached the length limit",
        ));
    }
    if matches!(
        choice.finish_reason,
        Some(ChatCompletionFinishReason::ContentFilter)
    ) {
        return Err(OpenAIError::new(
            ErrorKind::Parse,
            "chat completion ended because content was filtered",
        ));
    }

    let message = parse_chat_completion_message(choice.message, parse_content, strict_tool_names)?;
    Ok(ParsedChatCompletionChoice {
        index: choice.index,
        finish_reason: choice.finish_reason,
        message,
        logprobs: choice.logprobs,
        extra: choice.extra,
    })
}

fn parse_chat_completion_message<T>(
    message: ChatCompletionMessage,
    parse_content: bool,
    strict_tool_names: &BTreeSet<String>,
) -> Result<ParsedChatCompletionMessage<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    let parsed = if parse_content && message.refusal.is_none() {
        match message.content.as_deref() {
            Some(content) if !content.trim().is_empty() => {
                Some(serde_json::from_str(content).map_err(|error| {
                    OpenAIError::new(
                        ErrorKind::Parse,
                        format!("failed to parse chat completion structured output: {error}"),
                    )
                    .with_source(error)
                })?)
            }
            _ => None,
        }
    } else {
        None
    };

    let tool_calls = message
        .tool_calls
        .into_iter()
        .map(|tool_call| parse_chat_tool_call(tool_call, strict_tool_names))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ParsedChatCompletionMessage {
        role: message.role,
        content: message.content,
        refusal: message.refusal,
        parsed,
        annotations: message.annotations,
        audio: message.audio,
        function_call: message.function_call,
        tool_calls,
        extra: message.extra,
    })
}

fn parse_chat_tool_call(
    tool_call: ChatCompletionMessageToolCall,
    strict_tool_names: &BTreeSet<String>,
) -> Result<ParsedChatCompletionMessageToolCall, OpenAIError> {
    let parsed_arguments = match (
        tool_call.function.name.as_deref(),
        tool_call.function.arguments.as_deref(),
    ) {
        (Some(name), Some(arguments)) if strict_tool_names.contains(name) => {
            Some(serde_json::from_str(arguments).map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Parse,
                    format!("failed to parse strict chat tool arguments for `{name}`: {error}"),
                )
                .with_source(error)
            })?)
        }
        _ => None,
    };

    Ok(ParsedChatCompletionMessageToolCall {
        id: tool_call.id,
        index: tool_call.index,
        tool_type: tool_call.tool_type,
        function: ParsedToolCallFunction {
            name: tool_call.function.name,
            arguments: tool_call.function.arguments,
            parsed_arguments,
            extra: tool_call.function.extra,
        },
        custom: tool_call.custom,
        extra: tool_call.extra,
    })
}

fn strict_chat_function_tool_names(tools: &[ChatCompletionTool]) -> BTreeSet<String> {
    tools
        .iter()
        .filter_map(|tool| match tool {
            ChatCompletionTool::Function(tool) if tool.function.strict == Some(true) => {
                Some(tool.function.name.clone())
            }
            _ => None,
        })
        .collect()
}

/// Usage statistics for chat completions and chat streaming usage chunks.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionUsage {
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub completion_tokens_details: Option<ChatCompletionTokensDetails>,
    #[serde(default)]
    pub prompt_tokens_details: Option<ChatPromptTokensDetails>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Completion-token breakdown for chat usage.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionTokensDetails {
    #[serde(default)]
    pub accepted_prediction_tokens: Option<u64>,
    #[serde(default)]
    pub audio_tokens: Option<u64>,
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
    #[serde(default)]
    pub rejected_prediction_tokens: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Prompt-token breakdown for chat usage.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatPromptTokensDetails {
    #[serde(default)]
    pub audio_tokens: Option<u64>,
    #[serde(default)]
    pub cached_tokens: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Logprob payload for a chat completion choice.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionChoiceLogprobs {
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub content: Vec<ChatCompletionTokenLogprob>,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub refusal: Vec<ChatCompletionTokenLogprob>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Per-token logprob data for chat completions.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionTokenLogprob {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub bytes: Option<Vec<i64>>,
    #[serde(default)]
    pub logprob: f64,
    #[serde(default)]
    pub top_logprobs: Vec<ChatCompletionTopLogprob>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Top-logprob alternative for one chat token.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionTopLogprob {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub bytes: Option<Vec<i64>>,
    #[serde(default)]
    pub logprob: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Stored chat-completions list envelope.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct StoredChatCompletionsPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<ChatCompletion>,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl StoredChatCompletionsPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more && self.last_id.is_some()
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_next_page() {
            self.last_id.as_deref()
        } else {
            None
        }
    }
}

/// Stored chat-completion message list envelope.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct StoredChatCompletionMessagesPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<StoredChatCompletionMessage>,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl StoredChatCompletionMessagesPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more && self.last_id.is_some()
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_next_page() {
            self.last_id.as_deref()
        } else {
            None
        }
    }
}

/// Typed stored chat-completion message.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct StoredChatCompletionMessage {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub role: Option<ChatCompletionRole>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub tool_calls: Vec<ChatCompletionMessageToolCall>,
    #[serde(default)]
    pub function_call: Option<LegacyFunctionCall>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed stored-completion deletion marker.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DeletedChatCompletion {
    pub id: String,
    pub object: String,
    pub deleted: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed streamed chat-completion chunk.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub choices: Vec<ChatCompletionChunkChoice>,
    #[serde(default)]
    pub service_tier: Option<ServiceTier>,
    #[serde(default)]
    pub system_fingerprint: Option<String>,
    #[serde(default)]
    pub usage: Option<ChatCompletionUsage>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Choice inside a streamed chat-completion chunk.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatCompletionChunkChoice {
    pub index: usize,
    #[serde(default)]
    pub delta: ChatCompletionChunkDelta,
    #[serde(default)]
    pub finish_reason: Option<ChatCompletionFinishReason>,
    #[serde(default)]
    pub logprobs: Option<ChatCompletionChoiceLogprobs>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Delta payload inside a streamed chat-completion chunk.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionChunkDelta {
    #[serde(default)]
    pub role: Option<ChatCompletionRole>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub refusal: Option<String>,
    #[serde(default)]
    pub function_call: Option<LegacyFunctionCall>,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub tool_calls: Vec<ChatCompletionMessageToolCall>,
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

/// Compatibility stream that surfaces raw chunks plus a final accumulated snapshot.
#[derive(Debug)]
pub struct ChatCompletionStream {
    metadata: ResponseMetadata,
    chunks: VecDeque<ChatCompletionChunk>,
    final_completion: Option<ChatCompletion>,
    response_format: Option<ChatCompletionResponseFormat>,
    tools: Vec<ChatCompletionTool>,
    live: Option<LiveChatCompletionStreamHandle>,
    aborted: bool,
}

impl ChatCompletionStream {
    pub fn from_sse_chunks<I, B>(metadata: ResponseMetadata, chunks: I) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        let mut parser = SseParser::default();
        let mut surfaced = VecDeque::new();
        let mut accumulator = ChatCompletionAccumulator::default();

        for chunk in chunks {
            for frame in parser.push(chunk.as_ref().as_bytes())? {
                if let Some(parsed) = accumulator.ingest_frame(frame)? {
                    surfaced.push_back(parsed);
                }
            }
        }
        for frame in parser.finish()? {
            if let Some(parsed) = accumulator.ingest_frame(frame)? {
                surfaced.push_back(parsed);
            }
        }

        let final_completion = accumulator.finish()?;
        Ok(Self {
            metadata,
            chunks: surfaced,
            final_completion: Some(final_completion),
            response_format: None,
            tools: Vec::new(),
            live: None,
            aborted: false,
        })
    }

    pub fn with_parse_context(
        mut self,
        response_format: Option<ChatCompletionResponseFormat>,
        tools: Vec<ChatCompletionTool>,
    ) -> Self {
        self.response_format = response_format;
        self.tools = tools;
        self
    }

    pub fn next_chunk(&mut self) -> Option<ChatCompletionChunk> {
        if self.aborted {
            return None;
        }
        if self.chunks.is_empty() {
            self.fill_from_live();
        }
        let chunk = self.chunks.pop_front()?;
        self.drain_live_messages();
        if self.final_completion.is_none() {
            self.poll_live_messages(Duration::from_millis(5));
        }
        Some(chunk)
    }

    pub async fn next_chunk_async(&mut self) -> Option<ChatCompletionChunk> {
        self.next_chunk()
    }

    pub fn final_completion(&mut self) -> Result<&ChatCompletion, OpenAIError> {
        if self.aborted {
            return Err(OpenAIError::new(
                ErrorKind::Transport,
                "chat completion stream was aborted before completion",
            ));
        }

        if let Some(live) = &self.live {
            live.shared.wait_until_finished();
            if let Some(error) = live.shared.error() {
                return Err(error);
            }
        }
        self.drain_live_messages();
        if self.final_completion.is_none() {
            if let Some(live) = &self.live {
                self.final_completion = live.shared.final_completion_cloned();
            }
        }

        self.final_completion.as_ref().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                "chat completion stream ended without a terminal chunk",
            )
        })
    }

    pub fn final_message(
        &mut self,
        choice_index: usize,
    ) -> Result<&ChatCompletionMessage, OpenAIError> {
        let final_completion = self.final_completion()?;
        final_completion
            .choices
            .get(choice_index)
            .map(|choice| &choice.message)
            .ok_or_else(|| {
                OpenAIError::new(
                    ErrorKind::Validation,
                    format!("missing accumulated chat completion choice {choice_index}"),
                )
            })
    }

    pub fn parse_final<T>(&mut self) -> Result<ParsedChatCompletion<T>, OpenAIError>
    where
        T: DeserializeOwned,
    {
        let final_completion = self.final_completion()?.clone();
        parse_chat_completion_output(final_completion, self.response_format.as_ref(), &self.tools)
    }

    pub fn metadata(&self) -> &ResponseMetadata {
        &self.metadata
    }

    pub fn abort(&mut self) {
        self.aborted = true;
        self.chunks.clear();
        if let Some(live) = &mut self.live {
            let _ = live.abort.send(true);
            let _ = live.receiver.try_recv();
            live.join_worker();
        }
    }

    fn start_live(
        request: crate::core::request::PreparedRequest,
        options: crate::core::request::ResolvedRequestOptions,
        response_format: Option<ChatCompletionResponseFormat>,
        tools: Vec<ChatCompletionTool>,
    ) -> Result<Self, OpenAIError> {
        let (startup_tx, startup_rx) = mpsc::channel();
        let (chunk_tx, chunk_rx) = mpsc::channel();
        let (abort_tx, abort_rx) = watch::channel(false);
        let shared = Arc::new(LiveChatCompletionShared::default());
        let thread_shared = shared.clone();

        let worker = thread::spawn(move || {
            let runtime = match Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    let error = OpenAIError::new(
                        ErrorKind::Transport,
                        format!("failed to build chat streaming runtime: {error}"),
                    )
                    .with_source(error);
                    let _ = startup_tx.send(Err(error.clone()));
                    thread_shared.finish_with_error(error);
                    return;
                }
            };

            runtime.block_on(async move {
                match execute_text_stream(&request, &options).await {
                    Ok(response) => {
                        let metadata = response.metadata.clone();
                        let _ = startup_tx.send(Ok(metadata));
                        if let Err(error) = consume_live_stream(
                            response,
                            abort_rx,
                            chunk_tx.clone(),
                            thread_shared.clone(),
                        )
                        .await
                        {
                            thread_shared.finish_with_error(error.clone());
                            let _ = chunk_tx.send(LiveChatCompletionMessage::Error(error));
                        }
                    }
                    Err(error) => {
                        let _ = startup_tx.send(Err(error.clone()));
                        thread_shared.finish_with_error(error);
                    }
                }
            });
        });

        let metadata = startup_rx.recv().map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("chat stream worker exited before startup completed: {error}"),
            )
        })??;

        Ok(Self {
            metadata,
            chunks: VecDeque::new(),
            final_completion: None,
            response_format,
            tools,
            live: Some(LiveChatCompletionStreamHandle {
                receiver: chunk_rx,
                abort: abort_tx,
                worker: Some(worker),
                shared,
            }),
            aborted: false,
        })
    }

    fn fill_from_live(&mut self) {
        let Some(live) = self.live.as_mut() else {
            return;
        };

        let Some(message) = live.receiver.recv().ok() else {
            if self.final_completion.is_none() {
                self.final_completion = live.shared.final_completion_cloned();
            }
            live.join_worker();
            self.live = None;
            return;
        };
        self.process_live_message(message);

        while let Some(live) = self.live.as_mut() {
            match live.receiver.try_recv() {
                Ok(message) => self.process_live_message(message),
                Err(_) => break,
            }
        }
    }

    fn drain_live_messages(&mut self) {
        while let Some(live) = self.live.as_mut() {
            match live.receiver.try_recv() {
                Ok(message) => self.process_live_message(message),
                Err(_) => break,
            }
        }
    }

    fn poll_live_messages(&mut self, timeout: Duration) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        if let Ok(message) = live.receiver.recv_timeout(timeout) {
            self.process_live_message(message);
            self.drain_live_messages();
        }
    }

    fn process_live_message(&mut self, message: LiveChatCompletionMessage) {
        match message {
            LiveChatCompletionMessage::Chunk(chunk) => {
                self.chunks.push_back(*chunk);
            }
            LiveChatCompletionMessage::Finished => {
                if let Some(live) = self.live.as_mut() {
                    if self.final_completion.is_none() {
                        self.final_completion = live.shared.final_completion_cloned();
                    }
                    live.join_worker();
                }
                self.live = None;
            }
            LiveChatCompletionMessage::Error(error) => {
                if let Some(live) = self.live.as_mut() {
                    live.shared.finish_with_error(error);
                    live.join_worker();
                }
                self.live = None;
            }
        }
    }
}

impl Drop for ChatCompletionStream {
    fn drop(&mut self) {
        if let Some(live) = &mut self.live {
            let _ = live.abort.send(true);
            live.join_worker();
        }
    }
}

#[derive(Debug)]
enum LiveChatCompletionMessage {
    Chunk(Box<ChatCompletionChunk>),
    Finished,
    Error(OpenAIError),
}

#[derive(Debug)]
struct LiveChatCompletionStreamHandle {
    receiver: mpsc::Receiver<LiveChatCompletionMessage>,
    abort: watch::Sender<bool>,
    worker: Option<thread::JoinHandle<()>>,
    shared: Arc<LiveChatCompletionShared>,
}

impl LiveChatCompletionStreamHandle {
    fn join_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug, Default)]
struct LiveChatCompletionShared {
    state: Mutex<LiveChatCompletionSharedState>,
    done: Condvar,
}

impl LiveChatCompletionShared {
    fn finish_with_completion(&self, completion: ChatCompletion) {
        let mut state = self.state.lock().expect("chat completion shared state");
        state.final_completion = Some(completion);
        state.finished = true;
        self.done.notify_all();
    }

    fn finish_with_error(&self, error: OpenAIError) {
        let mut state = self.state.lock().expect("chat completion shared state");
        state.error = Some(error);
        state.finished = true;
        self.done.notify_all();
    }

    fn wait_until_finished(&self) {
        let mut state = self.state.lock().expect("chat completion shared state");
        while !state.finished {
            state = self.done.wait(state).expect("chat completion shared state");
        }
    }

    fn error(&self) -> Option<OpenAIError> {
        self.state
            .lock()
            .expect("chat completion shared state")
            .error
            .clone()
    }

    fn final_completion_cloned(&self) -> Option<ChatCompletion> {
        self.state
            .lock()
            .expect("chat completion shared state")
            .final_completion
            .clone()
    }
}

#[derive(Debug, Default)]
struct LiveChatCompletionSharedState {
    final_completion: Option<ChatCompletion>,
    error: Option<OpenAIError>,
    finished: bool,
}
#[derive(Clone, Debug, Default)]
struct ChatCompletionAccumulator {
    id: Option<String>,
    created: Option<i64>,
    model: Option<String>,
    service_tier: Option<ServiceTier>,
    system_fingerprint: Option<String>,
    usage: Option<ChatCompletionUsage>,
    choices: Vec<AccumulatedChoice>,
    seen_done: bool,
    seen_terminal_chunk: bool,
}

impl ChatCompletionAccumulator {
    fn ingest_frame(
        &mut self,
        frame: SseFrame,
    ) -> Result<Option<ChatCompletionChunk>, OpenAIError> {
        if frame.data.trim() == "[DONE]" {
            self.seen_done = true;
            return Ok(None);
        }

        let chunk = serde_json::from_str::<ChatCompletionChunk>(&frame.data).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to parse streamed chat completion chunk: {error}"),
            )
            .with_source(error)
        })?;
        self.apply_chunk(&chunk)?;
        Ok(Some(chunk))
    }

    fn apply_chunk(&mut self, chunk: &ChatCompletionChunk) -> Result<(), OpenAIError> {
        if self.id.is_none() {
            self.id = Some(chunk.id.clone());
        }
        if self.created.is_none() {
            self.created = Some(chunk.created);
        }
        if self.model.is_none() {
            self.model = chunk.model.clone();
        }
        if self.service_tier.is_none() {
            self.service_tier = chunk.service_tier.clone();
        }
        if self.system_fingerprint.is_none() {
            self.system_fingerprint = chunk.system_fingerprint.clone();
        }
        if chunk.usage.is_some() {
            self.usage = chunk.usage.clone();
        }

        for choice in &chunk.choices {
            while self.choices.len() <= choice.index {
                self.choices.push(AccumulatedChoice::default());
            }
            let entry = self.choices.get_mut(choice.index).ok_or_else(|| {
                OpenAIError::new(
                    ErrorKind::Validation,
                    format!(
                        "missing accumulated chat completion choice {}",
                        choice.index
                    ),
                )
            })?;

            if let Some(role) = &choice.delta.role {
                entry.message.role = Some(role.clone());
            }
            if let Some(content) = &choice.delta.content {
                entry
                    .message
                    .content
                    .get_or_insert_with(String::new)
                    .push_str(content);
            }
            if let Some(refusal) = &choice.delta.refusal {
                entry
                    .message
                    .refusal
                    .get_or_insert_with(String::new)
                    .push_str(refusal);
            }
            if let Some(function_call) = &choice.delta.function_call {
                let call = entry
                    .message
                    .function_call
                    .get_or_insert_with(Default::default);
                if let Some(name) = &function_call.name {
                    call.name = Some(name.clone());
                }
                if let Some(arguments) = &function_call.arguments {
                    call.arguments
                        .get_or_insert_with(String::new)
                        .push_str(arguments);
                }
            }
            for tool_call in &choice.delta.tool_calls {
                let index = tool_call.index.unwrap_or(entry.message.tool_calls.len());
                while entry.message.tool_calls.len() <= index {
                    entry
                        .message
                        .tool_calls
                        .push(ChatCompletionMessageToolCall::default());
                }
                let accumulated = entry.message.tool_calls.get_mut(index).ok_or_else(|| {
                    OpenAIError::new(
                        ErrorKind::Validation,
                        format!("missing accumulated tool call index {index}"),
                    )
                })?;
                accumulated.index = Some(index);
                if let Some(id) = &tool_call.id {
                    accumulated.id = Some(id.clone());
                }
                if let Some(tool_type) = &tool_call.tool_type {
                    accumulated.tool_type = Some(tool_type.clone());
                }
                if let Some(name) = &tool_call.function.name {
                    accumulated.function.name = Some(name.clone());
                }
                if let Some(arguments) = &tool_call.function.arguments {
                    accumulated
                        .function
                        .arguments
                        .get_or_insert_with(String::new)
                        .push_str(arguments);
                }
            }
            if let Some(finish_reason) = &choice.finish_reason {
                entry.finish_reason = Some(finish_reason.clone());
                self.seen_terminal_chunk = true;
            }
        }

        Ok(())
    }

    fn finish(self) -> Result<ChatCompletion, OpenAIError> {
        if !self.seen_terminal_chunk {
            return Err(OpenAIError::new(
                ErrorKind::Parse,
                "chat completion stream ended without a terminal chunk carrying finish_reason",
            ));
        }

        Ok(ChatCompletion {
            id: self.id.unwrap_or_default(),
            object: String::from("chat.completion"),
            created: self.created.unwrap_or_default(),
            model: self.model,
            service_tier: self.service_tier,
            system_fingerprint: self.system_fingerprint,
            choices: self
                .choices
                .into_iter()
                .enumerate()
                .map(|(index, choice)| ChatCompletionChoice {
                    index,
                    finish_reason: choice.finish_reason,
                    message: choice.message,
                    logprobs: None,
                    extra: BTreeMap::new(),
                })
                .collect(),
            usage: self.usage,
            extra: BTreeMap::new(),
        })
    }
}

#[derive(Clone, Debug, Default)]
struct AccumulatedChoice {
    message: ChatCompletionMessage,
    finish_reason: Option<ChatCompletionFinishReason>,
}

fn serialize_json_value<T>(label: &str, value: T) -> Result<Value, OpenAIError>
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Validation,
            format!("failed to serialize {label}: {error}"),
        )
        .with_source(error)
    })
}

fn validate_path_id<'a>(label: &str, value: &'a str) -> Result<&'a str, OpenAIError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(OpenAIError::new(
            ErrorKind::Validation,
            format!("{label} cannot be blank"),
        ));
    }
    Ok(trimmed)
}

fn append_query(path: &str, pairs: Vec<(String, String)>) -> String {
    if pairs.is_empty() {
        return path.to_string();
    }

    let query = pairs
        .into_iter()
        .map(|(key, value)| format!("{}={}", percent_encode(&key), percent_encode(&value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{path}?{query}")
}

fn percent_encode(value: &str) -> String {
    fn is_unreserved(byte: u8) -> bool {
        matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~')
    }

    let mut encoded = String::new();
    for byte in value.bytes() {
        if is_unreserved(byte) {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{:02X}", byte));
        }
    }
    encoded
}

fn map_live_transport_error(error: reqwest::Error) -> OpenAIError {
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Transport
    };
    OpenAIError::new(kind, error.to_string()).with_source(error)
}

async fn consume_live_stream(
    response: crate::core::transport::StreamingTextResponse,
    mut abort_rx: watch::Receiver<bool>,
    chunk_tx: mpsc::Sender<LiveChatCompletionMessage>,
    shared: Arc<LiveChatCompletionShared>,
) -> Result<(), OpenAIError> {
    let mut response = response.response;
    let mut parser = SseParser::default();
    let mut accumulator = ChatCompletionAccumulator::default();

    loop {
        tokio::select! {
            changed = abort_rx.changed() => {
                if changed.is_ok() && *abort_rx.borrow() {
                    let _ = chunk_tx.send(LiveChatCompletionMessage::Finished);
                    return Ok(());
                }
            }
            chunk = response.chunk() => {
                let chunk = chunk.map_err(map_live_transport_error)?;
                let Some(chunk) = chunk else {
                    break;
                };
                for frame in parser.push(chunk.as_ref())? {
                    if let Some(parsed) = accumulator.ingest_frame(frame)? {
                        if chunk_tx
                            .send(LiveChatCompletionMessage::Chunk(Box::new(parsed)))
                            .is_err()
                        {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    for frame in parser.finish()? {
        if let Some(parsed) = accumulator.ingest_frame(frame)? {
            if chunk_tx
                .send(LiveChatCompletionMessage::Chunk(Box::new(parsed)))
                .is_err()
            {
                return Ok(());
            }
        }
    }

    let final_completion = accumulator.finish()?;
    shared.finish_with_completion(final_completion);
    let _ = chunk_tx.send(LiveChatCompletionMessage::Finished);
    Ok(())
}
