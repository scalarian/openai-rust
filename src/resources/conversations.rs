use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    error::ErrorKind,
    resources::{
        common::ListOrder,
        responses::{
            ResponseApplyPatchOperation, ResponseCodeInterpreterOutput, ResponseComputerAction,
            ResponseContentType, ResponseFileSearchResult, ResponseIncludable,
            ResponseInputAudioData, ResponseInputItem, ResponseItemAction, ResponseItemEnvironment,
            ResponseItemOutput, ResponseItemRole, ResponseItemStatus, ResponseItemTool,
            ResponseItemType, ResponseMessagePhase, ResponseReasoningSummaryPart,
            ResponseTextAnnotation, ResponseTextLogprob, ResponseToolExecution,
        },
    },
};

/// Conversations API family.
#[derive(Clone, Debug)]
pub struct Conversations {
    runtime: Arc<ClientRuntime>,
}

impl Conversations {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns the nested conversation items helper surface.
    pub fn items(&self) -> Items {
        Items::new(self.runtime.clone())
    }

    /// Creates a conversation.
    pub fn create(
        &self,
        params: ConversationCreateParams,
    ) -> Result<ApiResponse<Conversation>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/conversations",
            &params,
            RequestOptions::default(),
        )
    }

    /// Retrieves a conversation by id.
    pub fn retrieve(
        &self,
        conversation_id: &str,
    ) -> Result<ApiResponse<Conversation>, OpenAIError> {
        let conversation_id = encode_path_id(validate_path_id("conversation_id", conversation_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/conversations/{conversation_id}"),
            RequestOptions::default(),
        )
    }

    /// Updates a conversation's metadata.
    pub fn update(
        &self,
        conversation_id: &str,
        params: ConversationUpdateParams,
    ) -> Result<ApiResponse<Conversation>, OpenAIError> {
        let conversation_id = encode_path_id(validate_path_id("conversation_id", conversation_id)?);
        self.runtime.execute_json_with_body(
            "POST",
            format!("/conversations/{conversation_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    /// Deletes a conversation and returns the typed deletion marker.
    pub fn delete(
        &self,
        conversation_id: &str,
    ) -> Result<ApiResponse<ConversationDeletedResource>, OpenAIError> {
        let conversation_id = encode_path_id(validate_path_id("conversation_id", conversation_id)?);
        self.runtime.execute_json(
            "DELETE",
            format!("/conversations/{conversation_id}"),
            RequestOptions::default(),
        )
    }
}

/// Nested conversation-items family.
#[derive(Clone, Debug)]
pub struct Items {
    runtime: Arc<ClientRuntime>,
}

impl Items {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates one or more items in an existing conversation.
    pub fn create(
        &self,
        conversation_id: &str,
        params: ConversationItemCreateParams,
    ) -> Result<ApiResponse<ConversationItemList>, OpenAIError> {
        let conversation_id = encode_path_id(validate_path_id("conversation_id", conversation_id)?);
        let path = append_query(
            &format!("/conversations/{conversation_id}/items"),
            params.to_query_pairs(),
        );
        self.runtime.execute_json_with_body(
            "POST",
            path,
            &params.into_request_body(),
            RequestOptions::default(),
        )
    }

    /// Retrieves a single conversation item.
    pub fn retrieve(
        &self,
        conversation_id: &str,
        item_id: &str,
        params: ConversationItemRetrieveParams,
    ) -> Result<ApiResponse<ConversationItem>, OpenAIError> {
        let conversation_id = encode_path_id(validate_path_id("conversation_id", conversation_id)?);
        let item_id = encode_path_id(validate_path_id("item_id", item_id)?);
        let path = append_query(
            &format!("/conversations/{conversation_id}/items/{item_id}"),
            params.to_query_pairs(),
        );
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Lists conversation items with cursor semantics.
    pub fn list(
        &self,
        conversation_id: &str,
        params: ConversationItemListParams,
    ) -> Result<ApiResponse<ConversationItemList>, OpenAIError> {
        let conversation_id = encode_path_id(validate_path_id("conversation_id", conversation_id)?);
        let path = append_query(
            &format!("/conversations/{conversation_id}/items"),
            params.to_query_pairs(),
        );
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Deletes a conversation item and returns the updated conversation.
    pub fn delete(
        &self,
        conversation_id: &str,
        item_id: &str,
    ) -> Result<ApiResponse<Conversation>, OpenAIError> {
        let conversation_id = encode_path_id(validate_path_id("conversation_id", conversation_id)?);
        let item_id = encode_path_id(validate_path_id("item_id", item_id)?);
        self.runtime.execute_json(
            "DELETE",
            format!("/conversations/{conversation_id}/items/{item_id}"),
            RequestOptions::default(),
        )
    }
}

/// Create-conversation body.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ConversationCreateParams {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub items: Vec<ResponseInputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Update-conversation body.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ConversationUpdateParams {
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Create-item request with query/body split.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ConversationItemCreateParams {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub items: Vec<ResponseInputItem>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub include: Vec<ResponseIncludable>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ConversationItemCreateParams {
    fn into_request_body(self) -> Value {
        let mut body = serde_json::Map::new();
        body.insert(
            String::from("items"),
            serde_json::to_value(self.items).unwrap_or_else(|_| Value::Array(Vec::new())),
        );
        for (key, value) in self.extra {
            body.insert(key, value);
        }
        Value::Object(body)
    }

    fn to_query_pairs(&self) -> Vec<(String, String)> {
        self.include
            .iter()
            .map(|include| (String::from("include"), include.as_str().to_string()))
            .collect()
    }
}

/// Retrieve-item query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConversationItemRetrieveParams {
    pub include: Vec<ResponseIncludable>,
}

impl ConversationItemRetrieveParams {
    fn to_query_pairs(&self) -> Vec<(String, String)> {
        self.include
            .iter()
            .map(|include| (String::from("include"), include.as_str().to_string()))
            .collect()
    }
}

/// List-items query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConversationItemListParams {
    pub after: Option<String>,
    pub include: Vec<ResponseIncludable>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl ConversationItemListParams {
    fn to_query_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        if let Some(after) = &self.after {
            pairs.push((String::from("after"), after.clone()));
        }
        for include in &self.include {
            pairs.push((String::from("include"), include.as_str().to_string()));
        }
        if let Some(limit) = self.limit {
            pairs.push((String::from("limit"), limit.to_string()));
        }
        if let Some(order) = &self.order {
            pairs.push((String::from("order"), order.as_str().to_string()));
        }
        pairs
    }
}

/// Typed conversation resource.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Conversation {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed conversation deletion marker.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversationDeletedResource {
    pub id: String,
    pub deleted: bool,
    pub object: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed conversation item envelope.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversationItemList {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<ConversationItem>,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ConversationItemList {
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

/// Typed conversation item with forward-compatible extra fields.
#[derive(Clone, Debug, PartialEq)]
pub struct ConversationItem {
    pub id: Option<String>,
    pub item_type: ResponseItemType,
    pub role: Option<ResponseItemRole>,
    pub name: Option<String>,
    pub arguments: Option<String>,
    pub arguments_json: Option<Value>,
    pub input: Option<String>,
    pub code: Option<String>,
    pub call_id: Option<String>,
    pub status: Option<ResponseItemStatus>,
    pub phase: Option<ResponseMessagePhase>,
    pub namespace: Option<String>,
    pub created_by: Option<String>,
    pub action: Option<ResponseItemAction>,
    pub actions: Option<Vec<ResponseComputerAction>>,
    pub operation: Option<ResponseApplyPatchOperation>,
    pub environment: Option<ResponseItemEnvironment>,
    pub execution: Option<ResponseToolExecution>,
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
    pub pending_safety_checks: Vec<ConversationComputerSafetyCheck>,
    pub acknowledged_safety_checks: Vec<ConversationComputerSafetyCheck>,
    pub content: Vec<ConversationItemContent>,
    pub extra: BTreeMap<String, Value>,
}

/// MCP list-tools entry inside a conversation item.
pub type ConversationMcpListTool = crate::resources::responses::ResponseMcpListTool;

/// Safety check entry used by computer-call conversation items.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversationComputerSafetyCheck {
    pub id: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl<'de> Deserialize<'de> for ConversationItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = WireConversationItem::deserialize(deserializer)?;
        Ok(value.into())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct WireConversationItem {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type")]
    item_type: ResponseItemType,
    #[serde(default)]
    role: Option<ResponseItemRole>,
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
    status: Option<ResponseItemStatus>,
    #[serde(default)]
    phase: Option<ResponseMessagePhase>,
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
    execution: Option<ResponseToolExecution>,
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
    pending_safety_checks: Vec<ConversationComputerSafetyCheck>,
    #[serde(default)]
    acknowledged_safety_checks: Vec<ConversationComputerSafetyCheck>,
    #[serde(default)]
    content: Vec<ConversationItemContent>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl From<WireConversationItem> for ConversationItem {
    fn from(value: WireConversationItem) -> Self {
        let arguments = value.arguments.as_ref().map(argument_value_to_string);
        let mut extra = value.extra;
        let summary = match value.summary {
            Some(Value::Array(summary)) => summary
                .into_iter()
                .map(crate::resources::responses::response_reasoning_summary_part_from_value)
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
            extra,
        }
    }
}

/// Typed conversation item content part.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversationItemContent {
    #[serde(rename = "type")]
    pub content_type: ResponseContentType,
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

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(&key, &value);
    }
    let query = serializer.finish();
    format!("{path}?{query}")
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
        ) {
            encoded.push(byte as char);
        } else {
            write_percent_encoded_byte(&mut encoded, byte);
        }
    }
    encoded
}

fn write_percent_encoded_byte(output: &mut String, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    output.push('%');
    output.push(HEX[(byte >> 4) as usize] as char);
    output.push(HEX[(byte & 0x0F) as usize] as char);
}

fn argument_value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}
