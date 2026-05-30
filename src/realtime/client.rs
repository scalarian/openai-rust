use std::{
    collections::BTreeMap,
    collections::VecDeque,
    fmt::{self, Write as _},
    sync::Arc,
};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use url::Url;

use crate::{
    OpenAIError,
    config::{ClientConfig, build_user_agent, normalize_base_url},
    core::{
        request::RequestOptions,
        response::ApiResponse,
        runtime::ClientRuntime,
        transport::{execute_bytes, execute_unit},
    },
    error::{ApiErrorKind, ErrorKind},
    helpers::multipart::MultipartBuilder,
};

use super::events::{
    RealtimeClientEvent, RealtimeConversationItem, RealtimeInclude, RealtimeOutputModality,
    RealtimeResponseCreateParams, RealtimeServerEvent, RealtimeSessionConfig, RealtimeSessionType,
    decode_server_event_text,
};

/// Realtime client-secret expiration anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealtimeSessionTTLAnchor {
    CreatedAt,
    Unknown(String),
}

impl RealtimeSessionTTLAnchor {
    pub fn as_str(&self) -> &str {
        match self {
            Self::CreatedAt => "created_at",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl AsRef<str> for RealtimeSessionTTLAnchor {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for RealtimeSessionTTLAnchor {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for RealtimeSessionTTLAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Default for RealtimeSessionTTLAnchor {
    fn default() -> Self {
        Self::Unknown(String::new())
    }
}

impl From<&str> for RealtimeSessionTTLAnchor {
    fn from(value: &str) -> Self {
        match value {
            "created_at" => Self::CreatedAt,
            _ => Self::Unknown(value.to_string()),
        }
    }
}

impl From<String> for RealtimeSessionTTLAnchor {
    fn from(value: String) -> Self {
        match value.as_str() {
            "created_at" => Self::CreatedAt,
            _ => Self::Unknown(value),
        }
    }
}

impl PartialEq<&str> for RealtimeSessionTTLAnchor {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<RealtimeSessionTTLAnchor> for &str {
    fn eq(&self, other: &RealtimeSessionTTLAnchor) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<String> for RealtimeSessionTTLAnchor {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<RealtimeSessionTTLAnchor> for String {
    fn eq(&self, other: &RealtimeSessionTTLAnchor) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Serialize for RealtimeSessionTTLAnchor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RealtimeSessionTTLAnchor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from(value))
    }
}

/// Realtime client-secret expiration settings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeSessionTTL {
    pub anchor: RealtimeSessionTTLAnchor,
    pub seconds: u64,
}

/// Typed client-secret token.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeClientSecret {
    pub value: String,
    pub expires_at: i64,
}

/// Client-secret creation parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RealtimeClientSecretCreateParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<RealtimeSessionTTL>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<RealtimeSessionConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(try_from = "RealtimeClientSecretCreateResponseWire")]
pub struct RealtimeClientSecretCreateResponse {
    pub client_secret: RealtimeClientSecret,
    pub session: RealtimeSessionConfig,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
struct RealtimeClientSecretCreateResponseWire {
    #[serde(default)]
    client_secret: Option<RealtimeClientSecret>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    expires_at: Option<i64>,
    session: RealtimeSessionConfig,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl TryFrom<RealtimeClientSecretCreateResponseWire> for RealtimeClientSecretCreateResponse {
    type Error = String;

    fn try_from(value: RealtimeClientSecretCreateResponseWire) -> Result<Self, Self::Error> {
        let client_secret = if let Some(client_secret) = value.client_secret {
            client_secret
        } else {
            RealtimeClientSecret {
                value: value
                    .value
                    .ok_or_else(|| String::from("missing realtime client secret value"))?,
                expires_at: value
                    .expires_at
                    .ok_or_else(|| String::from("missing realtime client secret expires_at"))?,
            }
        };

        Ok(Self {
            client_secret,
            session: value.session,
            extra: value.extra,
        })
    }
}

/// Call creation parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealtimeCallCreateParams {
    pub sdp: String,
    pub session: Option<RealtimeSessionConfig>,
}

/// Call acceptance parameters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeCallAcceptParams {
    #[serde(rename = "type")]
    pub session_type: RealtimeSessionType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<RealtimeInclude>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<Vec<RealtimeOutputModality>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracing: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for RealtimeCallAcceptParams {
    fn default() -> Self {
        Self {
            session_type: RealtimeSessionType::Realtime,
            audio: None,
            include: None,
            instructions: None,
            max_output_tokens: None,
            model: None,
            output_modalities: None,
            parallel_tool_calls: None,
            prompt: None,
            reasoning: None,
            tool_choice: None,
            tools: None,
            tracing: None,
            truncation: None,
            extra: BTreeMap::new(),
        }
    }
}

/// Call refer parameters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeCallReferParams {
    pub target_uri: String,
}

/// Call reject parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeCallRejectParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

/// Explicit Realtime websocket auth input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealtimeAuth {
    ApiKey(String),
    ClientSecret(String),
}

impl RealtimeAuth {
    pub fn api_key(value: impl Into<String>) -> Self {
        Self::ApiKey(value.into())
    }

    pub fn client_secret(value: impl Into<String>) -> Self {
        Self::ClientSecret(value.into())
    }

    fn token(&self) -> &str {
        match self {
            Self::ApiKey(value) | Self::ClientSecret(value) => value.as_str(),
        }
    }
}

/// Websocket target resolution inputs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RealtimeConnectOptions {
    pub model: Option<String>,
    pub call_id: Option<String>,
    pub auth: Option<RealtimeAuth>,
}

/// Resolved websocket target for Realtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRealtimeWsTarget {
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

/// Root Realtime family handle.
#[derive(Clone, Debug)]
pub struct Realtime {
    runtime: Arc<ClientRuntime>,
    client_secrets: ClientSecrets,
    calls: Calls,
}

impl Realtime {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self {
            runtime: runtime.clone(),
            client_secrets: ClientSecrets::new(runtime.clone()),
            calls: Calls::new(runtime),
        }
    }

    pub fn client_secrets(&self) -> &ClientSecrets {
        &self.client_secrets
    }

    pub fn calls(&self) -> &Calls {
        &self.calls
    }

    pub fn prepare_ws_target(
        &self,
        options: RealtimeConnectOptions,
    ) -> Result<PreparedRealtimeWsTarget, OpenAIError> {
        if options
            .model
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
            && options
                .call_id
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "Realtime websocket connections require either a model or a call_id",
            ));
        }

        let auth = options.auth;
        let (base_url, mut headers) = match auth.as_ref() {
            Some(RealtimeAuth::ClientSecret(_)) => {
                let config = self.runtime.config();
                (
                    normalize_base_url(
                        config
                            .base_url
                            .as_deref()
                            .unwrap_or(crate::DEFAULT_BASE_URL),
                    )?,
                    websocket_headers_from_config(config),
                )
            }
            _ => {
                let resolved = self.runtime.resolved_config()?;
                let headers = resolved.headers();
                (resolved.base_url, headers)
            }
        };

        let mut url = Url::parse(&base_url).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Configuration,
                format!("invalid OpenAI base URL `{}`: {error}", base_url),
            )
            .with_source(error)
        })?;
        let scheme = match url.scheme() {
            "http" => "ws",
            "https" => "wss",
            "ws" => "ws",
            "wss" => "wss",
            other => {
                return Err(OpenAIError::new(
                    ErrorKind::Configuration,
                    format!("unsupported base URL scheme for Realtime websocket: {other}"),
                ));
            }
        };
        url.set_scheme(scheme).map_err(|_| {
            OpenAIError::new(
                ErrorKind::Configuration,
                "failed to convert the configured base URL to a websocket target",
            )
        })?;

        let mut path = url.path().trim_end_matches('/').to_string();
        path.push_str("/realtime");
        url.set_path(&path);
        url.set_query(None);
        if let Some(model) = options
            .model
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            url.query_pairs_mut().append_pair("model", model);
        }
        if let Some(call_id) = options
            .call_id
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            url.query_pairs_mut().append_pair("call_id", call_id);
        }

        let auth = auth.unwrap_or_else(|| {
            let resolved = self.runtime.resolved_config().expect(
                "default websocket auth resolution should have already required an API key",
            );
            RealtimeAuth::api_key(resolved.api_key)
        });
        headers.insert(
            String::from("authorization"),
            format!("Bearer {}", auth.token()),
        );

        Ok(PreparedRealtimeWsTarget {
            url: url.to_string(),
            headers,
        })
    }

    pub async fn connect(
        &self,
        options: RealtimeConnectOptions,
    ) -> Result<RealtimeConnection, OpenAIError> {
        let target = self.prepare_ws_target(options)?;
        RealtimeConnection::connect(target).await
    }
}

/// Realtime client-secret REST helper family.
#[derive(Clone, Debug)]
pub struct ClientSecrets {
    runtime: Arc<ClientRuntime>,
}

impl ClientSecrets {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create(
        &self,
        params: RealtimeClientSecretCreateParams,
    ) -> Result<ApiResponse<RealtimeClientSecretCreateResponse>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/realtime/client_secrets",
            &params,
            RequestOptions::default(),
        )
    }
}

/// Realtime calls REST helper family.
#[derive(Clone, Debug)]
pub struct Calls {
    runtime: Arc<ClientRuntime>,
}

impl Calls {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create(
        &self,
        params: RealtimeCallCreateParams,
    ) -> Result<ApiResponse<Vec<u8>>, OpenAIError> {
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        if let Some(session) = params.session {
            let mut multipart = MultipartBuilder::new();
            multipart.add_file(
                "sdp",
                crate::helpers::multipart::MultipartFile::new(
                    "offer.sdp",
                    "application/sdp",
                    params.sdp.into_bytes(),
                ),
            );
            multipart.add_file(
                "session",
                crate::helpers::multipart::MultipartFile::new(
                    "session.json",
                    "application/json",
                    serde_json::to_vec(&session).map_err(|error| {
                        OpenAIError::new(
                            ErrorKind::Validation,
                            format!("failed to serialize realtime session: {error}"),
                        )
                        .with_source(error)
                    })?,
                ),
            );
            let multipart = multipart.build();
            let content_type = multipart.content_type();
            let mut request = self.runtime.prepare_request_with_body(
                "POST",
                "/realtime/calls",
                Some(multipart.into_body()),
            )?;
            request
                .headers
                .insert(String::from("content-type"), content_type);
            request
                .headers
                .insert(String::from("accept"), String::from("application/sdp"));
            execute_bytes(&request, &options)
        } else {
            let mut request = self.runtime.prepare_request_with_body(
                "POST",
                "/realtime/calls",
                Some(params.sdp.into_bytes()),
            )?;
            request.headers.insert(
                String::from("content-type"),
                String::from("application/sdp"),
            );
            request
                .headers
                .insert(String::from("accept"), String::from("application/sdp"));
            execute_bytes(&request, &options)
        }
    }

    pub fn accept(
        &self,
        call_id: &str,
        params: RealtimeCallAcceptParams,
    ) -> Result<ApiResponse<()>, OpenAIError> {
        self.execute_unit_json(
            format!(
                "/realtime/calls/{}/accept",
                encode_path_id(validate_path_id("call_id", call_id)?)
            ),
            &params,
        )
    }

    pub fn hangup(&self, call_id: &str) -> Result<ApiResponse<()>, OpenAIError> {
        let call_id = encode_path_id(validate_path_id("call_id", call_id)?);
        self.runtime.execute_unit(
            "POST",
            format!("/realtime/calls/{call_id}/hangup"),
            RequestOptions::default(),
        )
    }

    pub fn refer(
        &self,
        call_id: &str,
        params: RealtimeCallReferParams,
    ) -> Result<ApiResponse<()>, OpenAIError> {
        self.execute_unit_json(
            format!(
                "/realtime/calls/{}/refer",
                encode_path_id(validate_path_id("call_id", call_id)?)
            ),
            &params,
        )
    }

    pub fn reject(
        &self,
        call_id: &str,
        params: RealtimeCallRejectParams,
    ) -> Result<ApiResponse<()>, OpenAIError> {
        self.execute_unit_json(
            format!(
                "/realtime/calls/{}/reject",
                encode_path_id(validate_path_id("call_id", call_id)?)
            ),
            &params,
        )
    }

    fn execute_unit_json<B: Serialize>(
        &self,
        path: String,
        body: &B,
    ) -> Result<ApiResponse<()>, OpenAIError> {
        let mut request = self.runtime.prepare_json_request("POST", path, body)?;
        request
            .headers
            .insert(String::from("accept"), String::from("*/*"));
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        execute_unit(&request, &options)
    }
}

/// Minimal async Realtime websocket connection.
pub struct RealtimeConnection {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    buffered_events: VecDeque<Result<RealtimeServerEvent, OpenAIError>>,
    event_handlers: RealtimeEventHandlerRegistry,
    session_id: Option<String>,
    current_session: Option<RealtimeSessionConfig>,
    closed: bool,
}

/// Registered Realtime websocket event handler identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RealtimeEventHandlerId(u64);

struct RealtimeEventHandler {
    id: RealtimeEventHandlerId,
    once: bool,
    handler: Box<dyn FnMut(&RealtimeServerEvent) + Send>,
}

#[derive(Default)]
struct RealtimeEventHandlerRegistry {
    next_id: u64,
    handlers: BTreeMap<String, Vec<RealtimeEventHandler>>,
}

impl RealtimeEventHandlerRegistry {
    fn add<F>(
        &mut self,
        event_type: impl Into<String>,
        handler: F,
        once: bool,
    ) -> RealtimeEventHandlerId
    where
        F: FnMut(&RealtimeServerEvent) + Send + 'static,
    {
        let id = RealtimeEventHandlerId(self.next_id);
        self.next_id += 1;
        self.handlers
            .entry(event_type.into())
            .or_default()
            .push(RealtimeEventHandler {
                id,
                once,
                handler: Box::new(handler),
            });
        id
    }

    fn remove(&mut self, event_type: &str, id: RealtimeEventHandlerId) -> bool {
        let Some(handlers) = self.handlers.get_mut(event_type) else {
            return false;
        };
        let Some(index) = handlers.iter().position(|handler| handler.id == id) else {
            return false;
        };
        let _ = handlers.remove(index);
        if handlers.is_empty() {
            self.handlers.remove(event_type);
        }
        true
    }

    fn has_handlers(&self, event_type: &str) -> bool {
        self.handlers
            .get(event_type)
            .is_some_and(|handlers| !handlers.is_empty())
    }

    fn dispatch(&mut self, event_type: &str, event: &RealtimeServerEvent) {
        let Some(handlers) = self.handlers.get_mut(event_type) else {
            return;
        };

        let mut index = 0;
        while index < handlers.len() {
            let once = handlers[index].once;
            (handlers[index].handler)(event);
            if once {
                let _ = handlers.remove(index);
            } else {
                index += 1;
            }
        }
    }
}

impl RealtimeConnection {
    async fn connect(target: PreparedRealtimeWsTarget) -> Result<Self, OpenAIError> {
        let mut request = target.url.as_str().into_client_request().map_err(|error| {
            OpenAIError::new(
                ErrorKind::Configuration,
                format!("failed to build Realtime websocket request: {error}"),
            )
            .with_source(error)
        })?;
        for (name, value) in &target.headers {
            request.headers_mut().insert(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                    OpenAIError::new(
                        ErrorKind::Configuration,
                        format!("invalid Realtime websocket header name `{name}`: {error}"),
                    )
                    .with_source(error)
                })?,
                reqwest::header::HeaderValue::from_str(value).map_err(|error| {
                    OpenAIError::new(
                        ErrorKind::Configuration,
                        format!("invalid Realtime websocket header value for `{name}`: {error}"),
                    )
                    .with_source(error)
                })?,
            );
        }

        let (socket, _) = connect_async(request).await.map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("failed to connect Realtime websocket: {error}"),
            )
            .with_source(error)
        })?;

        let mut connection = Self {
            socket,
            buffered_events: VecDeque::new(),
            event_handlers: RealtimeEventHandlerRegistry::default(),
            session_id: None,
            current_session: None,
            closed: false,
        };
        connection.read_bootstrap_event().await?;
        Ok(connection)
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn current_session(&self) -> Option<&RealtimeSessionConfig> {
        self.current_session.as_ref()
    }

    /// Returns upstream-shaped session event helpers.
    pub fn session(&mut self) -> RealtimeSessionResource<'_> {
        RealtimeSessionResource { connection: self }
    }

    /// Returns upstream-shaped response event helpers.
    pub fn response(&mut self) -> RealtimeResponseResource<'_> {
        RealtimeResponseResource { connection: self }
    }

    /// Returns upstream-shaped input audio buffer event helpers.
    pub fn input_audio_buffer(&mut self) -> RealtimeInputAudioBufferResource<'_> {
        RealtimeInputAudioBufferResource { connection: self }
    }

    /// Returns upstream-shaped conversation event helpers.
    pub fn conversation(&mut self) -> RealtimeConversationResource<'_> {
        RealtimeConversationResource { connection: self }
    }

    /// Returns upstream-shaped output audio buffer event helpers.
    pub fn output_audio_buffer(&mut self) -> RealtimeOutputAudioBufferResource<'_> {
        RealtimeOutputAudioBufferResource { connection: self }
    }

    pub async fn send(&mut self, event: RealtimeClientEvent) -> Result<(), OpenAIError> {
        self.send_json_value(event.to_json_value()).await
    }

    /// Sends a caller-built Realtime client event JSON object.
    pub async fn send_json_value(&mut self, event: Value) -> Result<(), OpenAIError> {
        if self.closed {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "cannot send a Realtime event after the websocket has been closed",
            ));
        }
        let payload = serde_json::to_string(&event).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("failed to serialize Realtime client event: {error}"),
            )
            .with_source(error)
        })?;
        self.socket
            .send(Message::Text(payload.into()))
            .await
            .map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Transport,
                    format!("failed to send Realtime websocket event: {error}"),
                )
                .with_source(error)
            })
    }

    pub async fn next_event(&mut self) -> Option<Result<RealtimeServerEvent, OpenAIError>> {
        if let Some(buffered) = self.buffered_events.pop_front() {
            return Some(buffered);
        }
        if self.closed {
            return None;
        }

        loop {
            let message = match self.socket.next().await {
                Some(Ok(message)) => message,
                Some(Err(error)) => {
                    return Some(Err(OpenAIError::new(
                        ErrorKind::Transport,
                        format!("failed to read Realtime websocket frame: {error}"),
                    )
                    .with_source(error)));
                }
                None => {
                    self.closed = true;
                    return None;
                }
            };

            match message {
                Message::Text(text) => {
                    let event = decode_server_event_text(&text);
                    if let Ok(event) = &event {
                        self.observe_server_event(event);
                    }
                    return Some(event);
                }
                Message::Close(_) => {
                    self.closed = true;
                    return None;
                }
                Message::Ping(payload) => {
                    if let Err(error) = self.socket.send(Message::Pong(payload)).await {
                        return Some(Err(OpenAIError::new(
                            ErrorKind::Transport,
                            format!("failed to reply to Realtime websocket ping: {error}"),
                        )
                        .with_source(error)));
                    }
                }
                Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }

    /// Converts a raw websocket message into a typed Realtime server event.
    pub fn parse_event(&self, data: impl AsRef<[u8]>) -> Result<RealtimeServerEvent, OpenAIError> {
        let text = std::str::from_utf8(data.as_ref()).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to decode Realtime websocket event as UTF-8: {error}"),
            )
            .with_source(error)
        })?;
        decode_server_event_text(text)
    }

    /// Registers a handler for a specific event type.
    pub fn on<F>(&mut self, event_type: impl Into<String>, handler: F) -> RealtimeEventHandlerId
    where
        F: FnMut(&RealtimeServerEvent) + Send + 'static,
    {
        self.event_handlers.add(event_type, handler, false)
    }

    /// Registers a handler that is removed after the first matching event.
    pub fn once<F>(&mut self, event_type: impl Into<String>, handler: F) -> RealtimeEventHandlerId
    where
        F: FnMut(&RealtimeServerEvent) + Send + 'static,
    {
        self.event_handlers.add(event_type, handler, true)
    }

    /// Removes a previously registered event handler.
    pub fn off(&mut self, event_type: &str, handler_id: RealtimeEventHandlerId) -> bool {
        self.event_handlers.remove(event_type, handler_id)
    }

    /// Reads events until close, dispatching each one to registered handlers.
    pub async fn dispatch_events(&mut self) -> Result<(), OpenAIError> {
        while let Some(event) = self.next_event().await {
            let event = event?;
            let event_type = event.event_type();
            let has_specific_handlers = self.event_handlers.has_handlers(event_type);
            let has_generic_handlers = self.event_handlers.has_handlers("event");

            if let RealtimeServerEvent::Error { error, .. } = &event
                && !has_specific_handlers
                && !has_generic_handlers
            {
                return Err(OpenAIError::new(
                    ErrorKind::Api(ApiErrorKind::BadRequest),
                    format!("Realtime websocket error: {}", error.message),
                ));
            }

            self.event_handlers.dispatch(event_type, &event);
            self.event_handlers.dispatch("event", &event);
        }

        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), OpenAIError> {
        if self.closed {
            return Ok(());
        }
        self.socket.close(None).await.map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("failed to close Realtime websocket cleanly: {error}"),
            )
            .with_source(error)
        })?;
        self.closed = true;
        Ok(())
    }

    async fn read_bootstrap_event(&mut self) -> Result<(), OpenAIError> {
        while self.session_id.is_none() {
            let message = self.socket.next().await.ok_or_else(|| {
                OpenAIError::new(
                    ErrorKind::Transport,
                    "Realtime websocket closed before the initial session.created event",
                )
            })?;
            let message = message.map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Transport,
                    format!("failed to read Realtime bootstrap frame: {error}"),
                )
                .with_source(error)
            })?;
            match message {
                Message::Text(text) => {
                    let event = decode_server_event_text(&text)?;
                    self.observe_server_event(&event);
                    self.buffered_events.push_back(Ok(event));
                }
                Message::Close(_) => {
                    return Err(OpenAIError::new(
                        ErrorKind::Transport,
                        "Realtime websocket closed before the initial session.created event",
                    ));
                }
                Message::Ping(payload) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|error| {
                            OpenAIError::new(
                                ErrorKind::Transport,
                                format!("failed to reply to Realtime bootstrap ping: {error}"),
                            )
                            .with_source(error)
                        })?;
                }
                Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
        Ok(())
    }

    fn observe_server_event(&mut self, event: &RealtimeServerEvent) {
        match event {
            RealtimeServerEvent::SessionCreated { session, .. }
            | RealtimeServerEvent::SessionUpdated { session, .. } => {
                self.session_id = session.id.clone().or_else(|| self.session_id.clone());
                self.current_session = Some(session.clone());
            }
            _ => {}
        }
    }
}

/// Upstream-shaped Realtime session event helpers.
pub struct RealtimeSessionResource<'a> {
    connection: &'a mut RealtimeConnection,
}

impl RealtimeSessionResource<'_> {
    pub async fn update(
        self,
        session: RealtimeSessionConfig,
        event_id: Option<String>,
    ) -> Result<(), OpenAIError> {
        let mut object = event_object("session.update", event_id);
        object.insert(
            String::from("session"),
            serde_json::to_value(session).map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Validation,
                    format!("failed to serialize Realtime session.update event: {error}"),
                )
                .with_source(error)
            })?,
        );
        self.connection.send_json_value(Value::Object(object)).await
    }
}

/// Upstream-shaped Realtime response event helpers.
pub struct RealtimeResponseResource<'a> {
    connection: &'a mut RealtimeConnection,
}

impl RealtimeResponseResource<'_> {
    pub async fn create(
        self,
        response: Option<Value>,
        event_id: Option<String>,
    ) -> Result<(), OpenAIError> {
        let mut object = event_object("response.create", event_id);
        if let Some(response) = response {
            object.insert(String::from("response"), response);
        }
        self.connection.send_json_value(Value::Object(object)).await
    }

    pub async fn create_params(
        self,
        response: RealtimeResponseCreateParams,
        event_id: Option<String>,
    ) -> Result<(), OpenAIError> {
        let response = serde_json::to_value(response).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("failed to serialize Realtime response.create event: {error}"),
            )
            .with_source(error)
        })?;
        self.create(Some(response), event_id).await
    }

    pub async fn cancel(
        self,
        response_id: Option<String>,
        event_id: Option<String>,
    ) -> Result<(), OpenAIError> {
        let mut object = event_object("response.cancel", event_id);
        if let Some(response_id) = response_id {
            object.insert(String::from("response_id"), Value::String(response_id));
        }
        self.connection.send_json_value(Value::Object(object)).await
    }
}

/// Upstream-shaped Realtime input audio buffer event helpers.
pub struct RealtimeInputAudioBufferResource<'a> {
    connection: &'a mut RealtimeConnection,
}

impl RealtimeInputAudioBufferResource<'_> {
    pub async fn clear(self, event_id: Option<String>) -> Result<(), OpenAIError> {
        self.connection
            .send_json_value(Value::Object(event_object(
                "input_audio_buffer.clear",
                event_id,
            )))
            .await
    }

    pub async fn commit(self, event_id: Option<String>) -> Result<(), OpenAIError> {
        self.connection
            .send_json_value(Value::Object(event_object(
                "input_audio_buffer.commit",
                event_id,
            )))
            .await
    }

    pub async fn append(
        self,
        audio: impl Into<String>,
        event_id: Option<String>,
    ) -> Result<(), OpenAIError> {
        let mut object = event_object("input_audio_buffer.append", event_id);
        object.insert(String::from("audio"), Value::String(audio.into()));
        self.connection.send_json_value(Value::Object(object)).await
    }
}

/// Upstream-shaped Realtime conversation event helpers.
pub struct RealtimeConversationResource<'a> {
    connection: &'a mut RealtimeConnection,
}

impl<'a> RealtimeConversationResource<'a> {
    pub fn item(self) -> RealtimeConversationItemResource<'a> {
        RealtimeConversationItemResource {
            connection: self.connection,
        }
    }
}

/// Upstream-shaped Realtime conversation item event helpers.
pub struct RealtimeConversationItemResource<'a> {
    connection: &'a mut RealtimeConnection,
}

impl RealtimeConversationItemResource<'_> {
    pub async fn create(
        self,
        item: RealtimeConversationItem,
        previous_item_id: Option<String>,
        event_id: Option<String>,
    ) -> Result<(), OpenAIError> {
        let mut object = event_object("conversation.item.create", event_id);
        object.insert(
            String::from("item"),
            serde_json::to_value(item).map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Validation,
                    format!("failed to serialize Realtime conversation.item.create event: {error}"),
                )
                .with_source(error)
            })?,
        );
        if let Some(previous_item_id) = previous_item_id {
            object.insert(
                String::from("previous_item_id"),
                Value::String(previous_item_id),
            );
        }
        self.connection.send_json_value(Value::Object(object)).await
    }

    pub async fn delete(
        self,
        item_id: impl Into<String>,
        event_id: Option<String>,
    ) -> Result<(), OpenAIError> {
        let item_id = item_id.into();
        validate_path_id("item_id", &item_id)?;
        let mut object = event_object("conversation.item.delete", event_id);
        object.insert(String::from("item_id"), Value::String(item_id));
        self.connection.send_json_value(Value::Object(object)).await
    }

    pub async fn truncate(
        self,
        item_id: impl Into<String>,
        content_index: usize,
        audio_end_ms: u64,
        event_id: Option<String>,
    ) -> Result<(), OpenAIError> {
        let item_id = item_id.into();
        validate_path_id("item_id", &item_id)?;
        let mut object = event_object("conversation.item.truncate", event_id);
        object.insert(String::from("item_id"), Value::String(item_id));
        object.insert(String::from("content_index"), Value::from(content_index));
        object.insert(String::from("audio_end_ms"), Value::from(audio_end_ms));
        self.connection.send_json_value(Value::Object(object)).await
    }

    pub async fn retrieve(
        self,
        item_id: impl Into<String>,
        event_id: Option<String>,
    ) -> Result<(), OpenAIError> {
        let item_id = item_id.into();
        validate_path_id("item_id", &item_id)?;
        let mut object = event_object("conversation.item.retrieve", event_id);
        object.insert(String::from("item_id"), Value::String(item_id));
        self.connection.send_json_value(Value::Object(object)).await
    }
}

/// Upstream-shaped Realtime output audio buffer event helpers.
pub struct RealtimeOutputAudioBufferResource<'a> {
    connection: &'a mut RealtimeConnection,
}

impl RealtimeOutputAudioBufferResource<'_> {
    pub async fn clear(self, event_id: Option<String>) -> Result<(), OpenAIError> {
        self.connection
            .send_json_value(Value::Object(event_object(
                "output_audio_buffer.clear",
                event_id,
            )))
            .await
    }
}

fn websocket_headers_from_config(config: &ClientConfig) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    headers.insert(
        String::from("user-agent"),
        build_user_agent(config.user_agent.as_deref()),
    );
    if let Some(organization) = normalize_ws_header_value(config.organization.as_deref()) {
        headers.insert(String::from("openai-organization"), organization);
    }
    if let Some(project) = normalize_ws_header_value(config.project.as_deref()) {
        headers.insert(String::from("openai-project"), project);
    }
    headers
}

fn normalize_ws_header_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn event_object(event_type: &str, event_id: Option<String>) -> Map<String, Value> {
    let mut object = Map::new();
    object.insert(String::from("type"), Value::String(event_type.to_string()));
    if let Some(event_id) = event_id {
        object.insert(String::from("event_id"), Value::String(event_id));
    }
    object
}

fn validate_path_id<'a>(label: &str, value: &'a str) -> Result<&'a str, OpenAIError> {
    if value.trim().is_empty() {
        return Err(OpenAIError::new(
            ErrorKind::Validation,
            format!("{label} cannot be blank"),
        ));
    }
    Ok(value)
}

fn encode_path_id(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if matches!(
            byte,
            b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
        ) {
            encoded.push(byte as char);
        } else {
            let _ = write!(&mut encoded, "%{byte:02X}");
        }
    }
    encoded
}
