use std::{collections::BTreeMap, collections::VecDeque, fmt::Write as _, sync::Arc};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use url::Url;

use crate::{
    OpenAIError,
    core::{
        request::RequestOptions,
        response::ApiResponse,
        runtime::ClientRuntime,
        transport::{execute_bytes, execute_unit},
    },
    error::ErrorKind,
    helpers::multipart::MultipartBuilder,
};

use super::events::{
    RealtimeClientEvent, RealtimeOutputModality, RealtimeServerEvent, RealtimeSessionConfig,
    RealtimeSessionType, decode_server_event_text,
};

/// Realtime client-secret expiration settings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeSessionTTL {
    pub anchor: String,
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
    pub include: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<Vec<RealtimeOutputModality>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Value>,
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
            prompt: None,
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

        let resolved = self.runtime.resolved_config()?;
        let mut url = Url::parse(&resolved.base_url).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Configuration,
                format!("invalid OpenAI base URL `{}`: {error}", resolved.base_url),
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

        let mut headers = resolved.headers();
        let auth = options
            .auth
            .unwrap_or_else(|| RealtimeAuth::api_key(resolved.api_key));
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
    session_id: Option<String>,
    current_session: Option<RealtimeSessionConfig>,
    closed: bool,
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

    pub async fn send(&mut self, event: RealtimeClientEvent) -> Result<(), OpenAIError> {
        if self.closed {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "cannot send a Realtime event after the websocket has been closed",
            ));
        }
        let payload = serde_json::to_string(&event.to_json_value()).map_err(|error| {
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
