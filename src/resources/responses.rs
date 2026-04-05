use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use serde::{Deserialize, Serialize, Serializer, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{
        metadata::ResponseMetadata, request::RequestOptions, response::ApiResponse,
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
        let response = self.runtime.execute_text_with_body(
            "POST",
            "/responses",
            &body,
            RequestOptions::default(),
        )?;
        ResponseStream::from_sse_chunks_with_resume(response.metadata, [response.output], None)
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
        let response = self
            .runtime
            .execute_text("GET", &path, RequestOptions::default())?;
        ResponseStream::from_sse_chunks_with_resume(
            response.metadata,
            [response.output],
            resume_after,
        )
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
}

/// Request body for non-streamed response creation.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ResponseCreateParams {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
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

impl ResponseCreateParams {
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
    pub input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
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

impl ResponseParseParams {
    fn into_create_params(self) -> ResponseCreateParams {
        ResponseCreateParams {
            model: self.model,
            input: self.input,
            instructions: self.instructions,
            previous_response_id: self.previous_response_id,
            conversation: self.conversation,
            store: self.store,
            background: self.background,
            metadata: self.metadata,
            parallel_tool_calls: self.parallel_tool_calls,
            reasoning: self.reasoning,
            text: self.text,
            tool_choice: self.tool_choice,
            tools: self.tools,
            truncation: self.truncation,
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
            .and_then(|content| content.text.as_deref())
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
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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
#[derive(Clone, Debug)]
pub struct ResponseStream {
    metadata: ResponseMetadata,
    events: VecDeque<RecordedResponseEvent>,
    current_snapshot: Option<Response>,
    final_terminal: Option<ResponseStreamTerminal>,
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
        let recorded = self.events.pop_front()?;
        self.current_snapshot = recorded.snapshot_after_event;
        Some(recorded.event)
    }

    pub async fn next_event_async(&mut self) -> Option<ResponseStreamEvent> {
        self.next_event()
    }

    pub fn abort(&mut self) {
        self.aborted = true;
        self.events.clear();
    }

    pub fn terminal_state(&self) -> Option<&ResponseStreamTerminal> {
        self.final_terminal.as_ref()
    }

    pub fn metadata(&self) -> &ResponseMetadata {
        &self.metadata
    }

    pub fn final_response(&self) -> Result<&Response, OpenAIError> {
        if self.aborted {
            return Err(OpenAIError::new(
                ErrorKind::Transport,
                "response stream was aborted before completion",
            ));
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
        &self,
        text: Option<ResponseTextConfig>,
        tools: &[FunctionTool],
    ) -> Result<ParsedResponse<T>, OpenAIError>
    where
        T: DeserializeOwned,
    {
        let response = self.final_response()?.clone();
        parse_response_output(response, text.and_then(|text| text.format), tools)
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
struct StreamTextDonePayload {
    output_index: usize,
    content_index: usize,
    text: String,
}

#[derive(Clone, Debug)]
struct StreamAccumulator {
    visible_events: VecDeque<RecordedResponseEvent>,
    snapshot: Option<Response>,
    terminal: Option<ResponseStreamTerminal>,
    seen_done: bool,
    ordinal: u64,
    starting_after: Option<u64>,
}

impl StreamAccumulator {
    fn new(starting_after: Option<u64>) -> Self {
        Self {
            visible_events: VecDeque::new(),
            snapshot: None,
            terminal: None,
            seen_done: false,
            ordinal: 0,
            starting_after,
        }
    }

    fn ingest_frame(&mut self, frame: SseFrame) -> Result<(), OpenAIError> {
        if frame.data.trim() == "[DONE]" {
            self.seen_done = true;
            return Ok(());
        }

        let event_name = frame.event.unwrap_or_default();
        let surfaced = self.apply_event(&event_name, &frame.data)?;
        let hidden = self
            .starting_after
            .is_some_and(|starting_after| self.ordinal <= starting_after);
        self.ordinal += 1;
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
                    .text
                    .clone()
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
            let output_text = response.output_text().trim();
            if output_text.is_empty() {
                None
            } else {
                Some(serde_json::from_str(output_text).map_err(|error| {
                    OpenAIError::new(
                        ErrorKind::Parse,
                        format!("failed to parse structured output: {error}"),
                    )
                    .with_source(error)
                })?)
            }
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

fn stream_parse_error(error_event: &str, error: serde_json::Error) -> OpenAIError {
    OpenAIError::new(
        ErrorKind::Parse,
        format!("failed to parse streamed `{error_event}` payload: {error}"),
    )
    .with_source(error)
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
