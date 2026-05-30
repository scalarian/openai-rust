use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{
        request::RequestOptions, response::ApiResponse, runtime::ClientRuntime,
        transport::execute_json,
    },
    resources::files::{encode_path_id, validate_path_id},
};

const CHATKIT_BETA_HEADER: &str = "chatkit_beta=v1";
const ASSISTANTS_BETA_HEADER: &str = "assistants=v2";

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
    pub fn list(&self, params: BetaQueryParams) -> Result<ApiResponse<Value>, OpenAIError> {
        assistants_beta_get_query(&self.runtime, "/assistants", params.into_pairs())
    }

    /// Deletes one assistant by id.
    pub fn delete(&self, assistant_id: &str) -> Result<ApiResponse<Value>, OpenAIError> {
        let assistant_id = path_id("assistant_id", assistant_id)?;
        assistants_beta_delete(&self.runtime, format!("/assistants/{assistant_id}"))
    }
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
        params: BetaQueryParams,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        assistants_beta_get_query(
            &self.runtime,
            format!("/threads/{thread_id}/messages"),
            params.into_pairs(),
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
        params: BetaQueryParams,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        assistants_beta_get_query(
            &self.runtime,
            format!("/threads/{thread_id}/runs"),
            params.into_pairs(),
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
        query: BetaQueryParams,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let run_id = path_id("run_id", run_id)?;
        let step_id = path_id("step_id", step_id)?;
        assistants_beta_get_query(
            &self.runtime,
            format!("/threads/{thread_id}/runs/{run_id}/steps/{step_id}"),
            query.into_pairs(),
        )
    }

    /// Lists run steps.
    pub fn list(
        &self,
        thread_id: &str,
        run_id: &str,
        params: BetaQueryParams,
    ) -> Result<ApiResponse<Value>, OpenAIError> {
        let thread_id = path_id("thread_id", thread_id)?;
        let run_id = path_id("run_id", run_id)?;
        assistants_beta_get_query(
            &self.runtime,
            format!("/threads/{thread_id}/runs/{run_id}/steps"),
            params.into_pairs(),
        )
    }
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
    pub state_variables: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracing: Option<ChatKitWorkflowTracingParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
    pub workflow: Value,
    #[serde(default)]
    pub chatkit_configuration: Option<Value>,
    #[serde(default)]
    pub rate_limits: Option<ChatKitSessionRateLimits>,
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

/// ChatKit thread item list page.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatKitThreadItemPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<Value>,
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
