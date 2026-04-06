use std::{
    collections::BTreeMap,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{
        request::RequestOptions, response::ApiResponse, runtime::ClientRuntime,
        transport::execute_json,
    },
    error::ErrorKind,
    resources::files::{
        FileCreateParams, FilePurpose, FileUpload, Files, encode_path_id, validate_path_id,
    },
};

const VECTOR_STORE_BETA_HEADER: &str = "assistants=v2";
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Top-level vector stores API family.
#[derive(Clone, Debug)]
pub struct VectorStores {
    runtime: Arc<ClientRuntime>,
}

impl VectorStores {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns the nested vector-store files surface.
    pub fn files(&self) -> VectorStoreFiles {
        VectorStoreFiles::new(self.runtime.clone())
    }

    /// Returns the nested vector-store file-batches surface placeholder.
    pub fn file_batches(&self) -> VectorStoreFileBatches {
        VectorStoreFileBatches::new(self.runtime.clone())
    }

    /// Creates a vector store.
    pub fn create(
        &self,
        params: VectorStoreCreateParams,
    ) -> Result<ApiResponse<VectorStore>, OpenAIError> {
        execute_beta_json_with_body(
            &self.runtime,
            "POST",
            "/vector_stores",
            &params,
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Retrieves a vector store.
    pub fn retrieve(&self, vector_store_id: &str) -> Result<ApiResponse<VectorStore>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        execute_beta_json(
            &self.runtime,
            "GET",
            format!("/vector_stores/{vector_store_id}"),
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Updates a vector store.
    pub fn update(
        &self,
        vector_store_id: &str,
        params: VectorStoreUpdateParams,
    ) -> Result<ApiResponse<VectorStore>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        execute_beta_json_with_body(
            &self.runtime,
            "POST",
            format!("/vector_stores/{vector_store_id}"),
            &params,
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Lists vector stores with cursor pagination semantics.
    pub fn list(
        &self,
        params: VectorStoreListParams,
    ) -> Result<ApiResponse<VectorStoreListPage>, OpenAIError> {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(before) = params.before {
            serializer.append_pair("before", &before);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", &order);
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            String::from("/vector_stores")
        } else {
            format!("/vector_stores?{query}")
        };
        execute_beta_json(
            &self.runtime,
            "GET",
            path,
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Deletes a vector store.
    pub fn delete(
        &self,
        vector_store_id: &str,
    ) -> Result<ApiResponse<VectorStoreDeleteResponse>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        execute_beta_json(
            &self.runtime,
            "DELETE",
            format!("/vector_stores/{vector_store_id}"),
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Searches a vector store without collapsing the distinct POST-backed page contract.
    pub fn search(
        &self,
        vector_store_id: &str,
        params: VectorStoreSearchParams,
    ) -> Result<ApiResponse<VectorStoreSearchPage>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        execute_beta_json_with_body(
            &self.runtime,
            "POST",
            format!("/vector_stores/{vector_store_id}/search"),
            &params,
            RequestOptions::default(),
            Vec::new(),
        )
    }
}

/// Nested vector-store files API family.
#[derive(Clone, Debug)]
pub struct VectorStoreFiles {
    runtime: Arc<ClientRuntime>,
}

impl VectorStoreFiles {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Attaches an existing file to a vector store.
    pub fn create(
        &self,
        vector_store_id: &str,
        params: VectorStoreFileCreateParams,
    ) -> Result<ApiResponse<VectorStoreFile>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        execute_beta_json_with_body(
            &self.runtime,
            "POST",
            format!("/vector_stores/{vector_store_id}/files"),
            &params,
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Retrieves one attached vector-store file.
    pub fn retrieve(
        &self,
        vector_store_id: &str,
        file_id: &str,
    ) -> Result<ApiResponse<VectorStoreFile>, OpenAIError> {
        self.retrieve_with_headers(vector_store_id, file_id, Vec::new())
    }

    /// Updates per-file attributes.
    pub fn update(
        &self,
        vector_store_id: &str,
        file_id: &str,
        params: VectorStoreFileUpdateParams,
    ) -> Result<ApiResponse<VectorStoreFile>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        execute_beta_json_with_body(
            &self.runtime,
            "POST",
            format!("/vector_stores/{vector_store_id}/files/{file_id}"),
            &params,
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Lists files attached to a vector store.
    pub fn list(
        &self,
        vector_store_id: &str,
        params: VectorStoreFileListParams,
    ) -> Result<ApiResponse<VectorStoreFilesPage>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(before) = params.before {
            serializer.append_pair("before", &before);
        }
        if let Some(filter) = params.filter {
            serializer.append_pair("filter", &filter);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", &order);
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            format!("/vector_stores/{vector_store_id}/files")
        } else {
            format!("/vector_stores/{vector_store_id}/files?{query}")
        };
        execute_beta_json(
            &self.runtime,
            "GET",
            path,
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Removes a file attachment from the vector store.
    pub fn delete(
        &self,
        vector_store_id: &str,
        file_id: &str,
    ) -> Result<ApiResponse<VectorStoreFileDeleteResponse>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        execute_beta_json(
            &self.runtime,
            "DELETE",
            format!("/vector_stores/{vector_store_id}/files/{file_id}"),
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Retrieves parsed content for an attached vector-store file.
    pub fn content(
        &self,
        vector_store_id: &str,
        file_id: &str,
    ) -> Result<ApiResponse<VectorStoreFileContentPage>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        execute_beta_json(
            &self.runtime,
            "GET",
            format!("/vector_stores/{vector_store_id}/files/{file_id}/content"),
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Attaches a file and then polls until processing reaches a terminal state.
    pub fn create_and_poll(
        &self,
        vector_store_id: &str,
        params: VectorStoreFileCreateParams,
        options: VectorStoreFilePollOptions,
    ) -> Result<ApiResponse<VectorStoreFile>, OpenAIError> {
        let created = self.create(vector_store_id, params)?;
        self.poll(vector_store_id, &created.output.id, options)
    }

    /// Polls an attached file until it leaves the in-progress state.
    pub fn poll(
        &self,
        vector_store_id: &str,
        file_id: &str,
        options: VectorStoreFilePollOptions,
    ) -> Result<ApiResponse<VectorStoreFile>, OpenAIError> {
        let started = Instant::now();
        let poll_interval_ms = options
            .poll_interval
            .map(|interval| interval.as_millis().to_string());

        loop {
            let mut extra_headers = vec![(
                String::from("x-stainless-poll-helper"),
                String::from("true"),
            )];
            if let Some(value) = &poll_interval_ms {
                extra_headers.push((
                    String::from("x-stainless-custom-poll-interval"),
                    value.clone(),
                ));
            }

            let response = self.retrieve_with_headers(vector_store_id, file_id, extra_headers)?;
            let status = response
                .output
                .status
                .clone()
                .unwrap_or(VectorStoreFileStatus::Unknown);
            match status {
                VectorStoreFileStatus::InProgress => {
                    let sleep_interval = options.poll_interval.unwrap_or_else(|| {
                        response
                            .header("openai-poll-after-ms")
                            .and_then(|value| value.parse::<u64>().ok())
                            .map(Duration::from_millis)
                            .unwrap_or(DEFAULT_POLL_INTERVAL)
                    });
                    let elapsed = started.elapsed();
                    if elapsed > options.max_wait || elapsed + sleep_interval > options.max_wait {
                        return Err(OpenAIError::new(
                            ErrorKind::Timeout,
                            format!(
                                "Giving up on waiting for vector store file {file_id} to finish processing after {} milliseconds.",
                                options.max_wait.as_millis()
                            ),
                        ));
                    }
                    thread::sleep(sleep_interval);
                }
                VectorStoreFileStatus::Completed
                | VectorStoreFileStatus::Cancelled
                | VectorStoreFileStatus::Failed
                | VectorStoreFileStatus::Unknown => return Ok(response),
            }
        }
    }

    /// Uploads a normal File object with assistants purpose, then attaches it.
    pub fn upload(
        &self,
        vector_store_id: &str,
        params: VectorStoreFileUploadParams,
    ) -> Result<ApiResponse<VectorStoreFile>, OpenAIError> {
        let file = Files::new(self.runtime.clone()).create(FileCreateParams {
            file: params.file,
            purpose: FilePurpose::Assistants,
            expires_after: None,
        })?;
        self.create(
            vector_store_id,
            VectorStoreFileCreateParams {
                file_id: file.output.id,
                attributes: params.attributes,
                chunking_strategy: params.chunking_strategy,
            },
        )
    }

    /// Uploads via the Files API, attaches the resulting file, and polls for readiness.
    pub fn upload_and_poll(
        &self,
        vector_store_id: &str,
        params: VectorStoreFileUploadParams,
        options: VectorStoreFilePollOptions,
    ) -> Result<ApiResponse<VectorStoreFile>, OpenAIError> {
        let file = self.upload(vector_store_id, params)?;
        self.poll(vector_store_id, &file.output.id, options)
    }

    fn retrieve_with_headers(
        &self,
        vector_store_id: &str,
        file_id: &str,
        extra_headers: Vec<(String, String)>,
    ) -> Result<ApiResponse<VectorStoreFile>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        let file_id = encode_path_id(validate_path_id("file_id", file_id)?);
        execute_beta_json(
            &self.runtime,
            "GET",
            format!("/vector_stores/{vector_store_id}/files/{file_id}"),
            RequestOptions::default(),
            extra_headers,
        )
    }
}

/// Placeholder for the vector-store file-batches surface, implemented later in the milestone.
#[derive(Clone, Debug)]
pub struct VectorStoreFileBatches {
    runtime: Arc<ClientRuntime>,
}

impl VectorStoreFileBatches {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a vector-store file batch.
    pub fn create(
        &self,
        vector_store_id: &str,
        params: VectorStoreFileBatchCreateParams,
    ) -> Result<ApiResponse<VectorStoreFileBatch>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        execute_beta_json_with_body(
            &self.runtime,
            "POST",
            format!("/vector_stores/{vector_store_id}/file_batches"),
            &params,
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Retrieves a vector-store file batch.
    pub fn retrieve(
        &self,
        vector_store_id: &str,
        batch_id: &str,
    ) -> Result<ApiResponse<VectorStoreFileBatch>, OpenAIError> {
        self.retrieve_with_headers(vector_store_id, batch_id, Vec::new())
    }

    /// Cancels a vector-store file batch and returns the updated lifecycle resource.
    pub fn cancel(
        &self,
        vector_store_id: &str,
        batch_id: &str,
    ) -> Result<ApiResponse<VectorStoreFileBatch>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        let batch_id = encode_path_id(validate_path_id("batch_id", batch_id)?);
        execute_beta_json(
            &self.runtime,
            "POST",
            format!("/vector_stores/{vector_store_id}/file_batches/{batch_id}/cancel"),
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Lists files that belong to a vector-store file batch.
    pub fn list_files(
        &self,
        vector_store_id: &str,
        batch_id: &str,
        params: VectorStoreFileBatchListFilesParams,
    ) -> Result<ApiResponse<VectorStoreFilesPage>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        let batch_id = encode_path_id(validate_path_id("batch_id", batch_id)?);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(after) = params.after {
            serializer.append_pair("after", &after);
        }
        if let Some(before) = params.before {
            serializer.append_pair("before", &before);
        }
        if let Some(filter) = params.filter {
            serializer.append_pair("filter", &filter);
        }
        if let Some(limit) = params.limit {
            serializer.append_pair("limit", &limit.to_string());
        }
        if let Some(order) = params.order {
            serializer.append_pair("order", &order);
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            format!("/vector_stores/{vector_store_id}/file_batches/{batch_id}/files")
        } else {
            format!("/vector_stores/{vector_store_id}/file_batches/{batch_id}/files?{query}")
        };
        execute_beta_json(
            &self.runtime,
            "GET",
            path,
            RequestOptions::default(),
            Vec::new(),
        )
    }

    /// Creates a vector-store file batch and polls it to a terminal state.
    pub fn create_and_poll(
        &self,
        vector_store_id: &str,
        params: VectorStoreFileBatchCreateParams,
        options: VectorStoreFileBatchPollOptions,
    ) -> Result<ApiResponse<VectorStoreFileBatch>, OpenAIError> {
        let created = self.create(vector_store_id, params)?;
        self.poll(vector_store_id, &created.output.id, options)
    }

    /// Polls a vector-store file batch until it reaches a terminal state.
    pub fn poll(
        &self,
        vector_store_id: &str,
        batch_id: &str,
        options: VectorStoreFileBatchPollOptions,
    ) -> Result<ApiResponse<VectorStoreFileBatch>, OpenAIError> {
        let started = Instant::now();
        let poll_interval_ms = options
            .poll_interval
            .map(|interval| interval.as_millis().to_string());

        loop {
            let mut extra_headers = vec![(
                String::from("x-stainless-poll-helper"),
                String::from("true"),
            )];
            if let Some(value) = &poll_interval_ms {
                extra_headers.push((
                    String::from("x-stainless-custom-poll-interval"),
                    value.clone(),
                ));
            }

            let response = self.retrieve_with_headers(vector_store_id, batch_id, extra_headers)?;
            match response.output.status {
                VectorStoreFileBatchStatus::InProgress => {
                    let sleep_interval = options.poll_interval.unwrap_or_else(|| {
                        response
                            .header("openai-poll-after-ms")
                            .and_then(|value| value.parse::<u64>().ok())
                            .map(Duration::from_millis)
                            .unwrap_or(DEFAULT_POLL_INTERVAL)
                    });
                    let elapsed = started.elapsed();
                    if elapsed > options.max_wait || elapsed + sleep_interval > options.max_wait {
                        return Err(OpenAIError::new(
                            ErrorKind::Timeout,
                            format!(
                                "Giving up on waiting for vector store file batch {batch_id} to finish processing after {} milliseconds.",
                                options.max_wait.as_millis()
                            ),
                        ));
                    }
                    thread::sleep(sleep_interval);
                }
                VectorStoreFileBatchStatus::Completed
                | VectorStoreFileBatchStatus::Cancelled
                | VectorStoreFileBatchStatus::Failed
                | VectorStoreFileBatchStatus::Unknown => return Ok(response),
            }
        }
    }

    /// Uploads new files through the Files API, merges any existing file ids, then creates and polls the batch.
    pub fn upload_and_poll(
        &self,
        vector_store_id: &str,
        params: VectorStoreFileBatchUploadAndPollParams,
        options: VectorStoreFileBatchPollOptions,
    ) -> Result<ApiResponse<VectorStoreFileBatch>, OpenAIError> {
        if params.files.is_empty() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "No `files` provided to process. Use `create_and_poll` when you only have existing file ids.",
            ));
        }

        let mut file_ids = params.file_ids;
        let files = Files::new(self.runtime.clone());
        for file in params.files {
            let uploaded = files.create(FileCreateParams {
                file,
                purpose: FilePurpose::Assistants,
                expires_after: None,
            })?;
            file_ids.push(uploaded.output.id);
        }

        self.create_and_poll(
            vector_store_id,
            VectorStoreFileBatchCreateParams {
                attributes: None,
                chunking_strategy: None,
                file_ids,
                files: Vec::new(),
            },
            options,
        )
    }

    fn retrieve_with_headers(
        &self,
        vector_store_id: &str,
        batch_id: &str,
        extra_headers: Vec<(String, String)>,
    ) -> Result<ApiResponse<VectorStoreFileBatch>, OpenAIError> {
        let vector_store_id = encode_path_id(validate_path_id("vector_store_id", vector_store_id)?);
        let batch_id = encode_path_id(validate_path_id("batch_id", batch_id)?);
        execute_beta_json(
            &self.runtime,
            "GET",
            format!("/vector_stores/{vector_store_id}/file_batches/{batch_id}"),
            RequestOptions::default(),
            extra_headers,
        )
    }
}

/// Create-vector-store body.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct VectorStoreCreateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<FileChunkingStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<VectorStoreExpiresAfter>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub file_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Update-vector-store body.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct VectorStoreUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<VectorStoreExpiresAfter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// List-vector-stores query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VectorStoreListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<String>,
}

/// POST-backed vector-store search parameters.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VectorStoreSearchParams {
    pub query: VectorStoreSearchQuery,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_num_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking_options: Option<VectorStoreSearchRankingOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite_query: Option<bool>,
}

/// Search query string or query array.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum VectorStoreSearchQuery {
    Single(String),
    Multiple(Vec<String>),
}

/// Search ranking configuration.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct VectorStoreSearchRankingOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f64>,
}

/// File-create attach parameters.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VectorStoreFileCreateParams {
    pub file_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<FileChunkingStrategy>,
}

/// File-update parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct VectorStoreFileUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
}

/// File-list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VectorStoreFileListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub filter: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<String>,
}

/// File-batch create body.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct VectorStoreFileBatchCreateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<FileChunkingStrategy>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub file_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub files: Vec<VectorStoreFileBatchFile>,
}

/// One per-file entry for vector-store batch creation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VectorStoreFileBatchFile {
    pub file_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<FileChunkingStrategy>,
}

/// File-batch list-files query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VectorStoreFileBatchListFilesParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub filter: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<String>,
}

/// Polling options for vector-store file helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorStoreFilePollOptions {
    pub poll_interval: Option<Duration>,
    pub max_wait: Duration,
}

impl Default for VectorStoreFilePollOptions {
    fn default() -> Self {
        Self {
            poll_interval: None,
            max_wait: Duration::from_secs(30 * 60),
        }
    }
}

/// Polling options for vector-store file-batch helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorStoreFileBatchPollOptions {
    pub poll_interval: Option<Duration>,
    pub max_wait: Duration,
}

impl Default for VectorStoreFileBatchPollOptions {
    fn default() -> Self {
        Self {
            poll_interval: None,
            max_wait: Duration::from_secs(30 * 60),
        }
    }
}

/// Upload helper parameters for vector-store files.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorStoreFileUploadParams {
    pub file: FileUpload,
    pub attributes: Option<Value>,
    pub chunking_strategy: Option<FileChunkingStrategy>,
}

/// Upload-and-poll helper parameters for vector-store file batches.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VectorStoreFileBatchUploadAndPollParams {
    pub files: Vec<FileUpload>,
    pub file_ids: Vec<String>,
}

/// Shared file chunking strategy type for vector stores and vector-store files.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileChunkingStrategy {
    Auto,
    Static {
        #[serde(rename = "static")]
        static_config: StaticChunkingStrategy,
    },
    #[serde(other)]
    Other,
}

/// Static chunking settings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StaticChunkingStrategy {
    pub max_chunk_size_tokens: u32,
    pub chunk_overlap_tokens: u32,
}

/// Vector-store expiration policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct VectorStoreExpiresAfter {
    pub anchor: String,
    pub days: u32,
}

/// Vector-store status.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreStatus {
    Expired,
    InProgress,
    Completed,
    #[default]
    #[serde(other)]
    Unknown,
}

/// Attached vector-store file status.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreFileStatus {
    InProgress,
    Completed,
    Cancelled,
    Failed,
    #[default]
    #[serde(other)]
    Unknown,
}

/// Vector-store file batch status.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreFileBatchStatus {
    InProgress,
    Completed,
    Cancelled,
    Failed,
    #[default]
    #[serde(other)]
    Unknown,
}

/// Typed vector-store resource.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStore {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub file_counts: Option<VectorStoreFileCounts>,
    #[serde(default)]
    pub last_active_at: Option<u64>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<VectorStoreStatus>,
    #[serde(default)]
    pub usage_bytes: Option<u64>,
    #[serde(default)]
    pub expires_after: Option<VectorStoreExpiresAfter>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Vector-store file counters.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreFileCounts {
    #[serde(default)]
    pub in_progress: u64,
    #[serde(default)]
    pub completed: u64,
    #[serde(default)]
    pub failed: u64,
    #[serde(default)]
    pub cancelled: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Vector-store file batch resource.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreFileBatch {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub file_counts: VectorStoreFileCounts,
    pub status: VectorStoreFileBatchStatus,
    #[serde(default)]
    pub vector_store_id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page for vector-store lists.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreListPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<VectorStore>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl VectorStoreListPage {
    pub fn has_next_page(&self) -> bool {
        self.has_more
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_more {
            self.data.last().map(|store| store.id.as_str())
        } else {
            None
        }
    }
}

/// Typed vector-store deletion marker.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreDeleteResponse {
    pub id: String,
    #[serde(default)]
    pub object: String,
    pub deleted: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// One vector-store search hit.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreSearchResult {
    #[serde(default)]
    pub attributes: Option<Value>,
    #[serde(default)]
    pub content: Vec<VectorStoreSearchContentPart>,
    #[serde(default)]
    pub file_id: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub score: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Search-result content chunk.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreSearchContentPart {
    #[serde(default)]
    pub text: String,
    #[serde(default, rename = "type")]
    pub r#type: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Forward-compatible page for vector-store search results.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreSearchPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<VectorStoreSearchResult>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Attached vector-store file resource.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreFile {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub last_error: Option<VectorStoreFileLastError>,
    #[serde(default)]
    pub status: Option<VectorStoreFileStatus>,
    #[serde(default)]
    pub usage_bytes: u64,
    #[serde(default)]
    pub vector_store_id: String,
    #[serde(default)]
    pub attributes: Option<Value>,
    #[serde(default)]
    pub chunking_strategy: Option<FileChunkingStrategy>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Attached-file failure detail.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreFileLastError {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page for vector-store file lists.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreFilesPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<VectorStoreFile>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl VectorStoreFilesPage {
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

/// Deletion marker for removing a file from a vector store.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreFileDeleteResponse {
    pub id: String,
    #[serde(default)]
    pub object: String,
    pub deleted: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Parsed file-content item.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreFileContentPart {
    #[serde(default, rename = "type")]
    pub r#type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Forward-compatible content page for parsed vector-store file content.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VectorStoreFileContentPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<VectorStoreFileContentPart>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn execute_beta_json<T>(
    runtime: &Arc<ClientRuntime>,
    method: impl AsRef<str>,
    path: impl AsRef<str>,
    options: RequestOptions,
    extra_headers: Vec<(String, String)>,
) -> Result<ApiResponse<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    let mut request = runtime.prepare_request(method, path)?;
    request.headers.insert(
        String::from("openai-beta"),
        String::from(VECTOR_STORE_BETA_HEADER),
    );
    for (name, value) in extra_headers {
        request.headers.insert(name, value);
    }
    let resolved = runtime.resolve_request_options(&options)?;
    execute_json(&request, &resolved)
}

fn execute_beta_json_with_body<B, T>(
    runtime: &Arc<ClientRuntime>,
    method: impl AsRef<str>,
    path: impl AsRef<str>,
    body: &B,
    options: RequestOptions,
    extra_headers: Vec<(String, String)>,
) -> Result<ApiResponse<T>, OpenAIError>
where
    B: Serialize,
    T: DeserializeOwned,
{
    let mut request = runtime.prepare_json_request(method, path, body)?;
    request.headers.insert(
        String::from("openai-beta"),
        String::from(VECTOR_STORE_BETA_HEADER),
    );
    for (name, value) in extra_headers {
        request.headers.insert(name, value);
    }
    let resolved = runtime.resolve_request_options(&options)?;
    execute_json(&request, &resolved)
}
