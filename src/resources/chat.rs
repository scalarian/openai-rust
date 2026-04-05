use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::Duration,
};

use serde::{Deserialize, Deserializer, Serialize};
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
};

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

    /// Creates a streamed chat completion and accumulates the final message snapshot.
    pub fn stream(
        &self,
        params: ChatCompletionCreateParams,
    ) -> Result<ChatCompletionStream, OpenAIError> {
        let body = params.into_request_body(true);
        let request = self
            .runtime
            .prepare_json_request("POST", "/chat/completions", &body)?;
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;

        ChatCompletionStream::start_live(request, options)
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
    pub messages: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ChatCompletionCreateParams {
    fn into_request_body(self, stream: bool) -> Value {
        let mut value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        if let Value::Object(ref mut object) = value {
            object.insert(String::from("stream"), Value::Bool(stream));
        }
        value
    }
}

/// Metadata-only stored chat-completion update body.
#[derive(Clone, Debug, Default, Serialize)]
pub struct StoredChatCompletionUpdateParams {
    pub metadata: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Query parameters for stored chat-completion listing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoredChatCompletionsListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<String>,
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
            pairs.push((String::from("order"), order.clone()));
        }
        pairs
    }
}

/// Query parameters for stored chat-completion message listing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoredChatCompletionMessagesListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<String>,
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
            pairs.push((String::from("order"), order.clone()));
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
    pub usage: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed choice on a chat completion.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatCompletionChoice {
    pub index: usize,
    #[serde(default)]
    pub finish_reason: Option<String>,
    pub message: ChatCompletionMessage,
    #[serde(default)]
    pub logprobs: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed assistant/user/system message for chat completions.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionMessage {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub function_call: Option<LegacyFunctionCall>,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub tool_calls: Vec<ChatCompletionMessageToolCall>,
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
    pub tool_type: Option<String>,
    #[serde(default)]
    pub function: ToolCallFunction,
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
    pub role: Option<String>,
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
    pub finish_reason: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Delta payload inside a streamed chat-completion chunk.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ChatCompletionChunkDelta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub function_call: Option<LegacyFunctionCall>,
    #[serde(default)]
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
            live: None,
            aborted: false,
        })
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
                self.chunks.push_back(chunk);
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
    Chunk(ChatCompletionChunk),
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
            usage: None,
            extra: BTreeMap::new(),
        })
    }
}

#[derive(Clone, Debug, Default)]
struct AccumulatedChoice {
    message: ChatCompletionMessage,
    finish_reason: Option<String>,
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
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{path}?{query}")
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
                        if chunk_tx.send(LiveChatCompletionMessage::Chunk(parsed)).is_err() {
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
                .send(LiveChatCompletionMessage::Chunk(parsed))
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
