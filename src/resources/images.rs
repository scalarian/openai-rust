use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::runtime::Builder;

use crate::{
    OpenAIError,
    core::{
        metadata::ResponseMetadata,
        request::RequestOptions,
        runtime::ClientRuntime,
        transport::{execute_json, execute_text_stream},
    },
    error::ErrorKind,
    helpers::{
        multipart::{MultipartBuilder, MultipartFile},
        sse::{SseFrame, SseParser},
    },
};

/// Images API family.
#[derive(Clone, Debug)]
pub struct Images {
    runtime: Arc<ClientRuntime>,
}

impl Images {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates an image generation request and parses the typed JSON response.
    pub fn generate(
        &self,
        params: ImageGenerateParams,
    ) -> Result<crate::ApiResponse<ImagesResponse>, OpenAIError> {
        params.validate_for_generate()?;
        let body = params.into_request_body(false);
        self.runtime.execute_json_with_body(
            "POST",
            "/images/generations",
            &body,
            RequestOptions::default(),
        )
    }

    /// Streams image generation events until a terminal completed event is observed.
    pub fn generate_stream(
        &self,
        params: ImageGenerateParams,
    ) -> Result<ImageGenerationStream, OpenAIError> {
        let body = params.into_request_body(true);
        let request = self
            .runtime
            .prepare_json_request("POST", "/images/generations", &body)?;
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Transport,
                    format!("failed to build images streaming runtime: {error}"),
                )
                .with_source(error)
            })?;

        let (metadata, chunks) = runtime.block_on(async move {
            let response = execute_text_stream(&request, &options).await?;
            let metadata = response.metadata.clone();
            let mut response_body = response.response;
            let mut chunks = Vec::new();
            while let Some(chunk) = response_body
                .chunk()
                .await
                .map_err(map_live_transport_error)?
            {
                chunks.push(String::from_utf8_lossy(chunk.as_ref()).to_string());
            }
            Ok::<_, OpenAIError>((metadata, chunks))
        })?;

        ImageGenerationStream::from_sse_chunks(metadata, chunks)
    }

    /// Creates an edited image using multipart semantics.
    pub fn edit(
        &self,
        params: ImageEditParams,
    ) -> Result<crate::ApiResponse<ImagesResponse>, OpenAIError> {
        params.validate_for_edit()?;
        let multipart = params.into_multipart(false)?;
        self.execute_json_multipart("/images/edits", multipart)
    }

    /// Streams image-edit events using multipart semantics.
    pub fn edit_stream(&self, params: ImageEditParams) -> Result<ImageEditStream, OpenAIError> {
        let multipart = params.into_multipart(true)?;
        let mut request = self.runtime.prepare_request_with_body(
            "POST",
            "/images/edits",
            Some(multipart.body().to_vec()),
        )?;
        request
            .headers
            .insert(String::from("content-type"), multipart.content_type());
        request
            .headers
            .insert(String::from("accept"), String::from("text/event-stream"));
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Transport,
                    format!("failed to build images edit streaming runtime: {error}"),
                )
                .with_source(error)
            })?;

        let (metadata, chunks) = runtime.block_on(async move {
            let response = execute_text_stream(&request, &options).await?;
            let metadata = response.metadata.clone();
            let mut response_body = response.response;
            let mut chunks = Vec::new();
            while let Some(chunk) = response_body
                .chunk()
                .await
                .map_err(map_live_transport_error)?
            {
                chunks.push(String::from_utf8_lossy(chunk.as_ref()).to_string());
            }
            Ok::<_, OpenAIError>((metadata, chunks))
        })?;

        ImageEditStream::from_sse_chunks(metadata, chunks)
    }

    /// Creates a variation from one source image using the DALL·E-style multipart contract.
    pub fn create_variation(
        &self,
        params: ImageVariationParams,
    ) -> Result<crate::ApiResponse<ImagesResponse>, OpenAIError> {
        let multipart = params.into_multipart()?;
        self.execute_json_multipart("/images/variations", multipart)
    }

    fn execute_json_multipart(
        &self,
        path: &str,
        multipart: crate::helpers::multipart::MultipartPayload,
    ) -> Result<crate::ApiResponse<ImagesResponse>, OpenAIError> {
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
        execute_json(&request, &options)
    }
}

/// Uploadable image or mask input.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImageInput {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl ImageInput {
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

/// Image generation parameters.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ImageGenerateParams {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moderation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_compression: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_images: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ImageGenerateParams {
    fn validate_for_generate(&self) -> Result<(), OpenAIError> {
        if self.stream == Some(true) {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "images.generate() is non-streaming; call generate_stream() instead of setting `stream=true`",
            ));
        }
        Ok(())
    }

    fn into_request_body(self, stream: bool) -> Value {
        let mut value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        if let Value::Object(ref mut object) = value {
            object.insert(String::from("stream"), Value::Bool(stream));
        }
        value
    }
}

/// Image edit parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImageEditParams {
    pub images: Vec<ImageInput>,
    pub prompt: String,
    pub background: Option<String>,
    pub input_fidelity: Option<String>,
    pub mask: Option<ImageInput>,
    pub model: Option<String>,
    pub n: Option<u32>,
    pub output_compression: Option<u8>,
    pub output_format: Option<String>,
    pub partial_images: Option<u32>,
    pub quality: Option<String>,
    pub response_format: Option<String>,
    pub size: Option<String>,
    pub stream: Option<bool>,
    pub user: Option<String>,
    pub extra: BTreeMap<String, Value>,
}

impl ImageEditParams {
    fn validate_for_edit(&self) -> Result<(), OpenAIError> {
        if self.stream == Some(true) {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "images.edit() is non-streaming; call edit_stream() instead of setting `stream=true`",
            ));
        }
        Ok(())
    }

    fn into_multipart(
        self,
        stream: bool,
    ) -> Result<crate::helpers::multipart::MultipartPayload, OpenAIError> {
        if self.images.is_empty() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "images.edit requires at least one source image",
            ));
        }

        let mut builder = MultipartBuilder::new();
        for image in &self.images {
            builder.add_file("image", image.to_multipart_file());
        }
        builder.add_text("prompt", self.prompt);
        if let Some(mask) = self.mask {
            builder.add_file("mask", mask.to_multipart_file());
        }
        add_optional_text(&mut builder, "background", self.background);
        add_optional_text(&mut builder, "input_fidelity", self.input_fidelity);
        add_optional_text(&mut builder, "model", self.model);
        add_optional_text(&mut builder, "n", self.n.map(|value| value.to_string()));
        add_optional_text(
            &mut builder,
            "output_compression",
            self.output_compression.map(|value| value.to_string()),
        );
        add_optional_text(&mut builder, "output_format", self.output_format);
        add_optional_text(
            &mut builder,
            "partial_images",
            self.partial_images.map(|value| value.to_string()),
        );
        add_optional_text(&mut builder, "quality", self.quality);
        add_optional_text(&mut builder, "response_format", self.response_format);
        add_optional_text(&mut builder, "size", self.size);
        add_optional_text(&mut builder, "stream", Some(stream.to_string()));
        add_optional_text(&mut builder, "user", self.user);
        for (key, value) in self.extra {
            add_jsonish_extra(&mut builder, key, value);
        }
        Ok(builder.build())
    }
}

/// Image variation parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImageVariationParams {
    pub image: ImageInput,
    pub model: Option<String>,
    pub n: Option<u32>,
    pub response_format: Option<String>,
    pub size: Option<String>,
    pub user: Option<String>,
    pub extra: BTreeMap<String, Value>,
}

impl ImageVariationParams {
    fn into_multipart(self) -> Result<crate::helpers::multipart::MultipartPayload, OpenAIError> {
        if !self.image.content_type.to_ascii_lowercase().contains("png") {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "images.create_variation requires a PNG image input",
            ));
        }

        let mut builder = MultipartBuilder::new();
        builder.add_file("image", self.image.to_multipart_file());
        add_optional_text(&mut builder, "model", self.model);
        add_optional_text(&mut builder, "n", self.n.map(|value| value.to_string()));
        add_optional_text(&mut builder, "response_format", self.response_format);
        add_optional_text(&mut builder, "size", self.size);
        add_optional_text(&mut builder, "user", self.user);
        for (key, value) in self.extra {
            add_jsonish_extra(&mut builder, key, value);
        }
        Ok(builder.build())
    }
}

/// Typed images response.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImagesResponse {
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub data: Vec<ImageData>,
    #[serde(default)]
    pub usage: Option<ImageUsage>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// One generated image entry.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageData {
    #[serde(default)]
    pub b64_json: Option<String>,
    #[serde(default)]
    pub revised_prompt: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Token-usage details returned by GPT image models.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub input_tokens_details: ImageInputTokenDetails,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Input token split by modality.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageInputTokenDetails {
    #[serde(default)]
    pub image_tokens: u32,
    #[serde(default)]
    pub text_tokens: u32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Streamed image-generation partial event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageGenerationPartialImageEvent {
    pub b64_json: String,
    #[serde(default)]
    pub background: String,
    pub created_at: i64,
    #[serde(default)]
    pub output_format: String,
    pub partial_image_index: usize,
    #[serde(default)]
    pub quality: String,
    #[serde(default)]
    pub size: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Streamed image-generation completed event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageGenerationCompletedEvent {
    pub b64_json: String,
    #[serde(default)]
    pub background: String,
    pub created_at: i64,
    #[serde(default)]
    pub output_format: String,
    #[serde(default)]
    pub quality: String,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub usage: ImageUsage,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed image-generation stream event.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageGenerationStreamEvent {
    PartialImage(ImageGenerationPartialImageEvent),
    Completed(ImageGenerationCompletedEvent),
}

/// Streamed image-edit partial event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageEditPartialImageEvent {
    pub b64_json: String,
    #[serde(default)]
    pub background: String,
    pub created_at: i64,
    #[serde(default)]
    pub output_format: String,
    pub partial_image_index: usize,
    #[serde(default)]
    pub quality: String,
    #[serde(default)]
    pub size: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Streamed image-edit completed event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageEditCompletedEvent {
    pub b64_json: String,
    #[serde(default)]
    pub background: String,
    pub created_at: i64,
    #[serde(default)]
    pub output_format: String,
    #[serde(default)]
    pub quality: String,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub usage: ImageUsage,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed image-edit stream event.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageEditStreamEvent {
    PartialImage(ImageEditPartialImageEvent),
    Completed(ImageEditCompletedEvent),
}

/// Eagerly parsed image-generation stream transcript.
#[derive(Clone, Debug)]
pub struct ImageGenerationStream {
    metadata: ResponseMetadata,
    events: VecDeque<ImageGenerationStreamEvent>,
    final_completed: Option<ImageGenerationCompletedEvent>,
}

impl ImageGenerationStream {
    pub fn from_sse_chunks<I, B>(metadata: ResponseMetadata, chunks: I) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        let mut parser = SseParser::default();
        let mut accumulator = ImageGenerationAccumulator::default();

        for chunk in chunks {
            for frame in parser.push(chunk.as_ref().as_bytes())? {
                accumulator.ingest_frame(frame)?;
            }
        }
        for frame in parser.finish()? {
            accumulator.ingest_frame(frame)?;
        }

        let (events, final_completed) = accumulator.finish()?;
        Ok(Self {
            metadata,
            events,
            final_completed,
        })
    }

    pub fn next_event(&mut self) -> Option<ImageGenerationStreamEvent> {
        self.events.pop_front()
    }

    pub fn final_completed(&self) -> Result<&ImageGenerationCompletedEvent, OpenAIError> {
        self.final_completed.as_ref().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                "image generation stream ended without a terminal completed event",
            )
        })
    }

    pub fn metadata(&self) -> &ResponseMetadata {
        &self.metadata
    }
}

/// Eagerly parsed image-edit stream transcript.
#[derive(Clone, Debug)]
pub struct ImageEditStream {
    metadata: ResponseMetadata,
    events: VecDeque<ImageEditStreamEvent>,
    final_completed: Option<ImageEditCompletedEvent>,
}

impl ImageEditStream {
    pub fn from_sse_chunks<I, B>(metadata: ResponseMetadata, chunks: I) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        let mut parser = SseParser::default();
        let mut accumulator = ImageEditAccumulator::default();

        for chunk in chunks {
            for frame in parser.push(chunk.as_ref().as_bytes())? {
                accumulator.ingest_frame(frame)?;
            }
        }
        for frame in parser.finish()? {
            accumulator.ingest_frame(frame)?;
        }

        let (events, final_completed) = accumulator.finish()?;
        Ok(Self {
            metadata,
            events,
            final_completed,
        })
    }

    pub fn next_event(&mut self) -> Option<ImageEditStreamEvent> {
        self.events.pop_front()
    }

    pub fn final_completed(&self) -> Result<&ImageEditCompletedEvent, OpenAIError> {
        self.final_completed.as_ref().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                "image edit stream ended without a terminal completed event",
            )
        })
    }

    pub fn metadata(&self) -> &ResponseMetadata {
        &self.metadata
    }
}

#[derive(Clone, Debug, Default)]
struct ImageGenerationAccumulator {
    events: VecDeque<ImageGenerationStreamEvent>,
    final_completed: Option<ImageGenerationCompletedEvent>,
    saw_done: bool,
}

impl ImageGenerationAccumulator {
    fn ingest_frame(&mut self, frame: SseFrame) -> Result<(), OpenAIError> {
        if frame.data == "[DONE]" {
            self.saw_done = true;
            return Ok(());
        }

        let event_name = image_stream_event_name(&frame)?;
        match event_name.as_str() {
            "image_generation.partial_image" => {
                let event: ImageGenerationPartialImageEvent = serde_json::from_str(&frame.data)
                    .map_err(|error| stream_parse_error(&event_name, error))?;
                self.events
                    .push_back(ImageGenerationStreamEvent::PartialImage(event));
            }
            "image_generation.completed" => {
                let event: ImageGenerationCompletedEvent = serde_json::from_str(&frame.data)
                    .map_err(|error| stream_parse_error(&event_name, error))?;
                self.final_completed = Some(event.clone());
                self.events
                    .push_back(ImageGenerationStreamEvent::Completed(event));
            }
            other => {
                return Err(OpenAIError::new(
                    ErrorKind::Parse,
                    format!("unsupported image generation stream event `{other}`"),
                ));
            }
        }
        Ok(())
    }

    fn finish(
        self,
    ) -> Result<
        (
            VecDeque<ImageGenerationStreamEvent>,
            Option<ImageGenerationCompletedEvent>,
        ),
        OpenAIError,
    > {
        if self.saw_done && self.final_completed.is_none() {
            return Err(OpenAIError::new(
                ErrorKind::Parse,
                "image generation stream ended without a terminal completed event",
            ));
        }
        Ok((self.events, self.final_completed))
    }
}

#[derive(Clone, Debug, Default)]
struct ImageEditAccumulator {
    events: VecDeque<ImageEditStreamEvent>,
    final_completed: Option<ImageEditCompletedEvent>,
    saw_done: bool,
}

impl ImageEditAccumulator {
    fn ingest_frame(&mut self, frame: SseFrame) -> Result<(), OpenAIError> {
        if frame.data == "[DONE]" {
            self.saw_done = true;
            return Ok(());
        }

        let event_name = image_stream_event_name(&frame)?;
        match event_name.as_str() {
            "image_edit.partial_image" => {
                let event: ImageEditPartialImageEvent = serde_json::from_str(&frame.data)
                    .map_err(|error| stream_parse_error(&event_name, error))?;
                self.events
                    .push_back(ImageEditStreamEvent::PartialImage(event));
            }
            "image_edit.completed" => {
                let event: ImageEditCompletedEvent = serde_json::from_str(&frame.data)
                    .map_err(|error| stream_parse_error(&event_name, error))?;
                self.final_completed = Some(event.clone());
                self.events
                    .push_back(ImageEditStreamEvent::Completed(event));
            }
            other => {
                return Err(OpenAIError::new(
                    ErrorKind::Parse,
                    format!("unsupported image edit stream event `{other}`"),
                ));
            }
        }
        Ok(())
    }

    fn finish(
        self,
    ) -> Result<
        (
            VecDeque<ImageEditStreamEvent>,
            Option<ImageEditCompletedEvent>,
        ),
        OpenAIError,
    > {
        if self.saw_done && self.final_completed.is_none() {
            return Err(OpenAIError::new(
                ErrorKind::Parse,
                "image edit stream ended without a terminal completed event",
            ));
        }
        Ok((self.events, self.final_completed))
    }
}

fn add_optional_text(builder: &mut MultipartBuilder, name: &str, value: Option<String>) {
    if let Some(value) = value {
        builder.add_text(name.to_string(), value);
    }
}

fn add_jsonish_extra(builder: &mut MultipartBuilder, key: String, value: Value) {
    match value {
        Value::Null => {}
        Value::String(text) => {
            builder.add_text(key, text);
        }
        other => {
            builder.add_text(key, other.to_string());
        }
    }
}

fn image_stream_event_name(frame: &SseFrame) -> Result<String, OpenAIError> {
    if let Some(event) = frame.event.as_ref() {
        return Ok(event.clone());
    }

    let payload: Value = serde_json::from_str(&frame.data)
        .map_err(|error| stream_parse_error("image_event", error))?;
    payload
        .get("type")
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                "image stream event was missing both `event:` and payload `type`",
            )
        })
}

fn stream_parse_error(error_event: &str, error: serde_json::Error) -> OpenAIError {
    OpenAIError::new(
        ErrorKind::Parse,
        format!("failed to parse streamed `{error_event}` payload: {error}"),
    )
    .with_source(error)
}

fn map_live_transport_error(error: reqwest::Error) -> OpenAIError {
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Transport
    };
    OpenAIError::new(kind, error.to_string()).with_source(error)
}
