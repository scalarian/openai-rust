use std::{collections::BTreeMap, sync::Arc};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    error::ErrorKind,
};

/// Models API family.
#[derive(Clone, Debug)]
pub struct Models {
    runtime: Arc<ClientRuntime>,
}

impl Models {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Retrieves a model by id without rewriting caller-supplied ids.
    pub fn retrieve(&self, model_id: &str) -> Result<ApiResponse<Model>, OpenAIError> {
        let model_id = validate_path_id("model_id", model_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/models/{model_id}"),
            RequestOptions::default(),
        )
    }

    /// Lists models as a single forward-compatible page.
    pub fn list(&self) -> Result<ApiResponse<ModelsPage>, OpenAIError> {
        self.runtime
            .execute_json("GET", "/models", RequestOptions::default())
    }

    /// Deletes an owned model and preserves server permission/not-found semantics.
    pub fn delete(&self, model_id: &str) -> Result<ApiResponse<DeletedModel>, OpenAIError> {
        let model_id = validate_path_id("model_id", model_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/models/{model_id}"),
            RequestOptions::default(),
        )
    }
}

/// Typed model object.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created: Option<i64>,
    #[serde(default)]
    pub owned_by: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Single-page models list response.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ModelsPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<Model>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ModelsPage {
    pub fn has_next_page(&self) -> bool {
        false
    }

    pub fn next_after(&self) -> Option<&str> {
        None
    }
}

/// Model deletion marker.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct DeletedModel {
    pub id: String,
    #[serde(default)]
    pub object: String,
    pub deleted: bool,
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
