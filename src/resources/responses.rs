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
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use url::Url;

use crate::{
    ApiErrorPayload, OpenAIError,
    config::normalize_base_url,
    core::{
        metadata::ResponseMetadata,
        request::{PreparedRequest, RequestOptions, ResolvedRequestOptions},
        response::ApiResponse,
        runtime::ClientRuntime,
    },
    error::{ApiErrorKind, ErrorKind},
    helpers::sse::{SseFrame, SseParser},
    resources::containers::{ContainerMemoryLimit, ContainerNetworkPolicy, ContainerSkill},
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
    pub conversation: Option<ResponseConversation>,
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
    pub prompt: Option<ResponsePrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ResponseReasoning>,
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
    pub tool_choice: Option<ResponseToolChoice>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<ResponseTool>,
    /// Non-function Responses tools, such as built-in web/file search, MCP, code
    /// interpreter, image generation, shell, or custom tool payloads.
    #[serde(skip)]
    pub raw_tools: Vec<Value>,
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

    fn into_request_body(mut self) -> Value {
        let raw_tools = std::mem::take(&mut self.raw_tools);
        let mut value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        merge_raw_tools(&mut value, raw_tools);
        if let Value::Object(ref mut object) = value {
            object.insert(String::from("stream"), Value::Bool(false));
        }
        value
    }

    fn into_stream_request_body(mut self) -> Value {
        let raw_tools = std::mem::take(&mut self.raw_tools);
        let mut value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        merge_raw_tools(&mut value, raw_tools);
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
    pub conversation: Option<ResponseConversation>,
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
    pub prompt: Option<ResponsePrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ResponseReasoning>,
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
    pub tool_choice: Option<ResponseToolChoice>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<ResponseTool>,
    /// Non-function Responses tools, such as built-in web/file search, MCP, code
    /// interpreter, image generation, shell, or custom tool payloads.
    #[serde(skip)]
    pub raw_tools: Vec<Value>,
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
            raw_tools: self.raw_tools,
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
    pub conversation: Option<ResponseConversation>,
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
    pub reasoning: Option<ResponseReasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<ResponseTextConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ResponseToolChoice>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<ResponseTool>,
    /// Non-function Responses tools, such as built-in web/file search, MCP, code
    /// interpreter, image generation, shell, or custom tool payloads.
    #[serde(skip)]
    pub raw_tools: Vec<Value>,
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

    fn into_request_body(mut self) -> Value {
        let raw_tools = std::mem::take(&mut self.raw_tools);
        let mut value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        merge_raw_tools(&mut value, raw_tools);
        value
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
        let body = params.into_request_body();
        self.runtime.execute_json_with_body(
            "POST",
            "/responses/input_tokens",
            &body,
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

/// Conversation reference used by Responses requests and returned Responses.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ResponseConversation {
    Id(String),
    Object(ResponseConversationObject),
}

/// Object-shaped conversation reference.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseConversationObject {
    pub id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Reference to a reusable prompt template and its variables.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ResponsePrompt {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Reasoning configuration returned by or sent to the Responses API.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ResponseReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Public parsed response object with aggregated `output_text`.
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub id: String,
    pub object: String,
    pub created_at: f64,
    pub status: Option<String>,
    pub model: Option<String>,
    pub instructions: Option<Value>,
    pub output: Vec<ResponseOutputItem>,
    pub parallel_tool_calls: Option<bool>,
    pub previous_response_id: Option<String>,
    pub conversation: Option<ResponseConversation>,
    pub store: Option<bool>,
    pub background: Option<bool>,
    pub completed_at: Option<f64>,
    pub max_output_tokens: Option<u64>,
    pub max_tool_calls: Option<u64>,
    pub prompt: Option<ResponsePrompt>,
    pub prompt_cache_key: Option<String>,
    pub prompt_cache_retention: Option<String>,
    pub reasoning: Option<ResponseReasoning>,
    pub safety_identifier: Option<String>,
    pub service_tier: Option<String>,
    pub temperature: Option<f64>,
    pub text: Option<ResponseTextConfig>,
    pub tool_choice: Option<ResponseToolChoice>,
    pub tools: Vec<ResponseTool>,
    pub top_logprobs: Option<u64>,
    pub top_p: Option<f64>,
    pub truncation: Option<String>,
    pub user: Option<String>,
    pub usage: Option<ResponseUsage>,
    pub error: Option<ResponseError>,
    pub incomplete_details: Option<ResponseIncompleteDetails>,
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

/// Token usage details for a Responses API payload.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ResponseUsage {
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub input_tokens_details: Option<ResponseInputTokensDetails>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens_details: Option<ResponseOutputTokensDetails>,
    #[serde(default)]
    pub total_tokens: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Detailed breakdown of input tokens.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ResponseInputTokensDetails {
    #[serde(default)]
    pub cached_tokens: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Detailed breakdown of output tokens.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ResponseOutputTokensDetails {
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Error details returned on failed response objects.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ResponseError {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Details about why a response is incomplete.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ResponseIncompleteDetails {
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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
    pub usage: Option<ResponseUsage>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Common item shape used by response and compaction payloads.
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseOutputItem {
    pub id: Option<String>,
    pub item_type: String,
    pub role: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
    /// Raw `arguments` payload. Most tool calls use JSON strings, while
    /// `tool_search_call` returns an object.
    pub arguments_json: Option<Value>,
    pub input: Option<String>,
    pub code: Option<String>,
    pub call_id: Option<String>,
    pub status: Option<String>,
    pub phase: Option<String>,
    pub namespace: Option<String>,
    pub created_by: Option<String>,
    pub action: Option<ResponseItemAction>,
    pub actions: Option<Vec<ResponseComputerAction>>,
    pub operation: Option<ResponseApplyPatchOperation>,
    pub environment: Option<ResponseItemEnvironment>,
    pub execution: Option<String>,
    pub output: Option<ResponseItemOutput>,
    pub result: Option<String>,
    pub queries: Vec<String>,
    pub results: Option<Vec<ResponseFileSearchResult>>,
    pub server_label: Option<String>,
    pub approval_request_id: Option<String>,
    pub approve: Option<bool>,
    pub reason: Option<String>,
    pub error: Option<String>,
    pub tools: Vec<ResponseItemTool>,
    pub summary: Vec<ResponseReasoningSummaryPart>,
    pub encrypted_content: Option<String>,
    pub container_id: Option<String>,
    pub outputs: Option<Vec<ResponseCodeInterpreterOutput>>,
    pub max_output_length: Option<u64>,
    pub pending_safety_checks: Vec<ResponseComputerSafetyCheck>,
    pub acknowledged_safety_checks: Vec<ResponseComputerSafetyCheck>,
    pub content: Vec<ResponseContentPart>,
    pub parsed_arguments: Option<Value>,
    pub extra: BTreeMap<String, Value>,
}

/// Tool entry returned by an MCP list-tools output item.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseMcpListTool {
    #[serde(default)]
    pub input_schema: Value,
    pub name: String,
    #[serde(default)]
    pub annotations: Option<Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Tool entry returned on response output items.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseItemTool {
    McpList(ResponseMcpListTool),
    Definition(Box<ResponseTool>),
    Json(Value),
}

impl ResponseItemTool {
    pub fn as_mcp_list(&self) -> Option<&ResponseMcpListTool> {
        match self {
            Self::McpList(tool) => Some(tool),
            _ => None,
        }
    }

    pub fn as_definition(&self) -> Option<&ResponseTool> {
        match self {
            Self::Definition(tool) => Some(tool.as_ref()),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for ResponseItemTool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Object(object) => {
                let has_tool_type = object.get("type").and_then(Value::as_str).is_some();
                let value = Value::Object(object);
                if has_tool_type {
                    serde_json::from_value(value)
                        .map(Box::new)
                        .map(Self::Definition)
                        .map_err(serde::de::Error::custom)
                } else {
                    match serde_json::from_value(value.clone()) {
                        Ok(tool) => Ok(Self::McpList(tool)),
                        Err(_) => Ok(Self::Json(value)),
                    }
                }
            }
            value => Ok(Self::Json(value)),
        }
    }
}

/// Result item returned by a file-search tool call.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseFileSearchResult {
    #[serde(default)]
    pub attributes: Option<BTreeMap<String, ResponseFileSearchAttributeValue>>,
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Primitive attribute value attached to a file-search result.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ResponseFileSearchAttributeValue {
    String(String),
    Bool(bool),
    Number(f64),
}

/// Output generated by a code-interpreter call.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseCodeInterpreterOutput {
    Logs(ResponseCodeInterpreterOutputLogs),
    Image(ResponseCodeInterpreterOutputImage),
    Other {
        output_type: String,
        extra: BTreeMap<String, Value>,
    },
    Json(Value),
}

impl<'de> Deserialize<'de> for ResponseCodeInterpreterOutput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(object) = value else {
            return Ok(Self::Json(value));
        };
        let Some(output_type) = object
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return Ok(Self::Json(Value::Object(object)));
        };

        match output_type.as_str() {
            "logs" => serde_json::from_value(Value::Object(object))
                .map(Self::Logs)
                .map_err(serde::de::Error::custom),
            "image" => serde_json::from_value(Value::Object(object))
                .map(Self::Image)
                .map_err(serde::de::Error::custom),
            _ => {
                let mut extra = object;
                extra.remove("type");
                Ok(Self::Other {
                    output_type,
                    extra: extra.into_iter().collect(),
                })
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseCodeInterpreterOutputLogs {
    pub logs: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseCodeInterpreterOutputImage {
    pub url: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Safety check entry used by computer-call items.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseComputerSafetyCheck {
    pub id: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Operation requested by an apply-patch tool call.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseApplyPatchOperation {
    CreateFile(ResponseApplyPatchDiffOperation),
    DeleteFile(ResponseApplyPatchDeleteOperation),
    UpdateFile(ResponseApplyPatchDiffOperation),
    Other {
        operation_type: String,
        extra: BTreeMap<String, Value>,
    },
    Json(Value),
}

impl<'de> Deserialize<'de> for ResponseApplyPatchOperation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(object) = value else {
            return Ok(Self::Json(value));
        };
        let Some(operation_type) = object
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return Ok(Self::Json(Value::Object(object)));
        };

        match operation_type.as_str() {
            "create_file" => serde_json::from_value(Value::Object(object))
                .map(Self::CreateFile)
                .map_err(serde::de::Error::custom),
            "delete_file" => serde_json::from_value(Value::Object(object))
                .map(Self::DeleteFile)
                .map_err(serde::de::Error::custom),
            "update_file" => serde_json::from_value(Value::Object(object))
                .map(Self::UpdateFile)
                .map_err(serde::de::Error::custom),
            _ => {
                let mut extra = object;
                extra.remove("type");
                Ok(Self::Other {
                    operation_type,
                    extra: extra.into_iter().collect(),
                })
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseApplyPatchDiffOperation {
    pub diff: String,
    pub path: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseApplyPatchDeleteOperation {
    pub path: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Reasoning summary content emitted by reasoning output items.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseReasoningSummaryPart {
    #[serde(rename = "type", default = "unknown_reasoning_summary_type")]
    pub summary_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn unknown_reasoning_summary_type() -> String {
    String::from("unknown")
}

pub(crate) fn response_reasoning_summary_part_from_value(
    value: Value,
) -> ResponseReasoningSummaryPart {
    serde_json::from_value(value.clone()).unwrap_or_else(|_| ResponseReasoningSummaryPart {
        summary_type: String::from("unknown"),
        text: None,
        extra: BTreeMap::from([(String::from("value"), value)]),
    })
}

/// Action payload used by output items with a generic `action` field.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseItemAction {
    Computer(ResponseComputerAction),
    LocalShell(ResponseLocalShellAction),
    Shell(ResponseShellAction),
    Other {
        action_type: String,
        extra: BTreeMap<String, Value>,
    },
    Json(Value),
}

impl<'de> Deserialize<'de> for ResponseItemAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(object) = value else {
            return Ok(Self::Json(value));
        };

        if object.get("commands").is_some() {
            return serde_json::from_value(Value::Object(object))
                .map(Self::Shell)
                .map_err(serde::de::Error::custom);
        }

        let Some(action_type) = object
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return Ok(Self::Json(Value::Object(object)));
        };

        match action_type.as_str() {
            "exec" => serde_json::from_value(Value::Object(object))
                .map(Self::LocalShell)
                .map_err(serde::de::Error::custom),
            action_type if is_response_computer_action_type(action_type) => {
                serde_json::from_value(Value::Object(object))
                    .map(Self::Computer)
                    .map_err(serde::de::Error::custom)
            }
            _ => {
                let mut extra = object;
                extra.remove("type");
                Ok(Self::Other {
                    action_type,
                    extra: extra.into_iter().collect(),
                })
            }
        }
    }
}

fn is_response_computer_action_type(action_type: &str) -> bool {
    matches!(
        action_type,
        "click"
            | "double_click"
            | "drag"
            | "keypress"
            | "move"
            | "screenshot"
            | "scroll"
            | "type"
            | "wait"
    )
}

/// Execute a shell command on the local shell.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseLocalShellAction {
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Shell commands and limits for a managed shell tool call.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseShellAction {
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub max_output_length: Option<u64>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Environment payload used by shell-call items.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseItemEnvironment {
    Local(ResponseLocalEnvironment),
    ContainerReference(ResponseContainerReference),
    Other {
        environment_type: String,
        extra: BTreeMap<String, Value>,
    },
    Json(Value),
}

impl<'de> Deserialize<'de> for ResponseItemEnvironment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(object) = value else {
            return Ok(Self::Json(value));
        };
        let Some(environment_type) = object
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return Ok(Self::Json(Value::Object(object)));
        };

        match environment_type.as_str() {
            "local" => serde_json::from_value(Value::Object(object))
                .map(Self::Local)
                .map_err(serde::de::Error::custom),
            "container_reference" => serde_json::from_value(Value::Object(object))
                .map(Self::ContainerReference)
                .map_err(serde::de::Error::custom),
            _ => {
                let mut extra = object;
                extra.remove("type");
                Ok(Self::Other {
                    environment_type,
                    extra: extra.into_iter().collect(),
                })
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseLocalEnvironment {
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseContainerReference {
    pub container_id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Computer action requested by a computer-call output item.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseComputerAction {
    Click(ResponseComputerClickAction),
    DoubleClick(ResponseComputerPointAction),
    Drag(ResponseComputerDragAction),
    Keypress(ResponseComputerKeypressAction),
    Move(ResponseComputerPointAction),
    Screenshot,
    Scroll(ResponseComputerScrollAction),
    Type(ResponseComputerTypeAction),
    Wait,
    Other {
        action_type: String,
        extra: BTreeMap<String, Value>,
    },
    Raw(Value),
}

/// Output payload used by output items with a generic `output` field.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseItemOutput {
    Text(String),
    ComputerScreenshot(ResponseComputerScreenshotOutput),
    ContentList(Vec<ResponseContentPart>),
    Shell(Vec<ResponseShellOutput>),
    Json(Value),
}

impl<'de> Deserialize<'de> for ResponseItemOutput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(value) => Ok(Self::Text(value)),
            Value::Object(object)
                if object.get("type").and_then(Value::as_str) == Some("computer_screenshot") =>
            {
                serde_json::from_value(Value::Object(object))
                    .map(Self::ComputerScreenshot)
                    .map_err(serde::de::Error::custom)
            }
            Value::Array(items) if response_output_array_is_shell(&items) => {
                serde_json::from_value(Value::Array(items))
                    .map(Self::Shell)
                    .map_err(serde::de::Error::custom)
            }
            Value::Array(items) if response_output_array_is_content_list(&items) => {
                serde_json::from_value(Value::Array(items))
                    .map(Self::ContentList)
                    .map_err(serde::de::Error::custom)
            }
            other => Ok(Self::Json(other)),
        }
    }
}

fn response_output_array_is_shell(items: &[Value]) -> bool {
    !items.is_empty()
        && items.iter().all(|item| {
            item.get("outcome").is_some()
                && (item.get("stdout").is_some() || item.get("stderr").is_some())
        })
}

fn response_output_array_is_content_list(items: &[Value]) -> bool {
    !items.is_empty()
        && items.iter().all(|item| {
            matches!(
                item.get("type").and_then(Value::as_str),
                Some("input_text" | "input_image" | "input_file")
            )
        })
}

/// Computer screenshot output payload.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseComputerScreenshotOutput {
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Captured output emitted by a shell tool call.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseShellOutput {
    pub outcome: ResponseShellOutputOutcome,
    pub stderr: String,
    pub stdout: String,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Outcome for one emitted shell output chunk.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseShellOutputOutcome {
    Exit(ResponseShellExitOutcome),
    Timeout(ResponseShellTimeoutOutcome),
    Other {
        outcome_type: String,
        extra: BTreeMap<String, Value>,
    },
    Raw(Value),
}

impl<'de> Deserialize<'de> for ResponseShellOutputOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(mut object) = value else {
            return Ok(Self::Raw(value));
        };
        let Some(outcome_type) = object.remove("type").and_then(|value| match value {
            Value::String(value) => Some(value),
            _ => None,
        }) else {
            return Ok(Self::Other {
                outcome_type: String::from("unknown"),
                extra: object.into_iter().collect(),
            });
        };

        let value = Value::Object(object);
        match outcome_type.as_str() {
            "exit" => serde_json::from_value(value)
                .map(Self::Exit)
                .map_err(serde::de::Error::custom),
            "timeout" => serde_json::from_value(value)
                .map(Self::Timeout)
                .map_err(serde::de::Error::custom),
            _ => match value {
                Value::Object(object) => Ok(Self::Other {
                    outcome_type,
                    extra: object.into_iter().collect(),
                }),
                _ => unreachable!("shell outcome helper always receives an object"),
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseShellExitOutcome {
    pub exit_code: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseShellTimeoutOutcome {
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl<'de> Deserialize<'de> for ResponseComputerAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(mut object) = value else {
            return Ok(Self::Raw(value));
        };
        let Some(action_type) = object.remove("type").and_then(|value| match value {
            Value::String(value) => Some(value),
            _ => None,
        }) else {
            return Ok(Self::Other {
                action_type: String::from("unknown"),
                extra: object.into_iter().collect(),
            });
        };
        deserialize_response_computer_action_object(action_type, object)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseComputerClickAction {
    pub button: String,
    pub x: i64,
    pub y: i64,
    #[serde(default)]
    pub keys: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseComputerPointAction {
    pub x: i64,
    pub y: i64,
    #[serde(default)]
    pub keys: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseComputerDragAction {
    #[serde(default)]
    pub path: Vec<ResponseComputerDragPath>,
    #[serde(default)]
    pub keys: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseComputerDragPath {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseComputerKeypressAction {
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseComputerScrollAction {
    pub scroll_x: i64,
    pub scroll_y: i64,
    pub x: i64,
    pub y: i64,
    #[serde(default)]
    pub keys: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseComputerTypeAction {
    pub text: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn deserialize_response_computer_action_object(
    action_type: String,
    object: serde_json::Map<String, Value>,
) -> Result<ResponseComputerAction, String> {
    let value = Value::Object(object);
    match action_type.as_str() {
        "click" => serde_json::from_value(value)
            .map(ResponseComputerAction::Click)
            .map_err(|error| format!("invalid click computer action: {error}")),
        "double_click" => serde_json::from_value(value)
            .map(ResponseComputerAction::DoubleClick)
            .map_err(|error| format!("invalid double_click computer action: {error}")),
        "drag" => serde_json::from_value(value)
            .map(ResponseComputerAction::Drag)
            .map_err(|error| format!("invalid drag computer action: {error}")),
        "keypress" => serde_json::from_value(value)
            .map(ResponseComputerAction::Keypress)
            .map_err(|error| format!("invalid keypress computer action: {error}")),
        "move" => serde_json::from_value(value)
            .map(ResponseComputerAction::Move)
            .map_err(|error| format!("invalid move computer action: {error}")),
        "screenshot" => Ok(ResponseComputerAction::Screenshot),
        "scroll" => serde_json::from_value(value)
            .map(ResponseComputerAction::Scroll)
            .map_err(|error| format!("invalid scroll computer action: {error}")),
        "type" => serde_json::from_value(value)
            .map(ResponseComputerAction::Type)
            .map_err(|error| format!("invalid type computer action: {error}")),
        "wait" => Ok(ResponseComputerAction::Wait),
        _ => match value {
            Value::Object(object) => Ok(ResponseComputerAction::Other {
                action_type,
                extra: object.into_iter().collect(),
            }),
            _ => unreachable!("computer action helper always receives an object"),
        },
    }
}

impl<'de> Deserialize<'de> for ResponseOutputItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = WireResponseOutputItem::deserialize(deserializer)?;
        Ok(value.into())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct WireResponseOutputItem {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<Value>,
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    call_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    created_by: Option<String>,
    #[serde(default)]
    action: Option<ResponseItemAction>,
    #[serde(default)]
    actions: Option<Vec<ResponseComputerAction>>,
    #[serde(default)]
    operation: Option<ResponseApplyPatchOperation>,
    #[serde(default)]
    environment: Option<ResponseItemEnvironment>,
    #[serde(default)]
    execution: Option<String>,
    #[serde(default)]
    output: Option<ResponseItemOutput>,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    queries: Vec<String>,
    #[serde(default)]
    results: Option<Vec<ResponseFileSearchResult>>,
    #[serde(default)]
    server_label: Option<String>,
    #[serde(default)]
    approval_request_id: Option<String>,
    #[serde(default)]
    approve: Option<bool>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    tools: Vec<ResponseItemTool>,
    #[serde(default)]
    summary: Option<Value>,
    #[serde(default)]
    encrypted_content: Option<String>,
    #[serde(default)]
    container_id: Option<String>,
    #[serde(default)]
    outputs: Option<Vec<ResponseCodeInterpreterOutput>>,
    #[serde(default)]
    max_output_length: Option<u64>,
    #[serde(default)]
    pending_safety_checks: Vec<ResponseComputerSafetyCheck>,
    #[serde(default)]
    acknowledged_safety_checks: Vec<ResponseComputerSafetyCheck>,
    #[serde(default)]
    content: Vec<ResponseContentPart>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl From<WireResponseOutputItem> for ResponseOutputItem {
    fn from(value: WireResponseOutputItem) -> Self {
        let arguments = value.arguments.as_ref().map(argument_value_to_string);
        let mut extra = value.extra;
        let summary = match value.summary {
            Some(Value::Array(summary)) => summary
                .into_iter()
                .map(response_reasoning_summary_part_from_value)
                .collect(),
            Some(summary) => {
                extra.insert(String::from("summary"), summary);
                Vec::new()
            }
            None => Vec::new(),
        };
        Self {
            id: value.id,
            item_type: value.item_type,
            role: value.role,
            name: value.name,
            arguments,
            arguments_json: value.arguments,
            input: value.input,
            code: value.code,
            call_id: value.call_id,
            status: value.status,
            phase: value.phase,
            namespace: value.namespace,
            created_by: value.created_by,
            action: value.action,
            actions: value.actions,
            operation: value.operation,
            environment: value.environment,
            execution: value.execution,
            output: value.output,
            result: value.result,
            queries: value.queries,
            results: value.results,
            server_label: value.server_label,
            approval_request_id: value.approval_request_id,
            approve: value.approve,
            reason: value.reason,
            error: value.error,
            tools: value.tools,
            summary,
            encrypted_content: value.encrypted_content,
            container_id: value.container_id,
            outputs: value.outputs,
            max_output_length: value.max_output_length,
            pending_safety_checks: value.pending_safety_checks,
            acknowledged_safety_checks: value.acknowledged_safety_checks,
            content: value.content,
            parsed_arguments: None,
            extra,
        }
    }
}

fn argument_value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        value => value.to_string(),
    }
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
    #[serde(default)]
    pub annotations: Vec<ResponseTextAnnotation>,
    #[serde(default)]
    pub logprobs: Option<Vec<ResponseTextLogprob>>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub file_data: Option<String>,
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub file_url: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub input_audio: Option<ResponseInputAudioData>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ResponseContentPart {
    fn refusal_text(&self) -> Option<&str> {
        self.refusal.as_deref().or(self.text.as_deref())
    }
}

/// Annotation attached to an output-text content part.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseTextAnnotation {
    FileCitation(ResponseFileCitationAnnotation),
    UrlCitation(ResponseUrlCitationAnnotation),
    ContainerFileCitation(ResponseContainerFileCitationAnnotation),
    FilePath(ResponseFilePathAnnotation),
    Other {
        annotation_type: String,
        extra: BTreeMap<String, Value>,
    },
    Json(Value),
}

impl<'de> Deserialize<'de> for ResponseTextAnnotation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(object) = value else {
            return Ok(Self::Json(value));
        };
        let Some(annotation_type) = object
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return Ok(Self::Json(Value::Object(object)));
        };

        match annotation_type.as_str() {
            "file_citation" => serde_json::from_value(Value::Object(object))
                .map(Self::FileCitation)
                .map_err(serde::de::Error::custom),
            "url_citation" => serde_json::from_value(Value::Object(object))
                .map(Self::UrlCitation)
                .map_err(serde::de::Error::custom),
            "container_file_citation" => serde_json::from_value(Value::Object(object))
                .map(Self::ContainerFileCitation)
                .map_err(serde::de::Error::custom),
            "file_path" => serde_json::from_value(Value::Object(object))
                .map(Self::FilePath)
                .map_err(serde::de::Error::custom),
            _ => {
                let mut extra = object;
                extra.remove("type");
                Ok(Self::Other {
                    annotation_type,
                    extra: extra.into_iter().collect(),
                })
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseFileCitationAnnotation {
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub index: Option<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseUrlCitationAnnotation {
    #[serde(default)]
    pub end_index: Option<i64>,
    #[serde(default)]
    pub start_index: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseContainerFileCitationAnnotation {
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub end_index: Option<i64>,
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub start_index: Option<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseFilePathAnnotation {
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub index: Option<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseTextLogprob {
    pub token: String,
    pub bytes: Vec<i64>,
    pub logprob: f64,
    #[serde(default)]
    pub top_logprobs: Vec<ResponseTextTopLogprob>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseTextTopLogprob {
    pub token: String,
    pub bytes: Vec<i64>,
    pub logprob: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseInputAudioData {
    pub data: String,
    pub format: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn response_text_annotation_from_value(value: Value) -> ResponseTextAnnotation {
    serde_json::from_value(value.clone()).unwrap_or(ResponseTextAnnotation::Json(value))
}

/// Parsed non-stream response with structured output helper access.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedResponse<T> {
    pub id: String,
    pub object: String,
    pub created_at: f64,
    pub status: Option<String>,
    pub model: Option<String>,
    pub instructions: Option<Value>,
    pub output: Vec<ResponseOutputItem>,
    pub parallel_tool_calls: Option<bool>,
    pub previous_response_id: Option<String>,
    pub conversation: Option<ResponseConversation>,
    pub store: Option<bool>,
    pub background: Option<bool>,
    pub completed_at: Option<f64>,
    pub max_output_tokens: Option<u64>,
    pub max_tool_calls: Option<u64>,
    pub prompt: Option<ResponsePrompt>,
    pub prompt_cache_key: Option<String>,
    pub prompt_cache_retention: Option<String>,
    pub reasoning: Option<ResponseReasoning>,
    pub safety_identifier: Option<String>,
    pub service_tier: Option<String>,
    pub temperature: Option<f64>,
    pub text: Option<ResponseTextConfig>,
    pub tool_choice: Option<ResponseToolChoice>,
    pub tools: Vec<ResponseTool>,
    pub top_logprobs: Option<u64>,
    pub top_p: Option<f64>,
    pub truncation: Option<String>,
    pub user: Option<String>,
    pub usage: Option<ResponseUsage>,
    pub error: Option<ResponseError>,
    pub incomplete_details: Option<ResponseIncompleteDetails>,
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
    OutputItemAdded {
        output_index: usize,
        item: ResponseOutputItem,
    },
    OutputItemDone {
        output_index: usize,
        item: ResponseOutputItem,
    },
    ContentPartAdded {
        item_id: Option<String>,
        output_index: usize,
        content_index: usize,
        part: ResponseContentPart,
    },
    ContentPartDone {
        item_id: Option<String>,
        output_index: usize,
        content_index: usize,
        part: ResponseContentPart,
    },
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
    FunctionCallArgumentsDelta {
        item_id: Option<String>,
        output_index: usize,
        delta: String,
    },
    FunctionCallArgumentsDone {
        item_id: Option<String>,
        output_index: usize,
        name: String,
        arguments: String,
    },
    CustomToolCallInputDelta {
        item_id: Option<String>,
        output_index: usize,
        delta: String,
    },
    CustomToolCallInputDone {
        item_id: Option<String>,
        output_index: usize,
        input: String,
    },
    CodeInterpreterCallCodeDelta {
        item_id: Option<String>,
        output_index: usize,
        delta: String,
    },
    CodeInterpreterCallCodeDone {
        item_id: Option<String>,
        output_index: usize,
        code: String,
    },
    McpCallArgumentsDelta {
        item_id: Option<String>,
        output_index: usize,
        delta: String,
    },
    McpCallArgumentsDone {
        item_id: Option<String>,
        output_index: usize,
        arguments: String,
    },
    ToolCallStatus {
        event: String,
        item_id: Option<String>,
        output_index: usize,
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
    Error {
        code: Option<String>,
        message: String,
        param: Option<String>,
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
        tools: &[ResponseTool],
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
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
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
    JsonObject,
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
            Self::JsonSchema(config) => config.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ResponseFormatTextConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let format_type = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("response text format missing `type`"))?;
        match format_type {
            "text" => Ok(Self::Text),
            "json_object" => Ok(Self::JsonObject),
            "json_schema" => serde_json::from_value(value)
                .map(Self::JsonSchema)
                .map_err(serde::de::Error::custom),
            other => Err(serde::de::Error::custom(format!(
                "unknown response text format type `{other}`"
            ))),
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

impl<'de> Deserialize<'de> for ResponseFormatTextJSONSchemaConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireJsonSchemaFormat {
            name: String,
            schema: Value,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            strict: Option<bool>,
        }

        let value = WireJsonSchemaFormat::deserialize(deserializer)?;
        Ok(Self {
            name: value.name,
            schema: value.schema,
            description: value.description,
            strict: value.strict,
        })
    }
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

/// Responses API tool-choice selector.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseToolChoice {
    Auto,
    None,
    Required,
    Allowed {
        mode: String,
        tools: Vec<BTreeMap<String, Value>>,
    },
    FileSearch,
    WebSearchPreview,
    Computer,
    ComputerUsePreview,
    ComputerUse,
    WebSearchPreview20250311,
    ImageGeneration,
    CodeInterpreter,
    Function {
        name: String,
    },
    Mcp {
        server_label: String,
        name: Option<String>,
    },
    Custom {
        name: String,
    },
    ApplyPatch,
    Shell,
    Other {
        tool_type: String,
        extra: BTreeMap<String, Value>,
    },
}

impl Serialize for ResponseToolChoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::None => serializer.serialize_str("none"),
            Self::Required => serializer.serialize_str("required"),
            Self::Allowed { mode, tools } => {
                #[derive(Serialize)]
                struct WireAllowed<'a> {
                    #[serde(rename = "type")]
                    choice_type: &'static str,
                    mode: &'a str,
                    tools: &'a [BTreeMap<String, Value>],
                }

                WireAllowed {
                    choice_type: "allowed_tools",
                    mode,
                    tools,
                }
                .serialize(serializer)
            }
            Self::FileSearch => serialize_tool_choice_type(serializer, "file_search"),
            Self::WebSearchPreview => serialize_tool_choice_type(serializer, "web_search_preview"),
            Self::Computer => serialize_tool_choice_type(serializer, "computer"),
            Self::ComputerUsePreview => {
                serialize_tool_choice_type(serializer, "computer_use_preview")
            }
            Self::ComputerUse => serialize_tool_choice_type(serializer, "computer_use"),
            Self::WebSearchPreview20250311 => {
                serialize_tool_choice_type(serializer, "web_search_preview_2025_03_11")
            }
            Self::ImageGeneration => serialize_tool_choice_type(serializer, "image_generation"),
            Self::CodeInterpreter => serialize_tool_choice_type(serializer, "code_interpreter"),
            Self::Function { name } => {
                #[derive(Serialize)]
                struct WireFunction<'a> {
                    #[serde(rename = "type")]
                    choice_type: &'static str,
                    name: &'a str,
                }

                WireFunction {
                    choice_type: "function",
                    name,
                }
                .serialize(serializer)
            }
            Self::Mcp { server_label, name } => {
                #[derive(Serialize)]
                struct WireMcp<'a> {
                    #[serde(rename = "type")]
                    choice_type: &'static str,
                    server_label: &'a str,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    name: Option<&'a String>,
                }

                WireMcp {
                    choice_type: "mcp",
                    server_label,
                    name: name.as_ref(),
                }
                .serialize(serializer)
            }
            Self::Custom { name } => {
                #[derive(Serialize)]
                struct WireCustom<'a> {
                    #[serde(rename = "type")]
                    choice_type: &'static str,
                    name: &'a str,
                }

                WireCustom {
                    choice_type: "custom",
                    name,
                }
                .serialize(serializer)
            }
            Self::ApplyPatch => serialize_tool_choice_type(serializer, "apply_patch"),
            Self::Shell => serialize_tool_choice_type(serializer, "shell"),
            Self::Other { tool_type, extra } => {
                let mut object = serde_json::Map::new();
                object.insert(String::from("type"), Value::String(tool_type.clone()));
                for (key, value) in extra {
                    object.insert(key.clone(), value.clone());
                }
                Value::Object(object).serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for ResponseToolChoice {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(value) => match value.as_str() {
                "auto" => Ok(Self::Auto),
                "none" => Ok(Self::None),
                "required" => Ok(Self::Required),
                other => Err(serde::de::Error::custom(format!(
                    "unknown response tool choice option `{other}`"
                ))),
            },
            Value::Object(mut object) => {
                let tool_type = object
                    .remove("type")
                    .and_then(|value| match value {
                        Value::String(value) => Some(value),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        serde::de::Error::custom("response tool choice object missing `type`")
                    })?;
                deserialize_response_tool_choice_object(tool_type, object)
                    .map_err(serde::de::Error::custom)
            }
            _ => Err(serde::de::Error::custom(
                "response tool choice must be a string or object",
            )),
        }
    }
}

fn serialize_tool_choice_type<S>(serializer: S, tool_type: &'static str) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    struct WireTypeChoice {
        #[serde(rename = "type")]
        choice_type: &'static str,
    }

    WireTypeChoice {
        choice_type: tool_type,
    }
    .serialize(serializer)
}

fn deserialize_response_tool_choice_object(
    tool_type: String,
    mut object: serde_json::Map<String, Value>,
) -> Result<ResponseToolChoice, String> {
    match tool_type.as_str() {
        "allowed_tools" => {
            let mode = remove_required_string(&mut object, "mode", &tool_type)?;
            let tools = object
                .remove("tools")
                .ok_or_else(|| String::from("allowed_tools response tool choice missing `tools`"))
                .and_then(|value| {
                    serde_json::from_value::<Vec<BTreeMap<String, Value>>>(value)
                        .map_err(|error| format!("invalid allowed_tools `tools`: {error}"))
                })?;
            Ok(ResponseToolChoice::Allowed { mode, tools })
        }
        "file_search" => Ok(ResponseToolChoice::FileSearch),
        "web_search_preview" => Ok(ResponseToolChoice::WebSearchPreview),
        "computer" => Ok(ResponseToolChoice::Computer),
        "computer_use_preview" => Ok(ResponseToolChoice::ComputerUsePreview),
        "computer_use" => Ok(ResponseToolChoice::ComputerUse),
        "web_search_preview_2025_03_11" => Ok(ResponseToolChoice::WebSearchPreview20250311),
        "image_generation" => Ok(ResponseToolChoice::ImageGeneration),
        "code_interpreter" => Ok(ResponseToolChoice::CodeInterpreter),
        "function" => Ok(ResponseToolChoice::Function {
            name: remove_required_string(&mut object, "name", &tool_type)?,
        }),
        "mcp" => Ok(ResponseToolChoice::Mcp {
            server_label: remove_required_string(&mut object, "server_label", &tool_type)?,
            name: remove_optional_string(&mut object, "name")?,
        }),
        "custom" => Ok(ResponseToolChoice::Custom {
            name: remove_required_string(&mut object, "name", &tool_type)?,
        }),
        "apply_patch" => Ok(ResponseToolChoice::ApplyPatch),
        "shell" => Ok(ResponseToolChoice::Shell),
        _ => Ok(ResponseToolChoice::Other {
            tool_type,
            extra: object.into_iter().collect(),
        }),
    }
}

fn remove_required_string(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
    tool_type: &str,
) -> Result<String, String> {
    object
        .remove(field)
        .and_then(|value| match value {
            Value::String(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| format!("{tool_type} response tool choice missing string `{field}`"))
}

fn remove_optional_string(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>, String> {
    match object.remove(field) {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(format!(
            "response tool choice field `{field}` must be a string"
        )),
    }
}

/// Function tool definition for non-stream parse and input-token helpers.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FunctionTool {
    pub name: String,
    #[serde(default)]
    pub parameters: Value,
    #[serde(default)]
    pub strict: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
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

/// Tool definition for the Responses API.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseTool {
    Function(FunctionTool),
    FileSearch(ResponseFileSearchTool),
    WebSearch(ResponseWebSearchTool),
    WebSearch20250826(ResponseWebSearchTool),
    WebSearchPreview(ResponseWebSearchPreviewTool),
    WebSearchPreview20250311(ResponseWebSearchPreviewTool),
    Computer,
    ComputerUsePreview(ResponseComputerUsePreviewTool),
    Mcp(ResponseMcpTool),
    CodeInterpreter(ResponseCodeInterpreterTool),
    ImageGeneration(ResponseImageGenerationTool),
    LocalShell,
    Shell(ResponseShellTool),
    Custom(ResponseCustomTool),
    Namespace(ResponseNamespaceTool),
    ToolSearch(ResponseToolSearchTool),
    ApplyPatch,
    Other {
        tool_type: String,
        extra: BTreeMap<String, Value>,
    },
}

impl ResponseTool {
    pub fn as_function(&self) -> Option<&FunctionTool> {
        match self {
            Self::Function(tool) => Some(tool),
            _ => None,
        }
    }
}

impl From<FunctionTool> for ResponseTool {
    fn from(value: FunctionTool) -> Self {
        Self::Function(value)
    }
}

impl Serialize for ResponseTool {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Function(tool) => serialize_response_tool(serializer, "function", tool),
            Self::FileSearch(tool) => serialize_response_tool(serializer, "file_search", tool),
            Self::WebSearch(tool) => serialize_response_tool(serializer, "web_search", tool),
            Self::WebSearch20250826(tool) => {
                serialize_response_tool(serializer, "web_search_2025_08_26", tool)
            }
            Self::WebSearchPreview(tool) => {
                serialize_response_tool(serializer, "web_search_preview", tool)
            }
            Self::WebSearchPreview20250311(tool) => {
                serialize_response_tool(serializer, "web_search_preview_2025_03_11", tool)
            }
            Self::Computer => serialize_response_tool_unit(serializer, "computer"),
            Self::ComputerUsePreview(tool) => {
                serialize_response_tool(serializer, "computer_use_preview", tool)
            }
            Self::Mcp(tool) => serialize_response_tool(serializer, "mcp", tool),
            Self::CodeInterpreter(tool) => {
                serialize_response_tool(serializer, "code_interpreter", tool)
            }
            Self::ImageGeneration(tool) => {
                serialize_response_tool(serializer, "image_generation", tool)
            }
            Self::LocalShell => serialize_response_tool_unit(serializer, "local_shell"),
            Self::Shell(tool) => serialize_response_tool(serializer, "shell", tool),
            Self::Custom(tool) => serialize_response_tool(serializer, "custom", tool),
            Self::Namespace(tool) => serialize_response_tool(serializer, "namespace", tool),
            Self::ToolSearch(tool) => serialize_response_tool(serializer, "tool_search", tool),
            Self::ApplyPatch => serialize_response_tool_unit(serializer, "apply_patch"),
            Self::Other { tool_type, extra } => {
                let mut object = serde_json::Map::new();
                object.insert(String::from("type"), Value::String(tool_type.clone()));
                for (key, value) in extra {
                    object.insert(key.clone(), value.clone());
                }
                Value::Object(object).serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for ResponseTool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(mut object) = value else {
            return Err(serde::de::Error::custom(
                "response tool must be a JSON object",
            ));
        };
        let tool_type = object
            .remove("type")
            .and_then(|value| match value {
                Value::String(value) => Some(value),
                _ => None,
            })
            .ok_or_else(|| serde::de::Error::custom("response tool object missing `type`"))?;
        deserialize_response_tool_object(tool_type, object).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseFileSearchTool {
    #[serde(default)]
    pub vector_store_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<ResponseFileSearchFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_num_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking_options: Option<ResponseFileSearchRankingOptions>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFileSearchFilter {
    Eq {
        key: String,
        value: ResponseFileSearchFilterValue,
    },
    Ne {
        key: String,
        value: ResponseFileSearchFilterValue,
    },
    Gt {
        key: String,
        value: ResponseFileSearchFilterValue,
    },
    Gte {
        key: String,
        value: ResponseFileSearchFilterValue,
    },
    Lt {
        key: String,
        value: ResponseFileSearchFilterValue,
    },
    Lte {
        key: String,
        value: ResponseFileSearchFilterValue,
    },
    In {
        key: String,
        value: ResponseFileSearchFilterValue,
    },
    Nin {
        key: String,
        value: ResponseFileSearchFilterValue,
    },
    And {
        filters: Vec<ResponseFileSearchFilter>,
    },
    Or {
        filters: Vec<ResponseFileSearchFilter>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ResponseFileSearchFilterValue {
    String(String),
    Bool(bool),
    Number(f64),
    Array(Vec<ResponseFileSearchFilterArrayValue>),
    Json(Value),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ResponseFileSearchFilterArrayValue {
    String(String),
    Number(f64),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseFileSearchRankingOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hybrid_search: Option<ResponseFileSearchHybridSearch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseFileSearchHybridSearch {
    pub embedding_weight: f64,
    pub text_weight: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseWebSearchTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<ResponseWebSearchFilters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<ResponseWebSearchUserLocation>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseWebSearchPreviewTool {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub search_content_types: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<ResponseWebSearchUserLocation>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseWebSearchFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseWebSearchUserLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseComputerUsePreviewTool {
    pub display_height: u32,
    pub display_width: u32,
    pub environment: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseMcpTool {
    pub server_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<ResponseMcpAllowedTools>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_approval: Option<ResponseMcpRequireApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseMcpAllowedTools {
    Names(Vec<String>),
    Filter(ResponseMcpToolFilter),
    Json(Value),
}

impl Serialize for ResponseMcpAllowedTools {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Names(names) => names.serialize(serializer),
            Self::Filter(filter) => filter.serialize(serializer),
            Self::Json(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ResponseMcpAllowedTools {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Array(_) => match serde_json::from_value(value.clone()) {
                Ok(names) => Ok(Self::Names(names)),
                Err(_) => Ok(Self::Json(value)),
            },
            Value::Object(_) => serde_json::from_value(value.clone())
                .map(Self::Filter)
                .or_else(|_| Ok(Self::Json(value))),
            value => Ok(Self::Json(value)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseMcpToolFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_names: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseMcpRequireApproval {
    Always,
    Never,
    Filter(ResponseMcpApprovalFilter),
    Json(Value),
}

impl Serialize for ResponseMcpRequireApproval {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Always => serializer.serialize_str("always"),
            Self::Never => serializer.serialize_str("never"),
            Self::Filter(filter) => filter.serialize(serializer),
            Self::Json(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ResponseMcpRequireApproval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(value) if value == "always" => Ok(Self::Always),
            Value::String(value) if value == "never" => Ok(Self::Never),
            Value::Object(_) => serde_json::from_value(value.clone())
                .map(Self::Filter)
                .or_else(|_| Ok(Self::Json(value))),
            value => Ok(Self::Json(value)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseMcpApprovalFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always: Option<ResponseMcpApprovalToolFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub never: Option<ResponseMcpApprovalToolFilter>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type ResponseMcpApprovalToolFilter = ResponseMcpToolFilter;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseCodeInterpreterTool {
    pub container: ResponseCodeInterpreterContainer,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseCodeInterpreterContainer {
    Id(String),
    Auto {
        file_ids: Vec<String>,
        memory_limit: Option<ContainerMemoryLimit>,
        network_policy: Option<ContainerNetworkPolicy>,
        extra: BTreeMap<String, Value>,
    },
    Json(Value),
}

impl ResponseCodeInterpreterContainer {
    pub fn id(id: impl Into<String>) -> Self {
        Self::Id(id.into())
    }

    pub fn auto() -> Self {
        Self::Auto {
            file_ids: Vec::new(),
            memory_limit: None,
            network_policy: None,
            extra: BTreeMap::new(),
        }
    }
}

impl Serialize for ResponseCodeInterpreterContainer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Id(id) => serializer.serialize_str(id),
            Self::Auto {
                file_ids,
                memory_limit,
                network_policy,
                extra,
            } => {
                let mut object = serde_json::Map::new();
                for (key, value) in extra {
                    object.insert(key.clone(), value.clone());
                }
                object.insert(String::from("type"), Value::String(String::from("auto")));
                if !file_ids.is_empty() {
                    object.insert(
                        String::from("file_ids"),
                        serde_json::to_value(file_ids).map_err(serde::ser::Error::custom)?,
                    );
                }
                if let Some(memory_limit) = memory_limit {
                    object.insert(
                        String::from("memory_limit"),
                        serde_json::to_value(memory_limit).map_err(serde::ser::Error::custom)?,
                    );
                }
                if let Some(network_policy) = network_policy {
                    object.insert(
                        String::from("network_policy"),
                        serde_json::to_value(network_policy).map_err(serde::ser::Error::custom)?,
                    );
                }
                Value::Object(object).serialize(serializer)
            }
            Self::Json(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ResponseCodeInterpreterContainer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(id) => Ok(Self::Id(id)),
            Value::Object(mut object) => {
                let container_type = object.remove("type").and_then(|value| match value {
                    Value::String(value) => Some(value),
                    _ => None,
                });
                match container_type.as_deref() {
                    Some("auto") => {
                        let file_ids = match object.remove("file_ids") {
                            Some(Value::Null) | None => Vec::new(),
                            Some(value) => {
                                serde_json::from_value(value).map_err(serde::de::Error::custom)?
                            }
                        };
                        let memory_limit = match object.remove("memory_limit") {
                            Some(Value::Null) | None => None,
                            Some(value) => Some(
                                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                            ),
                        };
                        let network_policy = match object.remove("network_policy") {
                            Some(Value::Null) | None => None,
                            Some(value) => Some(
                                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                            ),
                        };
                        Ok(Self::Auto {
                            file_ids,
                            memory_limit,
                            network_policy,
                            extra: object.into_iter().collect(),
                        })
                    }
                    Some(container_type) => {
                        object.insert(
                            String::from("type"),
                            Value::String(String::from(container_type)),
                        );
                        Ok(Self::Json(Value::Object(object)))
                    }
                    None => Ok(Self::Json(Value::Object(object))),
                }
            }
            value => Ok(Self::Json(value)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseImageGenerationTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_fidelity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_image_mask: Option<ResponseImageGenerationInputImageMask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moderation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_compression: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_images: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseImageGenerationInputImageMask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseShellTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<ResponseShellEnvironment>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseShellEnvironment {
    ContainerAuto {
        file_ids: Vec<String>,
        memory_limit: Option<ContainerMemoryLimit>,
        network_policy: Option<ContainerNetworkPolicy>,
        skills: Vec<ContainerSkill>,
        extra: BTreeMap<String, Value>,
    },
    Local {
        skills: Vec<ResponseLocalSkill>,
        extra: BTreeMap<String, Value>,
    },
    ContainerReference(ResponseContainerReference),
    Other {
        environment_type: String,
        extra: BTreeMap<String, Value>,
    },
    Json(Value),
}

impl Serialize for ResponseShellEnvironment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::ContainerAuto {
                file_ids,
                memory_limit,
                network_policy,
                skills,
                extra,
            } => {
                let mut object = serde_json::Map::new();
                for (key, value) in extra {
                    object.insert(key.clone(), value.clone());
                }
                object.insert(
                    String::from("type"),
                    Value::String(String::from("container_auto")),
                );
                if !file_ids.is_empty() {
                    object.insert(
                        String::from("file_ids"),
                        serde_json::to_value(file_ids).map_err(serde::ser::Error::custom)?,
                    );
                }
                if let Some(memory_limit) = memory_limit {
                    object.insert(
                        String::from("memory_limit"),
                        serde_json::to_value(memory_limit).map_err(serde::ser::Error::custom)?,
                    );
                }
                if let Some(network_policy) = network_policy {
                    object.insert(
                        String::from("network_policy"),
                        serde_json::to_value(network_policy).map_err(serde::ser::Error::custom)?,
                    );
                }
                if !skills.is_empty() {
                    object.insert(
                        String::from("skills"),
                        serde_json::to_value(skills).map_err(serde::ser::Error::custom)?,
                    );
                }
                Value::Object(object).serialize(serializer)
            }
            Self::Local { skills, extra } => {
                let mut object = serde_json::Map::new();
                for (key, value) in extra {
                    object.insert(key.clone(), value.clone());
                }
                object.insert(String::from("type"), Value::String(String::from("local")));
                if !skills.is_empty() {
                    object.insert(
                        String::from("skills"),
                        serde_json::to_value(skills).map_err(serde::ser::Error::custom)?,
                    );
                }
                Value::Object(object).serialize(serializer)
            }
            Self::ContainerReference(environment) => {
                let mut object = serde_json::Map::new();
                for (key, value) in &environment.extra {
                    object.insert(key.clone(), value.clone());
                }
                object.insert(
                    String::from("type"),
                    Value::String(String::from("container_reference")),
                );
                object.insert(
                    String::from("container_id"),
                    Value::String(environment.container_id.clone()),
                );
                Value::Object(object).serialize(serializer)
            }
            Self::Other {
                environment_type,
                extra,
            } => {
                let mut object = serde_json::Map::new();
                for (key, value) in extra {
                    object.insert(key.clone(), value.clone());
                }
                object.insert(
                    String::from("type"),
                    Value::String(environment_type.clone()),
                );
                Value::Object(object).serialize(serializer)
            }
            Self::Json(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ResponseShellEnvironment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(mut object) = value else {
            return Ok(Self::Json(value));
        };
        let Some(environment_type) = object.remove("type").and_then(|value| match value {
            Value::String(value) => Some(value),
            _ => None,
        }) else {
            return Ok(Self::Json(Value::Object(object)));
        };

        match environment_type.as_str() {
            "container_auto" => {
                let file_ids = match object.remove("file_ids") {
                    Some(Value::Null) | None => Vec::new(),
                    Some(value) => {
                        serde_json::from_value(value).map_err(serde::de::Error::custom)?
                    }
                };
                let memory_limit = match object.remove("memory_limit") {
                    Some(Value::Null) | None => None,
                    Some(value) => {
                        Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?)
                    }
                };
                let network_policy = match object.remove("network_policy") {
                    Some(Value::Null) | None => None,
                    Some(value) => {
                        Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?)
                    }
                };
                let skills = match object.remove("skills") {
                    Some(Value::Null) | None => Vec::new(),
                    Some(value) => {
                        serde_json::from_value(value).map_err(serde::de::Error::custom)?
                    }
                };
                Ok(Self::ContainerAuto {
                    file_ids,
                    memory_limit,
                    network_policy,
                    skills,
                    extra: object.into_iter().collect(),
                })
            }
            "local" => {
                let skills = match object.remove("skills") {
                    Some(Value::Null) | None => Vec::new(),
                    Some(value) => {
                        serde_json::from_value(value).map_err(serde::de::Error::custom)?
                    }
                };
                Ok(Self::Local {
                    skills,
                    extra: object.into_iter().collect(),
                })
            }
            "container_reference" => {
                let container_id = object
                    .remove("container_id")
                    .and_then(|value| match value {
                        Value::String(value) => Some(value),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        serde::de::Error::custom(
                            "container_reference shell environment missing `container_id`",
                        )
                    })?;
                Ok(Self::ContainerReference(ResponseContainerReference {
                    container_id,
                    extra: object.into_iter().collect(),
                }))
            }
            _ => Ok(Self::Other {
                environment_type,
                extra: object.into_iter().collect(),
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseLocalSkill {
    pub description: String,
    pub name: String,
    pub path: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseCustomTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<ResponseCustomToolInputFormat>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseCustomToolInputFormat {
    Text,
    Grammar(ResponseCustomToolGrammar),
    Other {
        format_type: String,
        extra: BTreeMap<String, Value>,
    },
    Json(Value),
}

impl Serialize for ResponseCustomToolInputFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Text => {
                let mut object = serde_json::Map::new();
                object.insert(String::from("type"), Value::String(String::from("text")));
                Value::Object(object).serialize(serializer)
            }
            Self::Grammar(grammar) => {
                let mut value = serde_json::to_value(grammar).map_err(serde::ser::Error::custom)?;
                let object = value.as_object_mut().ok_or_else(|| {
                    serde::ser::Error::custom("custom tool grammar must serialize to an object")
                })?;
                object.insert(String::from("type"), Value::String(String::from("grammar")));
                value.serialize(serializer)
            }
            Self::Other { format_type, extra } => {
                let mut object = serde_json::Map::new();
                for (key, value) in extra {
                    object.insert(key.clone(), value.clone());
                }
                object.insert(String::from("type"), Value::String(format_type.clone()));
                Value::Object(object).serialize(serializer)
            }
            Self::Json(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ResponseCustomToolInputFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(object) = value else {
            return Ok(Self::Json(value));
        };
        let Some(format_type) = object
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return Ok(Self::Json(Value::Object(object)));
        };

        match format_type.as_str() {
            "text" => Ok(Self::Text),
            "grammar" => {
                let mut object = object;
                object.remove("type");
                serde_json::from_value(Value::Object(object))
                    .map(Self::Grammar)
                    .map_err(serde::de::Error::custom)
            }
            _ => {
                let mut extra = object;
                extra.remove("type");
                Ok(Self::Other {
                    format_type,
                    extra: extra.into_iter().collect(),
                })
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseCustomToolGrammar {
    pub definition: String,
    pub syntax: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseNamespaceTool {
    pub description: String,
    pub name: String,
    #[serde(default)]
    pub tools: Vec<ResponseTool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponseToolSearchTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn serialize_response_tool<S, T>(
    serializer: S,
    tool_type: &'static str,
    tool: &T,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    let mut value = serde_json::to_value(tool).map_err(serde::ser::Error::custom)?;
    let object = value.as_object_mut().ok_or_else(|| {
        serde::ser::Error::custom("response tool must serialize to a JSON object")
    })?;
    object.insert(String::from("type"), Value::String(String::from(tool_type)));
    value.serialize(serializer)
}

fn serialize_response_tool_unit<S>(
    serializer: S,
    tool_type: &'static str,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut object = serde_json::Map::new();
    object.insert(String::from("type"), Value::String(String::from(tool_type)));
    Value::Object(object).serialize(serializer)
}

fn deserialize_response_tool_object(
    tool_type: String,
    object: serde_json::Map<String, Value>,
) -> Result<ResponseTool, String> {
    let value = Value::Object(object);
    match tool_type.as_str() {
        "function" => serde_json::from_value(value)
            .map(ResponseTool::Function)
            .map_err(|error| format!("invalid function response tool: {error}")),
        "file_search" => serde_json::from_value(value)
            .map(ResponseTool::FileSearch)
            .map_err(|error| format!("invalid file_search response tool: {error}")),
        "web_search" => serde_json::from_value(value)
            .map(ResponseTool::WebSearch)
            .map_err(|error| format!("invalid web_search response tool: {error}")),
        "web_search_2025_08_26" => serde_json::from_value(value)
            .map(ResponseTool::WebSearch20250826)
            .map_err(|error| format!("invalid web_search_2025_08_26 response tool: {error}")),
        "web_search_preview" => serde_json::from_value(value)
            .map(ResponseTool::WebSearchPreview)
            .map_err(|error| format!("invalid web_search_preview response tool: {error}")),
        "web_search_preview_2025_03_11" => serde_json::from_value(value)
            .map(ResponseTool::WebSearchPreview20250311)
            .map_err(|error| {
                format!("invalid web_search_preview_2025_03_11 response tool: {error}")
            }),
        "computer" => Ok(ResponseTool::Computer),
        "computer_use_preview" => serde_json::from_value(value)
            .map(ResponseTool::ComputerUsePreview)
            .map_err(|error| format!("invalid computer_use_preview response tool: {error}")),
        "mcp" => serde_json::from_value(value)
            .map(ResponseTool::Mcp)
            .map_err(|error| format!("invalid mcp response tool: {error}")),
        "code_interpreter" => serde_json::from_value(value)
            .map(ResponseTool::CodeInterpreter)
            .map_err(|error| format!("invalid code_interpreter response tool: {error}")),
        "image_generation" => serde_json::from_value(value)
            .map(ResponseTool::ImageGeneration)
            .map_err(|error| format!("invalid image_generation response tool: {error}")),
        "local_shell" => Ok(ResponseTool::LocalShell),
        "shell" => serde_json::from_value(value)
            .map(ResponseTool::Shell)
            .map_err(|error| format!("invalid shell response tool: {error}")),
        "custom" => serde_json::from_value(value)
            .map(ResponseTool::Custom)
            .map_err(|error| format!("invalid custom response tool: {error}")),
        "namespace" => serde_json::from_value(value)
            .map(ResponseTool::Namespace)
            .map_err(|error| format!("invalid namespace response tool: {error}")),
        "tool_search" => serde_json::from_value(value)
            .map(ResponseTool::ToolSearch)
            .map_err(|error| format!("invalid tool_search response tool: {error}")),
        "apply_patch" => Ok(ResponseTool::ApplyPatch),
        _ => match value {
            Value::Object(object) => Ok(ResponseTool::Other {
                tool_type,
                extra: object.into_iter().collect(),
            }),
            _ => unreachable!("response tool helper always receives an object"),
        },
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct WireResponse {
    id: String,
    object: String,
    created_at: f64,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    instructions: Option<Value>,
    #[serde(default)]
    output: Vec<ResponseOutputItem>,
    #[serde(default)]
    parallel_tool_calls: Option<bool>,
    #[serde(default)]
    previous_response_id: Option<String>,
    #[serde(default)]
    conversation: Option<ResponseConversation>,
    #[serde(default)]
    store: Option<bool>,
    #[serde(default)]
    background: Option<bool>,
    #[serde(default)]
    completed_at: Option<f64>,
    #[serde(default)]
    max_output_tokens: Option<u64>,
    #[serde(default)]
    max_tool_calls: Option<u64>,
    #[serde(default)]
    prompt: Option<ResponsePrompt>,
    #[serde(default)]
    prompt_cache_key: Option<String>,
    #[serde(default)]
    prompt_cache_retention: Option<String>,
    #[serde(default)]
    reasoning: Option<ResponseReasoning>,
    #[serde(default)]
    safety_identifier: Option<String>,
    #[serde(default)]
    service_tier: Option<String>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    text: Option<ResponseTextConfig>,
    #[serde(default)]
    tool_choice: Option<ResponseToolChoice>,
    #[serde(default)]
    tools: Vec<ResponseTool>,
    #[serde(default)]
    top_logprobs: Option<u64>,
    #[serde(default)]
    top_p: Option<f64>,
    #[serde(default)]
    truncation: Option<String>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    usage: Option<ResponseUsage>,
    #[serde(default)]
    error: Option<ResponseError>,
    #[serde(default)]
    incomplete_details: Option<ResponseIncompleteDetails>,
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
            model: value.model,
            instructions: value.instructions,
            output: value.output,
            parallel_tool_calls: value.parallel_tool_calls,
            previous_response_id: value.previous_response_id,
            conversation: value.conversation,
            store: value.store,
            background: value.background,
            completed_at: value.completed_at,
            max_output_tokens: value.max_output_tokens,
            max_tool_calls: value.max_tool_calls,
            prompt: value.prompt,
            prompt_cache_key: value.prompt_cache_key,
            prompt_cache_retention: value.prompt_cache_retention,
            reasoning: value.reasoning,
            safety_identifier: value.safety_identifier,
            service_tier: value.service_tier,
            temperature: value.temperature,
            text: value.text,
            tool_choice: value.tool_choice,
            tools: value.tools,
            top_logprobs: value.top_logprobs,
            top_p: value.top_p,
            truncation: value.truncation,
            user: value.user,
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
    usage: Option<ResponseUsage>,
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
    #[serde(default)]
    item_id: Option<String>,
    output_index: usize,
    content_index: usize,
    part: ResponseContentPart,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamFunctionArgumentsDonePayload {
    output_index: usize,
    #[serde(default)]
    item_id: Option<String>,
    name: String,
    arguments: String,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamToolTextDeltaPayload {
    output_index: usize,
    #[serde(default)]
    item_id: Option<String>,
    delta: String,
}

#[derive(Clone, Debug, Deserialize)]
struct StreamToolTextDonePayload {
    output_index: usize,
    #[serde(default)]
    item_id: Option<String>,
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

#[derive(Clone, Debug, Deserialize)]
struct StreamErrorPayload {
    #[serde(default)]
    code: Option<String>,
    message: String,
    #[serde(default)]
    param: Option<String>,
}

#[derive(Clone, Debug)]
struct StreamAccumulator {
    visible_events: VecDeque<RecordedResponseEvent>,
    snapshot: Option<Response>,
    terminal: Option<ResponseStreamTerminal>,
    terminal_error: Option<OpenAIError>,
    seen_done: bool,
    starting_after: Option<u64>,
}

impl StreamAccumulator {
    fn new(starting_after: Option<u64>) -> Self {
        Self {
            visible_events: VecDeque::new(),
            snapshot: None,
            terminal: None,
            terminal_error: None,
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
        if let Some(error) = self.terminal_error {
            return Err(error);
        }
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
                self.insert_output_item(payload.output_index, payload.item.clone())?;
                Ok(Some(ResponseStreamEvent::OutputItemAdded {
                    output_index: payload.output_index,
                    item: payload.item,
                }))
            }
            "response.output_item.done" => {
                let payload: StreamOutputItemPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_output_item(payload.output_index, payload.item.clone())?;
                Ok(Some(ResponseStreamEvent::OutputItemDone {
                    output_index: payload.output_index,
                    item: payload.item,
                }))
            }
            "response.content_part.added" => {
                let payload: StreamContentPartPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.insert_content_part(
                    payload.output_index,
                    payload.content_index,
                    payload.part.clone(),
                )?;
                Ok(Some(ResponseStreamEvent::ContentPartAdded {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    content_index: payload.content_index,
                    part: payload.part,
                }))
            }
            "response.content_part.done" => {
                let payload: StreamContentPartPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.replace_content_part(
                    payload.output_index,
                    payload.content_index,
                    payload.part.clone(),
                )?;
                Ok(Some(ResponseStreamEvent::ContentPartDone {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    content_index: payload.content_index,
                    part: payload.part,
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
                Ok(Some(ResponseStreamEvent::FunctionCallArgumentsDelta {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    delta: payload.delta,
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
                    .name = Some(payload.name.clone());
                Ok(Some(ResponseStreamEvent::FunctionCallArgumentsDone {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    name: payload.name,
                    arguments: payload.arguments,
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
                Ok(Some(ResponseStreamEvent::CustomToolCallInputDelta {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    delta: payload.delta,
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
                Ok(Some(ResponseStreamEvent::CustomToolCallInputDone {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    input,
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
                Ok(Some(ResponseStreamEvent::CodeInterpreterCallCodeDelta {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    delta: payload.delta,
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
                Ok(Some(ResponseStreamEvent::CodeInterpreterCallCodeDone {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    code,
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
                Ok(Some(ResponseStreamEvent::McpCallArgumentsDelta {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    delta: payload.delta,
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
                Ok(Some(ResponseStreamEvent::McpCallArgumentsDone {
                    item_id: payload.item_id,
                    output_index: payload.output_index,
                    arguments,
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
                    _ => Ok(Some(ResponseStreamEvent::ToolCallStatus {
                        event: event_name.to_string(),
                        item_id: payload.item_id,
                        output_index: payload.output_index,
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
            "error" => {
                let payload: StreamErrorPayload = serde_json::from_str(data)
                    .map_err(|error| stream_parse_error(event_name, error))?;
                self.terminal_error = Some(
                    OpenAIError::new(
                        ErrorKind::Api(ApiErrorKind::BadRequest),
                        format!("response stream error: {}", payload.message),
                    )
                    .with_api_error(ApiErrorPayload {
                        message: payload.message.clone(),
                        error_type: None,
                        code: payload.code.clone(),
                        param: payload.param.clone(),
                    }),
                );
                Ok(Some(ResponseStreamEvent::Error {
                    code: payload.code,
                    message: payload.message,
                    param: payload.param,
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
        let updated_arguments = {
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
            if field == "arguments" {
                Some(target.clone())
            } else {
                None
            }
        };
        if let Some(arguments) = updated_arguments {
            output.arguments_json = Some(Value::String(arguments));
        }
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
            "arguments" => {
                output.arguments = Some(value.to_string());
                output.arguments_json = Some(Value::String(value.to_string()));
            }
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
        if annotation_index > content.annotations.len() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                format!("stream referenced missing annotation_index {annotation_index}"),
            ));
        }
        content.annotations.insert(
            annotation_index,
            response_text_annotation_from_value(annotation),
        );
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
        summary.insert(
            summary_index,
            response_reasoning_summary_part_from_value(part),
        );
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
        *slot = response_reasoning_summary_part_from_value(part);
        Ok(())
    }

    fn append_reasoning_summary_text(
        &mut self,
        output_index: usize,
        summary_index: usize,
        delta: &str,
    ) -> Result<(), OpenAIError> {
        let part = self.reasoning_summary_part_mut(output_index, summary_index)?;
        if part.summary_type != "summary_text" {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "stream addressed non-summary_text reasoning summary part",
            ));
        }
        let text = part.text.get_or_insert_with(String::new);
        text.push_str(delta);
        Ok(())
    }

    fn replace_reasoning_summary_text(
        &mut self,
        output_index: usize,
        summary_index: usize,
        text: &str,
    ) -> Result<(), OpenAIError> {
        let part = self.reasoning_summary_part_mut(output_index, summary_index)?;
        if part.summary_type != "summary_text" {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "stream addressed non-summary_text reasoning summary part",
            ));
        }
        part.text = Some(text.to_string());
        Ok(())
    }

    fn reasoning_summary_part_mut(
        &mut self,
        output_index: usize,
        summary_index: usize,
    ) -> Result<&mut ResponseReasoningSummaryPart, OpenAIError> {
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
    ) -> Result<&mut Vec<ResponseReasoningSummaryPart>, OpenAIError> {
        let output = self.get_output_mut(output_index, &["reasoning"])?;
        Ok(&mut output.summary)
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
    tools: &[ResponseTool],
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
        let Some(tool) = tools
            .iter()
            .filter_map(ResponseTool::as_function)
            .find(|tool| tool.name == name)
        else {
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
        model: response.model,
        instructions: response.instructions,
        output: response.output,
        parallel_tool_calls: response.parallel_tool_calls,
        previous_response_id: response.previous_response_id,
        conversation: response.conversation,
        store: response.store,
        background: response.background,
        completed_at: response.completed_at,
        max_output_tokens: response.max_output_tokens,
        max_tool_calls: response.max_tool_calls,
        prompt: response.prompt,
        prompt_cache_key: response.prompt_cache_key,
        prompt_cache_retention: response.prompt_cache_retention,
        reasoning: response.reasoning,
        safety_identifier: response.safety_identifier,
        service_tier: response.service_tier,
        temperature: response.temperature,
        text: response.text,
        tool_choice: response.tool_choice,
        tools: response.tools,
        top_logprobs: response.top_logprobs,
        top_p: response.top_p,
        truncation: response.truncation,
        user: response.user,
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

fn merge_raw_tools(value: &mut Value, raw_tools: Vec<Value>) {
    if raw_tools.is_empty() {
        return;
    }

    let Value::Object(object) = value else {
        return;
    };
    let tools = object
        .entry(String::from("tools"))
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Value::Array(tools) = tools {
        tools.extend(raw_tools);
    }
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
