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
        transport::{execute_bytes, execute_text_stream},
    },
    error::ErrorKind,
    helpers::{
        multipart::{MultipartBuilder, MultipartFile, MultipartPayload},
        sse::{SseFrame, SseParser},
    },
};

/// Audio API family.
#[derive(Clone, Debug)]
pub struct Audio {
    /// Audio transcription surface.
    pub transcriptions: Transcriptions,
    /// Audio translation surface.
    pub translations: Translations,
    /// Audio speech surface.
    pub speech: Speech,
}

impl Audio {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self {
            transcriptions: Transcriptions::new(runtime.clone()),
            translations: Translations::new(runtime.clone()),
            speech: Speech::new(runtime),
        }
    }
}

/// Uploadable audio input.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AudioInput {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl AudioInput {
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

/// Output format shared by transcription and translation APIs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioResponseFormat {
    Json,
    Text,
    Srt,
    VerboseJson,
    Vtt,
    DiarizedJson,
}

impl AudioResponseFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Text => "text",
            Self::Srt => "srt",
            Self::VerboseJson => "verbose_json",
            Self::Vtt => "vtt",
            Self::DiarizedJson => "diarized_json",
        }
    }
}

/// Additional transcription payload details to request.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionInclude {
    Logprobs,
}

impl TranscriptionInclude {
    fn as_str(self) -> &'static str {
        match self {
            Self::Logprobs => "logprobs",
        }
    }
}

/// Timestamp detail controls for verbose transcription responses.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionTimestampGranularity {
    Word,
    Segment,
}

impl TranscriptionTimestampGranularity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Word => "word",
            Self::Segment => "segment",
        }
    }
}

/// Optional server chunking strategy for transcription.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TranscriptionChunkingStrategy {
    #[default]
    Auto,
    ServerVad(TranscriptionVadConfig),
}

/// Server VAD tuning knobs for transcription chunking.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptionVadConfig {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<String>,
}

impl TranscriptionVadConfig {
    pub fn server_vad() -> Self {
        Self {
            kind: String::from("server_vad"),
            ..Default::default()
        }
    }
}

/// Audio transcription parameters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TranscriptionParams {
    pub file: AudioInput,
    pub model: String,
    pub chunking_strategy: Option<TranscriptionChunkingStrategy>,
    pub include: Vec<TranscriptionInclude>,
    pub known_speaker_names: Vec<String>,
    pub known_speaker_references: Vec<String>,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub response_format: Option<AudioResponseFormat>,
    pub stream: Option<bool>,
    pub temperature: Option<f32>,
    pub timestamp_granularities: Vec<TranscriptionTimestampGranularity>,
    pub extra: BTreeMap<String, Value>,
}

impl TranscriptionParams {
    fn response_format(&self) -> AudioResponseFormat {
        self.response_format.unwrap_or(AudioResponseFormat::Json)
    }

    fn validate_non_stream(&self) -> Result<(), OpenAIError> {
        if self.stream == Some(true) {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "audio.transcriptions.create() is non-streaming; call stream() instead of setting `stream=true`",
            ));
        }
        Ok(())
    }

    fn into_multipart(self, stream: bool) -> MultipartPayload {
        let response_format = self.response_format();
        let mut builder = MultipartBuilder::new();
        builder.add_file("file", self.file.to_multipart_file());
        builder.add_text("model", self.model);
        if let Some(chunking_strategy) = self.chunking_strategy {
            match chunking_strategy {
                TranscriptionChunkingStrategy::Auto => {
                    builder.add_text("chunking_strategy", "auto");
                }
                TranscriptionChunkingStrategy::ServerVad(config) => {
                    builder.add_text(
                        "chunking_strategy",
                        serde_json::to_string(&config)
                            .unwrap_or_else(|_| String::from("{\"type\":\"server_vad\"}")),
                    );
                }
            }
        }
        for include in self.include {
            builder.add_text("include", include.as_str());
        }
        for name in self.known_speaker_names {
            builder.add_text("known_speaker_names", name);
        }
        for reference in self.known_speaker_references {
            builder.add_text("known_speaker_references", reference);
        }
        add_optional_text(&mut builder, "language", self.language);
        add_optional_text(&mut builder, "prompt", self.prompt);
        add_optional_text(
            &mut builder,
            "response_format",
            Some(response_format.as_str().to_string()),
        );
        if stream {
            add_optional_text(&mut builder, "stream", Some(String::from("true")));
        }
        add_optional_text(
            &mut builder,
            "temperature",
            self.temperature.map(format_float),
        );
        for granularity in self.timestamp_granularities {
            builder.add_text("timestamp_granularities", granularity.as_str());
        }
        for (key, value) in self.extra {
            add_jsonish_extra(&mut builder, key, value);
        }
        builder.build()
    }
}

/// Audio translation parameters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TranslationParams {
    pub file: AudioInput,
    pub model: String,
    pub prompt: Option<String>,
    pub response_format: Option<AudioResponseFormat>,
    pub temperature: Option<f32>,
    pub extra: BTreeMap<String, Value>,
}

impl TranslationParams {
    fn response_format(&self) -> AudioResponseFormat {
        self.response_format.unwrap_or(AudioResponseFormat::Json)
    }

    fn into_multipart(self) -> MultipartPayload {
        let response_format = self.response_format();
        let mut builder = MultipartBuilder::new();
        builder.add_file("file", self.file.to_multipart_file());
        builder.add_text("model", self.model);
        add_optional_text(&mut builder, "prompt", self.prompt);
        add_optional_text(
            &mut builder,
            "response_format",
            Some(response_format.as_str().to_string()),
        );
        add_optional_text(
            &mut builder,
            "temperature",
            self.temperature.map(format_float),
        );
        for (key, value) in self.extra {
            add_jsonish_extra(&mut builder, key, value);
        }
        builder.build()
    }
}

/// TTS response format.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SpeechResponseFormat {
    Mp3,
    Opus,
    Aac,
    Flac,
    Wav,
    Pcm,
}

/// Speech streaming mode.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SpeechStreamFormat {
    Sse,
    Audio,
}

/// Named or custom speech voice selection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SpeechVoice {
    Named(String),
    Custom { id: String },
}

impl Default for SpeechVoice {
    fn default() -> Self {
        Self::Named(String::new())
    }
}

/// Audio speech parameters.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SpeechParams {
    pub input: String,
    pub model: String,
    pub voice: SpeechVoice,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<SpeechResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_format: Option<SpeechStreamFormat>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Audio transcription surface.
#[derive(Clone, Debug)]
pub struct Transcriptions {
    runtime: Arc<ClientRuntime>,
}

impl Transcriptions {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a non-streamed transcription and dispatches the typed response by format.
    pub fn create(
        &self,
        params: TranscriptionParams,
    ) -> Result<crate::ApiResponse<TranscriptionResponse>, OpenAIError> {
        params.validate_non_stream()?;
        let response_format = params.response_format();
        let response = self.execute_multipart(params.into_multipart(false), false)?;
        parse_transcription_response(response, response_format)
    }

    /// Streams transcription SSE events and preserves terminal text plus segment metadata.
    pub fn stream(&self, params: TranscriptionParams) -> Result<TranscriptionStream, OpenAIError> {
        let multipart = params.into_multipart(true);
        let mut request = self.runtime.prepare_request_with_body(
            "POST",
            "/audio/transcriptions",
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
                    format!("failed to build transcription streaming runtime: {error}"),
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

        TranscriptionStream::from_sse_chunks(metadata, chunks)
    }

    fn execute_multipart(
        &self,
        multipart: MultipartPayload,
        stream: bool,
    ) -> Result<crate::ApiResponse<Vec<u8>>, OpenAIError> {
        let content_type = multipart.content_type();
        let mut request = self.runtime.prepare_request_with_body(
            "POST",
            "/audio/transcriptions",
            Some(multipart.into_body()),
        )?;
        request
            .headers
            .insert(String::from("content-type"), content_type);
        request.headers.insert(
            String::from("accept"),
            if stream {
                String::from("text/event-stream")
            } else {
                String::from("*/*")
            },
        );
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        execute_bytes(&request, &options)
    }
}

/// Audio translation surface.
#[derive(Clone, Debug)]
pub struct Translations {
    runtime: Arc<ClientRuntime>,
}

impl Translations {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates an audio translation and dispatches the typed response by format.
    pub fn create(
        &self,
        params: TranslationParams,
    ) -> Result<crate::ApiResponse<TranslationResponse>, OpenAIError> {
        let response_format = params.response_format();
        let multipart = params.into_multipart();
        let content_type = multipart.content_type();
        let mut request = self.runtime.prepare_request_with_body(
            "POST",
            "/audio/translations",
            Some(multipart.into_body()),
        )?;
        request
            .headers
            .insert(String::from("content-type"), content_type);
        request
            .headers
            .insert(String::from("accept"), String::from("*/*"));
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        let response = execute_bytes(&request, &options)?;
        parse_translation_response(response, response_format)
    }
}

/// Audio speech surface.
#[derive(Clone, Debug)]
pub struct Speech {
    runtime: Arc<ClientRuntime>,
}

impl Speech {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a speech generation request and returns the raw binary/event bytes.
    pub fn create(&self, params: SpeechParams) -> Result<crate::ApiResponse<Vec<u8>>, OpenAIError> {
        let mut request = self
            .runtime
            .prepare_json_request("POST", "/audio/speech", &params)?;
        request.headers.insert(
            String::from("accept"),
            String::from("application/octet-stream"),
        );
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;
        execute_bytes(&request, &options)
    }
}

/// Top-level non-stream transcription response.
#[derive(Clone, Debug, PartialEq)]
pub enum TranscriptionResponse {
    Json(Transcription),
    VerboseJson(VerboseTranscription),
    DiarizedJson(DiarizedTranscription),
    Text(String),
    Srt(String),
    Vtt(String),
}

/// Top-level translation response.
#[derive(Clone, Debug, PartialEq)]
pub enum TranslationResponse {
    Json(Translation),
    VerboseJson(TranslationVerbose),
    Text(String),
    Srt(String),
    Vtt(String),
}

/// Non-stream transcription payload.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Transcription {
    pub text: String,
    #[serde(default)]
    pub logprobs: Vec<TranscriptionLogprob>,
    #[serde(default)]
    pub usage: Option<AudioUsage>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Verbose transcription payload.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct VerboseTranscription {
    #[serde(default)]
    pub duration: f32,
    #[serde(default)]
    pub language: String,
    pub text: String,
    #[serde(default)]
    pub segments: Option<Vec<TranscriptionSegment>>,
    #[serde(default)]
    pub usage: Option<AudioUsage>,
    #[serde(default)]
    pub words: Option<Vec<TranscriptionWord>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Diarized transcription payload.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct DiarizedTranscription {
    #[serde(default)]
    pub duration: f32,
    #[serde(default)]
    pub segments: Vec<TranscriptionTextSegmentEvent>,
    #[serde(default)]
    pub task: String,
    pub text: String,
    #[serde(default)]
    pub usage: Option<AudioUsage>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// JSON translation payload.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Translation {
    pub text: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Verbose translation payload.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TranslationVerbose {
    #[serde(default)]
    pub duration: f32,
    #[serde(default)]
    pub language: String,
    pub text: String,
    #[serde(default)]
    pub segments: Option<Vec<TranscriptionSegment>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Shared token/duration usage envelope.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AudioUsage {
    #[serde(default)]
    pub input_tokens: Option<u32>,
    #[serde(default)]
    pub output_tokens: Option<u32>,
    #[serde(default)]
    pub total_tokens: Option<u32>,
    #[serde(default)]
    pub seconds: Option<f32>,
    #[serde(default, rename = "input_token_details")]
    pub input_token_details: Option<AudioInputTokenDetails>,
    #[serde(default, rename = "type")]
    pub usage_type: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl AudioUsage {
    pub fn total_tokens(&self) -> Option<u32> {
        self.total_tokens
    }

    pub fn seconds(&self) -> Option<f32> {
        self.seconds
    }
}

/// Token billing split by modality.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AudioInputTokenDetails {
    #[serde(default)]
    pub audio_tokens: Option<u32>,
    #[serde(default)]
    pub text_tokens: Option<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Token-level logprob detail.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TranscriptionLogprob {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub bytes: Option<Vec<u8>>,
    #[serde(default)]
    pub logprob: Option<f32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Verbose/translation segment detail.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TranscriptionSegment {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub avg_logprob: f32,
    #[serde(default)]
    pub compression_ratio: f32,
    #[serde(default)]
    pub end: f32,
    #[serde(default)]
    pub no_speech_prob: f32,
    #[serde(default)]
    pub seek: i64,
    #[serde(default)]
    pub start: f32,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub tokens: Vec<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Verbose transcription word timestamp.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TranscriptionWord {
    #[serde(default)]
    pub end: f32,
    #[serde(default)]
    pub start: f32,
    #[serde(default)]
    pub word: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Streamed text delta event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TranscriptionTextDeltaEvent {
    pub delta: String,
    #[serde(default)]
    pub segment_id: Option<String>,
    #[serde(default)]
    pub logprobs: Vec<TranscriptionLogprob>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Streamed text done event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TranscriptionTextDoneEvent {
    pub text: String,
    #[serde(default)]
    pub logprobs: Vec<TranscriptionLogprob>,
    #[serde(default)]
    pub usage: Option<AudioUsage>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Streamed diarized segment event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TranscriptionTextSegmentEvent {
    pub id: String,
    #[serde(default)]
    pub end: f32,
    #[serde(default)]
    pub speaker: String,
    #[serde(default)]
    pub start: f32,
    #[serde(default)]
    pub text: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed transcription stream events.
#[derive(Clone, Debug, PartialEq)]
pub enum TranscriptionStreamEvent {
    TextDelta(TranscriptionTextDeltaEvent),
    TextSegment(TranscriptionTextSegmentEvent),
    TextDone(TranscriptionTextDoneEvent),
}

/// Eagerly parsed transcription stream transcript.
#[derive(Clone, Debug)]
pub struct TranscriptionStream {
    metadata: ResponseMetadata,
    events: VecDeque<TranscriptionStreamEvent>,
    final_text: Option<String>,
    final_usage: Option<AudioUsage>,
    segments: Vec<TranscriptionTextSegmentEvent>,
}

impl TranscriptionStream {
    pub fn from_sse_chunks<I, B>(metadata: ResponseMetadata, chunks: I) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        let mut parser = SseParser::default();
        let mut accumulator = TranscriptionAccumulator::default();

        for chunk in chunks {
            for frame in parser.push(chunk.as_ref().as_bytes())? {
                accumulator.ingest_frame(frame)?;
            }
        }
        for frame in parser.finish()? {
            accumulator.ingest_frame(frame)?;
        }

        accumulator.finish(metadata)
    }

    pub fn next_event(&mut self) -> Option<TranscriptionStreamEvent> {
        self.events.pop_front()
    }

    pub fn final_text(&self) -> Result<&str, OpenAIError> {
        self.final_text.as_deref().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                "transcription stream ended without a terminal transcript.text.done event",
            )
        })
    }

    pub fn final_usage(&self) -> Result<&AudioUsage, OpenAIError> {
        self.final_usage.as_ref().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                "transcription stream ended without terminal usage metadata",
            )
        })
    }

    pub fn segments(&self) -> &[TranscriptionTextSegmentEvent] {
        &self.segments
    }

    pub fn metadata(&self) -> &ResponseMetadata {
        &self.metadata
    }
}

#[derive(Clone, Debug, Default)]
struct TranscriptionAccumulator {
    events: VecDeque<TranscriptionStreamEvent>,
    final_text: Option<String>,
    final_usage: Option<AudioUsage>,
    segments: Vec<TranscriptionTextSegmentEvent>,
    saw_done: bool,
}

impl TranscriptionAccumulator {
    fn ingest_frame(&mut self, frame: SseFrame) -> Result<(), OpenAIError> {
        if frame.data == "[DONE]" {
            self.saw_done = true;
            return Ok(());
        }

        let event_name = transcription_stream_event_name(&frame)?;
        match event_name.as_str() {
            "transcript.text.delta" => {
                let event: TranscriptionTextDeltaEvent = serde_json::from_str(&frame.data)
                    .map_err(|error| stream_parse_error(&event_name, error))?;
                self.events
                    .push_back(TranscriptionStreamEvent::TextDelta(event));
            }
            "transcript.text.segment" => {
                let event: TranscriptionTextSegmentEvent = serde_json::from_str(&frame.data)
                    .map_err(|error| stream_parse_error(&event_name, error))?;
                self.segments.push(event.clone());
                self.events
                    .push_back(TranscriptionStreamEvent::TextSegment(event));
            }
            "transcript.text.done" => {
                let event: TranscriptionTextDoneEvent = serde_json::from_str(&frame.data)
                    .map_err(|error| stream_parse_error(&event_name, error))?;
                self.final_text = Some(event.text.clone());
                self.final_usage = event.usage.clone();
                self.events
                    .push_back(TranscriptionStreamEvent::TextDone(event));
            }
            other => {
                return Err(OpenAIError::new(
                    ErrorKind::Parse,
                    format!("unsupported transcription stream event `{other}`"),
                ));
            }
        }
        Ok(())
    }

    fn finish(self, metadata: ResponseMetadata) -> Result<TranscriptionStream, OpenAIError> {
        if self.saw_done && self.final_text.is_none() {
            return Err(OpenAIError::new(
                ErrorKind::Parse,
                "transcription stream ended without a terminal transcript.text.done event",
            ));
        }

        Ok(TranscriptionStream {
            metadata,
            events: self.events,
            final_text: self.final_text,
            final_usage: self.final_usage,
            segments: self.segments,
        })
    }
}

fn parse_transcription_response(
    response: crate::ApiResponse<Vec<u8>>,
    format: AudioResponseFormat,
) -> Result<crate::ApiResponse<TranscriptionResponse>, OpenAIError> {
    let (metadata, body) = response.into_parts();
    let output = match format {
        AudioResponseFormat::Json => {
            TranscriptionResponse::Json(parse_json_body::<Transcription>(&body, &metadata)?)
        }
        AudioResponseFormat::VerboseJson => {
            TranscriptionResponse::VerboseJson(parse_json_body::<VerboseTranscription>(
                &body, &metadata,
            )?)
        }
        AudioResponseFormat::DiarizedJson => TranscriptionResponse::DiarizedJson(
            parse_json_body::<DiarizedTranscription>(&body, &metadata)?,
        ),
        AudioResponseFormat::Text => {
            TranscriptionResponse::Text(parse_text_body(&body, &metadata, "transcription text")?)
        }
        AudioResponseFormat::Srt => {
            TranscriptionResponse::Srt(parse_text_body(&body, &metadata, "transcription srt")?)
        }
        AudioResponseFormat::Vtt => {
            TranscriptionResponse::Vtt(parse_text_body(&body, &metadata, "transcription vtt")?)
        }
    };
    Ok(crate::ApiResponse { output, metadata })
}

fn parse_translation_response(
    response: crate::ApiResponse<Vec<u8>>,
    format: AudioResponseFormat,
) -> Result<crate::ApiResponse<TranslationResponse>, OpenAIError> {
    let (metadata, body) = response.into_parts();
    let output =
        match format {
            AudioResponseFormat::Json => {
                TranslationResponse::Json(parse_json_body::<Translation>(&body, &metadata)?)
            }
            AudioResponseFormat::VerboseJson => TranslationResponse::VerboseJson(
                parse_json_body::<TranslationVerbose>(&body, &metadata)?,
            ),
            AudioResponseFormat::Text => {
                TranslationResponse::Text(parse_text_body(&body, &metadata, "translation text")?)
            }
            AudioResponseFormat::Srt => {
                TranslationResponse::Srt(parse_text_body(&body, &metadata, "translation srt")?)
            }
            AudioResponseFormat::Vtt => {
                TranslationResponse::Vtt(parse_text_body(&body, &metadata, "translation vtt")?)
            }
            AudioResponseFormat::DiarizedJson => {
                return Err(OpenAIError::new(
                    ErrorKind::Validation,
                    "audio.translations does not support `diarized_json` response format",
                ));
            }
        };
    Ok(crate::ApiResponse { output, metadata })
}

fn parse_json_body<T>(body: &[u8], metadata: &ResponseMetadata) -> Result<T, OpenAIError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_slice(body).map_err(|error| {
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
    })
}

fn parse_text_body(
    body: &[u8],
    metadata: &ResponseMetadata,
    label: &str,
) -> Result<String, OpenAIError> {
    String::from_utf8(body.to_vec()).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to decode {label} response as UTF-8: {error}"),
        )
        .with_response_metadata(
            metadata.status_code,
            metadata.headers.clone(),
            metadata.request_id.clone(),
        )
        .with_source(error)
    })
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

fn format_float(value: f32) -> String {
    let mut text = value.to_string();
    if text.ends_with(".0") {
        text.truncate(text.len() - 2);
    }
    text
}

fn transcription_stream_event_name(frame: &SseFrame) -> Result<String, OpenAIError> {
    if let Some(event) = frame.event.as_ref() {
        return Ok(event.clone());
    }

    let payload: Value = serde_json::from_str(&frame.data)
        .map_err(|error| stream_parse_error("transcription_event", error))?;
    payload
        .get("type")
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                "transcription stream event was missing both `event:` and payload `type`",
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
