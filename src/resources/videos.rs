use std::{
    collections::BTreeMap,
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
    resources::files::{encode_path_id, validate_path_id},
};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Videos API family.
#[derive(Clone, Debug)]
pub struct Videos {
    runtime: Arc<ClientRuntime>,
}

impl Videos {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a new video generation job.
    pub fn create(&self, params: VideoCreateParams) -> Result<ApiResponse<Video>, OpenAIError> {
        let multipart = params.into_multipart();
        self.execute_json_multipart("/videos", multipart)
    }

    /// Creates a new video and polls until the job reaches a terminal state.
    pub fn create_and_poll(
        &self,
        params: VideoCreateParams,
        options: VideoPollOptions,
    ) -> Result<ApiResponse<Video>, OpenAIError> {
        let created = self.create(params)?;
        self.poll(&created.output.id, options)
    }

    /// Polls a video until it leaves an active state.
    pub fn poll(
        &self,
        video_id: &str,
        options: VideoPollOptions,
    ) -> Result<ApiResponse<Video>, OpenAIError> {
        let started = Instant::now();
        let custom_poll_interval_ms = options
            .poll_interval
            .map(|interval| interval.as_millis().to_string());

        loop {
            let mut extra_headers = vec![(
                String::from("x-stainless-poll-helper"),
                String::from("true"),
            )];
            if let Some(value) = &custom_poll_interval_ms {
                extra_headers.push((
                    String::from("x-stainless-custom-poll-interval"),
                    value.clone(),
                ));
            }

            let response = self.retrieve_with_headers(video_id, extra_headers)?;
            match response.output.status.clone() {
                VideoStatus::Queued | VideoStatus::InProgress => {
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
                                "Giving up on waiting for video {video_id} to finish processing after {} milliseconds.",
                                options.max_wait.as_millis()
                            ),
                        ));
                    }
                    thread::sleep(sleep_interval);
                }
                VideoStatus::Completed | VideoStatus::Failed | VideoStatus::Unknown(_) => {
                    return Ok(response);
                }
            }
        }
    }

    /// Retrieves a video job by id.
    pub fn retrieve(&self, video_id: &str) -> Result<ApiResponse<Video>, OpenAIError> {
        self.retrieve_with_headers(video_id, Vec::new())
    }

    /// Lists videos with cursor pagination semantics.
    pub fn list(&self, params: VideoListParams) -> Result<ApiResponse<VideosPage>, OpenAIError> {
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
            String::from("/videos")
        } else {
            format!("/videos?{query}")
        };
        self.runtime
            .execute_json("GET", path, RequestOptions::default())
    }

    /// Deletes a video job.
    pub fn delete(&self, video_id: &str) -> Result<ApiResponse<VideoDeleteResponse>, OpenAIError> {
        let video_id = encode_path_id(validate_path_id("video_id", video_id)?);
        self.runtime.execute_json(
            "DELETE",
            format!("/videos/{video_id}"),
            RequestOptions::default(),
        )
    }

    /// Creates a character from an uploaded video.
    pub fn create_character(
        &self,
        params: VideoCreateCharacterParams,
    ) -> Result<ApiResponse<VideoCharacter>, OpenAIError> {
        let mut builder = MultipartBuilder::new();
        builder.add_text("name", params.name);
        builder.add_file("video", params.video.to_multipart_file());
        let multipart = builder.build();
        let content_type = multipart.content_type();
        let mut request = self.runtime.prepare_request_with_body(
            "POST",
            "/videos/characters",
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

    /// Retrieves a character created from a prior upload.
    pub fn get_character(
        &self,
        character_id: &str,
    ) -> Result<ApiResponse<VideoCharacter>, OpenAIError> {
        let character_id = encode_path_id(validate_path_id("character_id", character_id)?);
        self.runtime.execute_json(
            "GET",
            format!("/videos/characters/{character_id}"),
            RequestOptions::default(),
        )
    }

    /// Downloads rendered video bytes or derived preview assets.
    pub fn download_content(
        &self,
        video_id: &str,
        params: VideoDownloadContentParams,
    ) -> Result<ApiResponse<Vec<u8>>, OpenAIError> {
        let video_id = encode_path_id(validate_path_id("video_id", video_id)?);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        if let Some(variant) = params.variant {
            serializer.append_pair("variant", variant.as_str());
        }
        let query = serializer.finish();
        let path = if query.is_empty() {
            format!("/videos/{video_id}/content")
        } else {
            format!("/videos/{video_id}/content?{query}")
        };
        let mut request = self.runtime.prepare_request("GET", path)?;
        request
            .headers
            .insert(String::from("accept"), String::from("application/binary"));
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        execute_bytes(&request, &options)
    }

    /// Creates an edit job from an existing video id or uploaded video bytes.
    pub fn edit(&self, params: VideoEditParams) -> Result<ApiResponse<Video>, OpenAIError> {
        let multipart = params.into_multipart();
        self.execute_json_multipart("/videos/edits", multipart)
    }

    /// Creates an extension job from an existing video id or uploaded video bytes.
    pub fn extend(&self, params: VideoExtendParams) -> Result<ApiResponse<Video>, OpenAIError> {
        let multipart = params.into_multipart();
        self.execute_json_multipart("/videos/extensions", multipart)
    }

    /// Creates a remix job for an existing video.
    pub fn remix(
        &self,
        video_id: &str,
        params: VideoRemixParams,
    ) -> Result<ApiResponse<Video>, OpenAIError> {
        let video_id = encode_path_id(validate_path_id("video_id", video_id)?);
        self.runtime.execute_json_with_body(
            "POST",
            format!("/videos/{video_id}/remix"),
            &params,
            RequestOptions::default(),
        )
    }

    fn retrieve_with_headers(
        &self,
        video_id: &str,
        extra_headers: Vec<(String, String)>,
    ) -> Result<ApiResponse<Video>, OpenAIError> {
        let video_id = encode_path_id(validate_path_id("video_id", video_id)?);
        let mut request = self
            .runtime
            .prepare_request("GET", format!("/videos/{video_id}"))?;
        for (name, value) in extra_headers {
            request.headers.insert(name, value);
        }
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        let response = execute_bytes(&request, &options)?;
        parse_json_bytes_response(response)
    }

    fn execute_json_multipart(
        &self,
        path: &str,
        multipart: crate::helpers::multipart::MultipartPayload,
    ) -> Result<ApiResponse<Video>, OpenAIError> {
        let content_type = multipart.content_type();
        let mut request =
            self.runtime
                .prepare_request_with_body("POST", path, Some(multipart.into_body()))?;
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
}

/// Uploaded video bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoUpload {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl VideoUpload {
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

/// Nested create reference metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct VideoReferenceAsset {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

impl VideoReferenceAsset {
    pub fn file_id(file_id: impl Into<String>) -> Self {
        Self {
            file_id: Some(file_id.into()),
            image_url: None,
        }
    }

    pub fn image_url(image_url: impl Into<String>) -> Self {
        Self {
            file_id: None,
            image_url: Some(image_url.into()),
        }
    }
}

/// Create-time reference input for videos.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VideoCreateReference {
    Upload(VideoUpload),
    Asset(VideoReferenceAsset),
}

/// Seconds allowed on create.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoCreateSeconds {
    S4,
    S8,
    S12,
}

impl VideoCreateSeconds {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::S4 => "4",
            Self::S8 => "8",
            Self::S12 => "12",
        }
    }
}

/// Seconds allowed on extend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoExtendSeconds {
    S4,
    S8,
    S12,
    S16,
    S20,
}

impl VideoExtendSeconds {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::S4 => "4",
            Self::S8 => "8",
            Self::S12 => "12",
            Self::S16 => "16",
            Self::S20 => "20",
        }
    }
}

/// Supported video sizes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoSize {
    Portrait720,
    Landscape720,
    Portrait1024,
    Landscape1792,
}

impl VideoSize {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Portrait720 => "720x1280",
            Self::Landscape720 => "1280x720",
            Self::Portrait1024 => "1024x1792",
            Self::Landscape1792 => "1792x1024",
        }
    }
}

/// Video model selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VideoModel {
    Sora2,
    Sora2Pro,
    Custom(String),
}

impl VideoModel {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Sora2 => "sora-2",
            Self::Sora2Pro => "sora-2-pro",
            Self::Custom(value) => value.as_str(),
        }
    }
}

/// Video create params.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoCreateParams {
    pub prompt: String,
    pub input_reference: Option<VideoCreateReference>,
    pub model: Option<VideoModel>,
    pub seconds: Option<VideoCreateSeconds>,
    pub size: Option<VideoSize>,
}

impl VideoCreateParams {
    fn into_multipart(self) -> crate::helpers::multipart::MultipartPayload {
        let mut builder = MultipartBuilder::new();
        builder.add_text("prompt", self.prompt);
        if let Some(model) = self.model {
            builder.add_text("model", model.as_str());
        }
        if let Some(seconds) = self.seconds {
            builder.add_text("seconds", seconds.as_str());
        }
        if let Some(size) = self.size {
            builder.add_text("size", size.as_str());
        }
        if let Some(input_reference) = self.input_reference {
            match input_reference {
                VideoCreateReference::Upload(upload) => {
                    builder.add_file("input_reference", upload.to_multipart_file());
                }
                VideoCreateReference::Asset(reference) => {
                    if let Some(file_id) = reference.file_id {
                        builder.add_text("input_reference[file_id]", file_id);
                    }
                    if let Some(image_url) = reference.image_url {
                        builder.add_text("input_reference[image_url]", image_url);
                    }
                }
            }
        }
        builder.build()
    }
}

/// Video list params.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VideoListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<VideoOrder>,
}

/// Video list order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoOrder {
    Asc,
    Desc,
}

impl VideoOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Variants downloadable from `/content`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoContentVariant {
    Video,
    Thumbnail,
    Spritesheet,
}

impl VideoContentVariant {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Thumbnail => "thumbnail",
            Self::Spritesheet => "spritesheet",
        }
    }
}

/// Content download params.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VideoDownloadContentParams {
    pub variant: Option<VideoContentVariant>,
}

/// Source selection used by edit and extend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VideoSource {
    Upload(VideoUpload),
    Id(String),
}

impl VideoSource {
    pub fn upload(upload: VideoUpload) -> Self {
        Self::Upload(upload)
    }

    pub fn id(id: impl Into<String>) -> Self {
        Self::Id(id.into())
    }
}

/// Video edit params.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoEditParams {
    pub prompt: String,
    pub video: VideoSource,
}

impl VideoEditParams {
    fn into_multipart(self) -> crate::helpers::multipart::MultipartPayload {
        let mut builder = MultipartBuilder::new();
        builder.add_text("prompt", self.prompt);
        match self.video {
            VideoSource::Upload(upload) => {
                builder.add_file("video", upload.to_multipart_file());
            }
            VideoSource::Id(id) => {
                builder.add_text("video[id]", id);
            }
        };
        builder.build()
    }
}

/// Video extend params.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoExtendParams {
    pub prompt: String,
    pub seconds: VideoExtendSeconds,
    pub video: VideoSource,
}

impl VideoExtendParams {
    fn into_multipart(self) -> crate::helpers::multipart::MultipartPayload {
        let mut builder = MultipartBuilder::new();
        builder.add_text("prompt", self.prompt);
        builder.add_text("seconds", self.seconds.as_str());
        match self.video {
            VideoSource::Upload(upload) => {
                builder.add_file("video", upload.to_multipart_file());
            }
            VideoSource::Id(id) => {
                builder.add_text("video[id]", id);
            }
        };
        builder.build()
    }
}

/// Video remix params.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VideoRemixParams {
    pub prompt: String,
}

/// Character creation params.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoCreateCharacterParams {
    pub name: String,
    pub video: VideoUpload,
}

/// Polling options for video jobs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VideoPollOptions {
    pub poll_interval: Option<Duration>,
    pub max_wait: Duration,
}

impl Default for VideoPollOptions {
    fn default() -> Self {
        Self {
            poll_interval: None,
            max_wait: Duration::from_secs(30 * 60),
        }
    }
}

/// Video status enum.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VideoStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
    Unknown(String),
}

impl Default for VideoStatus {
    fn default() -> Self {
        Self::Unknown(String::from("unknown"))
    }
}

impl<'de> Deserialize<'de> for VideoStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "queued" => Self::Queued,
            "in_progress" => Self::InProgress,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Unknown(value),
        })
    }
}

/// Error payload returned by a failed video job.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct VideoError {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed video job resource.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct Video {
    pub id: String,
    #[serde(default)]
    pub completed_at: Option<u64>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub error: Option<VideoError>,
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub progress: u64,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub remixed_from_video_id: Option<String>,
    #[serde(default)]
    pub seconds: String,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub status: VideoStatus,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page for video jobs.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct VideosPage {
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub data: Vec<Video>,
    #[serde(default)]
    pub first_id: Option<String>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl VideosPage {
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

/// Video delete response.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct VideoDeleteResponse {
    pub id: String,
    pub deleted: bool,
    #[serde(default)]
    pub object: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Character resource returned by character endpoints.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct VideoCharacter {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub name: Option<String>,
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
