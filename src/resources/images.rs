use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
    thread,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use serde_json::Value;
use tokio::runtime::Builder;

use crate::{
    OpenAIError,
    core::{
        metadata::ResponseMetadata,
        request::RequestOptions,
        runtime::ClientRuntime,
        transport::{execute_bytes, execute_text_stream},
    },
    error::ErrorKind,
    helpers::{
        media::{DecodedMedia, MediaDecodeMode, decode_media_response, parse_sse_frames},
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
        let request = self
            .runtime
            .prepare_json_request("POST", "/images/generations", &body)?;
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        let response = execute_bytes(&request, &options)?;
        decode_images_json_response(response)
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
        ImageGenerationStream::start_live(request, options)
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
        ImageEditStream::start_live(request, options)
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
        let response = execute_bytes(&request, &options)?;
        decode_images_json_response(response)
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageBackground {
    Transparent,
    Opaque,
    #[default]
    Auto,
}

impl ImageBackground {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Transparent => "transparent",
            Self::Opaque => "opaque",
            Self::Auto => "auto",
        }
    }
}

impl AsRef<str> for ImageBackground {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageResponseBackground {
    Transparent,
    Opaque,
}

impl ImageResponseBackground {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Transparent => "transparent",
            Self::Opaque => "opaque",
        }
    }
}

impl AsRef<str> for ImageResponseBackground {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageOutputFormat {
    #[default]
    Png,
    Jpeg,
    Webp,
}

impl ImageOutputFormat {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
        }
    }
}

impl AsRef<str> for ImageOutputFormat {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageModeration {
    Low,
    Auto,
}

impl ImageModeration {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Auto => "auto",
        }
    }
}

impl AsRef<str> for ImageModeration {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageInputFidelity {
    High,
    Low,
}

impl ImageInputFidelity {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Low => "low",
        }
    }
}

impl AsRef<str> for ImageInputFidelity {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenerateQuality {
    Standard,
    Hd,
    Low,
    Medium,
    High,
    Auto,
}

impl ImageGenerateQuality {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Hd => "hd",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Auto => "auto",
        }
    }
}

impl AsRef<str> for ImageGenerateQuality {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageEditQuality {
    Standard,
    Low,
    Medium,
    High,
    Auto,
}

impl ImageEditQuality {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Auto => "auto",
        }
    }
}

impl AsRef<str> for ImageEditQuality {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageResponseQuality {
    Low,
    Medium,
    High,
}

impl ImageResponseQuality {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl AsRef<str> for ImageResponseQuality {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageStreamQuality {
    Low,
    Medium,
    High,
    #[default]
    Auto,
}

impl ImageStreamQuality {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Auto => "auto",
        }
    }
}

impl AsRef<str> for ImageStreamQuality {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageResponseFormat {
    Url,
    B64Json,
}

impl ImageResponseFormat {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::B64Json => "b64_json",
        }
    }
}

impl AsRef<str> for ImageResponseFormat {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageStyle {
    Vivid,
    Natural,
}

impl ImageStyle {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Vivid => "vivid",
            Self::Natural => "natural",
        }
    }
}

impl AsRef<str> for ImageStyle {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageGenerateSize {
    Auto,
    Size256x256,
    Size512x512,
    Size1024x1024,
    Size1536x1024,
    Size1024x1536,
    Size1792x1024,
    Size1024x1792,
    Custom(String),
}

impl ImageGenerateSize {
    pub fn custom(value: impl Into<String>) -> Self {
        Self::Custom(value.into())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Auto => "auto",
            Self::Size256x256 => "256x256",
            Self::Size512x512 => "512x512",
            Self::Size1024x1024 => "1024x1024",
            Self::Size1536x1024 => "1536x1024",
            Self::Size1024x1536 => "1024x1536",
            Self::Size1792x1024 => "1792x1024",
            Self::Size1024x1792 => "1024x1792",
            Self::Custom(value) => value.as_str(),
        }
    }
}

impl AsRef<str> for ImageGenerateSize {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Serialize for ImageGenerateSize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ImageGenerateSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "auto" => Self::Auto,
            "256x256" => Self::Size256x256,
            "512x512" => Self::Size512x512,
            "1024x1024" => Self::Size1024x1024,
            "1536x1024" => Self::Size1536x1024,
            "1024x1536" => Self::Size1024x1536,
            "1792x1024" => Self::Size1792x1024,
            "1024x1792" => Self::Size1024x1792,
            _ if !value.is_empty() => Self::Custom(value),
            _ => return Err(D::Error::custom("image size must not be empty")),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageEditSize {
    Auto,
    Size256x256,
    Size512x512,
    Size1024x1024,
    Size1536x1024,
    Size1024x1536,
    Custom(String),
}

impl ImageEditSize {
    pub fn custom(value: impl Into<String>) -> Self {
        Self::Custom(value.into())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Auto => "auto",
            Self::Size256x256 => "256x256",
            Self::Size512x512 => "512x512",
            Self::Size1024x1024 => "1024x1024",
            Self::Size1536x1024 => "1536x1024",
            Self::Size1024x1536 => "1024x1536",
            Self::Custom(value) => value.as_str(),
        }
    }
}

impl AsRef<str> for ImageEditSize {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Serialize for ImageEditSize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ImageEditSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "auto" => Self::Auto,
            "256x256" => Self::Size256x256,
            "512x512" => Self::Size512x512,
            "1024x1024" => Self::Size1024x1024,
            "1536x1024" => Self::Size1536x1024,
            "1024x1536" => Self::Size1024x1536,
            _ if !value.is_empty() => Self::Custom(value),
            _ => return Err(D::Error::custom("image size must not be empty")),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImageVariationSize {
    #[serde(rename = "256x256")]
    Size256x256,
    #[serde(rename = "512x512")]
    Size512x512,
    #[serde(rename = "1024x1024")]
    Size1024x1024,
}

impl ImageVariationSize {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Size256x256 => "256x256",
            Self::Size512x512 => "512x512",
            Self::Size1024x1024 => "1024x1024",
        }
    }
}

impl AsRef<str> for ImageVariationSize {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImageResponseSize {
    #[serde(rename = "1024x1024")]
    Size1024x1024,
    #[serde(rename = "1024x1536")]
    Size1024x1536,
    #[serde(rename = "1536x1024")]
    Size1536x1024,
}

impl ImageResponseSize {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Size1024x1024 => "1024x1024",
            Self::Size1024x1536 => "1024x1536",
            Self::Size1536x1024 => "1536x1024",
        }
    }
}

impl AsRef<str> for ImageResponseSize {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImageStreamSize {
    #[serde(rename = "1024x1024")]
    Size1024x1024,
    #[serde(rename = "1024x1536")]
    Size1024x1536,
    #[serde(rename = "1536x1024")]
    Size1536x1024,
    #[serde(rename = "auto")]
    #[default]
    Auto,
}

impl ImageStreamSize {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Size1024x1024 => "1024x1024",
            Self::Size1024x1536 => "1024x1536",
            Self::Size1536x1024 => "1536x1024",
            Self::Auto => "auto",
        }
    }
}

impl AsRef<str> for ImageStreamSize {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Image generation parameters.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ImageGenerateParams {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ImageBackground>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moderation: Option<ImageModeration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_compression: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<ImageOutputFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_images: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageGenerateQuality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ImageResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageGenerateSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<ImageStyle>,
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
    pub image: Vec<ImageInput>,
    pub images: Vec<ImageInput>,
    pub prompt: String,
    pub background: Option<ImageBackground>,
    pub input_fidelity: Option<ImageInputFidelity>,
    pub mask: Option<ImageInput>,
    pub model: Option<String>,
    pub n: Option<u32>,
    pub output_compression: Option<u8>,
    pub output_format: Option<ImageOutputFormat>,
    pub partial_images: Option<u32>,
    pub quality: Option<ImageEditQuality>,
    pub response_format: Option<ImageResponseFormat>,
    pub size: Option<ImageEditSize>,
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
        mut self,
        stream: bool,
    ) -> Result<crate::helpers::multipart::MultipartPayload, OpenAIError> {
        self.image.append(&mut self.images);
        if self.image.is_empty() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "images.edit requires at least one source image",
            ));
        }

        let mut builder = MultipartBuilder::new();
        for image in &self.image {
            builder.add_file("image", image.to_multipart_file());
        }
        builder.add_text("prompt", self.prompt);
        if let Some(mask) = self.mask {
            builder.add_file("mask", mask.to_multipart_file());
        }
        add_optional_text(
            &mut builder,
            "background",
            optional_literal(self.background),
        );
        add_optional_text(
            &mut builder,
            "input_fidelity",
            optional_literal(self.input_fidelity),
        );
        add_optional_text(&mut builder, "model", self.model);
        add_optional_text(&mut builder, "n", self.n.map(|value| value.to_string()));
        add_optional_text(
            &mut builder,
            "output_compression",
            self.output_compression.map(|value| value.to_string()),
        );
        add_optional_text(
            &mut builder,
            "output_format",
            optional_literal(self.output_format),
        );
        add_optional_text(
            &mut builder,
            "partial_images",
            self.partial_images.map(|value| value.to_string()),
        );
        add_optional_text(&mut builder, "quality", optional_literal(self.quality));
        add_optional_text(
            &mut builder,
            "response_format",
            optional_literal(self.response_format),
        );
        add_optional_text(&mut builder, "size", optional_literal(self.size));
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
    pub response_format: Option<ImageResponseFormat>,
    pub size: Option<ImageVariationSize>,
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
        add_optional_text(
            &mut builder,
            "response_format",
            optional_literal(self.response_format),
        );
        add_optional_text(&mut builder, "size", optional_literal(self.size));
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
    pub background: Option<ImageResponseBackground>,
    #[serde(default)]
    pub data: Vec<ImageData>,
    #[serde(default)]
    pub output_format: Option<ImageOutputFormat>,
    #[serde(default)]
    pub quality: Option<ImageResponseQuality>,
    #[serde(default)]
    pub size: Option<ImageResponseSize>,
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
    pub output_tokens_details: Option<ImageOutputTokenDetails>,
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

/// Output token split by modality.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageOutputTokenDetails {
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
    pub background: ImageBackground,
    pub created_at: i64,
    #[serde(default)]
    pub output_format: ImageOutputFormat,
    pub partial_image_index: usize,
    #[serde(default)]
    pub quality: ImageStreamQuality,
    #[serde(default)]
    pub size: ImageStreamSize,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Streamed image-generation completed event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageGenerationCompletedEvent {
    pub b64_json: String,
    #[serde(default)]
    pub background: ImageBackground,
    pub created_at: i64,
    #[serde(default)]
    pub output_format: ImageOutputFormat,
    #[serde(default)]
    pub quality: ImageStreamQuality,
    #[serde(default)]
    pub size: ImageStreamSize,
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
    pub background: ImageBackground,
    pub created_at: i64,
    #[serde(default)]
    pub output_format: ImageOutputFormat,
    pub partial_image_index: usize,
    #[serde(default)]
    pub quality: ImageStreamQuality,
    #[serde(default)]
    pub size: ImageStreamSize,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Streamed image-edit completed event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ImageEditCompletedEvent {
    pub b64_json: String,
    #[serde(default)]
    pub background: ImageBackground,
    pub created_at: i64,
    #[serde(default)]
    pub output_format: ImageOutputFormat,
    #[serde(default)]
    pub quality: ImageStreamQuality,
    #[serde(default)]
    pub size: ImageStreamSize,
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
#[derive(Debug)]
pub struct ImageGenerationStream {
    metadata: ResponseMetadata,
    events: VecDeque<ImageGenerationStreamEvent>,
    final_completed: Option<ImageGenerationCompletedEvent>,
    terminal_error: Option<OpenAIError>,
    live: Option<LiveImageGenerationHandle>,
}

impl ImageGenerationStream {
    pub fn from_sse_chunks<I, B>(metadata: ResponseMetadata, chunks: I) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        let mut accumulator = ImageGenerationAccumulator::default();

        for frame in parse_sse_frames(chunks)? {
            accumulator.ingest_frame(frame)?;
        }

        let (events, final_completed) = accumulator.finish()?;
        Ok(Self {
            metadata,
            events,
            final_completed,
            terminal_error: None,
            live: None,
        })
    }

    pub fn next_event(&mut self) -> Option<ImageGenerationStreamEvent> {
        if self.events.is_empty() {
            self.fill_from_live();
        }
        self.events.pop_front()
    }

    pub fn final_completed(&mut self) -> Result<&ImageGenerationCompletedEvent, OpenAIError> {
        self.drain_live_messages();
        while self.live.is_some() {
            self.fill_from_live();
        }
        if let Some(error) = self.terminal_error.clone() {
            return Err(error);
        }
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

    fn start_live(
        request: crate::core::request::PreparedRequest,
        options: crate::core::request::ResolvedRequestOptions,
    ) -> Result<Self, OpenAIError> {
        let (startup_tx, startup_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let runtime = match Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    let error = OpenAIError::new(
                        ErrorKind::Transport,
                        format!("failed to build images streaming runtime: {error}"),
                    )
                    .with_source(error);
                    let _ = startup_tx.send(Err(error));
                    return;
                }
            };

            runtime.block_on(async move {
                match execute_text_stream(&request, &options).await {
                    Ok(response) => {
                        let metadata = response.metadata.clone();
                        let _ = startup_tx.send(Ok(metadata));
                        if let Err(error) =
                            consume_live_generation_stream(response, event_tx.clone()).await
                        {
                            let _ = event_tx.send(LiveImageGenerationMessage::Error(error));
                        }
                    }
                    Err(error) => {
                        let _ = startup_tx.send(Err(error));
                    }
                }
            });
        });

        let metadata = startup_rx.recv().map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("image generation stream worker exited before startup completed: {error}"),
            )
        })??;

        Ok(Self {
            metadata,
            events: VecDeque::new(),
            final_completed: None,
            terminal_error: None,
            live: Some(LiveImageGenerationHandle {
                receiver: event_rx,
                worker: Some(worker),
            }),
        })
    }

    fn fill_from_live(&mut self) {
        let Some(live) = self.live.as_mut() else {
            return;
        };

        let Some(message) = live.receiver.recv().ok() else {
            live.join_worker();
            self.live = None;
            return;
        };
        self.process_live_message(message);
        self.drain_live_messages();
    }

    fn drain_live_messages(&mut self) {
        while let Some(live) = self.live.as_mut() {
            match live.receiver.try_recv() {
                Ok(message) => self.process_live_message(message),
                Err(_) => break,
            }
        }
    }

    fn process_live_message(&mut self, message: LiveImageGenerationMessage) {
        match message {
            LiveImageGenerationMessage::Event(event) => {
                if let ImageGenerationStreamEvent::Completed(completed) = event.as_ref() {
                    self.final_completed = Some(completed.clone());
                }
                self.events.push_back(*event);
            }
            LiveImageGenerationMessage::Finished => {
                if let Some(live) = self.live.as_mut() {
                    live.join_worker();
                }
                self.live = None;
            }
            LiveImageGenerationMessage::Error(error) => {
                self.terminal_error = Some(error);
                if let Some(live) = self.live.as_mut() {
                    live.join_worker();
                }
                self.live = None;
            }
        }
    }
}

/// Eagerly parsed image-edit stream transcript.
#[derive(Debug)]
pub struct ImageEditStream {
    metadata: ResponseMetadata,
    events: VecDeque<ImageEditStreamEvent>,
    final_completed: Option<ImageEditCompletedEvent>,
    terminal_error: Option<OpenAIError>,
    live: Option<LiveImageEditHandle>,
}

impl ImageEditStream {
    pub fn from_sse_chunks<I, B>(metadata: ResponseMetadata, chunks: I) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        let mut accumulator = ImageEditAccumulator::default();

        for frame in parse_sse_frames(chunks)? {
            accumulator.ingest_frame(frame)?;
        }

        let (events, final_completed) = accumulator.finish()?;
        Ok(Self {
            metadata,
            events,
            final_completed,
            terminal_error: None,
            live: None,
        })
    }

    pub fn next_event(&mut self) -> Option<ImageEditStreamEvent> {
        if self.events.is_empty() {
            self.fill_from_live();
        }
        self.events.pop_front()
    }

    pub fn final_completed(&mut self) -> Result<&ImageEditCompletedEvent, OpenAIError> {
        self.drain_live_messages();
        while self.live.is_some() {
            self.fill_from_live();
        }
        if let Some(error) = self.terminal_error.clone() {
            return Err(error);
        }
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

    fn start_live(
        request: crate::core::request::PreparedRequest,
        options: crate::core::request::ResolvedRequestOptions,
    ) -> Result<Self, OpenAIError> {
        let (startup_tx, startup_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let runtime = match Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    let error = OpenAIError::new(
                        ErrorKind::Transport,
                        format!("failed to build images edit streaming runtime: {error}"),
                    )
                    .with_source(error);
                    let _ = startup_tx.send(Err(error));
                    return;
                }
            };

            runtime.block_on(async move {
                match execute_text_stream(&request, &options).await {
                    Ok(response) => {
                        let metadata = response.metadata.clone();
                        let _ = startup_tx.send(Ok(metadata));
                        if let Err(error) =
                            consume_live_edit_stream(response, event_tx.clone()).await
                        {
                            let _ = event_tx.send(LiveImageEditMessage::Error(error));
                        }
                    }
                    Err(error) => {
                        let _ = startup_tx.send(Err(error));
                    }
                }
            });
        });

        let metadata = startup_rx.recv().map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("image edit stream worker exited before startup completed: {error}"),
            )
        })??;

        Ok(Self {
            metadata,
            events: VecDeque::new(),
            final_completed: None,
            terminal_error: None,
            live: Some(LiveImageEditHandle {
                receiver: event_rx,
                worker: Some(worker),
            }),
        })
    }

    fn fill_from_live(&mut self) {
        let Some(live) = self.live.as_mut() else {
            return;
        };

        let Some(message) = live.receiver.recv().ok() else {
            live.join_worker();
            self.live = None;
            return;
        };
        self.process_live_message(message);
        self.drain_live_messages();
    }

    fn drain_live_messages(&mut self) {
        while let Some(live) = self.live.as_mut() {
            match live.receiver.try_recv() {
                Ok(message) => self.process_live_message(message),
                Err(_) => break,
            }
        }
    }

    fn process_live_message(&mut self, message: LiveImageEditMessage) {
        match message {
            LiveImageEditMessage::Event(event) => {
                if let ImageEditStreamEvent::Completed(completed) = event.as_ref() {
                    self.final_completed = Some(completed.clone());
                }
                self.events.push_back(*event);
            }
            LiveImageEditMessage::Finished => {
                if let Some(live) = self.live.as_mut() {
                    live.join_worker();
                }
                self.live = None;
            }
            LiveImageEditMessage::Error(error) => {
                self.terminal_error = Some(error);
                if let Some(live) = self.live.as_mut() {
                    live.join_worker();
                }
                self.live = None;
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ImageGenerationAccumulator {
    events: VecDeque<ImageGenerationStreamEvent>,
    final_completed: Option<ImageGenerationCompletedEvent>,
}

impl ImageGenerationAccumulator {
    fn ingest_frame(&mut self, frame: SseFrame) -> Result<(), OpenAIError> {
        if frame.data == "[DONE]" {
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
        if self.final_completed.is_none() {
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
}

impl ImageEditAccumulator {
    fn ingest_frame(&mut self, frame: SseFrame) -> Result<(), OpenAIError> {
        if frame.data == "[DONE]" {
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
        if self.final_completed.is_none() {
            return Err(OpenAIError::new(
                ErrorKind::Parse,
                "image edit stream ended without a terminal completed event",
            ));
        }
        Ok((self.events, self.final_completed))
    }
}

#[derive(Debug)]
enum LiveImageGenerationMessage {
    Event(Box<ImageGenerationStreamEvent>),
    Finished,
    Error(OpenAIError),
}

#[derive(Debug)]
struct LiveImageGenerationHandle {
    receiver: Receiver<LiveImageGenerationMessage>,
    worker: Option<thread::JoinHandle<()>>,
}

impl LiveImageGenerationHandle {
    fn join_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug)]
enum LiveImageEditMessage {
    Event(Box<ImageEditStreamEvent>),
    Finished,
    Error(OpenAIError),
}

#[derive(Debug)]
struct LiveImageEditHandle {
    receiver: Receiver<LiveImageEditMessage>,
    worker: Option<thread::JoinHandle<()>>,
}

impl LiveImageEditHandle {
    fn join_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn add_optional_text(builder: &mut MultipartBuilder, name: &str, value: Option<String>) {
    if let Some(value) = value {
        builder.add_text(name.to_string(), value);
    }
}

fn optional_literal<T>(value: Option<T>) -> Option<String>
where
    T: AsRef<str>,
{
    value.map(|value| value.as_ref().to_string())
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

async fn consume_live_generation_stream(
    response: crate::core::transport::StreamingTextResponse,
    event_tx: mpsc::Sender<LiveImageGenerationMessage>,
) -> Result<(), OpenAIError> {
    let mut response = response.response;
    let mut parser = SseParser::default();
    let mut accumulator = ImageGenerationAccumulator::default();

    while let Some(chunk) = response.chunk().await.map_err(map_live_transport_error)? {
        for frame in parser.push(chunk.as_ref())? {
            accumulator.ingest_frame(frame)?;
            drain_generation_events(&mut accumulator, &event_tx);
        }
    }

    for frame in parser.finish()? {
        accumulator.ingest_frame(frame)?;
        drain_generation_events(&mut accumulator, &event_tx);
    }

    let _ = accumulator.finish()?;
    let _ = event_tx.send(LiveImageGenerationMessage::Finished);
    Ok(())
}

fn drain_generation_events(
    accumulator: &mut ImageGenerationAccumulator,
    event_tx: &mpsc::Sender<LiveImageGenerationMessage>,
) {
    while let Some(event) = accumulator.events.pop_front() {
        if event_tx
            .send(LiveImageGenerationMessage::Event(Box::new(event)))
            .is_err()
        {
            break;
        }
    }
}

async fn consume_live_edit_stream(
    response: crate::core::transport::StreamingTextResponse,
    event_tx: mpsc::Sender<LiveImageEditMessage>,
) -> Result<(), OpenAIError> {
    let mut response = response.response;
    let mut parser = SseParser::default();
    let mut accumulator = ImageEditAccumulator::default();

    while let Some(chunk) = response.chunk().await.map_err(map_live_transport_error)? {
        for frame in parser.push(chunk.as_ref())? {
            accumulator.ingest_frame(frame)?;
            drain_edit_events(&mut accumulator, &event_tx);
        }
    }

    for frame in parser.finish()? {
        accumulator.ingest_frame(frame)?;
        drain_edit_events(&mut accumulator, &event_tx);
    }

    let _ = accumulator.finish()?;
    let _ = event_tx.send(LiveImageEditMessage::Finished);
    Ok(())
}

fn drain_edit_events(
    accumulator: &mut ImageEditAccumulator,
    event_tx: &mpsc::Sender<LiveImageEditMessage>,
) {
    while let Some(event) = accumulator.events.pop_front() {
        if event_tx
            .send(LiveImageEditMessage::Event(Box::new(event)))
            .is_err()
        {
            break;
        }
    }
}

fn decode_images_json_response(
    response: crate::ApiResponse<Vec<u8>>,
) -> Result<crate::ApiResponse<ImagesResponse>, OpenAIError> {
    let decoded =
        decode_media_response::<serde_json::Value>(response, MediaDecodeMode::Json, "images JSON")?;
    let (metadata, body) = decoded.into_parts();
    let DecodedMedia::Json(value) = body else {
        return Err(OpenAIError::new(
            ErrorKind::Parse,
            "images endpoint expected a JSON response body",
        ));
    };
    let output = serde_json::from_value(value).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse OpenAI success response: {error}"),
        )
        .with_source(error)
    })?;
    Ok(crate::ApiResponse { output, metadata })
}

fn map_live_transport_error(error: reqwest::Error) -> OpenAIError {
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Transport
    };
    OpenAIError::new(kind, error.to_string()).with_source(error)
}
