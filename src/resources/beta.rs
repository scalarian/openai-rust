use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::{runtime::Builder, sync::watch};

use crate::{
    OpenAIError,
    core::{
        metadata::ResponseMetadata,
        request::{PreparedRequest, RequestOptions, ResolvedRequestOptions},
        response::ApiResponse,
        runtime::ClientRuntime,
        transport::{execute_json, execute_text_stream},
    },
    error::ErrorKind,
    helpers::sse::{SseFrame, SseParser},
    resources::{
        common::{ListOrder, ReasoningEffort},
        files::{encode_path_id, validate_path_id},
    },
};

const CHATKIT_BETA_HEADER: &str = "chatkit_beta=v1";
const ASSISTANTS_BETA_HEADER: &str = "assistants=v2";

macro_rules! beta_string_literal_enum {
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

        impl PartialEq<String> for $name {
            fn eq(&self, other: &String) -> bool {
                self.as_str() == other.as_str()
            }
        }

        impl PartialEq<$name> for String {
            fn eq(&self, other: &$name) -> bool {
                self.as_str() == other.as_str()
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

beta_string_literal_enum! {
    /// Role accepted when creating deprecated beta thread messages.
    pub enum BetaThreadMessageRole {
        User => "user",
        Assistant => "assistant",
    }
}

beta_string_literal_enum! {
    /// Truncation strategy type for deprecated beta thread runs.
    pub enum BetaTruncationStrategyType {
        Auto => "auto",
        LastMessages => "last_messages",
    }
}

beta_string_literal_enum! {
    /// Ranker identifiers accepted by deprecated beta assistant file-search tools.
    pub enum BetaAssistantFileSearchRanker {
        Auto => "auto",
        Default2024_08_21 => "default_2024_08_21",
    }
}

beta_string_literal_enum! {
    /// Additional beta Assistants run-step fields that can be included in responses.
    pub enum BetaThreadRunStepInclude {
        StepDetailsToolCallsFileSearchResultsContent => "step_details.tool_calls[*].file_search.results[*].content",
    }
}

beta_string_literal_enum! {
    /// ChatKit attachment discriminator.
    pub enum ChatKitAttachmentType {
        Image => "image",
        File => "file",
    }
}

beta_string_literal_enum! {
    /// ChatKit client tool-call status.
    pub enum ChatKitClientToolCallStatus {
        InProgress => "in_progress",
        Completed => "completed",
    }
}

beta_string_literal_enum! {
    /// ChatKit task subtype.
    pub enum ChatKitTaskType {
        Custom => "custom",
        Thought => "thought",
    }
}

/// Beta API family.
#[derive(Clone, Debug)]
pub struct Beta {
    runtime: Arc<ClientRuntime>,
}

impl Beta {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns the stable chat compatibility surface through the upstream beta alias.
    pub fn chat(&self) -> crate::resources::chat::Chat {
        crate::resources::chat::Chat::new(self.runtime.clone())
    }

    /// Returns beta realtime REST endpoints.
    pub fn realtime(&self) -> BetaRealtime {
        BetaRealtime::new(self.runtime.clone())
    }

    /// Returns deprecated beta Assistants endpoints.
    pub fn assistants(&self) -> BetaAssistants {
        BetaAssistants::new(self.runtime.clone())
    }

    /// Returns deprecated beta Threads endpoints.
    pub fn threads(&self) -> BetaThreads {
        BetaThreads::new(self.runtime.clone())
    }

    /// Returns ChatKit beta endpoints.
    pub fn chatkit(&self) -> ChatKit {
        ChatKit::new(self.runtime.clone())
    }
}

/// Deprecated beta Assistants API family.
#[derive(Clone, Debug)]
pub struct BetaAssistants {
    runtime: Arc<ClientRuntime>,
}

impl BetaAssistants {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates an assistant.
    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<Value>, OpenAIError> {
        assistants_beta_post_body(&self.runtime, "/assistants", &params)
    }

    /// Retrieves one assistant by id.
    pub fn retrieve(&self, assistant_id: &str) -> Result<ApiResponse<Value>, OpenAIError> {
        let assistant_id = path_id("assistant_id", assistant_id)?;
        assistants_beta_get(&self.runtime, format!("/assistants/{assistant_id}"))
    }

    /// Updates one assistant by id.
    pub fn update<B: Serialize>(
        &self,
        assistant_id: &str,
        params: B,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let assistant_id = path_id("assistant_id", assistant_id)?;
        assistants_beta_post_body(
            &self.runtime,
            format!("/assistants/{assistant_id}"),
            &params,
        )
    }

    /// Lists assistants.
    pub fn list(
        &self,
        params: impl Into<BetaQueryParams>,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        assistants_beta_get_query(&self.runtime, "/assistants", params.into().into_pairs())
    }

    /// Deletes one assistant by id.
    pub fn delete(&self, assistant_id: &str) -> Result<ApiResponse<Value>, OpenAIError> {
        let assistant_id = path_id("assistant_id", assistant_id)?;
        assistants_beta_delete(&self.runtime, format!("/assistants/{assistant_id}"))
    }
}

/// Deprecated beta assistant creation parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaAssistantCreateParams {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<BetaAssistantResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_resources: Option<BetaToolResources>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<BetaAssistantTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
}

/// Deprecated beta assistant update parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaAssistantUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<BetaAssistantResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_resources: Option<BetaToolResourceOverrides>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<BetaAssistantTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
}

/// Deprecated beta assistant response format selector.
#[derive(Clone, Debug, PartialEq)]
pub enum BetaAssistantResponseFormat {
    Auto,
    Text,
    JsonObject,
    JsonSchema(BetaAssistantResponseFormatJsonSchema),
}

impl Serialize for BetaAssistantResponseFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
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
                    json_schema: &'a BetaAssistantResponseFormatJsonSchema,
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

/// JSON-schema response format payload for deprecated beta assistants.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaAssistantResponseFormatJsonSchema {
    pub name: String,
    pub schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Deprecated beta assistant tool-choice selector.
#[derive(Clone, Debug, PartialEq)]
pub enum BetaAssistantToolChoice {
    None,
    Auto,
    Required,
    Function(BetaAssistantToolChoiceFunction),
    CodeInterpreter,
    FileSearch,
}

impl Serialize for BetaAssistantToolChoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
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
                    function: &'a BetaAssistantToolChoiceFunction,
                }

                FunctionChoice {
                    choice_type: "function",
                    function,
                }
                .serialize(serializer)
            }
            Self::CodeInterpreter => {
                #[derive(Serialize)]
                struct ToolChoice<'a> {
                    #[serde(rename = "type")]
                    choice_type: &'a str,
                }

                ToolChoice {
                    choice_type: "code_interpreter",
                }
                .serialize(serializer)
            }
            Self::FileSearch => {
                #[derive(Serialize)]
                struct ToolChoice<'a> {
                    #[serde(rename = "type")]
                    choice_type: &'a str,
                }

                ToolChoice {
                    choice_type: "file_search",
                }
                .serialize(serializer)
            }
        }
    }
}

/// Function name for a deprecated beta assistant named tool choice.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaAssistantToolChoiceFunction {
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Deprecated beta thread/run truncation strategy.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaTruncationStrategy {
    #[serde(rename = "type")]
    pub strategy_type: BetaTruncationStrategyType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_messages: Option<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl BetaTruncationStrategy {
    pub fn auto() -> Self {
        Self {
            strategy_type: BetaTruncationStrategyType::Auto,
            last_messages: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn last_messages(last_messages: u32) -> Self {
        Self {
            strategy_type: BetaTruncationStrategyType::LastMessages,
            last_messages: Some(last_messages),
            extra: BTreeMap::new(),
        }
    }
}

/// Tool definition accepted by deprecated beta assistants and runs.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BetaAssistantTool {
    CodeInterpreter,
    FileSearch {
        #[serde(skip_serializing_if = "Option::is_none")]
        file_search: Option<BetaAssistantFileSearchTool>,
    },
    Function {
        function: BetaAssistantFunctionDefinition,
    },
}

impl BetaAssistantTool {
    pub fn code_interpreter() -> Self {
        Self::CodeInterpreter
    }

    pub fn file_search(file_search: Option<BetaAssistantFileSearchTool>) -> Self {
        Self::FileSearch { file_search }
    }

    pub fn function(function: BetaAssistantFunctionDefinition) -> Self {
        Self::Function { function }
    }
}

/// File-search overrides for deprecated beta assistant tools.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaAssistantFileSearchTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_num_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking_options: Option<BetaAssistantFileSearchRankingOptions>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Ranking options for deprecated beta file-search tools.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BetaAssistantFileSearchRankingOptions {
    pub score_threshold: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranker: Option<BetaAssistantFileSearchRanker>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Function definition for deprecated beta assistant tools.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaAssistantFunctionDefinition {
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

/// Tool resources accepted when creating beta assistants or threads.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaToolResources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_interpreter: Option<BetaToolResourcesCodeInterpreter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_search: Option<BetaToolResourcesFileSearch>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Code-interpreter file resources.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaToolResourcesCodeInterpreter {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub file_ids: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// File-search resources accepted by create calls.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaToolResourcesFileSearch {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub vector_store_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub vector_stores: Vec<BetaToolResourcesVectorStore>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Inline vector-store creation helper for beta tool resources.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaToolResourcesVectorStore {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<BetaVectorStoreChunkingStrategy>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub file_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Vector-store chunking strategy for beta tool-resource helpers.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BetaVectorStoreChunkingStrategy {
    Auto,
    Static {
        #[serde(rename = "static")]
        static_config: BetaVectorStoreStaticChunkingStrategy,
    },
}

/// Static vector-store chunking configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BetaVectorStoreStaticChunkingStrategy {
    pub chunk_overlap_tokens: u32,
    pub max_chunk_size_tokens: u32,
}

/// Tool-resource overrides accepted by beta update and run top-level calls.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaToolResourceOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_interpreter: Option<BetaToolResourcesCodeInterpreter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_search: Option<BetaToolResourceFileSearchOverrides>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// File-search resource override list.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaToolResourceFileSearchOverrides {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub vector_store_ids: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Deprecated beta assistant list parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BetaAssistantListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

/// Deprecated beta Threads API family.
#[derive(Clone, Debug)]
pub struct BetaThreads {
    runtime: Arc<ClientRuntime>,
}

impl BetaThreads {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns thread message endpoints.
    pub fn messages(&self) -> BetaThreadMessages {
        BetaThreadMessages::new(self.runtime.clone())
    }

    /// Returns thread run endpoints.
    pub fn runs(&self) -> BetaThreadRuns {
        BetaThreadRuns::new(self.runtime.clone())
    }

    /// Creates an empty thread.
    pub fn create_empty(&self) -> Result<ApiResponse<Value>, OpenAIError> {
        let params = Value::Object(serde_json::Map::new());
        assistants_beta_post_body(&self.runtime, "/threads", &params)
    }

    /// Creates a thread.
    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<Value>, OpenAIError> {
        assistants_beta_post_body(&self.runtime, "/threads", &params)
    }

    /// Retrieves one thread by id.
    pub fn retrieve(&self, thread_id: &str) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        assistants_beta_get(&self.runtime, format!("/threads/{thread_id}"))
    }

    /// Updates one thread by id.
    pub fn update<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        assistants_beta_post_body(&self.runtime, format!("/threads/{thread_id}"), &params)
    }

    /// Deletes one thread by id.
    pub fn delete(&self, thread_id: &str) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        assistants_beta_delete(&self.runtime, format!("/threads/{thread_id}"))
    }

    /// Creates a thread and starts a run.
    pub fn create_and_run<B: Serialize>(
        &self,
        params: B,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        assistants_beta_post_body(&self.runtime, "/threads/runs", &params)
    }

    /// Creates a thread and starts a streamed run.
    pub fn create_and_run_stream<B: Serialize>(
        &self,
        params: B,
    ) -> Result<BetaAssistantStream, OpenAIError> {
        let body = body_with_stream_flag(params)?;
        assistants_beta_post_stream_value(
            &self.runtime,
            "/threads/runs",
            &body,
            Some("threads.create_and_run_stream"),
        )
    }

    /// Creates a thread, starts a run, and polls that run until it reaches a terminal state.
    pub fn create_and_run_poll<B: Serialize>(
        &self,
        params: B,
        options: BetaRunPollOptions,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let created = self.create_and_run(params)?;
        let run_id = value_string_field(&created.output, "id")?;
        let thread_id = value_string_field(&created.output, "thread_id")?;
        self.runs().poll(&thread_id, &run_id, options)
    }
}

/// Deprecated beta thread creation parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaThreadCreateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<BetaThreadMessageCreateParams>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_resources: Option<BetaToolResources>,
}

/// Deprecated beta thread update parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaThreadUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_resources: Option<BetaToolResourceOverrides>,
}

/// Deprecated beta create-thread-and-run parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaThreadCreateAndRunParams {
    pub assistant_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<BetaAssistantResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread: Option<BetaThreadCreateParams>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<BetaAssistantToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_resources: Option<BetaToolResourceOverrides>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<BetaAssistantTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation_strategy: Option<BetaTruncationStrategy>,
}

/// Deprecated beta thread message endpoints.
#[derive(Clone, Debug)]
pub struct BetaThreadMessages {
    runtime: Arc<ClientRuntime>,
}

impl BetaThreadMessages {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a message within a thread.
    pub fn create<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        assistants_beta_post_body(
            &self.runtime,
            format!("/threads/{thread_id}/messages"),
            &params,
        )
    }

    /// Retrieves one message within a thread.
    pub fn retrieve(
        &self,
        thread_id: &str,
        message_id: &str,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let message_id = path_id("message_id", message_id)?;
        assistants_beta_get(
            &self.runtime,
            format!("/threads/{thread_id}/messages/{message_id}"),
        )
    }

    /// Updates one message within a thread.
    pub fn update<B: Serialize>(
        &self,
        thread_id: &str,
        message_id: &str,
        params: B,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let message_id = path_id("message_id", message_id)?;
        assistants_beta_post_body(
            &self.runtime,
            format!("/threads/{thread_id}/messages/{message_id}"),
            &params,
        )
    }

    /// Lists messages within a thread.
    pub fn list(
        &self,
        thread_id: &str,
        params: impl Into<BetaQueryParams>,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        assistants_beta_get_query(
            &self.runtime,
            format!("/threads/{thread_id}/messages"),
            params.into().into_pairs(),
        )
    }

    /// Deletes one message within a thread.
    pub fn delete(
        &self,
        thread_id: &str,
        message_id: &str,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let message_id = path_id("message_id", message_id)?;
        assistants_beta_delete(
            &self.runtime,
            format!("/threads/{thread_id}/messages/{message_id}"),
        )
    }
}

/// Deprecated beta thread message creation parameters.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BetaThreadMessageCreateParams {
    pub role: BetaThreadMessageRole,
    pub content: BetaThreadMessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<BetaThreadMessageAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
}

/// Deprecated beta thread message content.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BetaThreadMessageContent {
    Text(String),
    Parts(Vec<BetaThreadMessageContentPart>),
}

impl From<String> for BetaThreadMessageContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for BetaThreadMessageContent {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<Vec<BetaThreadMessageContentPart>> for BetaThreadMessageContent {
    fn from(value: Vec<BetaThreadMessageContentPart>) -> Self {
        Self::Parts(value)
    }
}

/// Deprecated beta thread message multimodal content part.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BetaThreadMessageContentPart {
    Text {
        text: String,
    },
    ImageFile {
        image_file: BetaThreadMessageImageFile,
    },
    ImageUrl {
        image_url: BetaThreadMessageImageUrl,
    },
}

impl BetaThreadMessageContentPart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn image_file(
        file_id: impl Into<String>,
        detail: Option<BetaThreadMessageImageDetail>,
    ) -> Self {
        Self::ImageFile {
            image_file: BetaThreadMessageImageFile {
                file_id: file_id.into(),
                detail,
            },
        }
    }

    pub fn image_url(url: impl Into<String>, detail: Option<BetaThreadMessageImageDetail>) -> Self {
        Self::ImageUrl {
            image_url: BetaThreadMessageImageUrl {
                url: url.into(),
                detail,
            },
        }
    }
}

/// Deprecated beta thread image detail control.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetaThreadMessageImageDetail {
    Auto,
    Low,
    High,
}

/// Deprecated beta thread message image-file descriptor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BetaThreadMessageImageFile {
    pub file_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<BetaThreadMessageImageDetail>,
}

/// Deprecated beta thread message image-url descriptor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BetaThreadMessageImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<BetaThreadMessageImageDetail>,
}

/// Deprecated beta thread message file attachment.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BetaThreadMessageAttachment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<BetaThreadMessageAttachmentTool>,
}

/// Deprecated beta thread message attachment tool target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BetaThreadMessageAttachmentTool {
    CodeInterpreter,
    FileSearch,
}

/// Deprecated beta thread message update parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaThreadMessageUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
}

/// Deprecated beta thread message list parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BetaThreadMessageListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
    pub run_id: Option<String>,
}

/// Deprecated beta thread run endpoints.
#[derive(Clone, Debug)]
pub struct BetaThreadRuns {
    runtime: Arc<ClientRuntime>,
}

impl BetaThreadRuns {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns run step endpoints.
    pub fn steps(&self) -> BetaThreadRunSteps {
        BetaThreadRunSteps::new(self.runtime.clone())
    }

    /// Creates a run within a thread.
    pub fn create<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        self.create_with_query(thread_id, params, BetaQueryParams::default())
    }

    /// Creates a run within a thread with additional query parameters.
    pub fn create_with_query<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
        query: BetaQueryParams,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        assistants_beta_post_body(
            &self.runtime,
            path_with_query(format!("/threads/{thread_id}/runs"), query.into_pairs()),
            &params,
        )
    }

    /// Creates a streamed run within a thread.
    pub fn create_stream<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
    ) -> Result<BetaAssistantStream, OpenAIError> {
        self.create_stream_with_query(thread_id, params, BetaQueryParams::default())
    }

    /// Alias for upstream's `create_and_stream` helper.
    pub fn create_and_stream<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
    ) -> Result<BetaAssistantStream, OpenAIError> {
        self.create_stream(thread_id, params)
    }

    /// Creates a streamed run within a thread with additional query parameters.
    pub fn create_stream_with_query<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
        query: BetaQueryParams,
    ) -> Result<BetaAssistantStream, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let body = body_with_stream_flag(params)?;
        assistants_beta_post_stream_value(
            &self.runtime,
            path_with_query(format!("/threads/{thread_id}/runs"), query.into_pairs()),
            &body,
            Some("threads.runs.create_and_stream"),
        )
    }

    /// Alias for the upstream run stream helper.
    pub fn stream<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
    ) -> Result<BetaAssistantStream, OpenAIError> {
        self.create_stream_with_query(thread_id, params, BetaQueryParams::default())
    }

    /// Creates a run and polls it until it reaches a terminal state.
    pub fn create_and_poll<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
        options: BetaRunPollOptions,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        self.create_and_poll_with_query(thread_id, params, BetaQueryParams::default(), options)
    }

    /// Creates a run with query parameters and polls it until it reaches a terminal state.
    pub fn create_and_poll_with_query<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
        query: BetaQueryParams,
        options: BetaRunPollOptions,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let created = self.create_with_query(thread_id, params, query)?;
        let run_id = value_string_field(&created.output, "id")?;
        self.poll(thread_id, &run_id, options)
    }

    /// Retrieves one run within a thread.
    pub fn retrieve(
        &self,
        thread_id: &str,
        run_id: &str,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let run_id = path_id("run_id", run_id)?;
        assistants_beta_get(&self.runtime, format!("/threads/{thread_id}/runs/{run_id}"))
    }

    /// Updates one run within a thread.
    pub fn update<B: Serialize>(
        &self,
        thread_id: &str,
        run_id: &str,
        params: B,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let run_id = path_id("run_id", run_id)?;
        assistants_beta_post_body(
            &self.runtime,
            format!("/threads/{thread_id}/runs/{run_id}"),
            &params,
        )
    }

    /// Lists runs within a thread.
    pub fn list(
        &self,
        thread_id: &str,
        params: impl Into<BetaQueryParams>,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        assistants_beta_get_query(
            &self.runtime,
            format!("/threads/{thread_id}/runs"),
            params.into().into_pairs(),
        )
    }

    /// Cancels one run within a thread.
    pub fn cancel(&self, thread_id: &str, run_id: &str) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let run_id = path_id("run_id", run_id)?;
        assistants_beta_post(
            &self.runtime,
            format!("/threads/{thread_id}/runs/{run_id}/cancel"),
        )
    }

    /// Submits tool outputs for a run.
    pub fn submit_tool_outputs<B: Serialize>(
        &self,
        thread_id: &str,
        run_id: &str,
        params: B,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let run_id = path_id("run_id", run_id)?;
        assistants_beta_post_body(
            &self.runtime,
            format!("/threads/{thread_id}/runs/{run_id}/submit_tool_outputs"),
            &params,
        )
    }

    /// Submits tool outputs for a run and streams subsequent run events.
    pub fn submit_tool_outputs_stream<B: Serialize>(
        &self,
        thread_id: &str,
        run_id: &str,
        params: B,
    ) -> Result<BetaAssistantStream, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let run_id = path_id("run_id", run_id)?;
        let body = body_with_stream_flag(params)?;
        assistants_beta_post_stream_value(
            &self.runtime,
            format!("/threads/{thread_id}/runs/{run_id}/submit_tool_outputs"),
            &body,
            Some("threads.runs.submit_tool_outputs_stream"),
        )
    }

    /// Submits tool outputs and polls the run until it reaches a terminal state.
    pub fn submit_tool_outputs_and_poll<B: Serialize>(
        &self,
        thread_id: &str,
        run_id: &str,
        params: B,
        options: BetaRunPollOptions,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let submitted = self.submit_tool_outputs(thread_id, run_id, params)?;
        let run_id = value_string_field(&submitted.output, "id").unwrap_or_else(|_| run_id.into());
        self.poll(thread_id, &run_id, options)
    }

    /// Polls a run until it reaches a terminal Assistants status.
    pub fn poll(
        &self,
        thread_id: &str,
        run_id: &str,
        options: BetaRunPollOptions,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let started_at = Instant::now();
        loop {
            let response = self.retrieve(thread_id, run_id)?;
            if is_terminal_run_status(&response.output) {
                return Ok(response);
            }

            let sleep_interval = poll_interval(&response, &options)?;
            let elapsed = started_at.elapsed();
            if elapsed > options.max_wait || elapsed + sleep_interval > options.max_wait {
                return Err(OpenAIError::new(
                    ErrorKind::Timeout,
                    "beta Assistants run polling exceeded max_wait",
                ));
            }
            thread::sleep(sleep_interval);
        }
    }
}

/// Deprecated beta thread run creation parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaThreadRunCreateParams {
    pub assistant_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_messages: Option<Vec<BetaThreadRunAdditionalMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<BetaAssistantResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<BetaAssistantToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<BetaAssistantTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation_strategy: Option<BetaTruncationStrategy>,
}

/// Additional message to add before creating a deprecated beta thread run.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BetaThreadRunAdditionalMessage {
    pub role: BetaThreadMessageRole,
    pub content: BetaThreadMessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<BetaThreadMessageAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
}

/// Deprecated beta thread run update parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaThreadRunUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
}

/// Deprecated beta thread run list parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BetaThreadRunListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

/// Deprecated beta run tool output payload.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BetaThreadRunToolOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Deprecated beta submit-tool-outputs parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BetaThreadRunSubmitToolOutputsParams {
    pub tool_outputs: Vec<BetaThreadRunToolOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Deprecated beta run step endpoints.
#[derive(Clone, Debug)]
pub struct BetaThreadRunSteps {
    runtime: Arc<ClientRuntime>,
}

impl BetaThreadRunSteps {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Retrieves one run step.
    pub fn retrieve(
        &self,
        thread_id: &str,
        run_id: &str,
        step_id: &str,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        self.retrieve_with_query(thread_id, run_id, step_id, BetaQueryParams::default())
    }

    /// Retrieves one run step with additional query parameters.
    pub fn retrieve_with_query(
        &self,
        thread_id: &str,
        run_id: &str,
        step_id: &str,
        query: impl Into<BetaQueryParams>,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let run_id = path_id("run_id", run_id)?;
        let step_id = path_id("step_id", step_id)?;
        assistants_beta_get_query(
            &self.runtime,
            format!("/threads/{thread_id}/runs/{run_id}/steps/{step_id}"),
            query.into().into_pairs(),
        )
    }

    /// Lists run steps.
    pub fn list(
        &self,
        thread_id: &str,
        run_id: &str,
        params: impl Into<BetaQueryParams>,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let run_id = path_id("run_id", run_id)?;
        assistants_beta_get_query(
            &self.runtime,
            format!("/threads/{thread_id}/runs/{run_id}/steps"),
            params.into().into_pairs(),
        )
    }
}

/// Deprecated beta run-step retrieve query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BetaThreadRunStepRetrieveParams {
    pub include: Option<Vec<BetaThreadRunStepInclude>>,
}

/// Deprecated beta run-step list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BetaThreadRunStepListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub include: Option<Vec<BetaThreadRunStepInclude>>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

/// Flexible query parameters for deprecated beta endpoints.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BetaQueryParams {
    pairs: Vec<(String, String)>,
}

impl BetaQueryParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.pairs.push((key.into(), value.to_string()));
        self
    }

    pub fn push_opt<T: ToString>(mut self, key: impl Into<String>, value: Option<T>) -> Self {
        if let Some(value) = value {
            self.pairs.push((key.into(), value.to_string()));
        }
        self
    }

    pub fn push_repeated<T, I>(mut self, key: impl Into<String>, values: I) -> Self
    where
        T: ToString,
        I: IntoIterator<Item = T>,
    {
        let key = key.into();
        for value in values {
            self.pairs.push((key.clone(), value.to_string()));
        }
        self
    }

    pub fn push_array<T, I>(mut self, key: impl Into<String>, values: I) -> Self
    where
        T: ToString,
        I: IntoIterator<Item = T>,
    {
        let key = format!("{}[]", key.into());
        for value in values {
            self.pairs.push((key.clone(), value.to_string()));
        }
        self
    }

    fn into_pairs(self) -> Vec<(String, String)> {
        self.pairs
    }
}

impl<K, V, const N: usize> From<[(K, V); N]> for BetaQueryParams
where
    K: Into<String>,
    V: ToString,
{
    fn from(value: [(K, V); N]) -> Self {
        let mut params = Self::new();
        for (key, value) in value {
            params = params.push(key, value);
        }
        params
    }
}

impl From<BetaAssistantListParams> for BetaQueryParams {
    fn from(value: BetaAssistantListParams) -> Self {
        Self::new()
            .push_opt("after", value.after)
            .push_opt("before", value.before)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

impl From<BetaThreadMessageListParams> for BetaQueryParams {
    fn from(value: BetaThreadMessageListParams) -> Self {
        Self::new()
            .push_opt("after", value.after)
            .push_opt("before", value.before)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
            .push_opt("run_id", value.run_id)
    }
}

impl From<BetaThreadRunListParams> for BetaQueryParams {
    fn from(value: BetaThreadRunListParams) -> Self {
        Self::new()
            .push_opt("after", value.after)
            .push_opt("before", value.before)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

impl From<BetaThreadRunStepRetrieveParams> for BetaQueryParams {
    fn from(value: BetaThreadRunStepRetrieveParams) -> Self {
        let mut params = Self::new();
        if let Some(include) = value.include {
            params = params.push_array("include", include);
        }
        params
    }
}

impl From<BetaThreadRunStepListParams> for BetaQueryParams {
    fn from(value: BetaThreadRunStepListParams) -> Self {
        let mut params = Self::new()
            .push_opt("after", value.after)
            .push_opt("before", value.before)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order);
        if let Some(include) = value.include {
            params = params.push_array("include", include);
        }
        params
    }
}

/// Polling options for deprecated beta Assistants run helpers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BetaRunPollOptions {
    pub poll_interval: Option<Duration>,
    pub max_wait: Duration,
}

impl Default for BetaRunPollOptions {
    fn default() -> Self {
        Self {
            poll_interval: None,
            max_wait: Duration::from_secs(600),
        }
    }
}

/// One raw Assistants stream event.
#[derive(Clone, Debug, PartialEq)]
pub struct BetaAssistantStreamEvent {
    pub event: Option<String>,
    pub data: Value,
    pub raw_data: String,
}

/// Stream of raw Assistants SSE events.
#[derive(Debug)]
pub struct BetaAssistantStream {
    metadata: ResponseMetadata,
    events: VecDeque<BetaAssistantStreamEvent>,
    live: Option<LiveBetaAssistantStreamHandle>,
    aborted: bool,
}

impl BetaAssistantStream {
    pub fn from_sse_chunks<I, B>(metadata: ResponseMetadata, chunks: I) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        let mut parser = SseParser::default();
        let mut events = VecDeque::new();
        let mut seen_done = false;

        for chunk in chunks {
            for frame in parser.push(chunk.as_ref().as_bytes())? {
                if let Some(event) = parse_beta_assistant_frame(frame, &mut seen_done)? {
                    events.push_back(event);
                }
            }
        }
        for frame in parser.finish()? {
            if let Some(event) = parse_beta_assistant_frame(frame, &mut seen_done)? {
                events.push_back(event);
            }
        }
        if !seen_done {
            return Err(OpenAIError::new(
                ErrorKind::Transport,
                "beta Assistants stream ended before [DONE]",
            ));
        }

        Ok(Self {
            metadata,
            events,
            live: None,
            aborted: false,
        })
    }

    fn start_live(
        request: PreparedRequest,
        options: ResolvedRequestOptions,
    ) -> Result<Self, OpenAIError> {
        let (startup_tx, startup_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let (abort_tx, abort_rx) = watch::channel(false);

        let worker = thread::spawn(move || {
            let runtime = match Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    let error = OpenAIError::new(
                        ErrorKind::Transport,
                        format!("failed to build beta Assistants stream runtime: {error}"),
                    )
                    .with_source(error);
                    let _ = startup_tx.send(Err(error.clone()));
                    let _ = event_tx.send(LiveBetaAssistantStreamMessage::Error(error));
                    return;
                }
            };

            runtime.block_on(async move {
                match execute_text_stream(&request, &options).await {
                    Ok(response) => {
                        let _ = startup_tx.send(Ok(response.metadata.clone()));
                        if let Err(error) =
                            consume_beta_assistant_live_stream(response, abort_rx, event_tx.clone())
                                .await
                        {
                            let _ = event_tx.send(LiveBetaAssistantStreamMessage::Error(error));
                        }
                    }
                    Err(error) => {
                        let _ = startup_tx.send(Err(error.clone()));
                        let _ = event_tx.send(LiveBetaAssistantStreamMessage::Error(error));
                    }
                }
            });
        });

        let metadata = startup_rx.recv().map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("beta Assistants stream worker exited before startup completed: {error}"),
            )
        })??;

        Ok(Self {
            metadata,
            events: VecDeque::new(),
            live: Some(LiveBetaAssistantStreamHandle {
                receiver: event_rx,
                abort: abort_tx,
                worker: Some(worker),
            }),
            aborted: false,
        })
    }

    pub fn next_event(&mut self) -> Result<Option<BetaAssistantStreamEvent>, OpenAIError> {
        if self.aborted {
            return Ok(None);
        }
        if self.events.is_empty() {
            self.fill_from_live()?;
        }
        Ok(self.events.pop_front())
    }

    pub async fn next_event_async(
        &mut self,
    ) -> Result<Option<BetaAssistantStreamEvent>, OpenAIError> {
        self.next_event()
    }

    pub fn abort(&mut self) {
        self.aborted = true;
        if let Some(live) = &mut self.live {
            let _ = live.abort.send(true);
            live.join_worker();
        }
        self.live = None;
        self.events.clear();
    }

    pub fn metadata(&self) -> &ResponseMetadata {
        &self.metadata
    }

    fn fill_from_live(&mut self) -> Result<(), OpenAIError> {
        let Some(live) = self.live.as_ref() else {
            return Ok(());
        };

        let Some(message) = live.receiver.recv().ok() else {
            if let Some(mut live) = self.live.take() {
                live.join_worker();
            }
            return Ok(());
        };
        self.process_live_message(message)?;

        while let Some(live) = self.live.as_ref() {
            match live.receiver.try_recv() {
                Ok(message) => self.process_live_message(message)?,
                Err(_) => break,
            }
        }
        Ok(())
    }

    fn process_live_message(
        &mut self,
        message: LiveBetaAssistantStreamMessage,
    ) -> Result<(), OpenAIError> {
        match message {
            LiveBetaAssistantStreamMessage::Event(event) => self.events.push_back(event),
            LiveBetaAssistantStreamMessage::Finished => {
                if let Some(mut live) = self.live.take() {
                    live.join_worker();
                }
            }
            LiveBetaAssistantStreamMessage::Error(error) => {
                if let Some(mut live) = self.live.take() {
                    live.join_worker();
                }
                return Err(error);
            }
        }
        Ok(())
    }
}

impl Drop for BetaAssistantStream {
    fn drop(&mut self) {
        if let Some(live) = &mut self.live {
            let _ = live.abort.send(true);
            live.join_worker();
        }
    }
}

#[derive(Debug)]
enum LiveBetaAssistantStreamMessage {
    Event(BetaAssistantStreamEvent),
    Finished,
    Error(OpenAIError),
}

#[derive(Debug)]
struct LiveBetaAssistantStreamHandle {
    receiver: mpsc::Receiver<LiveBetaAssistantStreamMessage>,
    abort: watch::Sender<bool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl LiveBetaAssistantStreamHandle {
    fn join_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// ChatKit beta API family.
#[derive(Clone, Debug)]
pub struct ChatKit {
    runtime: Arc<ClientRuntime>,
}

/// Beta realtime REST API family.
#[derive(Clone, Debug)]
pub struct BetaRealtime {
    runtime: Arc<ClientRuntime>,
}

impl BetaRealtime {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns beta realtime session-token endpoints.
    pub fn sessions(&self) -> BetaRealtimeSessions {
        BetaRealtimeSessions::new(self.runtime.clone())
    }

    /// Returns beta realtime transcription-session token endpoints.
    pub fn transcription_sessions(&self) -> BetaRealtimeTranscriptionSessions {
        BetaRealtimeTranscriptionSessions::new(self.runtime.clone())
    }
}

/// Beta realtime session-token endpoints.
#[derive(Clone, Debug)]
pub struct BetaRealtimeSessions {
    runtime: Arc<ClientRuntime>,
}

impl BetaRealtimeSessions {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates an ephemeral Realtime API token with session configuration.
    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<Value>, OpenAIError> {
        realtime_beta_post_body(&self.runtime, "/realtime/sessions", &params)
    }
}

/// Beta realtime transcription-session token endpoints.
#[derive(Clone, Debug)]
pub struct BetaRealtimeTranscriptionSessions {
    runtime: Arc<ClientRuntime>,
}

impl BetaRealtimeTranscriptionSessions {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates an ephemeral Realtime transcription API token.
    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<Value>, OpenAIError> {
        realtime_beta_post_body(&self.runtime, "/realtime/transcription_sessions", &params)
    }
}

/// Beta realtime session-token creation parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeSessionCreateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<BetaRealtimeClientSecret>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_format: Option<BetaRealtimeAudioFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_noise_reduction:
        Option<BetaRealtimeNullable<BetaRealtimeInputAudioNoiseReduction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_transcription:
        Option<BetaRealtimeNullable<BetaRealtimeInputAudioTranscription>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_response_output_tokens: Option<BetaRealtimeMaxResponseOutputTokens>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<BetaRealtimeModality>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<BetaRealtimeSessionModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_audio_format: Option<BetaRealtimeAudioFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<BetaRealtimeTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracing: Option<BetaRealtimeNullable<BetaRealtimeTracing>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_detection: Option<BetaRealtimeNullable<BetaRealtimeTurnDetection>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
}

/// Beta realtime transcription-session token creation parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeTranscriptionSessionCreateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<BetaRealtimeTranscriptionClientSecret>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_format: Option<BetaRealtimeAudioFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_noise_reduction:
        Option<BetaRealtimeNullable<BetaRealtimeInputAudioNoiseReduction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_transcription:
        Option<BetaRealtimeNullable<BetaRealtimeInputAudioTranscription>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<BetaRealtimeModality>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_detection: Option<BetaRealtimeNullable<BetaRealtimeTurnDetection>>,
}

/// Nullable realtime config slot used when `null` disables an active config.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BetaRealtimeNullable<T> {
    Value(T),
    Null,
}

impl<T> From<T> for BetaRealtimeNullable<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

/// Realtime audio wire formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetaRealtimeAudioFormat {
    Pcm16,
    G711Ulaw,
    G711Alaw,
}

/// Realtime response modalities.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetaRealtimeModality {
    Text,
    Audio,
}

beta_string_literal_enum! {
    /// Beta realtime session models accepted by session creation.
    pub enum BetaRealtimeSessionModel {
        GptRealtime => "gpt-realtime",
        GptRealtime2025_08_28 => "gpt-realtime-2025-08-28",
        Gpt4oRealtimePreview => "gpt-4o-realtime-preview",
        Gpt4oRealtimePreview2024_10_01 => "gpt-4o-realtime-preview-2024-10-01",
        Gpt4oRealtimePreview2024_12_17 => "gpt-4o-realtime-preview-2024-12-17",
        Gpt4oRealtimePreview2025_06_03 => "gpt-4o-realtime-preview-2025-06-03",
        Gpt4oMiniRealtimePreview => "gpt-4o-mini-realtime-preview",
        Gpt4oMiniRealtimePreview2024_12_17 => "gpt-4o-mini-realtime-preview-2024-12-17",
    }
}

/// Realtime client-secret creation options.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeClientSecret {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<BetaRealtimeClientSecretExpiresAfter>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Realtime transcription client-secret creation options.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeTranscriptionClientSecret {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<BetaRealtimeClientSecretExpiresAt>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Realtime client-secret expiration relative to token creation.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeClientSecretExpiresAfter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<BetaRealtimeClientSecretAnchor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Realtime transcription client-secret expiration relative to token creation.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeClientSecretExpiresAt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<BetaRealtimeClientSecretAnchor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Realtime client-secret expiration anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetaRealtimeClientSecretAnchor {
    CreatedAt,
}

/// Realtime input-audio noise-reduction config.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeInputAudioNoiseReduction {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub noise_reduction_type: Option<BetaRealtimeNoiseReductionType>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Realtime input-audio noise-reduction modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetaRealtimeNoiseReductionType {
    NearField,
    FarField,
}

/// Realtime input-audio transcription config.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeInputAudioTranscription {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<BetaRealtimeInputAudioTranscriptionModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

beta_string_literal_enum! {
    /// Models accepted by beta realtime input-audio transcription.
    pub enum BetaRealtimeInputAudioTranscriptionModel {
        Gpt4oTranscribe => "gpt-4o-transcribe",
        Gpt4oMiniTranscribe => "gpt-4o-mini-transcribe",
        Whisper1 => "whisper-1",
    }
}

/// Realtime max response output token limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BetaRealtimeMaxResponseOutputTokens {
    Tokens(u64),
    Inf,
}

impl Serialize for BetaRealtimeMaxResponseOutputTokens {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Tokens(tokens) => serializer.serialize_u64(*tokens),
            Self::Inf => serializer.serialize_str("inf"),
        }
    }
}

/// Realtime function tool definition.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<BetaRealtimeToolType>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Realtime tool kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetaRealtimeToolType {
    Function,
}

/// Realtime tracing config.
#[derive(Clone, Debug, PartialEq)]
pub enum BetaRealtimeTracing {
    Auto,
    Configuration(BetaRealtimeTracingConfiguration),
}

impl Serialize for BetaRealtimeTracing {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Configuration(config) => config.serialize(serializer),
        }
    }
}

/// Realtime tracing configuration object.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeTracingConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Realtime server or semantic VAD turn-detection config.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BetaRealtimeTurnDetection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_response: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eagerness: Option<BetaRealtimeTurnDetectionEagerness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt_response: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub turn_detection_type: Option<BetaRealtimeTurnDetectionType>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Realtime turn-detection mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetaRealtimeTurnDetectionType {
    ServerVad,
    SemanticVad,
}

/// Realtime semantic VAD eagerness.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetaRealtimeTurnDetectionEagerness {
    Low,
    Medium,
    High,
    Auto,
}

impl ChatKit {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn sessions(&self) -> ChatKitSessions {
        ChatKitSessions::new(self.runtime.clone())
    }

    pub fn threads(&self) -> ChatKitThreads {
        ChatKitThreads::new(self.runtime.clone())
    }
}

/// ChatKit session endpoints.
#[derive(Clone, Debug)]
pub struct ChatKitSessions {
    runtime: Arc<ClientRuntime>,
}

impl ChatKitSessions {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a ChatKit session.
    pub fn create(
        &self,
        params: ChatKitSessionCreateParams,
    ) -> Result<ApiResponse<ChatKitSession>, OpenAIError> {
        chatkit_post_body(&self.runtime, "/chatkit/sessions", &params)
    }

    /// Cancels an active ChatKit session.
    pub fn cancel(&self, session_id: &str) -> Result<ApiResponse<ChatKitSession>, OpenAIError> {
        let session_id = path_id("session_id", session_id)?;
        chatkit_post(
            &self.runtime,
            format!("/chatkit/sessions/{session_id}/cancel"),
        )
    }
}

/// ChatKit thread endpoints.
#[derive(Clone, Debug)]
pub struct ChatKitThreads {
    runtime: Arc<ClientRuntime>,
}

impl ChatKitThreads {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Retrieves one ChatKit thread by id.
    pub fn retrieve(&self, thread_id: &str) -> Result<ApiResponse<ChatKitThread>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        chatkit_get(&self.runtime, format!("/chatkit/threads/{thread_id}"))
    }

    /// Lists ChatKit threads with cursor pagination and optional user filtering.
    pub fn list(
        &self,
        params: ChatKitThreadListParams,
    ) -> Result<ApiResponse<ChatKitThreadPage>, OpenAIError> {
        chatkit_get_query(&self.runtime, "/chatkit/threads", params.into_query())
    }

    /// Deletes a ChatKit thread and its stored items.
    pub fn delete(
        &self,
        thread_id: &str,
    ) -> Result<ApiResponse<ChatKitThreadDeleteResponse>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        chatkit_delete(&self.runtime, format!("/chatkit/threads/{thread_id}"))
    }

    /// Lists items within one ChatKit thread.
    pub fn list_items(
        &self,
        thread_id: &str,
        params: ChatKitThreadItemListParams,
    ) -> Result<ApiResponse<ChatKitThreadItemPage>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        chatkit_get_query(
            &self.runtime,
            format!("/chatkit/threads/{thread_id}/items"),
            params.into_query(),
        )
    }
}

/// Create-session request body.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatKitSessionCreateParams {
    pub user: String,
    pub workflow: ChatKitSessionWorkflowParam,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chatkit_configuration: Option<ChatKitConfigurationParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<ChatKitSessionExpiresAfterParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limits: Option<ChatKitSessionRateLimitsParam>,
}

/// Workflow reference and optional workflow invocation overrides.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatKitSessionWorkflowParam {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_variables: Option<BTreeMap<String, ChatKitStateValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracing: Option<ChatKitWorkflowTracingParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Primitive ChatKit workflow state variable value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatKitStateValue {
    String(String),
    Bool(bool),
    Number(f64),
}

/// Per-session workflow tracing override.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatKitWorkflowTracingParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Controls when a ChatKit session expires.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatKitSessionExpiresAfterParam {
    pub anchor: ChatKitSessionExpiryAnchor,
    pub seconds: u64,
}

/// Supported ChatKit session expiration anchors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatKitSessionExpiryAnchor {
    CreatedAt,
}

/// Per-session ChatKit rate limit overrides.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatKitSessionRateLimitsParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_requests_per_1_minute: Option<u32>,
}

/// Per-session ChatKit feature configuration overrides.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatKitConfigurationParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_thread_titling: Option<ChatKitAutomaticThreadTitlingParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_upload: Option<ChatKitFileUploadParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<ChatKitHistoryParam>,
}

/// Automatic thread-title generation configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatKitAutomaticThreadTitlingParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// ChatKit upload configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatKitFileUploadParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_files: Option<u32>,
}

/// ChatKit history-retention configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatKitHistoryParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_threads: Option<u32>,
}

/// Workflow metadata returned for a ChatKit session.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitWorkflow {
    pub id: String,
    #[serde(default)]
    pub state_variables: Option<BTreeMap<String, ChatKitStateValue>>,
    pub tracing: ChatKitWorkflowTracing,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Resolved workflow tracing settings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ChatKitWorkflowTracing {
    pub enabled: bool,
}

/// Resolved ChatKit feature configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ChatKitConfiguration {
    pub automatic_thread_titling: ChatKitAutomaticThreadTitling,
    pub file_upload: ChatKitFileUpload,
    pub history: ChatKitHistory,
}

/// Resolved automatic thread-title generation configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ChatKitAutomaticThreadTitling {
    pub enabled: bool,
}

/// Resolved ChatKit upload configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ChatKitFileUpload {
    pub enabled: bool,
    #[serde(default)]
    pub max_file_size: Option<u64>,
    #[serde(default)]
    pub max_files: Option<u32>,
}

/// Resolved ChatKit history-retention configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ChatKitHistory {
    pub enabled: bool,
    #[serde(default)]
    pub recent_threads: Option<u32>,
}

/// ChatKit session resource.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitSession {
    pub id: String,
    pub object: String,
    pub client_secret: String,
    pub expires_at: u64,
    pub max_requests_per_1_minute: u32,
    pub status: ChatKitSessionStatus,
    pub user: String,
    pub workflow: ChatKitWorkflow,
    pub chatkit_configuration: ChatKitConfiguration,
    pub rate_limits: ChatKitSessionRateLimits,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// ChatKit session lifecycle status.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatKitSessionStatus {
    Active,
    Expired,
    Cancelled,
}

/// Resolved ChatKit session rate limit fields.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatKitSessionRateLimits {
    pub max_requests_per_1_minute: u32,
}

/// Thread list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChatKitThreadListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ChatKitOrder>,
    pub user: Option<String>,
}

impl ChatKitThreadListParams {
    fn into_query(self) -> Vec<(String, String)> {
        let mut query = Vec::new();
        push_opt(&mut query, "after", self.after);
        push_opt(&mut query, "before", self.before);
        push_opt(&mut query, "limit", self.limit);
        if let Some(order) = self.order {
            query.push((String::from("order"), order.as_str().to_string()));
        }
        push_opt(&mut query, "user", self.user);
        query
    }
}

/// Thread item list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChatKitThreadItemListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ChatKitOrder>,
}

impl ChatKitThreadItemListParams {
    fn into_query(self) -> Vec<(String, String)> {
        let mut query = Vec::new();
        push_opt(&mut query, "after", self.after);
        push_opt(&mut query, "before", self.before);
        push_opt(&mut query, "limit", self.limit);
        if let Some(order) = self.order {
            query.push((String::from("order"), order.as_str().to_string()));
        }
        query
    }
}

/// ChatKit list sort order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatKitOrder {
    Asc,
    Desc,
}

impl ChatKitOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// ChatKit thread resource.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitThread {
    pub id: String,
    pub object: String,
    pub created_at: u64,
    pub status: ChatKitThreadStatus,
    #[serde(default)]
    pub title: Option<String>,
    pub user: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// ChatKit thread status.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ChatKitThreadStatus {
    Active,
    Locked {
        #[serde(default)]
        reason: Option<String>,
    },
    Closed {
        #[serde(default)]
        reason: Option<String>,
    },
}

/// ChatKit thread list page.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitThreadPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<ChatKitThread>,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatKitThreadPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        self.last_id.as_deref().filter(|_| self.has_more)
    }
}

/// ChatKit thread item variants.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ChatKitThreadItem {
    #[serde(rename = "chatkit.user_message")]
    UserMessage {
        id: String,
        object: String,
        created_at: u64,
        thread_id: String,
        #[serde(default)]
        attachments: Vec<ChatKitAttachment>,
        content: Vec<ChatKitUserMessageContent>,
        #[serde(default)]
        inference_options: Option<ChatKitInferenceOptions>,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    #[serde(rename = "chatkit.assistant_message")]
    AssistantMessage {
        id: String,
        object: String,
        created_at: u64,
        thread_id: String,
        content: Vec<ChatKitResponseOutputText>,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    #[serde(rename = "chatkit.widget")]
    Widget {
        id: String,
        object: String,
        created_at: u64,
        thread_id: String,
        widget: String,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    #[serde(rename = "chatkit.client_tool_call")]
    ClientToolCall {
        id: String,
        object: String,
        created_at: u64,
        thread_id: String,
        arguments: String,
        call_id: String,
        name: String,
        #[serde(default)]
        output: Option<String>,
        status: ChatKitClientToolCallStatus,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    #[serde(rename = "chatkit.task")]
    Task {
        id: String,
        object: String,
        created_at: u64,
        thread_id: String,
        #[serde(default)]
        heading: Option<String>,
        #[serde(default)]
        summary: Option<String>,
        task_type: ChatKitTaskType,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    #[serde(rename = "chatkit.task_group")]
    TaskGroup {
        id: String,
        object: String,
        created_at: u64,
        thread_id: String,
        tasks: Vec<ChatKitTaskGroupTask>,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
}

/// ChatKit attachment metadata included on thread items.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitAttachment {
    pub id: String,
    pub mime_type: String,
    pub name: String,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(rename = "type")]
    pub attachment_type: ChatKitAttachmentType,
}

/// User-authored ChatKit message content.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ChatKitUserMessageContent {
    #[serde(rename = "input_text")]
    InputText { text: String },
    #[serde(rename = "quoted_text")]
    QuotedText { text: String },
}

/// ChatKit inference overrides applied to a user message.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitInferenceOptions {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tool_choice: Option<ChatKitInferenceToolChoice>,
}

/// Preferred ChatKit tool choice.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitInferenceToolChoice {
    pub id: String,
}

/// Assistant response text with annotations.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitResponseOutputText {
    #[serde(default)]
    pub annotations: Vec<ChatKitResponseAnnotation>,
    pub text: String,
}

/// Annotation attached to ChatKit response text.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ChatKitResponseAnnotation {
    #[serde(rename = "file")]
    File { source: ChatKitFileAnnotationSource },
    #[serde(rename = "url")]
    Url { source: ChatKitUrlAnnotationSource },
}

/// File annotation source.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitFileAnnotationSource {
    pub filename: String,
}

/// URL annotation source.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitUrlAnnotationSource {
    pub url: String,
}

/// Task entry within a ChatKit task group.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitTaskGroupTask {
    #[serde(default)]
    pub heading: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub task_type: ChatKitTaskType,
}

/// ChatKit thread item list page.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitThreadItemPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<ChatKitThreadItem>,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatKitThreadItemPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        self.last_id.as_deref().filter(|_| self.has_more)
    }
}

/// ChatKit thread deletion response.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatKitThreadDeleteResponse {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub deleted: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn push_opt<T: ToString>(query: &mut Vec<(String, String)>, key: &str, value: Option<T>) {
    if let Some(value) = value {
        query.push((key.to_string(), value.to_string()));
    }
}

fn path_id(name: &str, value: &str) -> Result<String, OpenAIError> {
    Ok(encode_path_id(validate_path_id(name, value)?))
}

fn path_with_query(base: impl Into<String>, query: Vec<(String, String)>) -> String {
    let base = base.into();
    if query.is_empty() {
        return base;
    }

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in query {
        serializer.append_pair(&key, &value);
    }
    format!("{base}?{}", serializer.finish())
}

fn chatkit_get<T>(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
) -> Result<ApiResponse<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    chatkit_execute(runtime, "GET", path, None)
}

fn chatkit_get_query<T>(
    runtime: &ClientRuntime,
    base: impl Into<String>,
    query: Vec<(String, String)>,
) -> Result<ApiResponse<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    chatkit_get(runtime, path_with_query(base, query))
}

fn chatkit_post<T>(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
) -> Result<ApiResponse<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    chatkit_execute(runtime, "POST", path, None)
}

fn chatkit_post_body<B, T>(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
    body: &B,
) -> Result<ApiResponse<T>, OpenAIError>
where
    B: Serialize,
    T: DeserializeOwned,
{
    let mut request = runtime.prepare_json_request("POST", path, body)?;
    request.headers.insert(
        String::from("openai-beta"),
        String::from(CHATKIT_BETA_HEADER),
    );
    let options = runtime.resolve_request_options(&RequestOptions::default())?;
    execute_json(&request, &options)
}

fn assistants_beta_post_stream_value(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
    body: &Value,
    stream_helper: Option<&str>,
) -> Result<BetaAssistantStream, OpenAIError> {
    let mut request = runtime.prepare_json_request("POST", path, body)?;
    request
        .headers
        .insert(String::from("accept"), String::from("text/event-stream"));
    request.headers.insert(
        String::from("openai-beta"),
        String::from(ASSISTANTS_BETA_HEADER),
    );
    if let Some(stream_helper) = stream_helper {
        request.headers.insert(
            String::from("x-stainless-stream-helper"),
            String::from(stream_helper),
        );
        request.headers.insert(
            String::from("x-stainless-custom-event-handler"),
            String::from("false"),
        );
    }
    let options = runtime.resolve_request_options(&RequestOptions::default())?;
    BetaAssistantStream::start_live(request, options)
}

fn assistants_beta_get(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
) -> Result<ApiResponse<Value>, OpenAIError> {
    assistants_beta_execute(runtime, "GET", path, None)
}

fn assistants_beta_get_query(
    runtime: &ClientRuntime,
    base: impl Into<String>,
    query: Vec<(String, String)>,
) -> Result<ApiResponse<Value>, OpenAIError> {
    assistants_beta_get(runtime, path_with_query(base, query))
}

fn assistants_beta_post(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
) -> Result<ApiResponse<Value>, OpenAIError> {
    assistants_beta_execute(runtime, "POST", path, None)
}

fn assistants_beta_post_body<B>(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
    body: &B,
) -> Result<ApiResponse<Value>, OpenAIError>
where
    B: Serialize,
{
    let mut request = runtime.prepare_json_request("POST", path, body)?;
    request.headers.insert(
        String::from("openai-beta"),
        String::from(ASSISTANTS_BETA_HEADER),
    );
    let options = runtime.resolve_request_options(&RequestOptions::default())?;
    execute_json(&request, &options)
}

fn body_with_stream_flag<B: Serialize>(params: B) -> Result<Value, OpenAIError> {
    let mut value = serde_json::to_value(params).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to serialize beta Assistants stream body: {error}"),
        )
        .with_source(error)
    })?;
    let Value::Object(map) = &mut value else {
        return Err(OpenAIError::new(
            ErrorKind::Validation,
            "beta Assistants stream body must serialize to a JSON object",
        ));
    };
    map.insert(String::from("stream"), Value::Bool(true));
    Ok(value)
}

fn value_string_field(value: &Value, field: &str) -> Result<String, OpenAIError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(String::from)
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("beta Assistants response did not include a non-empty `{field}`"),
            )
        })
}

fn is_terminal_run_status(value: &Value) -> bool {
    matches!(
        value.get("status").and_then(Value::as_str),
        Some("requires_action" | "cancelled" | "completed" | "failed" | "expired" | "incomplete")
    )
}

fn poll_interval(
    response: &ApiResponse<Value>,
    options: &BetaRunPollOptions,
) -> Result<Duration, OpenAIError> {
    if let Some(interval) = options.poll_interval {
        return Ok(interval);
    }
    let Some(header) = response.header("openai-poll-after-ms") else {
        return Ok(Duration::from_secs(1));
    };
    let millis = header.parse::<u64>().map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("invalid openai-poll-after-ms header `{header}`: {error}"),
        )
        .with_source(error)
    })?;
    Ok(Duration::from_millis(millis))
}

fn parse_beta_assistant_frame(
    frame: SseFrame,
    seen_done: &mut bool,
) -> Result<Option<BetaAssistantStreamEvent>, OpenAIError> {
    if frame.data.trim() == "[DONE]" {
        *seen_done = true;
        return Ok(None);
    }
    let data = serde_json::from_str::<Value>(&frame.data).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse beta Assistants stream event: {error}"),
        )
        .with_source(error)
    })?;
    Ok(Some(BetaAssistantStreamEvent {
        event: frame.event,
        raw_data: frame.data,
        data,
    }))
}

async fn consume_beta_assistant_live_stream(
    response: crate::core::transport::StreamingTextResponse,
    mut abort_rx: watch::Receiver<bool>,
    event_tx: mpsc::Sender<LiveBetaAssistantStreamMessage>,
) -> Result<(), OpenAIError> {
    let mut response = response.response;
    let mut parser = SseParser::default();
    let mut seen_done = false;

    loop {
        tokio::select! {
            changed = abort_rx.changed() => {
                if changed.is_ok() && *abort_rx.borrow() {
                    let _ = event_tx.send(LiveBetaAssistantStreamMessage::Finished);
                    return Ok(());
                }
            }
            chunk = response.chunk() => {
                let chunk = chunk.map_err(map_beta_live_transport_error)?;
                let Some(chunk) = chunk else {
                    break;
                };
                for frame in parser.push(chunk.as_ref())? {
                    if let Some(event) = parse_beta_assistant_frame(frame, &mut seen_done)? {
                        if event_tx.send(LiveBetaAssistantStreamMessage::Event(event)).is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    for frame in parser.finish()? {
        if let Some(event) = parse_beta_assistant_frame(frame, &mut seen_done)? {
            if event_tx
                .send(LiveBetaAssistantStreamMessage::Event(event))
                .is_err()
            {
                return Ok(());
            }
        }
    }
    if !seen_done {
        return Err(OpenAIError::new(
            ErrorKind::Transport,
            "beta Assistants stream ended before [DONE]",
        ));
    }
    let _ = event_tx.send(LiveBetaAssistantStreamMessage::Finished);
    Ok(())
}

fn map_beta_live_transport_error(error: reqwest::Error) -> OpenAIError {
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Transport
    };
    OpenAIError::new(kind, error.to_string()).with_source(error)
}

fn assistants_beta_delete(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
) -> Result<ApiResponse<Value>, OpenAIError> {
    assistants_beta_execute(runtime, "DELETE", path, None)
}

fn assistants_beta_execute(
    runtime: &ClientRuntime,
    method: impl AsRef<str>,
    path: impl AsRef<str>,
    body: Option<Vec<u8>>,
) -> Result<ApiResponse<Value>, OpenAIError> {
    let mut request = runtime.prepare_request_with_body(method, path, body)?;
    request
        .headers
        .insert(String::from("accept"), String::from("application/json"));
    request.headers.insert(
        String::from("openai-beta"),
        String::from(ASSISTANTS_BETA_HEADER),
    );
    let options = runtime.resolve_request_options(&RequestOptions::default())?;
    execute_json(&request, &options)
}

fn realtime_beta_post_body<B>(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
    body: &B,
) -> Result<ApiResponse<Value>, OpenAIError>
where
    B: Serialize,
{
    let mut request = runtime.prepare_json_request("POST", path, body)?;
    request.headers.insert(
        String::from("openai-beta"),
        String::from(ASSISTANTS_BETA_HEADER),
    );
    let options = runtime.resolve_request_options(&RequestOptions::default())?;
    execute_json(&request, &options)
}

fn chatkit_delete<T>(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
) -> Result<ApiResponse<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    chatkit_execute(runtime, "DELETE", path, None)
}

fn chatkit_execute<T>(
    runtime: &ClientRuntime,
    method: impl AsRef<str>,
    path: impl AsRef<str>,
    body: Option<Vec<u8>>,
) -> Result<ApiResponse<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    let mut request = runtime.prepare_request_with_body(method, path, body)?;
    request
        .headers
        .insert(String::from("accept"), String::from("application/json"));
    request.headers.insert(
        String::from("openai-beta"),
        String::from(CHATKIT_BETA_HEADER),
    );
    let options = runtime.resolve_request_options(&RequestOptions::default())?;
    execute_json(&request, &options)
}
