use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    error::ErrorKind,
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
        let conversation_id = validate_path_id("conversation_id", conversation_id)?;
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
        let conversation_id = validate_path_id("conversation_id", conversation_id)?;
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
        let conversation_id = validate_path_id("conversation_id", conversation_id)?;
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
        let conversation_id = validate_path_id("conversation_id", conversation_id)?;
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
        let conversation_id = validate_path_id("conversation_id", conversation_id)?;
        let item_id = validate_path_id("item_id", item_id)?;
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
        let conversation_id = validate_path_id("conversation_id", conversation_id)?;
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
        let conversation_id = validate_path_id("conversation_id", conversation_id)?;
        let item_id = validate_path_id("item_id", item_id)?;
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
    pub items: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Update-conversation body.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ConversationUpdateParams {
    pub metadata: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Create-item request with query/body split.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ConversationItemCreateParams {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub items: Vec<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub include: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ConversationItemCreateParams {
    fn into_request_body(self) -> Value {
        let mut body = serde_json::Map::new();
        body.insert(String::from("items"), Value::Array(self.items));
        for (key, value) in self.extra {
            body.insert(key, value);
        }
        Value::Object(body)
    }

    fn to_query_pairs(&self) -> Vec<(String, String)> {
        self.include
            .iter()
            .map(|include| (String::from("include"), include.clone()))
            .collect()
    }
}

/// Retrieve-item query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConversationItemRetrieveParams {
    pub include: Vec<String>,
}

impl ConversationItemRetrieveParams {
    fn to_query_pairs(&self) -> Vec<(String, String)> {
        self.include
            .iter()
            .map(|include| (String::from("include"), include.clone()))
            .collect()
    }
}

/// List-items query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConversationItemListParams {
    pub after: Option<String>,
    pub include: Vec<String>,
    pub limit: Option<u32>,
    pub order: Option<String>,
}

impl ConversationItemListParams {
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

/// Typed conversation resource.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Conversation {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    #[serde(default)]
    pub metadata: Value,
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
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversationItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub content: Vec<ConversationItemContent>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed conversation item content part.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversationItemContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(default)]
    pub text: Option<String>,
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

    let query = pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{path}?{query}")
}
