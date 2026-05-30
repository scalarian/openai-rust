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

    /// Creates a streamed run within a thread.
    pub fn create_stream<B: Serialize>(
        &self,
        thread_id: &str,
        params: B,
    ) -> Result<BetaAssistantStream, OpenAIError> {
        self.create_stream_with_query(thread_id, params, BetaQueryParams::default())
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
