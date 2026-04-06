use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    error::ErrorKind,
    helpers::multipart::{MultipartBuilder, MultipartFile},
    resources::files::{
        FileExpiresAfter, FileObject, FilePurpose, encode_path_id, validate_path_id,
    },
};

/// Default chunk size for chunked uploads (64 MiB).
pub const DEFAULT_PART_SIZE: usize = 64 * 1024 * 1024;

/// Uploads API family.
#[derive(Clone, Debug)]
pub struct Uploads {
    runtime: Arc<ClientRuntime>,
}

impl Uploads {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a pending upload resource.
    pub fn create(&self, params: UploadCreateParams) -> Result<ApiResponse<Upload>, OpenAIError> {
        self.runtime
            .execute_json_with_body("POST", "/uploads", &params, RequestOptions::default())
    }

    /// Adds one multipart part to an upload.
    pub fn add_part(
        &self,
        upload_id: &str,
        part: UploadPartInput,
    ) -> Result<ApiResponse<UploadPart>, OpenAIError> {
        let upload_id = encode_path_id(validate_path_id("upload_id", upload_id)?);
        let multipart = part.into_multipart();
        let content_type = multipart.content_type();
        let mut request = self.runtime.prepare_request_with_body(
            "POST",
            format!("/uploads/{upload_id}/parts"),
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
        let response = crate::core::transport::execute_bytes(&request, &options)?;
        parse_json_bytes_response(response)
    }

    /// Completes an upload using the caller-supplied part ordering.
    pub fn complete(
        &self,
        upload_id: &str,
        params: UploadCompleteParams,
    ) -> Result<ApiResponse<Upload>, OpenAIError> {
        let upload_id = encode_path_id(validate_path_id("upload_id", upload_id)?);
        self.runtime.execute_json_with_body(
            "POST",
            format!("/uploads/{upload_id}/complete"),
            &params,
            RequestOptions::default(),
        )
    }

    /// Cancels an upload.
    pub fn cancel(&self, upload_id: &str) -> Result<ApiResponse<Upload>, OpenAIError> {
        let upload_id = encode_path_id(validate_path_id("upload_id", upload_id)?);
        self.runtime.execute_json(
            "POST",
            format!("/uploads/{upload_id}/cancel"),
            RequestOptions::default(),
        )
    }

    /// Splits a path-backed or in-memory file into sequential multipart parts.
    pub fn upload_file_chunked(
        &self,
        params: UploadChunkedParams,
    ) -> Result<ApiResponse<Upload>, OpenAIError> {
        let part_size = params.part_size.unwrap_or(DEFAULT_PART_SIZE);
        if part_size == 0 {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "chunked upload part_size must be greater than zero",
            ));
        }

        let resolved = ResolvedChunkedUpload::from_source(params.source)?;
        let created = self.create(UploadCreateParams {
            bytes: resolved.byte_length,
            filename: resolved.filename,
            mime_type: params.mime_type,
            purpose: params.purpose,
            expires_after: None,
        })?;

        let mut part_ids = Vec::new();
        for chunk in resolved.bytes.chunks(part_size) {
            let part = self.add_part(
                &created.output.id,
                UploadPartInput::new("part.bin", "application/octet-stream", chunk.to_vec()),
            )?;
            part_ids.push(part.output.id);
        }

        self.complete(
            &created.output.id,
            UploadCompleteParams {
                part_ids,
                md5: params.md5,
            },
        )
    }
}

/// Upload creation parameters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UploadCreateParams {
    pub bytes: u64,
    pub filename: String,
    pub mime_type: String,
    pub purpose: FilePurpose,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<FileExpiresAfter>,
}

/// Upload part input for multipart `/parts` creation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UploadPartInput {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl UploadPartInput {
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

    fn into_multipart(self) -> crate::helpers::multipart::MultipartPayload {
        let mut builder = MultipartBuilder::new();
        builder.add_file(
            "data",
            MultipartFile::new(self.filename, self.content_type, self.bytes),
        );
        builder.build()
    }
}

/// Completion parameters for an upload.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UploadCompleteParams {
    pub part_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
}

/// Typed upload object.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Upload {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub purpose: Option<FilePurpose>,
    pub status: UploadStatus,
    #[serde(default)]
    pub file: Option<FileObject>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Upload lifecycle status.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadStatus {
    #[default]
    Pending,
    Completed,
    Cancelled,
    Expired,
}

/// Upload part object.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct UploadPart {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub upload_id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Source kinds for the chunked upload helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChunkedUploadSource {
    Path(PathBuf),
    InMemory {
        bytes: Vec<u8>,
        filename: Option<String>,
        byte_length: Option<u64>,
    },
}

/// Parameters for the chunked upload helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadChunkedParams {
    pub source: ChunkedUploadSource,
    pub mime_type: String,
    pub purpose: FilePurpose,
    pub part_size: Option<usize>,
    pub md5: Option<String>,
}

struct ResolvedChunkedUpload {
    filename: String,
    byte_length: u64,
    bytes: Vec<u8>,
}

impl ResolvedChunkedUpload {
    fn from_source(source: ChunkedUploadSource) -> Result<Self, OpenAIError> {
        match source {
            ChunkedUploadSource::Path(path) => {
                let filename = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .ok_or_else(|| {
                        OpenAIError::new(
                            ErrorKind::Validation,
                            format!("path `{}` does not have a file name", path.display()),
                        )
                    })?;
                let bytes = fs::read(&path).map_err(|error| {
                    OpenAIError::new(
                        ErrorKind::Transport,
                        format!(
                            "failed to read chunked upload source `{}`: {error}",
                            path.display()
                        ),
                    )
                    .with_source(error)
                })?;
                Ok(Self {
                    filename,
                    byte_length: bytes.len() as u64,
                    bytes,
                })
            }
            ChunkedUploadSource::InMemory {
                bytes,
                filename,
                byte_length,
            } => {
                let filename = filename.ok_or_else(|| {
                    OpenAIError::new(
                        ErrorKind::Validation,
                        "The `filename` argument must be given for in-memory files",
                    )
                })?;
                let byte_length = byte_length.ok_or_else(|| {
                    OpenAIError::new(
                        ErrorKind::Validation,
                        "The `bytes` argument must be given for in-memory files",
                    )
                })?;
                if byte_length != bytes.len() as u64 {
                    return Err(OpenAIError::new(
                        ErrorKind::Validation,
                        format!(
                            "declared byte length {byte_length} did not match in-memory data length {}",
                            bytes.len()
                        ),
                    ));
                }
                Ok(Self {
                    filename,
                    byte_length,
                    bytes,
                })
            }
        }
    }
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
