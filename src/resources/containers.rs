use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{
        request::RequestOptions, response::ApiResponse, runtime::ClientRuntime,
        transport::execute_bytes,
    },
    error::ErrorKind,
    helpers::multipart::{MultipartBuilder, MultipartFile},
    resources::files::{encode_path_id, validate_path_id},
};

/// Containers API family.
#[derive(Clone, Debug)]
pub struct Containers {
    runtime: Arc<ClientRuntime>,
}

impl Containers {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns the nested container-files surface.
    pub fn files(&self) -> ContainerFiles {
        ContainerFiles::new(self.runtime.clone())
    }

    /// Creates a container.
    pub fn create(
        &self,
        params: ContainerCreateParams,
    ) -> Result<ApiResponse<Container>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/containers",
            &params,
            RequestOptions::default(),
        )
    }

    /// Retrieves a container by id.
    pub fn retrieve(&self, container_id: &str) -> Result<ApiResponse<Container>, OpenAIError> {
        let container_id = encode_path_id(validate_path_id("container_id", container_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/containers/{container_id}"),
            RequestOptions::default(),
        )
    }

    /// Lists containers with cursor pagination semantics.
    pub fn list(
        &self,
        params: ContainerListParams,
    ) -> Result<ApiResponse<ContainersPage>, OpenAIError> {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(name) = params.name {
            serializer.append_pair("name", &name);
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", order.as_str());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            String::from("/containers")
        } else {
            format!("/containers?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Deletes a container and tolerates empty-body success responses.
    pub fn delete(&self, container_id: &str) -> Result<ApiResponse<()>, OpenAIError> {
        let container_id = encode_path_id(validate_path_id("container_id", container_id)?);
        self.runtime.execute_unit(
            "DELETE",
            format!("/containers/{container_id}"),
            RequestOptions::default(),
        )
    }
}

/// Nested container-files API family.
#[derive(Clone, Debug)]
pub struct ContainerFiles {
    runtime: Arc<ClientRuntime>,
}

impl ContainerFiles {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a container file either from raw uploaded bytes or a copied file id.
    pub fn create(
        &self,
        container_id: &str,
        params: ContainerFileCreateParams,
    ) -> Result<ApiResponse<ContainerFile>, OpenAIError> {
        let container_id = encode_path_id(validate_path_id("container_id", container_id)?);
        match params {
            ContainerFileCreateParams::Upload(file) => {
                let mut builder = MultipartBuilder::new();
                builder.add_file(
                    "file",
                    MultipartFile::new(file.filename, file.content_type, file.bytes),
                );
                let multipart = builder.build();
                let content_type = multipart.content_type();
                let mut request = self.runtime.prepare_request_with_body(
                    "POST",
                    format!("/containers/{container_id}/files"),
                    Some(multipart.into_body()),
                )?;
                request
                    .headers
                    .insert(String::from("content-type"), content_type);
                request
                    .headers
                    .insert(String::from("accept"), String::from("application/json"));
                let options = self
                    .runtime
                    .resolve_request_options(&RequestOptions::default())?;
                let response = execute_bytes(&request, &options)?;
                parse_json_bytes_response(response)
            }
            ContainerFileCreateParams::FileId(file_id) => {
                let file_id = validate_path_id("file_id", &file_id)?.to_string();
                self.runtime.execute_json_with_body(
                    "POST",
                    format!("/containers/{container_id}/files"),
                    &ContainerFileByIdBody { file_id },
                    RequestOptions::default(),
                )
            }
        }
    }

    /// Retrieves container-file metadata.
    pub fn retrieve(
        &self,
        container_id: &str,
        file_id: &str,
    ) -> Result<ApiResponse<ContainerFile>, OpenAIError> {
        let container_id = encode_path_id(validate_path_id("container_id", container_id)?);
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/containers/{container_id}/files/{file_id}"),
            RequestOptions::default(),
        )
    }

    /// Lists files within a container.
    pub fn list(
        &self,
        container_id: &str,
        params: ContainerFileListParams,
    ) -> Result<ApiResponse<ContainerFilesPage>, OpenAIError> {
        let container_id = encode_path_id(validate_path_id("container_id", container_id)?);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", order.as_str());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            format!("/containers/{container_id}/files")
        } else {
            format!("/containers/{container_id}/files?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Downloads raw container-file bytes.
    pub fn content(
        &self,
        container_id: &str,
        file_id: &str,
    ) -> Result<ApiResponse<Vec<u8>>, OpenAIError> {
        let container_id = encode_path_id(validate_path_id("container_id", container_id)?);
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        let mut request = self.runtime.prepare_request(
            "GET",
            format!("/containers/{container_id}/files/{file_id}/content"),
        )?;
        request
            .headers
            .insert(String::from("accept"), String::from("application/binary"));
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        execute_bytes(&request, &options)
    }

    /// Deletes a container file and returns the typed deletion result.
    pub fn delete(
        &self,
        container_id: &str,
        file_id: &str,
    ) -> Result<ApiResponse<ContainerFileDeleteResponse>, OpenAIError> {
        let container_id = encode_path_id(validate_path_id("container_id", container_id)?);
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        let mut request = self.runtime.prepare_request(
            "DELETE",
            format!("/containers/{container_id}/files/{file_id}"),
        )?;
        request
            .headers
            .insert(String::from("accept"), String::from("*/*"));
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        let response = execute_bytes(&request, &options)?;
        parse_json_bytes_response(response)
    }
}

/// Container creation parameters.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ContainerCreateParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<ContainerExpiresAfter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<ContainerMemoryLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_policy: Option<ContainerNetworkPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<ContainerSkill>>,
}

/// Container expiration policy.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerExpiresAfter {
    pub anchor: String,
    pub minutes: u64,
}

/// Memory limit for a container.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContainerMemoryLimit {
    #[serde(rename = "1g")]
    G1,
    #[serde(rename = "4g")]
    G4,
    #[serde(rename = "16g")]
    G16,
    #[serde(rename = "64g")]
    G64,
}

/// Container network policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContainerNetworkPolicy {
    Allowlist {
        allowed_domains: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        domain_secrets: Option<Vec<DomainSecret>>,
    },
    Disabled,
}

/// Documented read-time container network policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContainerReadNetworkPolicy {
    Allowlist { allowed_domains: Vec<String> },
    Disabled,
}

/// Domain-scoped secret injected into an allowlisted network policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DomainSecret {
    pub domain: String,
    pub name: String,
    pub value: String,
}

/// Skill selector supported by containers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContainerSkill {
    Reference(ContainerSkillReference),
    Inline(ContainerInlineSkill),
}

/// Reference to a persisted skill.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContainerSkillReference {
    pub skill_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl ContainerSkillReference {
    pub fn new(skill_id: impl Into<String>) -> Self {
        Self {
            skill_id: skill_id.into(),
            kind: String::from("skill_reference"),
            version: None,
        }
    }
}

/// Inline skill payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContainerInlineSkill {
    pub description: String,
    pub name: String,
    pub source: ContainerInlineSkillSource,
    #[serde(rename = "type")]
    pub kind: String,
}

impl ContainerInlineSkill {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        source: ContainerInlineSkillSource,
    ) -> Self {
        Self {
            description: description.into(),
            name: name.into(),
            source,
            kind: String::from("inline"),
        }
    }
}

/// Inline skill source metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContainerInlineSkillSource {
    pub data: String,
    pub media_type: String,
    #[serde(rename = "type")]
    pub source_type: String,
}

impl ContainerInlineSkillSource {
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            media_type: String::from("application/zip"),
            source_type: String::from("base64"),
        }
    }
}

/// List parameters for containers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContainerListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub name: Option<String>,
    pub order: Option<ContainerOrder>,
}

/// Container list sort direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerOrder {
    Asc,
    Desc,
}

impl ContainerOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Lifecycle status for a container.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContainerStatus {
    Running,
    Deleted,
    Failed,
    Creating,
    Unknown(String),
}

impl Default for ContainerStatus {
    fn default() -> Self {
        Self::Unknown(String::from("unknown"))
    }
}

impl<'de> Deserialize<'de> for ContainerStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "running" => Self::Running,
            "deleted" => Self::Deleted,
            "failed" => Self::Failed,
            "creating" => Self::Creating,
            _ => Self::Unknown(value),
        })
    }
}

/// Typed container resource.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Container {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub status: Option<ContainerStatus>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub expires_after: Option<ContainerExpiresAfter>,
    #[serde(default)]
    pub last_active_at: Option<u64>,
    #[serde(default)]
    pub memory_limit: Option<ContainerMemoryLimit>,
    #[serde(default)]
    pub network_policy: Option<ContainerReadNetworkPolicy>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page for containers.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ContainersPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<Container>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ContainersPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|container| container.id.as_str())
        } else {
            None
        }
    }
}

/// Multipart upload file input for container files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerFileUpload {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl ContainerFileUpload {
    pub fn new(
        filename: impl Into<String>,
        content_type: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            filename: filename.into(),
            content_type: content_type.into(),
            bytes: bytes.into(),
        }
    }
}

/// Container-file create input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContainerFileCreateParams {
    Upload(ContainerFileUpload),
    FileId(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ContainerFileByIdBody {
    file_id: String,
}

/// Container-file list parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContainerFileListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ContainerFileOrder>,
}

/// Container-file list sort direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerFileOrder {
    Asc,
    Desc,
}

impl ContainerFileOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Container-file source discriminator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContainerFileSource {
    User,
    Assistant,
    Unknown(String),
}

impl Default for ContainerFileSource {
    fn default() -> Self {
        Self::Unknown(String::from("unknown"))
    }
}

impl<'de> Deserialize<'de> for ContainerFileSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "user" => Self::User,
            "assistant" => Self::Assistant,
            _ => Self::Unknown(value),
        })
    }
}

/// Typed container-file metadata.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ContainerFile {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub container_id: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub source: Option<ContainerFileSource>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page for container files.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ContainerFilesPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<ContainerFile>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ContainerFilesPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|file| file.id.as_str())
        } else {
            None
        }
    }
}

/// Container-file deletion result.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ContainerFileDeleteResponse {
    pub id: String,
    #[serde(default)]
    pub object: String,
    pub deleted: bool,
    #[serde(default)]
    pub container_id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn parse_json_bytes_response<T>(
    response: ApiResponse<Vec<u8>>,
) -> Result<ApiResponse<T>, OpenAIError>
where
    T: for<'de> Deserialize<'de>,
{
    let ApiResponse { output, metadata } = response;
    let parsed = serde_json::from_slice(&output).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse OpenAI success response: {error}"),
        )
        .with_response_metadata(
            metadata.status_code,
            metadata.headers.clone(),
            metadata.request_id.clone(),
        )
        .with_source(error)
    })?;
    Ok(ApiResponse {
        output: parsed,
        metadata,
    })
}
