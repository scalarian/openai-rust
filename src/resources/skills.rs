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

/// Skills API family.
#[derive(Clone, Debug)]
pub struct Skills {
    runtime: Arc<ClientRuntime>,
}

impl Skills {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Accesses binary content retrieval for skill bundles.
    pub fn content(&self) -> SkillContent {
        SkillContent::new(self.runtime.clone())
    }

    /// Accesses versioned skill operations.
    pub fn versions(&self) -> SkillVersions {
        SkillVersions::new(self.runtime.clone())
    }

    /// Creates a skill from uploaded files or a single zip bundle.
    pub fn create(&self, params: SkillCreateParams) -> Result<ApiResponse<Skill>, OpenAIError> {
        let multipart = params.into_multipart();
        let content_type = multipart.content_type();
        let mut request = self.runtime.prepare_request_with_body(
            "POST",
            "/skills",
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

    /// Retrieves a skill by id.
    pub fn retrieve(&self, skill_id: &str) -> Result<ApiResponse<Skill>, OpenAIError> {
        let skill_id = encode_path_id(validate_path_id("skill_id", skill_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/skills/{skill_id}"),
            RequestOptions::default(),
        )
    }

    /// Updates a skill's default version pointer.
    pub fn update(
        &self,
        skill_id: &str,
        params: SkillUpdateParams,
    ) -> Result<ApiResponse<Skill>, OpenAIError> {
        let skill_id = encode_path_id(validate_path_id("skill_id", skill_id)?);
        self.runtime.execute_json_with_body(
            "POST",
            format!("/skills/{skill_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    /// Lists project skills with cursor pagination semantics.
    pub fn list(&self, params: SkillListParams) -> Result<ApiResponse<SkillsPage>, OpenAIError> {
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
            String::from("/skills")
        } else {
            format!("/skills?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Deletes a skill.
    pub fn delete(&self, skill_id: &str) -> Result<ApiResponse<SkillDeleteResponse>, OpenAIError> {
        let skill_id = encode_path_id(validate_path_id("skill_id", skill_id)?);
        self.runtime.execute_json(
            "DELETE",
            format!("/skills/{skill_id}"),
            RequestOptions::default(),
        )
    }
}

/// Skill bundle content endpoint.
#[derive(Clone, Debug)]
pub struct SkillContent {
    runtime: Arc<ClientRuntime>,
}

impl SkillContent {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Downloads the current default skill bundle as raw bytes.
    pub fn retrieve(&self, skill_id: &str) -> Result<ApiResponse<Vec<u8>>, OpenAIError> {
        let skill_id = encode_path_id(validate_path_id("skill_id", skill_id)?);
        let mut request = self
            .runtime
            .prepare_request("GET", format!("/skills/{skill_id}/content"))?;
        request
            .headers
            .insert(String::from("accept"), String::from("application/binary"));
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        execute_bytes(&request, &options)
    }
}

/// Skill-version API family.
#[derive(Clone, Debug)]
pub struct SkillVersions {
    runtime: Arc<ClientRuntime>,
}

impl SkillVersions {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Accesses version-specific binary content retrieval.
    pub fn content(&self) -> SkillVersionContent {
        SkillVersionContent::new(self.runtime.clone())
    }

    /// Creates an immutable skill version.
    pub fn create(
        &self,
        skill_id: &str,
        params: SkillVersionCreateParams,
    ) -> Result<ApiResponse<SkillVersion>, OpenAIError> {
        let skill_id = encode_path_id(validate_path_id("skill_id", skill_id)?);
        let multipart = params.into_multipart();
        let content_type = multipart.content_type();
        let mut request = self.runtime.prepare_request_with_body(
            "POST",
            format!("/skills/{skill_id}/versions"),
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

    /// Retrieves a specific skill version.
    pub fn retrieve(
        &self,
        skill_id: &str,
        version: &str,
    ) -> Result<ApiResponse<SkillVersion>, OpenAIError> {
        let skill_id = encode_path_id(validate_path_id("skill_id", skill_id)?);
        let version = encode_path_id(validate_path_id("version", version)?);
        self.runtime.execute_json(
            "GET",
            format!("/skills/{skill_id}/versions/{version}"),
            RequestOptions::default(),
        )
    }

    /// Lists versions for a skill.
    pub fn list(
        &self,
        skill_id: &str,
        params: SkillVersionListParams,
    ) -> Result<ApiResponse<SkillVersionsPage>, OpenAIError> {
        let skill_id = encode_path_id(validate_path_id("skill_id", skill_id)?);
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
            format!("/skills/{skill_id}/versions")
        } else {
            format!("/skills/{skill_id}/versions?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Deletes a version from a skill.
    pub fn delete(
        &self,
        skill_id: &str,
        version: &str,
    ) -> Result<ApiResponse<SkillVersionDeleteResponse>, OpenAIError> {
        let skill_id = encode_path_id(validate_path_id("skill_id", skill_id)?);
        let version = encode_path_id(validate_path_id("version", version)?);
        self.runtime.execute_json(
            "DELETE",
            format!("/skills/{skill_id}/versions/{version}"),
            RequestOptions::default(),
        )
    }
}

/// Version-specific skill content endpoint.
#[derive(Clone, Debug)]
pub struct SkillVersionContent {
    runtime: Arc<ClientRuntime>,
}

impl SkillVersionContent {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Downloads a specific version bundle as raw bytes.
    pub fn retrieve(
        &self,
        skill_id: &str,
        version: &str,
    ) -> Result<ApiResponse<Vec<u8>>, OpenAIError> {
        let skill_id = encode_path_id(validate_path_id("skill_id", skill_id)?);
        let version = encode_path_id(validate_path_id("version", version)?);
        let mut request = self.runtime.prepare_request(
            "GET",
            format!("/skills/{skill_id}/versions/{version}/content"),
        )?;
        request
            .headers
            .insert(String::from("accept"), String::from("application/binary"));
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        execute_bytes(&request, &options)
    }
}

/// Uploaded skill file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillFileUpload {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl SkillFileUpload {
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

    fn to_multipart_file(&self) -> MultipartFile {
        MultipartFile::new(
            self.filename.clone(),
            self.content_type.clone(),
            self.bytes.clone(),
        )
    }
}

/// Public create shape for skill uploads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillFilesParam {
    Single(SkillFileUpload),
    Multiple(Vec<SkillFileUpload>),
}

impl SkillFilesParam {
    fn append_to_builder(self, builder: &mut MultipartBuilder) {
        match self {
            Self::Single(file) => {
                builder.add_file("files", file.to_multipart_file());
            }
            Self::Multiple(files) => {
                for file in files {
                    builder.add_file("files", file.to_multipart_file());
                }
            }
        }
    }
}

/// Skill create params.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SkillCreateParams {
    pub files: Option<SkillFilesParam>,
}

impl SkillCreateParams {
    fn into_multipart(self) -> crate::helpers::multipart::MultipartPayload {
        let mut builder = MultipartBuilder::new();
        if let Some(files) = self.files {
            files.append_to_builder(&mut builder);
        }
        builder.build()
    }
}

/// Skill update params.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SkillUpdateParams {
    pub default_version: String,
}

/// Skill list params.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SkillListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<SkillOrder>,
}

/// Shared skill ordering enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillOrder {
    Asc,
    Desc,
}

impl SkillOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Typed skill resource.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct Skill {
    pub id: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub default_version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub latest_version: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub object: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page of skills.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct SkillsPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<Skill>,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl SkillsPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if !self.has_more {
            return None;
        }
        self.last_id
            .as_deref()
            .or_else(|| self.data.last().map(|item| item.id.as_str()))
    }
}

/// Skill delete response.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct SkillDeleteResponse {
    pub id: String,
    pub deleted: bool,
    #[serde(default)]
    pub object: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Skill-version create params.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SkillVersionCreateParams {
    pub default: Option<bool>,
    pub files: Option<SkillFilesParam>,
}

impl SkillVersionCreateParams {
    fn into_multipart(self) -> crate::helpers::multipart::MultipartPayload {
        let mut builder = MultipartBuilder::new();
        if let Some(default) = self.default {
            builder.add_text("default", if default { "true" } else { "false" });
        }
        if let Some(files) = self.files {
            files.append_to_builder(&mut builder);
        }
        builder.build()
    }
}

/// Version list params.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SkillVersionListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<SkillOrder>,
}

/// Typed skill version.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct SkillVersion {
    pub id: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub skill_id: String,
    #[serde(default)]
    pub version: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page for skill versions.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct SkillVersionsPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<SkillVersion>,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl SkillVersionsPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if !self.has_more {
            return None;
        }
        self.last_id
            .as_deref()
            .or_else(|| self.data.last().map(|item| item.id.as_str()))
    }
}

/// Delete response for skill versions.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct SkillVersionDeleteResponse {
    pub id: String,
    pub deleted: bool,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub version: String,
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
