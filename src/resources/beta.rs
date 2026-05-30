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
const REALTIME_BETA_HEADER: &str = "assistants=v2";

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

    /// Returns ChatKit beta endpoints.
    pub fn chatkit(&self) -> ChatKit {
        ChatKit::new(self.runtime.clone())
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
        String::from(REALTIME_BETA_HEADER),
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
