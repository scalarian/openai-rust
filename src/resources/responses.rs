use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize, Serializer, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    error::ErrorKind,
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
