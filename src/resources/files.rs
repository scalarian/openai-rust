use std::{
    collections::BTreeMap,
    fmt::{self, Write as _},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

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
};

/// Files API family.
#[derive(Clone, Debug)]
pub struct Files {
    runtime: Arc<ClientRuntime>,
}

impl Files {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Uploads a file using multipart form data.
    pub fn create(&self, params: FileCreateParams) -> Result<ApiResponse<FileObject>, OpenAIError> {
        let multipart = params.into_multipart();
        let content_type = multipart.content_type();
        let mut request = self.runtime.prepare_request_with_body(
            "POST",
            "/files",
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

    /// Lists files with cursor pagination controls.
    pub fn list(&self, params: FileListParams) -> Result<ApiResponse<FilesPage>, OpenAIError> {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", &order);
        }
        if let Some(purpose) = params.purpose {
            serializer.append_pair("purpose", purpose.as_str());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            String::from("/files")
        } else {
            format!("/files?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Retrieves one file object by id.
    pub fn retrieve(&self, file_id: &str) -> Result<ApiResponse<FileObject>, OpenAIError> {
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/files/{file_id}"),
            RequestOptions::default(),
        )
    }

    /// Deletes a file and returns the typed deletion marker.
    pub fn delete(&self, file_id: &str) -> Result<ApiResponse<FileDeleteResponse>, OpenAIError> {
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        self.runtime.execute_json(
            "DELETE",
            format!("/files/{file_id}"),
            RequestOptions::default(),
        )
    }

    /// Downloads file contents as raw bytes.
    pub fn content(&self, file_id: &str) -> Result<ApiResponse<Vec<u8>>, OpenAIError> {
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        let mut request = self
            .runtime
            .prepare_request("GET", format!("/files/{file_id}/content"))?;
        request.headers.insert(
            String::from("accept"),
            String::from("application/octet-stream"),
        );
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        execute_bytes(&request, &options)
    }

    /// Polls retrieve until the file reaches a terminal processing state or times out.
    pub fn wait_for_processing(
        &self,
        file_id: &str,
        options: WaitForProcessingOptions,
    ) -> Result<ApiResponse<FileObject>, OpenAIError> {
        let started = Instant::now();
        let mut response = self.retrieve(file_id)?;

        while !response.output.is_terminal_processing_state() {
            let elapsed = started.elapsed();
            if elapsed > options.max_wait || elapsed + options.poll_interval > options.max_wait {
                return Err(OpenAIError::new(
                    ErrorKind::Timeout,
                    format!(
                        "Giving up on waiting for file {file_id} to finish processing after {} milliseconds.",
                        options.max_wait.as_millis()
                    ),
                ));
            }
            thread::sleep(options.poll_interval);
            response = self.retrieve(file_id)?;
        }

        Ok(response)
    }
}

/// Multipart upload file input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileUpload {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl FileUpload {
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

/// File creation parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileCreateParams {
    pub file: FileUpload,
    pub purpose: FilePurpose,
    pub expires_after: Option<FileExpiresAfter>,
}

impl FileCreateParams {
    fn into_multipart(self) -> crate::helpers::multipart::MultipartPayload {
        let mut builder = MultipartBuilder::new();
        builder.add_file("file", self.file.to_multipart_file());
        builder.add_text("purpose", self.purpose.to_string());
        if let Some(expires_after) = self.expires_after {
            builder.add_text(
                "expires_after",
                serde_json::to_string(&expires_after).unwrap_or_else(|_| String::from("{}")),
            );
        }
        builder.build()
    }
}

/// File expiration policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileExpiresAfter {
    pub anchor: String,
    pub seconds: u64,
}

/// List parameters for files.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<String>,
    pub purpose: Option<FilePurpose>,
}

/// Public file purpose enum shared with uploads.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FilePurpose {
    #[serde(rename = "assistants")]
    Assistants,
    #[serde(rename = "assistants_output")]
    AssistantsOutput,
    #[serde(rename = "batch")]
    Batch,
    #[serde(rename = "batch_output")]
    BatchOutput,
    #[serde(rename = "fine-tune")]
    FineTune,
    #[serde(rename = "fine-tune-results")]
    FineTuneResults,
    #[serde(rename = "vision")]
    Vision,
    #[serde(rename = "user_data")]
    UserData,
    #[serde(rename = "evals")]
    Evals,
}

impl FilePurpose {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Assistants => "assistants",
            Self::AssistantsOutput => "assistants_output",
            Self::Batch => "batch",
            Self::BatchOutput => "batch_output",
            Self::FineTune => "fine-tune",
            Self::FineTuneResults => "fine-tune-results",
            Self::Vision => "vision",
            Self::UserData => "user_data",
            Self::Evals => "evals",
        }
    }
}

impl fmt::Display for FilePurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// File processing status.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Uploaded,
    Processed,
    Error,
    Deleted,
    #[serde(other)]
    Unknown,
}

/// Typed file object.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FileObject {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub purpose: Option<FilePurpose>,
    #[serde(default)]
    pub status: Option<FileStatus>,
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub status_details: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FileObject {
    pub fn is_terminal_processing_state(&self) -> bool {
        matches!(
            self.status,
            Some(FileStatus::Processed | FileStatus::Error | FileStatus::Deleted)
        )
    }
}

/// Cursor page for file lists.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FilesPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<FileObject>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FilesPage {
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

/// File deletion response.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FileDeleteResponse {
    pub id: String,
    #[serde(default)]
    pub object: String,
    pub deleted: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Polling options for `wait_for_processing`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WaitForProcessingOptions {
    pub poll_interval: Duration,
    pub max_wait: Duration,
}

impl Default for WaitForProcessingOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            max_wait: Duration::from_secs(30 * 60),
        }
    }
}

pub(crate) fn validate_path_id<'a>(label: &str, value: &'a str) -> Result<&'a str, OpenAIError> {
    if value.trim().is_empty() {
        return Err(OpenAIError::new(
            ErrorKind::Validation,
            format!("{label} cannot be blank"),
        ));
    }
    Ok(value)
}

pub(crate) fn encode_path_id(value: &str) -> String {
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
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
        ) {
            encoded.push(byte as char);
        } else {
            let _ = write!(&mut encoded, "%{byte:02X}");
        }
    }
    encoded
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
