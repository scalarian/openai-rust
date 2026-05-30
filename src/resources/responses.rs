use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc, Condvar, Mutex,
        mpsc::{self, Receiver},
    },
    thread,
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize, Serializer, de::DeserializeOwned};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use url::Url;

use crate::{
    OpenAIError,
    config::normalize_base_url,
    core::{
        metadata::ResponseMetadata,
        request::{PreparedRequest, RequestOptions, ResolvedRequestOptions},
        response::ApiResponse,
        runtime::ClientRuntime,
    },
    error::{ApiErrorKind, ErrorKind},
    helpers::sse::{SseFrame, SseParser},
};

/// Primary Responses API family.
#[derive(Clone, Debug)]
pub struct Responses {
    runtime: Arc<ClientRuntime>,
}

impl Responses {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns the nested input-tokens helper surface.
    pub fn input_tokens(&self) -> InputTokens {
        InputTokens::new(self.runtime.clone())
    }

    /// Returns the nested input-items helper surface.
    pub fn input_items(&self) -> InputItems {
        InputItems::new(self.runtime.clone())
    }

    /// Creates a non-streamed response and computes the `output_text` helper.
    pub fn create(
        &self,
        params: ResponseCreateParams,
    ) -> Result<ApiResponse<Response>, OpenAIError> {
        let body = params.into_request_body();
        let response = self.runtime.execute_json_with_body::<_, WireResponse>(
            "POST",
            "/responses",
            &body,
            RequestOptions::default(),
        )?;
        Ok(map_response(response))
    }

    /// Creates a non-streamed structured response and parses strict tool arguments.
    pub fn parse<T>(
        &self,
        params: ResponseParseParams,
    ) -> Result<ApiResponse<ParsedResponse<T>>, OpenAIError>
    where
        T: DeserializeOwned,
    {
        let text_format = params
            .text
            .as_ref()
            .and_then(|text| text.format.as_ref())
            .cloned();
        let tools = params.tools.clone();
        let response = self.create(params.into_create_params())?;
        let parsed = parse_response_output::<T>(response.output, text_format, &tools)?;
        Ok(ApiResponse {
            output: parsed,
            metadata: response.metadata,
        })
    }

    /// Creates a streamed response transcript and exposes a deterministic state machine.
    pub fn stream(&self, params: ResponseCreateParams) -> Result<ResponseStream, OpenAIError> {
        let body = params.into_stream_request_body();
        let request = self
            .runtime
            .prepare_json_request("POST", "/responses", &body)?;
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        ResponseStream::start_live(request, options, None)
    }

    /// Retrieves a stored response and recomputes the `output_text` helper.
    pub fn retrieve(
        &self,
        response_id: &str,
        params: ResponseRetrieveParams,
    ) -> Result<ApiResponse<Response>, OpenAIError> {
        let response_id = validate_path_id("response_id", response_id)?;
        let path = append_query(
            &format!("/responses/{response_id}"),
            params.to_query_pairs(),
        );
        let response =
            self.runtime
                .execute_json::<WireResponse>("GET", &path, RequestOptions::default())?;
        Ok(map_response(response))
    }

    /// Resumes a background stream using `starting_after` and stream retrieval semantics.
    pub fn resume_stream(
        &self,
        response_id: &str,
        mut params: ResponseRetrieveParams,
    ) -> Result<ResponseStream, OpenAIError> {
        let response_id = validate_path_id("response_id", response_id)?;
        params.stream = Some(true);
        let resume_after = params.starting_after;
        let path = append_query(
            &format!("/responses/{response_id}"),
            params.to_query_pairs(),
        );
        let request = self.runtime.prepare_request("GET", &path)?;
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        ResponseStream::start_live(request, options, resume_after)
    }

    /// Deletes a stored response and returns unit on success.
    pub fn delete(&self, response_id: &str) -> Result<ApiResponse<()>, OpenAIError> {
        let response_id = validate_path_id("response_id", response_id)?;
        self.runtime.execute_unit(
            "DELETE",
            format!("/responses/{response_id}"),
            RequestOptions::default(),
        )
    }

    /// Cancels a background response and returns the updated response object.
    pub fn cancel(&self, response_id: &str) -> Result<ApiResponse<Response>, OpenAIError> {
        let response_id = validate_path_id("response_id", response_id)?;
        let response = self.runtime.execute_json_with_body::<_, WireResponse>(
            "POST",
            format!("/responses/{response_id}/cancel"),
            &Value::Object(Default::default()),
            RequestOptions::default(),
        )?;
        Ok(map_response(response))
    }

    /// Compacts prior conversation state into a typed compaction object.
    pub fn compact(
        &self,
        params: ResponseCompactParams,
    ) -> Result<ApiResponse<CompactedResponse>, OpenAIError> {
        let response = self
            .runtime
            .execute_json_with_body::<_, WireCompactedResponse>(
                "POST",
                "/responses/compact",
                &params,
                RequestOptions::default(),
            )?;
        Ok(ApiResponse {
            output: response.output.into(),
            metadata: response.metadata,
        })
    }

    /// Resolves the persistent Responses websocket target without opening a socket.
    pub fn prepare_ws_target(
        &self,
        options: ResponsesConnectOptions,
    ) -> Result<PreparedResponsesWsTarget, OpenAIError> {
        prepare_responses_ws_target(&self.runtime, options)
    }

    /// Connects to the persistent Responses websocket endpoint.
    pub async fn connect(
        &self,
        options: ResponsesConnectOptions,
    ) -> Result<ResponsesConnection, OpenAIError> {
        let target = self.prepare_ws_target(options)?;
        ResponsesConnection::connect(target).await
    }
}

/// Websocket connection options for the persistent Responses API.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResponsesConnectOptions {
    pub extra_query: Vec<(String, String)>,
    pub extra_headers: BTreeMap<String, String>,
}

impl ResponsesConnectOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.extra_query.push((key.into(), value.to_string()));
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.insert(name.into(), value.into());
        self
    }
}

/// Resolved websocket target for the persistent Responses API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedResponsesWsTarget {
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

/// One JSON event received from the persistent Responses websocket.
#[derive(Clone, Debug, PartialEq)]
pub struct ResponsesConnectionEvent {
    pub event_type: Option<String>,
    pub payload: Value,
}

/// Registered Responses websocket event handler identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResponsesEventHandlerId(u64);

struct ResponsesEventHandler {
    id: ResponsesEventHandlerId,
    once: bool,
    handler: Box<dyn FnMut(&ResponsesConnectionEvent) + Send>,
}

#[derive(Default)]
struct ResponsesEventHandlerRegistry {
    next_id: u64,
    handlers: BTreeMap<String, Vec<ResponsesEventHandler>>,
}

impl ResponsesEventHandlerRegistry {
    fn add<F>(
        &mut self,
        event_type: impl Into<String>,
        handler: F,
        once: bool,
    ) -> ResponsesEventHandlerId
    where
        F: FnMut(&ResponsesConnectionEvent) + Send + 'static,
    {
        let id = ResponsesEventHandlerId(self.next_id);
        self.next_id += 1;
        self.handlers
            .entry(event_type.into())
            .or_default()
            .push(ResponsesEventHandler {
                id,
                once,
                handler: Box::new(handler),
            });
        id
    }

    fn remove(&mut self, event_type: &str, id: ResponsesEventHandlerId) -> bool {
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

    fn dispatch(&mut self, event_type: &str, event: &ResponsesConnectionEvent) {
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

/// Minimal async websocket connection for the persistent Responses API.
pub struct ResponsesConnection {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    event_handlers: ResponsesEventHandlerRegistry,
    closed: bool,
}

impl ResponsesConnection {
    async fn connect(target: PreparedResponsesWsTarget) -> Result<Self, OpenAIError> {
        let mut request = target.url.as_str().into_client_request().map_err(|error| {
            OpenAIError::new(
                ErrorKind::Configuration,
                format!("failed to build Responses websocket request: {error}"),
            )
            .with_source(error)
        })?;
        for (name, value) in &target.headers {
            request.headers_mut().insert(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                    OpenAIError::new(
                        ErrorKind::Configuration,
                        format!("invalid Responses websocket header name `{name}`: {error}"),
                    )
                    .with_source(error)
                })?,
                reqwest::header::HeaderValue::from_str(value).map_err(|error| {
                    OpenAIError::new(
                        ErrorKind::Configuration,
                        format!("invalid Responses websocket header value for `{name}`: {error}"),
                    )
                    .with_source(error)
                })?,
            );
        }

        let (socket, _) = connect_async(request).await.map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("failed to connect Responses websocket: {error}"),
            )
            .with_source(error)
        })?;

        Ok(Self {
            socket,
            event_handlers: ResponsesEventHandlerRegistry::default(),
            closed: false,
        })
    }

    pub async fn send<B: Serialize>(&mut self, event: B) -> Result<(), OpenAIError> {
        let payload = serde_json::to_string(&event).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("failed to serialize Responses websocket event: {error}"),
            )
            .with_source(error)
        })?;
        self.send_raw(payload).await
    }

    pub async fn send_raw(&mut self, payload: impl Into<String>) -> Result<(), OpenAIError> {
        if self.closed {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "cannot send a Responses websocket event after the socket has been closed",
            ));
        }
        self.socket
            .send(Message::Text(payload.into().into()))
            .await
            .map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Transport,
                    format!("failed to send Responses websocket event: {error}"),
                )
                .with_source(error)
            })
    }

    pub async fn recv(&mut self) -> Option<Result<ResponsesConnectionEvent, OpenAIError>> {
        loop {
            let message = self.recv_message().await?;
            match message {
                Ok(Message::Text(text)) => {
                    return Some(parse_responses_ws_event(&text));
                }
                Ok(Message::Binary(bytes)) => {
                    let text = match String::from_utf8(bytes.to_vec()) {
                        Ok(text) => text,
                        Err(error) => {
                            return Some(Err(OpenAIError::new(
                                ErrorKind::Parse,
                                format!(
                                    "failed to decode Responses websocket binary event as UTF-8: {error}"
                                ),
                            )
                            .with_source(error)));
                        }
                    };
                    return Some(parse_responses_ws_event(&text));
                }
                Ok(Message::Ping(payload)) => {
                    if let Err(error) = self.socket.send(Message::Pong(payload)).await {
                        return Some(Err(OpenAIError::new(
                            ErrorKind::Transport,
                            format!("failed to reply to Responses websocket ping: {error}"),
                        )
                        .with_source(error)));
                    }
                }
                Ok(Message::Pong(_) | Message::Frame(_)) => {}
                Ok(Message::Close(_)) => {
                    self.closed = true;
                    return None;
                }
                Err(error) => return Some(Err(error)),
            }
        }
    }

    pub async fn recv_bytes(&mut self) -> Option<Result<Vec<u8>, OpenAIError>> {
        loop {
            let message = self.recv_message().await?;
            match message {
                Ok(Message::Text(text)) => return Some(Ok(text.as_bytes().to_vec())),
                Ok(Message::Binary(bytes)) => return Some(Ok(bytes.to_vec())),
                Ok(Message::Ping(payload)) => {
                    if let Err(error) = self.socket.send(Message::Pong(payload)).await {
                        return Some(Err(OpenAIError::new(
                            ErrorKind::Transport,
                            format!("failed to reply to Responses websocket ping: {error}"),
                        )
                        .with_source(error)));
                    }
                }
                Ok(Message::Pong(_) | Message::Frame(_)) => {}
                Ok(Message::Close(_)) => {
                    self.closed = true;
                    return None;
                }
                Err(error) => return Some(Err(error)),
            }
        }
    }

    /// Converts a raw websocket message into a Responses connection event.
    pub fn parse_event(
        &self,
        data: impl AsRef<[u8]>,
    ) -> Result<ResponsesConnectionEvent, OpenAIError> {
        let text = std::str::from_utf8(data.as_ref()).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to decode Responses websocket event as UTF-8: {error}"),
            )
            .with_source(error)
        })?;
        parse_responses_ws_event(text)
    }

    /// Registers a handler for a specific event type.
    pub fn on<F>(&mut self, event_type: impl Into<String>, handler: F) -> ResponsesEventHandlerId
    where
        F: FnMut(&ResponsesConnectionEvent) + Send + 'static,
    {
        self.event_handlers.add(event_type, handler, false)
    }

    /// Registers a handler that is removed after the first matching event.
    pub fn once<F>(&mut self, event_type: impl Into<String>, handler: F) -> ResponsesEventHandlerId
    where
        F: FnMut(&ResponsesConnectionEvent) + Send + 'static,
    {
        self.event_handlers.add(event_type, handler, true)
    }

    /// Removes a previously registered event handler.
    pub fn off(&mut self, event_type: &str, handler_id: ResponsesEventHandlerId) -> bool {
        self.event_handlers.remove(event_type, handler_id)
    }

    /// Reads events until close, dispatching each one to registered handlers.
    pub async fn dispatch_events(&mut self) -> Result<(), OpenAIError> {
        while let Some(event) = self.recv().await {
            let event = event?;
            let event_type = event.event_type.as_deref().unwrap_or_default();
            let has_specific_handlers = self.event_handlers.has_handlers(event_type);
            let has_generic_handlers = self.event_handlers.has_handlers("event");

            if event_type == "error" && !has_specific_handlers && !has_generic_handlers {
                return Err(OpenAIError::new(
                    ErrorKind::Api(ApiErrorKind::BadRequest),
                    format!("Responses websocket error: {}", event.payload),
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
                format!("failed to close Responses websocket cleanly: {error}"),
            )
            .with_source(error)
        })?;
        self.closed = true;
        Ok(())
    }

    async fn recv_message(&mut self) -> Option<Result<Message, OpenAIError>> {
        if self.closed {
            return None;
        }
        match self.socket.next().await {
            Some(Ok(message)) => Some(Ok(message)),
            Some(Err(error)) => Some(Err(OpenAIError::new(
                ErrorKind::Transport,
                format!("failed to read Responses websocket frame: {error}"),
            )
            .with_source(error))),
            None => {
                self.closed = true;
                None
            }
        }
    }
}

/// Request body for non-streamed response creation.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ResponseCreateParams {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub context_management: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation: Option<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<ResponseTextConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<FunctionTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ResponseCreateParams {
    pub fn with_serialized_input<T>(mut self, input: T) -> Result<Self, OpenAIError>
    where
        T: Serialize,
    {
        self.input = Some(serialize_json_value("responses.input", input)?);
        Ok(self)
    }

    fn into_request_body(self) -> Value {
        let mut value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        if let Value::Object(ref mut object) = value {
            object.insert(String::from("stream"), Value::Bool(false));
        }
        value
    }

    fn into_stream_request_body(self) -> Value {
        let mut value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        if let Value::Object(ref mut object) = value {
            object.insert(String::from("stream"), Value::Bool(true));
        }
        value
    }
}

/// Request body for structured non-streamed response parsing.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ResponseParseParams {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub context_management: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation: Option<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<ResponseTextConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<FunctionTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ResponseParseParams {
    fn into_create_params(self) -> ResponseCreateParams {
        ResponseCreateParams {
            model: self.model,
            background: self.background,
            context_management: self.context_management,
            conversation: self.conversation,
            include: self.include,
            input: self.input,
            instructions: self.instructions,
            max_output_tokens: self.max_output_tokens,
            max_tool_calls: self.max_tool_calls,
            metadata: self.metadata,
            parallel_tool_calls: self.parallel_tool_calls,
            previous_response_id: self.previous_response_id,
            prompt: self.prompt,
            prompt_cache_key: self.prompt_cache_key,
            prompt_cache_retention: self.prompt_cache_retention,
            reasoning: self.reasoning,
            safety_identifier: self.safety_identifier,
            service_tier: self.service_tier,
            store: self.store,
            stream: None,
            stream_options: self.stream_options,
            temperature: self.temperature,
            text: self.text,
            tool_choice: self.tool_choice,
            tools: self.tools,
            top_logprobs: self.top_logprobs,
            top_p: self.top_p,
            truncation: self.truncation,
            user: self.user,
            extra: self.extra,
        }
    }
}

/// Query parameters for non-streamed response retrieval.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResponseRetrieveParams {
    pub include: Vec<String>,
    pub include_obfuscation: Option<bool>,
    pub starting_after: Option<u64>,
    pub stream: Option<bool>,
}

impl ResponseRetrieveParams {
    fn to_query_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        for include in &self.include {
            pairs.push((String::from("include"), include.clone()));
        }
        if let Some(include_obfuscation) = self.include_obfuscation {
            pairs.push((
                String::from("include_obfuscation"),
                include_obfuscation.to_string(),
            ));
        }
        if let Some(starting_after) = self.starting_after {
            pairs.push((String::from("starting_after"), starting_after.to_string()));
        }
        if let Some(stream) = self.stream {
            pairs.push((String::from("stream"), stream.to_string()));
        }
        pairs
    }
}

/// Request body for response compaction.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ResponseCompactParams {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Input-token count helper params mirroring response creation fields.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ResponseInputTokensCountParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<ResponseTextConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<FunctionTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ResponseInputTokensCountParams {
    pub fn with_serialized_input<T>(mut self, input: T) -> Result<Self, OpenAIError>
    where
        T: Serialize,
    {
        self.input = Some(serialize_json_value("responses.input_tokens.input", input)?);
        Ok(self)
    }
}

/// Input-item list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResponseInputItemsListParams {
    pub after: Option<String>,
    pub include: Vec<String>,
    pub limit: Option<u32>,
    pub order: Option<String>,
}

impl ResponseInputItemsListParams {
    fn to_query_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        if let Some(after) = &self.after {
            pairs.push((String::from("after"), after.clone()));
        }
        for include in &self.include {
            pairs.push((String::from("include"), include.clone()));
        }
        if let Some(limit) = self.limit {
            pairs.push((String::from("limit"), limit.to_string()));
        }
        if let Some(order) = &self.order {
            pairs.push((String::from("order"), order.clone()));
        }
        pairs
    }
}

/// Nested Responses input-tokens helper surface.
#[derive(Clone, Debug)]
pub struct InputTokens {
    runtime: Arc<ClientRuntime>,
}

impl InputTokens {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn count(
        &self,
        params: ResponseInputTokensCountParams,
    ) -> Result<ApiResponse<InputTokenCount>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/responses/input_tokens",
            &params,
            RequestOptions::default(),
        )
    }
}

/// Nested Responses input-items helper surface.
#[derive(Clone, Debug)]
pub struct InputItems {
    runtime: Arc<ClientRuntime>,
}

impl InputItems {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn list(
        &self,
        response_id: &str,
        params: ResponseInputItemsListParams,
    ) -> Result<ApiResponse<ResponseInputItemsPage>, OpenAIError> {
        let response_id = validate_path_id("response_id", response_id)?;
        let path = append_query(
            &format!("/responses/{response_id}/input_items"),
            params.to_query_pairs(),
        );
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }
}

/// Public typed input-token count response.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct InputTokenCount {
    pub object: String,
    pub input_tokens: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Public parsed response object with aggregated `output_text`.
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub status: Option<String>,
    pub output: Vec<ResponseOutputItem>,
    pub previous_response_id: Option<String>,
    pub conversation: Option<Value>,
    pub store: Option<bool>,
    pub background: Option<bool>,
    pub usage: Value,
    pub error: Option<Value>,
    pub incomplete_details: Option<Value>,
    pub metadata: Option<Value>,
    pub extra: BTreeMap<String, Value>,
    output_text: String,
}

impl Response {
    pub fn output_text(&self) -> &str {
        &self.output_text
    }

    pub fn refusal_text(&self) -> Option<&str> {
        self.output
            .iter()
            .filter(|item| item.item_type == "message")
            .flat_map(|item| item.content.iter())
            .find(|content| content.content_type == "refusal")
            .and_then(ResponseContentPart::refusal_text)
    }
}

/// Public parsed response compaction object.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CompactedResponse {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    #[serde(default)]
    pub output: Vec<ResponseOutputItem>,
    #[serde(default)]
    pub usage: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Common item shape used by response and compaction payloads.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseOutputItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
    #[serde(default)]
    pub input: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub call_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub content: Vec<ResponseContentPart>,
    #[serde(skip)]
    pub parsed_arguments: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Content part shape needed for output-text aggregation.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseContentPart {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub refusal: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ResponseContentPart {
    fn refusal_text(&self) -> Option<&str> {
        self.refusal.as_deref().or(self.text.as_deref())
    }
}

/// Parsed non-stream response with structured output helper access.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedResponse<T> {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub status: Option<String>,
    pub output: Vec<ResponseOutputItem>,
    pub previous_response_id: Option<String>,
    pub conversation: Option<Value>,
    pub store: Option<bool>,
    pub background: Option<bool>,
    pub usage: Value,
    pub error: Option<Value>,
    pub incomplete_details: Option<Value>,
    pub metadata: Option<Value>,
    pub extra: BTreeMap<String, Value>,
    output_text: String,
    output_parsed: Option<T>,
}

impl<T> ParsedResponse<T> {
    pub fn output_text(&self) -> &str {
        &self.output_text
    }

    pub fn output_parsed(&self) -> Option<&T> {
        self.output_parsed.as_ref()
    }
}

/// User-visible streamed Responses events.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseStreamEvent {
    Created {
        response: Response,
    },
    Queued {
        response: Response,
    },
    InProgress {
        response: Response,
    },
    AudioDelta {
        delta: String,
    },
    AudioDone,
    AudioTranscriptDelta {
        delta: String,
    },
    AudioTranscriptDone,
    OutputTextDelta {
        output_index: usize,
        content_index: usize,
        delta: String,
    },
    OutputTextDone {
        output_index: usize,
        content_index: usize,
        text: String,
    },
    ReasoningTextDelta {
        output_index: usize,
        content_index: usize,
        delta: String,
    },
    ReasoningTextDone {
        output_index: usize,
        content_index: usize,
        text: String,
    },
    RefusalDelta {
        output_index: usize,
        content_index: usize,
        delta: String,
    },
    RefusalDone {
        output_index: usize,
        content_index: usize,
        text: String,
    },
    ImageGenerationCallInProgress {
        item_id: Option<String>,
        output_index: usize,
    },
    ImageGenerationCallGenerating {
        item_id: Option<String>,
        output_index: usize,
    },
    ImageGenerationCallPartialImage {
        item_id: Option<String>,
        output_index: usize,
        partial_image_b64: String,
        partial_image_index: usize,
    },
    ImageGenerationCallCompleted {
        item_id: Option<String>,
        output_index: usize,
    },
    OutputTextAnnotationAdded {
        item_id: Option<String>,
        output_index: usize,
        content_index: usize,
        annotation_index: usize,
        annotation: Value,
    },
    ReasoningSummaryPartAdded {
        item_id: Option<String>,
        output_index: usize,
        summary_index: usize,
        part: Value,
    },
    ReasoningSummaryPartDone {
        item_id: Option<String>,
        output_index: usize,
        summary_index: usize,
        part: Value,
    },
    ReasoningSummaryTextDelta {
        item_id: Option<String>,
        output_index: usize,
        summary_index: usize,
        delta: String,
    },
    ReasoningSummaryTextDone {
        item_id: Option<String>,
        output_index: usize,
        summary_index: usize,
        text: String,
    },
    Completed {
        response: Response,
    },
    Failed {
        response: Response,
    },
    Incomplete {
        response: Response,
    },
    Unknown {
        event: String,
        data: Value,
    },
}

/// Terminal streamed Responses state.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseStreamTerminal {
    Completed(Response),
    Failed(Response),
    Incomplete(Response),
}

#[derive(Clone, Debug, PartialEq)]
struct RecordedResponseEvent {
    event: ResponseStreamEvent,
    snapshot_after_event: Option<Response>,
}

/// Eagerly parsed streamed Responses transcript with sync/async consumption helpers.
#[derive(Debug)]
pub struct ResponseStream {
    metadata: ResponseMetadata,
    events: VecDeque<RecordedResponseEvent>,
    current_snapshot: Option<Response>,
    final_terminal: Option<ResponseStreamTerminal>,
    live: Option<LiveStreamHandle>,
    aborted: bool,
}

impl ResponseStream {
    pub fn from_sse_chunks<I, B>(metadata: ResponseMetadata, chunks: I) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        Self::from_sse_chunks_with_resume(metadata, chunks, None)
    }

    pub fn current_response(&self) -> Option<&Response> {
        self.current_snapshot.as_ref()
    }

    pub fn next_event(&mut self) -> Option<ResponseStreamEvent> {
        if self.aborted {
            return None;
        }
        if self.events.is_empty() {
            self.fill_from_live();
        }
        let recorded = self.events.pop_front()?;
        self.current_snapshot = recorded.snapshot_after_event;
        self.drain_live_messages();
        if self.final_terminal.is_none() {
            self.poll_live_messages(Duration::from_millis(5));
        }
        Some(recorded.event)
    }

    pub async fn next_event_async(&mut self) -> Option<ResponseStreamEvent> {
        self.next_event()
    }

    pub fn abort(&mut self) {
        self.aborted = true;
        self.events.clear();
        if let Some(live) = &mut self.live {
            let _ = live.abort.send(true);
            let _ = live.receiver.try_recv();
            live.join_worker();
        }
    }

    pub fn terminal_state(&self) -> Option<&ResponseStreamTerminal> {
        self.final_terminal.as_ref()
    }

    pub fn metadata(&self) -> &ResponseMetadata {
        &self.metadata
    }

    pub fn final_response(&mut self) -> Result<&Response, OpenAIError> {
        if self.aborted {
            return Err(OpenAIError::new(
                ErrorKind::Transport,
                "response stream was aborted before completion",
            ));
        }

        if let Some(live) = &self.live {
            live.shared.wait_until_finished();
            if let Some(error) = live.shared.error() {
                return Err(error);
            }
        }
        self.drain_live_messages();
        if self.final_terminal.is_none() {
            if let Some(live) = &self.live {
                self.final_terminal = live.shared.terminal_cloned();
            }
        }

        match self.final_terminal.as_ref() {
            Some(ResponseStreamTerminal::Completed(response)) => Ok(response),
            Some(ResponseStreamTerminal::Failed(_)) => Err(OpenAIError::new(
                ErrorKind::Api(ApiErrorKind::Server),
                "response stream ended in a failed terminal state",
            )),
            Some(ResponseStreamTerminal::Incomplete(_)) => Err(OpenAIError::new(
                ErrorKind::Parse,
                "response stream ended in an incomplete terminal state",
            )),
            None => Err(OpenAIError::new(
                ErrorKind::Parse,
                "response stream ended without a terminal state",
            )),
        }
    }

    pub fn parse_final<T>(
        &mut self,
        text: Option<ResponseTextConfig>,
        tools: &[FunctionTool],
    ) -> Result<ParsedResponse<T>, OpenAIError>
    where
        T: DeserializeOwned,
    {
        let response = self.final_response()?.clone();
        parse_response_output(response, text.and_then(|text| text.format), tools)
    }

    fn start_live(
        request: PreparedRequest,
        options: ResolvedRequestOptions,
        starting_after: Option<u64>,
    ) -> Result<Self, OpenAIError> {
        let (startup_tx, startup_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let (abort_tx, abort_rx) = watch::channel(false);
        let shared = Arc::new(LiveStreamShared::default());
        let thread_shared = shared.clone();

        let worker = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let error = OpenAIError::new(
                        ErrorKind::Transport,
                        format!("failed to build streaming runtime: {error}"),
                    )
                    .with_source(error);
                    let _ = startup_tx.send(Err(error.clone()));
                    thread_shared.finish_with_error(error);
                    return;
                }
            };

            runtime.block_on(async move {
                match crate::core::transport::execute_text_stream(&request, &options).await {
                    Ok(response) => {
                        let metadata = response.metadata.clone();
                        let _ = startup_tx.send(Ok(metadata.clone()));
                        let stream_events = event_tx.clone();
                        if let Err(error) = consume_live_stream(
                            response,
                            starting_after,
                            abort_rx,
                            stream_events,
                            thread_shared.clone(),
                        )
                        .await
                        {
                            thread_shared.finish_with_error(error.clone());
                            let _ = event_tx.send(LiveStreamMessage::Error(error));
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
                format!("stream worker exited before startup completed: {error}"),
            )
        })??;

        Ok(Self {
            metadata,
            events: VecDeque::new(),
            current_snapshot: None,
            final_terminal: None,
            live: Some(LiveStreamHandle {
                receiver: event_rx,
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
            if self.final_terminal.is_none() {
                self.final_terminal = live.shared.terminal_cloned();
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

    fn process_live_message(&mut self, message: LiveStreamMessage) {
        match message {
            LiveStreamMessage::Event(recorded) => {
                if let Some(terminal) = terminal_from_event(&recorded.event) {
                    self.final_terminal = Some(terminal);
                }
                self.events.push_back(*recorded);
            }
            LiveStreamMessage::Finished => {
                if let Some(live) = self.live.as_mut() {
                    if self.final_terminal.is_none() {
                        self.final_terminal = live.shared.terminal_cloned();
                    }
                    live.join_worker();
                }
                self.live = None;
            }
            LiveStreamMessage::Error(error) => {
                if let Some(live) = self.live.as_mut() {
                    live.shared.finish_with_error(error);
                    live.join_worker();
                }
                self.live = None;
            }
        }
    }

    pub(crate) fn from_sse_chunks_with_resume<I, B>(
        metadata: ResponseMetadata,
        chunks: I,
        starting_after: Option<u64>,
    ) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        let mut parser = SseParser::default();
        let mut state = StreamAccumulator::new(starting_after);
        for chunk in chunks {
            for frame in parser.push(chunk.as_ref().as_bytes())? {
                state.ingest_frame(frame)?;
            }
        }
        for frame in parser.finish()? {
            state.ingest_frame(frame)?;
        }
        state.finish(metadata)
    }
}

impl Drop for ResponseStream {
    fn drop(&mut self) {
        if let Some(live) = &mut self.live {
            let _ = live.abort.send(true);
            live.join_worker();
        }
    }
}

#[derive(Debug)]
enum LiveStreamMessage {
    Event(Box<RecordedResponseEvent>),
    Finished,
    Error(OpenAIError),
}

#[derive(Debug)]
struct LiveStreamHandle {
    receiver: Receiver<LiveStreamMessage>,
    abort: watch::Sender<bool>,
    worker: Option<thread::JoinHandle<()>>,
    shared: Arc<LiveStreamShared>,
}

impl LiveStreamHandle {
    fn join_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug, Default)]
struct LiveStreamShared {
    state: Mutex<LiveStreamSharedState>,
    done: Condvar,
}

impl LiveStreamShared {
    fn finish_with_terminal(&self, terminal: Option<ResponseStreamTerminal>) {
        let mut state = self.state.lock().expect("live stream shared state");
        state.terminal = terminal;
        state.finished = true;
        self.done.notify_all();
    }

    fn finish_with_error(&self, error: OpenAIError) {
        let mut state = self.state.lock().expect("live stream shared state");
        state.error = Some(error);
        state.finished = true;
        self.done.notify_all();
    }

    fn wait_until_finished(&self) {
        let mut state = self.state.lock().expect("live stream shared state");
        while !state.finished {
            state = self.done.wait(state).expect("live stream shared state");
        }
    }

    fn terminal_cloned(&self) -> Option<ResponseStreamTerminal> {
        self.state
            .lock()
            .expect("live stream shared state")
            .terminal
            .clone()
    }

    fn error(&self) -> Option<OpenAIError> {
        self.state
            .lock()
            .expect("live stream shared state")
            .error
            .clone()
    }
}

#[derive(Debug, Default)]
struct LiveStreamSharedState {
    terminal: Option<ResponseStreamTerminal>,
    error: Option<OpenAIError>,
    finished: bool,
}

/// Public typed list envelope for `responses.input_items.list`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseInputItemsPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<ResponseOutputItem>,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ResponseInputItemsPage {
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

/// Text response config for create/parse/input-token helpers.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct ResponseTextConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<ResponseFormatTextConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<String>,
}

/// Response text format variants.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseFormatTextConfig {
    Text,
    JsonSchema(ResponseFormatTextJSONSchemaConfig),
}

impl Serialize for ResponseFormatTextConfig {
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
            Self::JsonSchema(config) => config.serialize(serializer),
        }
    }
}

/// JSON-schema response format for structured output parsing.
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseFormatTextJSONSchemaConfig {
    pub name: String,
    pub schema: Value,
    pub description: Option<String>,
    pub strict: Option<bool>,
}

impl Serialize for ResponseFormatTextJSONSchemaConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct WireJsonSchemaFormat<'a> {
            name: &'a str,
            schema: &'a Value,
            #[serde(rename = "type")]
            format_type: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<&'a String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            strict: Option<bool>,
        }

        WireJsonSchemaFormat {
            name: &self.name,
            schema: &self.schema,
            format_type: "json_schema",
            description: self.description.as_ref(),
            strict: self.strict,
        }
        .serialize(serializer)
    }
}

/// Function tool definition for non-stream parse and input-token helpers.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionTool {
    pub name: String,
    pub parameters: Value,
    pub strict: Option<bool>,
    pub description: Option<String>,
    pub defer_loading: Option<bool>,
}

impl Serialize for FunctionTool {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct WireFunctionTool<'a> {
            #[serde(rename = "type")]
            tool_type: &'a str,
            name: &'a str,
            parameters: &'a Value,
            #[serde(skip_serializing_if = "Option::is_none")]
            strict: Option<bool>,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<&'a String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            defer_loading: Option<bool>,
        }

        WireFunctionTool {
            tool_type: "function",
            name: &self.name,
            parameters: &self.parameters,
            strict: self.strict,
            description: self.description.as_ref(),
            defer_loading: self.defer_loading,
        }
        .serialize(serializer)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct WireResponse {
    id: String,
    object: String,
    created_at: i64,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    output: Vec<ResponseOutputItem>,
    #[serde(default)]
    previous_response_id: Option<String>,
    #[serde(default)]
    conversation: Option<Value>,
    #[serde(default)]
    store: Option<bool>,
    #[serde(default)]
    background: Option<bool>,
    #[serde(default)]
    usage: Value,
    #[serde(default)]
    error: Option<Value>,
    #[serde(default)]
    incomplete_details: Option<Value>,
    #[serde(default)]
    metadata: Option<Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl From<WireResponse> for Response {
    fn from(value: WireResponse) -> Self {
        let output_text = aggregate_output_text(&value.output);
        Self {
            id: value.id,
            object: value.object,
            created_at: value.created_at,
            status: value.status,
            output: value.output,
            previous_response_id: value.previous_response_id,
            conversation: value.conversation,
            store: value.store,
            background: value.background,
            usage: value.usage,
            error: value.error,
            incomplete_details: value.incomplete_details,
            metadata: value.metadata,
            extra: value.extra,
            output_text,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct WireCompactedResponse {
    id: String,
    object: String,
    created_at: i64,
    #[serde(default)]
    output: Vec<ResponseOutputItem>,
    #[serde(default)]
    usage: Value,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl From<WireCompactedResponse> for CompactedResponse {
    fn from(value: WireCompactedResponse) -> Self {
        Self {
            id: value.id,
            object: value.object,
            created_at: value.created_at,
            output: value.output,
            usage: value.usage,
            extra: value.extra,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct StreamTextDeltaPayload {
    output_index: usize,
    content_index: usize,
    delta: String,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamAudioDeltaPayload {
    delta: String,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamTextDonePayload {
    output_index: usize,
    content_index: usize,
    text: String,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamOutputItemPayload {
    output_index: usize,
    item: ResponseOutputItem,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamContentPartPayload {
    output_index: usize,
    content_index: usize,
    part: ResponseContentPart,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamFunctionArgumentsDonePayload {
    output_index: usize,
    name: String,
    arguments: String,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamToolTextDeltaPayload {
    output_index: usize,
    delta: String,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamToolTextDonePayload {
    output_index: usize,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
    #[serde(default)]
    code: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamToolStatusPayload {
    output_index: usize,
    #[serde(default)]
    item_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamImageGenerationPartialImagePayload {
    output_index: usize,
    #[serde(default)]
    item_id: Option<String>,
    partial_image_b64: String,
    partial_image_index: usize,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamOutputTextAnnotationAddedPayload {
    output_index: usize,
    content_index: usize,
    annotation_index: usize,
    #[serde(default)]
    item_id: Option<String>,
    annotation: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamReasoningSummaryPartPayload {
    output_index: usize,
    summary_index: usize,
    #[serde(default)]
    item_id: Option<String>,
    part: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamReasoningSummaryTextDeltaPayload {
    output_index: usize,
    summary_index: usize,
    #[serde(default)]
    item_id: Option<String>,
    delta: String,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamReasoningSummaryTextDonePayload {
    output_index: usize,
    summary_index: usize,
    #[serde(default)]
    item_id: Option<String>,
    text: String,
}

#[derive(Clone, Debug)]
struct StreamAccumulator {
    visible_events: VecDeque<RecordedResponseEvent>,
    snapshot: Option<Response>,
    terminal: Option<ResponseStreamTerminal>,
    seen_done: bool,
    starting_after: Option<u64>,
}

impl StreamAccumulator {
    fn new(starting_after: Option<u64>) -> Self {
        Self {
            visible_events: VecDeque::new(),
            snapshot: None,
            terminal: None,
            seen_done: false,
            starting_after,
        }
    }

    fn ingest_frame(&mut self, frame: SseFrame) -> Result<(), OpenAIError> {
        if frame.data.trim() == "[DONE]" {
            self.seen_done = true;
            return Ok(());
        }

        let event_name = frame.event.unwrap_or_default();
        let sequence_number = extract_sequence_number(&frame.data);
        let surfaced = self.apply_event(&event_name, &frame.data)?;
        let hidden = self
            .starting_after
            .zip(sequence_number)
            .is_some_and(|(starting_after, sequence_number)| sequence_number <= starting_after);
        if hidden {
            return Ok(());
        }
        if let Some(event) = surfaced {
            self.visible_events.push_back(RecordedResponseEvent {
                event,
                snapshot_after_event: self.snapshot.clone(),
            });
        }
        Ok(())
    }

    fn finish(self, metadata: ResponseMetadata) -> Result<ResponseStream, OpenAIError> {
        if self.seen_done && self.terminal.is_none() {
            return Err(OpenAIError::new(
                ErrorKind::Parse,
                "response stream received [DONE] before any terminal response event",
            ));
        }
        if !self.seen_done && self.terminal.is_none() {
            return Err(OpenAIError::new(
                ErrorKind::Parse,
                "response stream ended without a terminal response event",
            ));
        }

        Ok(ResponseStream {
            metadata,
            events: self.visible_events,
            current_snapshot: None,
            final_terminal: self.terminal,
            live: None,
            aborted: false,
        })
    }

    fn apply_event(
        &mut self,
        event_name: &str,
        data: &str,
    ) -> Result<Option<ResponseStreamEvent>, OpenAIError> {
        match event_name {
            "response.created" => {
                let response: Response = serde_json::from_str::<WireResponse>(data)
                    .map(Response::from)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.snapshot = Some(response.clone());
                Ok(Some(ResponseStreamEvent::Created { response }))
            }
            "response.queued" => {
                let response: Response = serde_json::from_str::<WireResponse>(data)
                    .map(Response::from)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.snapshot = Some(response.clone());
                Ok(Some(ResponseStreamEvent::Queued { response }))
            }
            "response.in_progress" => {
                let response: Response = serde_json::from_str::<WireResponse>(data)
                    .map(Response::from)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.snapshot = Some(response.clone());
                Ok(Some(ResponseStreamEvent::InProgress { response }))
            }
            "response.output_item.added" => {
                let payload: StreamOutputItemPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.insert_output_item(payload.output_index, payload.item)?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.output_item.done" => {
                let payload: StreamOutputItemPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_output_item(payload.output_index, payload.item)?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.content_part.added" => {
                let payload: StreamContentPartPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.insert_content_part(
                    payload.output_index,
                    payload.content_index,
                    payload.part,
                )?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.content_part.done" => {
                let payload: StreamContentPartPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_content_part(
                    payload.output_index,
                    payload.content_index,
                    payload.part,
                )?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.output_text.delta" => {
                let payload: StreamTextDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.append_content_text(
                    payload.output_index,
                    payload.content_index,
                    "output_text",
                    &payload.delta,
                )?;
                Ok(Some(ResponseStreamEvent::OutputTextDelta {
                    output_index: payload.output_index,
                    content_index: payload.content_index,
                    delta: payload.delta,
                }))
            }
            "response.output_text.done" => {
                let payload: StreamTextDonePayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_content_text(
                    payload.output_index,
                    payload.content_index,
                    "output_text",
                    &payload.text,
                )?;
                Ok(Some(ResponseStreamEvent::OutputTextDone {
                    output_index: payload.output_index,
                    content_index: payload.content_index,
                    text: payload.text,
                }))
            }
            "response.audio.delta" => {
                let payload: StreamAudioDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                Ok(Some(ResponseStreamEvent::AudioDelta {
                    delta: payload.delta,
                }))
            }
            "response.audio.done" => Ok(Some(ResponseStreamEvent::AudioDone)),
            "response.audio.transcript.delta" => {
                let payload: StreamAudioDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                Ok(Some(ResponseStreamEvent::AudioTranscriptDelta {
                    delta: payload.delta,
                }))
            }
            "response.audio.transcript.done" => Ok(Some(ResponseStreamEvent::AudioTranscriptDone)),
            "response.function_call_arguments.delta" => {
                let payload: StreamToolTextDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.append_output_text_field(
                    payload.output_index,
                    &["function_call"],
                    "arguments",
                    &payload.delta,
                )?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.function_call_arguments.done" => {
                let payload: StreamFunctionArgumentsDonePayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_output_text_field(
                    payload.output_index,
                    &["function_call"],
                    "arguments",
                    &payload.arguments,
                )?;
                self.get_output_mut(payload.output_index, &["function_call"])?
                    .name = Some(payload.name);
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.custom_tool_call_input.delta" => {
                let payload: StreamToolTextDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.append_output_text_field(
                    payload.output_index,
                    &["custom_tool_call"],
                    "input",
                    &payload.delta,
                )?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.custom_tool_call_input.done" => {
                let payload: StreamToolTextDonePayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                let input = payload.input.or(payload.text).ok_or_else(|| {
                    OpenAIError::new(
                        ErrorKind::Parse,
                        "response.custom_tool_call_input.done payload missing input",
                    )
                })?;
                self.replace_output_text_field(
                    payload.output_index,
                    &["custom_tool_call"],
                    "input",
                    &input,
                )?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.code_interpreter_call_code.delta" => {
                let payload: StreamToolTextDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.append_output_text_field(
                    payload.output_index,
                    &["code_interpreter_call"],
                    "code",
                    &payload.delta,
                )?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.code_interpreter_call_code.done" => {
                let payload: StreamToolTextDonePayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                let code = payload.code.or(payload.text).ok_or_else(|| {
                    OpenAIError::new(
                        ErrorKind::Parse,
                        "response.code_interpreter_call_code.done payload missing code",
                    )
                })?;
                self.replace_output_text_field(
                    payload.output_index,
                    &["code_interpreter_call"],
                    "code",
                    &code,
                )?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.mcp_call_arguments.delta" => {
                let payload: StreamToolTextDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.append_output_text_field(
                    payload.output_index,
                    &["mcp_call"],
                    "arguments",
                    &payload.delta,
                )?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.mcp_call_arguments.done" => {
                let payload: StreamToolTextDonePayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                let arguments = payload.arguments.ok_or_else(|| {
                    OpenAIError::new(
                        ErrorKind::Parse,
                        "response.mcp_call_arguments.done payload missing arguments",
                    )
                })?;
                self.replace_output_text_field(
                    payload.output_index,
                    &["mcp_call"],
                    "arguments",
                    &arguments,
                )?;
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: event_name.to_string(),
                    data: serde_json::from_str(data)
                        .unwrap_or_else(|_| Value::String(data.to_string())),
                }))
            }
            "response.output_text.annotation.added" => {
                let payload: StreamOutputTextAnnotationAddedPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.insert_output_text_annotation(
                    payload.output_index,
                    payload.content_index,
                    payload.annotation_index,
                    payload.annotation.clone(),
                )?;
                Ok(Some(ResponseStreamEvent::OutputTextAnnotationAdded {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    content_index: payload.content_index,
                    annotation_index: payload.annotation_index,
                    annotation: payload.annotation,
                }))
            }
            "response.reasoning_text.delta" => {
                let payload: StreamTextDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.append_content_text(
                    payload.output_index,
                    payload.content_index,
                    "reasoning_text",
                    &payload.delta,
                )?;
                Ok(Some(ResponseStreamEvent::ReasoningTextDelta {
                    output_index: payload.output_index,
                    content_index: payload.content_index,
                    delta: payload.delta,
                }))
            }
            "response.reasoning_text.done" => {
                let payload: StreamTextDonePayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_content_text(
                    payload.output_index,
                    payload.content_index,
                    "reasoning_text",
                    &payload.text,
                )?;
                Ok(Some(ResponseStreamEvent::ReasoningTextDone {
                    output_index: payload.output_index,
                    content_index: payload.content_index,
                    text: payload.text,
                }))
            }
            "response.reasoning_summary_part.added" => {
                let payload: StreamReasoningSummaryPartPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.insert_reasoning_summary_part(
                    payload.output_index,
                    payload.summary_index,
                    payload.part.clone(),
                )?;
                Ok(Some(ResponseStreamEvent::ReasoningSummaryPartAdded {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    summary_index: payload.summary_index,
                    part: payload.part,
                }))
            }
            "response.reasoning_summary_part.done" => {
                let payload: StreamReasoningSummaryPartPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_reasoning_summary_part(
                    payload.output_index,
                    payload.summary_index,
                    payload.part.clone(),
                )?;
                Ok(Some(ResponseStreamEvent::ReasoningSummaryPartDone {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    summary_index: payload.summary_index,
                    part: payload.part,
                }))
            }
            "response.reasoning_summary_text.delta" => {
                let payload: StreamReasoningSummaryTextDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.append_reasoning_summary_text(
                    payload.output_index,
                    payload.summary_index,
                    &payload.delta,
                )?;
                Ok(Some(ResponseStreamEvent::ReasoningSummaryTextDelta {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    summary_index: payload.summary_index,
                    delta: payload.delta,
                }))
            }
            "response.reasoning_summary_text.done" => {
                let payload: StreamReasoningSummaryTextDonePayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_reasoning_summary_text(
                    payload.output_index,
                    payload.summary_index,
                    &payload.text,
                )?;
                Ok(Some(ResponseStreamEvent::ReasoningSummaryTextDone {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    summary_index: payload.summary_index,
                    text: payload.text,
                }))
            }
            "response.refusal.delta" => {
                let payload: StreamTextDeltaPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.append_content_text(
                    payload.output_index,
                    payload.content_index,
                    "refusal",
                    &payload.delta,
                )?;
                Ok(Some(ResponseStreamEvent::RefusalDelta {
                    output_index: payload.output_index,
                    content_index: payload.content_index,
                    delta: payload.delta,
                }))
            }
            "response.refusal.done" => {
                let payload: StreamTextDonePayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_content_text(
                    payload.output_index,
                    payload.content_index,
                    "refusal",
                    &payload.text,
                )?;
                Ok(Some(ResponseStreamEvent::RefusalDone {
                    output_index: payload.output_index,
                    content_index: payload.content_index,
                    text: payload.text,
                }))
            }
            "response.completed" => {
                let response: Response = serde_json::from_str::<WireResponse>(data)
                    .map(Response::from)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.snapshot = Some(response.clone());
                self.terminal = Some(ResponseStreamTerminal::Completed(response.clone()));
                Ok(Some(ResponseStreamEvent::Completed { response }))
            }
            "response.failed" => {
                let response: Response = serde_json::from_str::<WireResponse>(data)
                    .map(Response::from)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.snapshot = Some(response.clone());
                self.terminal = Some(ResponseStreamTerminal::Failed(response.clone()));
                Ok(Some(ResponseStreamEvent::Failed { response }))
            }
            "response.incomplete" => {
                let response: Response = serde_json::from_str::<WireResponse>(data)
                    .map(Response::from)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.snapshot = Some(response.clone());
                self.terminal = Some(ResponseStreamTerminal::Incomplete(response.clone()));
                Ok(Some(ResponseStreamEvent::Incomplete { response }))
            }
            "response.file_search_call.in_progress"
            | "response.file_search_call.searching"
            | "response.file_search_call.completed"
            | "response.web_search_call.in_progress"
            | "response.web_search_call.searching"
            | "response.web_search_call.completed"
            | "response.code_interpreter_call.in_progress"
            | "response.code_interpreter_call.interpreting"
            | "response.code_interpreter_call.completed"
            | "response.code_interpreter_call.failed"
            | "response.image_generation_call.in_progress"
            | "response.image_generation_call.generating"
            | "response.image_generation_call.completed"
            | "response.mcp_call.in_progress"
            | "response.mcp_call.completed"
            | "response.mcp_call.failed"
            | "response.mcp_list_tools.in_progress"
            | "response.mcp_list_tools.completed"
            | "response.mcp_list_tools.failed" => {
                let payload: StreamToolStatusPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                let status = event_name.rsplit('.').next().unwrap_or_default();
                self.set_output_status(payload.output_index, status)?;
                match event_name {
                    "response.image_generation_call.in_progress" => {
                        Ok(Some(ResponseStreamEvent::ImageGenerationCallInProgress {
                            item_id: payload.item_id,
                            output_index: payload.output_index,
                        }))
                    }
                    "response.image_generation_call.generating" => {
                        Ok(Some(ResponseStreamEvent::ImageGenerationCallGenerating {
                            item_id: payload.item_id,
                            output_index: payload.output_index,
                        }))
                    }
                    "response.image_generation_call.completed" => {
                        Ok(Some(ResponseStreamEvent::ImageGenerationCallCompleted {
                            item_id: payload.item_id,
                            output_index: payload.output_index,
                        }))
                    }
                    _ => Ok(Some(ResponseStreamEvent::Unknown {
                        event: event_name.to_string(),
                        data: serde_json::from_str(data)
                            .unwrap_or_else(|_| Value::String(data.to_string())),
                    })),
                }
            }
            "response.image_generation_call.partial_image" => {
                let payload: StreamImageGenerationPartialImagePayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                Ok(Some(ResponseStreamEvent::ImageGenerationCallPartialImage {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    partial_image_b64: payload.partial_image_b64,
                    partial_image_index: payload.partial_image_index,
                }))
            }
            other => {
                let data =
                    serde_json::from_str(data).unwrap_or_else(|_| Value::String(data.to_string()));
                Ok(Some(ResponseStreamEvent::Unknown {
                    event: other.to_string(),
                    data,
                }))
            }
        }
    }

    fn append_content_text(
        &mut self,
        output_index: usize,
        content_index: usize,
        expected_type: &str,
        delta: &str,
    ) -> Result<(), OpenAIError> {
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "received stream delta before response.created",
            )
        })?;
        let content = get_content_mut(snapshot, output_index, content_index, expected_type)?;
        let current = content.text.get_or_insert_with(String::new);
        current.push_str(delta);
        snapshot.output_text = aggregate_output_text(&snapshot.output);
        Ok(())
    }

    fn replace_content_text(
        &mut self,
        output_index: usize,
        content_index: usize,
        expected_type: &str,
        text: &str,
    ) -> Result<(), OpenAIError> {
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "received stream completion before response.created",
            )
        })?;
        let content = get_content_mut(snapshot, output_index, content_index, expected_type)?;
        content.text = Some(text.to_string());
        snapshot.output_text = aggregate_output_text(&snapshot.output);
        Ok(())
    }

    fn insert_output_item(
        &mut self,
        output_index: usize,
        item: ResponseOutputItem,
    ) -> Result<(), OpenAIError> {
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "received output item before response.created",
            )
        })?;
        if output_index > snapshot.output.len() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing output_index {output_index}"),
            ));
        }
        snapshot.output.insert(output_index, item);
        snapshot.output_text = aggregate_output_text(&snapshot.output);
        Ok(())
    }

    fn replace_output_item(
        &mut self,
        output_index: usize,
        item: ResponseOutputItem,
    ) -> Result<(), OpenAIError> {
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "received output item completion before response.created",
            )
        })?;
        let output = snapshot.output.get_mut(output_index).ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing output_index {output_index}"),
            )
        })?;
        *output = item;
        snapshot.output_text = aggregate_output_text(&snapshot.output);
        Ok(())
    }

    fn insert_content_part(
        &mut self,
        output_index: usize,
        content_index: usize,
        part: ResponseContentPart,
    ) -> Result<(), OpenAIError> {
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "received content part before response.created",
            )
        })?;
        let item = snapshot.output.get_mut(output_index).ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing output_index {output_index}"),
            )
        })?;
        if item.item_type != "message" {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced non-message output item at index {output_index}"),
            ));
        }
        if content_index > item.content.len() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing content_index {content_index}"),
            ));
        }
        item.content.insert(content_index, part);
        snapshot.output_text = aggregate_output_text(&snapshot.output);
        Ok(())
    }

    fn replace_content_part(
        &mut self,
        output_index: usize,
        content_index: usize,
        part: ResponseContentPart,
    ) -> Result<(), OpenAIError> {
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "received content part completion before response.created",
            )
        })?;
        let item = snapshot.output.get_mut(output_index).ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing output_index {output_index}"),
            )
        })?;
        if item.item_type != "message" {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced non-message output item at index {output_index}"),
            ));
        }
        let content = item.content.get_mut(content_index).ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing content_index {content_index}"),
            )
        })?;
        *content = part;
        snapshot.output_text = aggregate_output_text(&snapshot.output);
        Ok(())
    }

    fn append_output_text_field(
        &mut self,
        output_index: usize,
        expected_types: &[&str],
        field: &str,
        delta: &str,
    ) -> Result<(), OpenAIError> {
        let output = self.get_output_mut(output_index, expected_types)?;
        let target = match field {
            "arguments" => output.arguments.get_or_insert_with(String::new),
            "input" => output.input.get_or_insert_with(String::new),
            "code" => output.code.get_or_insert_with(String::new),
            _ => {
                return Err(OpenAIError::new(
                    ErrorKind::Validation,
                    format!("unsupported streamed output field `{field}`"),
                ));
            }
        };
        target.push_str(delta);
        Ok(())
    }

    fn replace_output_text_field(
        &mut self,
        output_index: usize,
        expected_types: &[&str],
        field: &str,
        value: &str,
    ) -> Result<(), OpenAIError> {
        let output = self.get_output_mut(output_index, expected_types)?;
        match field {
            "arguments" => output.arguments = Some(value.to_string()),
            "input" => output.input = Some(value.to_string()),
            "code" => output.code = Some(value.to_string()),
            _ => {
                return Err(OpenAIError::new(
                    ErrorKind::Validation,
                    format!("unsupported streamed output field `{field}`"),
                ));
            }
        }
        Ok(())
    }

    fn set_output_status(&mut self, output_index: usize, status: &str) -> Result<(), OpenAIError> {
        let output = self.get_output_mut(output_index, &[])?;
        output.status = Some(status.to_string());
        Ok(())
    }

    fn insert_output_text_annotation(
        &mut self,
        output_index: usize,
        content_index: usize,
        annotation_index: usize,
        annotation: Value,
    ) -> Result<(), OpenAIError> {
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "received output text annotation before response.created",
            )
        })?;
        let content = get_content_mut(snapshot, output_index, content_index, "output_text")?;
        let annotations = content
            .extra
            .entry(String::from("annotations"))
            .or_insert_with(|| Value::Array(Vec::new()));
        let annotations = annotations.as_array_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "stream addressed non-array output text annotations",
            )
        })?;
        if annotation_index > annotations.len() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing annotation_index {annotation_index}"),
            ));
        }
        annotations.insert(annotation_index, annotation);
        Ok(())
    }

    fn insert_reasoning_summary_part(
        &mut self,
        output_index: usize,
        summary_index: usize,
        part: Value,
    ) -> Result<(), OpenAIError> {
        let summary = self.reasoning_summary_mut(output_index)?;
        if summary_index > summary.len() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing summary_index {summary_index}"),
            ));
        }
        summary.insert(summary_index, part);
        Ok(())
    }

    fn replace_reasoning_summary_part(
        &mut self,
        output_index: usize,
        summary_index: usize,
        part: Value,
    ) -> Result<(), OpenAIError> {
        let summary = self.reasoning_summary_mut(output_index)?;
        let slot = summary.get_mut(summary_index).ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing summary_index {summary_index}"),
            )
        })?;
        *slot = part;
        Ok(())
    }

    fn append_reasoning_summary_text(
        &mut self,
        output_index: usize,
        summary_index: usize,
        delta: &str,
    ) -> Result<(), OpenAIError> {
        let part = self.reasoning_summary_part_mut(output_index, summary_index)?;
        let part = part.as_object_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "stream addressed non-object reasoning summary part",
            )
        })?;
        let text = part
            .entry(String::from("text"))
            .or_insert_with(|| Value::String(String::new()));
        let updated = {
            let current = text.as_str().ok_or_else(|| {
                OpenAIError::new(
                    ErrorKind::Validation,
                    "stream addressed non-string reasoning summary text",
                )
            })?;
            format!("{current}{delta}")
        };
        *text = Value::String(updated);
        Ok(())
    }

    fn replace_reasoning_summary_text(
        &mut self,
        output_index: usize,
        summary_index: usize,
        text: &str,
    ) -> Result<(), OpenAIError> {
        let part = self.reasoning_summary_part_mut(output_index, summary_index)?;
        let part = part.as_object_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "stream addressed non-object reasoning summary part",
            )
        })?;
        part.insert(String::from("text"), Value::String(text.to_string()));
        Ok(())
    }

    fn reasoning_summary_part_mut(
        &mut self,
        output_index: usize,
        summary_index: usize,
    ) -> Result<&mut Value, OpenAIError> {
        let summary = self.reasoning_summary_mut(output_index)?;
        summary.get_mut(summary_index).ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing summary_index {summary_index}"),
            )
        })
    }

    fn reasoning_summary_mut(
        &mut self,
        output_index: usize,
    ) -> Result<&mut Vec<Value>, OpenAIError> {
        let output = self.get_output_mut(output_index, &["reasoning"])?;
        let summary = output
            .extra
            .entry(String::from("summary"))
            .or_insert_with(|| Value::Array(Vec::new()));
        summary.as_array_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "stream addressed non-array reasoning summary",
            )
        })
    }

    fn get_output_mut<'a>(
        &'a mut self,
        output_index: usize,
        expected_types: &[&str],
    ) -> Result<&'a mut ResponseOutputItem, OpenAIError> {
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "received output mutation before response.created",
            )
        })?;
        let output = snapshot.output.get_mut(output_index).ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing output_index {output_index}"),
            )
        })?;
        if !expected_types.is_empty() && !expected_types.contains(&output.item_type.as_str()) {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                format!(
                    "stream addressed output item type `{}` but expected one of {:?}",
                    output.item_type, expected_types
                ),
            ));
        }
        Ok(output)
    }
}

fn map_response(response: ApiResponse<WireResponse>) -> ApiResponse<Response> {
    ApiResponse {
        output: response.output.into(),
        metadata: response.metadata,
    }
}

fn parse_response_output<T>(
    mut response: Response,
    text_format: Option<ResponseFormatTextConfig>,
    tools: &[FunctionTool],
) -> Result<ParsedResponse<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    for item in &response.output {
        if item.item_type != "message" {
            continue;
        }
        for content in &item.content {
            if content.content_type == "refusal" {
                let refusal = content
                    .refusal
                    .clone()
                    .or_else(|| content.text.clone())
                    .unwrap_or_else(|| String::from("model refusal"));
                return Err(OpenAIError::new(
                    ErrorKind::Parse,
                    format!("response refusal prevents structured parsing: {refusal}"),
                ));
            }
        }
    }

    for item in &mut response.output {
        if item.item_type != "function_call" {
            continue;
        }
        let Some(name) = item.name.as_deref() else {
            continue;
        };
        let Some(arguments) = item.arguments.as_deref() else {
            continue;
        };
        let Some(tool) = tools.iter().find(|tool| tool.name == name) else {
            continue;
        };
        if tool.strict == Some(true) {
            let parsed_arguments = serde_json::from_str(arguments).map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Parse,
                    format!("failed to parse strict tool arguments for `{name}`: {error}"),
                )
                .with_source(error)
            })?;
            item.parsed_arguments = Some(parsed_arguments);
        }
    }

    let output_parsed = match text_format {
        Some(ResponseFormatTextConfig::JsonSchema(_)) => {
            parse_structured_output_from_content_parts(&response.output)?
        }
        _ => None,
    };

    Ok(ParsedResponse {
        id: response.id,
        object: response.object,
        created_at: response.created_at,
        status: response.status,
        output: response.output,
        previous_response_id: response.previous_response_id,
        conversation: response.conversation,
        store: response.store,
        background: response.background,
        usage: response.usage,
        error: response.error,
        incomplete_details: response.incomplete_details,
        metadata: response.metadata,
        extra: response.extra,
        output_text: response.output_text,
        output_parsed,
    })
}

fn aggregate_output_text(output: &[ResponseOutputItem]) -> String {
    let mut text = String::new();
    for item in output {
        if item.item_type != "message" {
            continue;
        }
        for content in &item.content {
            if content.content_type == "output_text" {
                if let Some(part) = &content.text {
                    text.push_str(part);
                }
            }
        }
    }
    text
}

fn parse_structured_output_from_content_parts<T>(
    output: &[ResponseOutputItem],
) -> Result<Option<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    let mut first_parse_error = None;

    for item in output {
        if item.item_type != "message" {
            continue;
        }

        for content in &item.content {
            if content.content_type != "output_text" {
                continue;
            }

            let Some(text) = content.text.as_deref().map(str::trim) else {
                continue;
            };
            if text.is_empty() {
                continue;
            }

            match serde_json::from_str(text) {
                Ok(parsed) => return Ok(Some(parsed)),
                Err(error) => {
                    if first_parse_error.is_none() {
                        first_parse_error = Some(error);
                    }
                }
            }
        }
    }

    if let Some(error) = first_parse_error {
        Err(OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse structured output: {error}"),
        )
        .with_source(error))
    } else {
        Ok(None)
    }
}

fn get_content_mut<'a>(
    response: &'a mut Response,
    output_index: usize,
    content_index: usize,
    expected_type: &str,
) -> Result<&'a mut ResponseContentPart, OpenAIError> {
    let item = response.output.get_mut(output_index).ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Validation,
            format!("stream referenced missing output_index {output_index}"),
        )
    })?;
    if item.item_type != "message" {
        return Err(OpenAIError::new(
            ErrorKind::Validation,
            format!("stream referenced non-message output item at index {output_index}"),
        ));
    }
    let content = item.content.get_mut(content_index).ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Validation,
            format!("stream referenced missing content_index {content_index}"),
        )
    })?;
    if content.content_type != expected_type {
        return Err(OpenAIError::new(
            ErrorKind::Validation,
            format!(
                "stream addressed content type `{}` but expected `{expected_type}`",
                content.content_type
            ),
        ));
    }
    Ok(content)
}

async fn consume_live_stream(
    response: crate::core::transport::StreamingTextResponse,
    starting_after: Option<u64>,
    mut abort_rx: watch::Receiver<bool>,
    event_tx: mpsc::Sender<LiveStreamMessage>,
    shared: Arc<LiveStreamShared>,
) -> Result<(), OpenAIError> {
    let metadata = response.metadata.clone();
    let mut response = response.response;
    let mut parser = SseParser::default();
    let mut state = StreamAccumulator::new(starting_after);

    loop {
        tokio::select! {
            changed = abort_rx.changed() => {
                if changed.is_ok() && *abort_rx.borrow() {
                    shared.finish_with_terminal(None);
                    let _ = event_tx.send(LiveStreamMessage::Finished);
                    return Ok(());
                }
            }
            chunk = response.chunk() => {
                let chunk = chunk.map_err(map_live_transport_error)?;
                let Some(chunk) = chunk else {
                    break;
                };
                for frame in parser.push(chunk.as_ref())? {
                    state.ingest_frame(frame)?;
                    drain_visible_events(&mut state, &event_tx);
                }
            }
        }
    }

    for frame in parser.finish()? {
        state.ingest_frame(frame)?;
        drain_visible_events(&mut state, &event_tx);
    }
    let finished = state.finish(metadata)?;
    let terminal = finished.final_terminal.clone();
    shared.finish_with_terminal(terminal);
    let _ = event_tx.send(LiveStreamMessage::Finished);
    Ok(())
}

fn drain_visible_events(state: &mut StreamAccumulator, event_tx: &mpsc::Sender<LiveStreamMessage>) {
    while let Some(recorded) = state.visible_events.pop_front() {
        if event_tx
            .send(LiveStreamMessage::Event(Box::new(recorded)))
            .is_err()
        {
            break;
        }
    }
}

fn extract_sequence_number(data: &str) -> Option<u64> {
    serde_json::from_str::<Value>(data)
        .ok()?
        .get("sequence_number")?
        .as_u64()
}

fn terminal_from_event(event: &ResponseStreamEvent) -> Option<ResponseStreamTerminal> {
    match event {
        ResponseStreamEvent::Completed { response } => {
            Some(ResponseStreamTerminal::Completed(response.clone()))
        }
        ResponseStreamEvent::Failed { response } => {
            Some(ResponseStreamTerminal::Failed(response.clone()))
        }
        ResponseStreamEvent::Incomplete { response } => {
            Some(ResponseStreamTerminal::Incomplete(response.clone()))
        }
        _ => None,
    }
}

fn map_live_transport_error(error: reqwest::Error) -> OpenAIError {
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Transport
    };
    OpenAIError::new(kind, error.to_string()).with_source(error)
}

fn stream_parse_error(error_event: &str, error: serde_json::Error) -> OpenAIError {
    OpenAIError::new(
        ErrorKind::Parse,
        format!("failed to parse streamed `{error_event}` payload: {error}"),
    )
    .with_source(error)
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

fn prepare_responses_ws_target(
    runtime: &ClientRuntime,
    options: ResponsesConnectOptions,
) -> Result<PreparedResponsesWsTarget, OpenAIError> {
    let resolved = runtime.resolved_config()?;
    let mut headers = resolved.headers();
    for (name, value) in options.extra_headers {
        headers.insert(name.to_ascii_lowercase(), value);
    }

    let base_url = normalize_base_url(&resolved.base_url)?;
    let mut url = Url::parse(&base_url).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Configuration,
            format!("invalid OpenAI base URL `{base_url}`: {error}"),
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
                format!("unsupported base URL scheme for Responses websocket: {other}"),
            ));
        }
    };
    url.set_scheme(scheme).map_err(|_| {
        OpenAIError::new(
            ErrorKind::Configuration,
            "failed to convert the configured base URL to a Responses websocket target",
        )
    })?;

    let mut path = url.path().trim_end_matches('/').to_string();
    path.push_str("/responses");
    url.set_path(&path);
    url.set_query(None);
    if !options.extra_query.is_empty() {
        let mut query = url.query_pairs_mut();
        for (key, value) in options.extra_query {
            query.append_pair(&key, &value);
        }
    }

    Ok(PreparedResponsesWsTarget {
        url: url.to_string(),
        headers,
    })
}

fn parse_responses_ws_event(text: &str) -> Result<ResponsesConnectionEvent, OpenAIError> {
    let payload = serde_json::from_str::<Value>(text).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Responses websocket event: {error}"),
        )
        .with_source(error)
    })?;
    let event_type = payload
        .get("type")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    Ok(ResponsesConnectionEvent {
        event_type,
        payload,
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
