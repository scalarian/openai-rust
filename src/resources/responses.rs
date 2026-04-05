use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
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
    pub content: Vec<ResponseContentPart>,
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
