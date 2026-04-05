use std::{collections::BTreeMap, sync::Arc};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    error::ErrorKind,
};

/// Embeddings API family.
#[derive(Clone, Debug)]
pub struct Embeddings {
    runtime: Arc<ClientRuntime>,
}

impl Embeddings {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates embeddings while preserving input order and compatibility-oriented encoding defaults.
    pub fn create(
        &self,
        params: EmbeddingCreateParams,
    ) -> Result<ApiResponse<EmbeddingCreateResponse>, OpenAIError> {
        let requested_format = params.encoding_format.clone();
        let body = params.into_request_body();
        let response = self
            .runtime
            .execute_json_with_body::<_, WireEmbeddingCreateResponse>(
                "POST",
                "/embeddings",
                &body,
                RequestOptions::default(),
            )?;
        map_embedding_response(response, requested_format)
    }
}

/// Embedding creation parameters.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EmbeddingCreateParams {
    pub model: String,
    pub input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<EmbeddingEncodingFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl EmbeddingCreateParams {
    fn into_request_body(self) -> Value {
        let mut value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        if let Value::Object(ref mut object) = value {
            object
                .entry(String::from("encoding_format"))
                .or_insert_with(|| Value::String(String::from("base64")));
        }
        value
    }
}

/// Supported embedding encoding formats.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingEncodingFormat {
    Float,
    Base64,
}

/// Typed embeddings response.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddingCreateResponse {
    pub object: String,
    pub data: Vec<Embedding>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

/// Typed embedding entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Embedding {
    pub object: String,
    pub index: usize,
    pub embedding: EmbeddingVector,
    pub extra: BTreeMap<String, Value>,
}

/// Public embedding payload, returned as floats or raw base64 per request semantics.
#[derive(Clone, Debug, PartialEq)]
pub enum EmbeddingVector {
    Float(Vec<f32>),
    Base64(String),
}

impl EmbeddingVector {
    pub fn as_float_slice(&self) -> Option<&[f32]> {
        match self {
            Self::Float(values) => Some(values.as_slice()),
            Self::Base64(_) => None,
        }
    }

    pub fn as_base64(&self) -> Option<&str> {
        match self {
            Self::Float(_) => None,
            Self::Base64(value) => Some(value.as_str()),
        }
    }
}

/// Usage totals returned by embeddings.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct EmbeddingUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
struct WireEmbeddingCreateResponse {
    #[serde(default)]
    object: String,
    #[serde(default)]
    data: Vec<WireEmbedding>,
    #[serde(default)]
    model: String,
    #[serde(default)]
    usage: EmbeddingUsage,
}

#[derive(Clone, Debug, Deserialize)]
struct WireEmbedding {
    #[serde(default)]
    object: String,
    index: usize,
    embedding: WireEmbeddingVector,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum WireEmbeddingVector {
    Float(Vec<f32>),
    Base64(String),
}

fn map_embedding_response(
    response: ApiResponse<WireEmbeddingCreateResponse>,
    requested_format: Option<EmbeddingEncodingFormat>,
) -> Result<ApiResponse<EmbeddingCreateResponse>, OpenAIError> {
    let (metadata, output) = response.into_parts();
    let prefer_base64 = matches!(requested_format, Some(EmbeddingEncodingFormat::Base64));
    let data = output
        .data
        .into_iter()
        .map(|item| map_embedding(item, prefer_base64, &metadata))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ApiResponse {
        metadata,
        output: EmbeddingCreateResponse {
            object: output.object,
            data,
            model: output.model,
            usage: output.usage,
        },
    })
}

fn map_embedding(
    embedding: WireEmbedding,
    prefer_base64: bool,
    metadata: &crate::ResponseMetadata,
) -> Result<Embedding, OpenAIError> {
    let vector = match (prefer_base64, embedding.embedding) {
        (true, WireEmbeddingVector::Base64(value)) => EmbeddingVector::Base64(value),
        (true, WireEmbeddingVector::Float(values)) => EmbeddingVector::Float(values),
        (false, WireEmbeddingVector::Float(values)) => EmbeddingVector::Float(values),
        (false, WireEmbeddingVector::Base64(value)) => {
            EmbeddingVector::Float(decode_base64_embedding(&value, metadata)?)
        }
    };

    Ok(Embedding {
        object: embedding.object,
        index: embedding.index,
        embedding: vector,
        extra: embedding.extra,
    })
}

fn decode_base64_embedding(
    encoded: &str,
    metadata: &crate::ResponseMetadata,
) -> Result<Vec<f32>, OpenAIError> {
    let bytes = STANDARD.decode(encoded).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to decode base64 embedding payload: {error}"),
        )
        .with_response_metadata(
            metadata.status_code(),
            metadata.headers.clone(),
            metadata.request_id().map(str::to_owned),
        )
        .with_source(error)
    })?;

    if bytes.len() % 4 != 0 {
        return Err(OpenAIError::new(
            ErrorKind::Parse,
            format!(
                "base64 embedding payload had {} bytes, which is not a multiple of four",
                bytes.len()
            ),
        )
        .with_response_metadata(
            metadata.status_code(),
            metadata.headers.clone(),
            metadata.request_id().map(str::to_owned),
        ));
    }

    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}
