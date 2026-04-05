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
        metadata::ResponseMetadata, request::RequestOptions, runtime::ClientRuntime,
        transport::execute_text_stream,
    },
    error::ErrorKind,
    helpers::sse::{SseFrame, SseParser},
};

/// Legacy `/v1/completions` compatibility surface.
///
/// This namespace is intentionally secondary to `client.responses()` and exists for
/// callers that still need text-completion semantics.
#[derive(Clone, Debug)]
pub struct Completions {
    runtime: Arc<ClientRuntime>,
}

impl Completions {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Creates a non-streamed legacy text completion.
    pub fn create(
        &self,
        params: CompletionCreateParams,
    ) -> Result<crate::core::response::ApiResponse<Completion>, OpenAIError> {
        params.validate_for_create()?;
        let body = params.into_request_body(false);
        self.runtime.execute_json_with_body(
            "POST",
            "/completions",
            &body,
            RequestOptions::default(),
        )
    }

    /// Opens a streamed legacy text completion.
    pub fn stream(&self, params: CompletionCreateParams) -> Result<CompletionStream, OpenAIError> {
        params.validate_for_stream()?;
        let body = params.into_request_body(true);
        let request = self
            .runtime
            .prepare_json_request("POST", "/completions", &body)?;
        let options = self
            .runtime
            .resolve_request_options(&RequestOptions::default())?;

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Transport,
                    format!("failed to build legacy completions streaming runtime: {error}"),
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

        CompletionStream::from_sse_chunks(metadata, chunks)
    }
}

/// Request body for legacy completions.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CompletionCreateParams {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_of: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echo: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl CompletionCreateParams {
    fn validate_for_create(&self) -> Result<(), OpenAIError> {
        if self.stream == Some(true) {
            if self.best_of.is_some() {
                return Err(OpenAIError::new(
                    ErrorKind::Validation,
                    "legacy completions does not allow `best_of` with `stream=true`; use non-streamed create or remove `best_of`",
                ));
            }
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "legacy completions create() is non-streaming; call stream() instead of setting `stream=true`",
            ));
        }
        Ok(())
    }

    fn validate_for_stream(&self) -> Result<(), OpenAIError> {
        if self.best_of.is_some() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "legacy completions streaming does not allow `best_of` with `stream=true`",
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

/// Typed legacy text-completion object.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Completion {
    pub id: String,
    #[serde(default)]
    pub object: String,
    pub created: i64,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub choices: Vec<CompletionChoice>,
    #[serde(default)]
    pub usage: Option<Value>,
    #[serde(default)]
    pub system_fingerprint: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Choice inside a legacy text completion.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CompletionChoice {
    #[serde(default)]
    pub finish_reason: Option<String>,
    pub index: usize,
    #[serde(default)]
    pub logprobs: Option<Value>,
    #[serde(default)]
    pub text: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Stream wrapper for legacy text completions.
#[derive(Clone, Debug)]
pub struct CompletionStream {
    metadata: ResponseMetadata,
    completions: VecDeque<Completion>,
    final_completion: Completion,
}

impl CompletionStream {
    pub fn from_sse_chunks<I, B>(metadata: ResponseMetadata, chunks: I) -> Result<Self, OpenAIError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<str>,
    {
        let mut parser = SseParser::default();
        let mut surfaced = VecDeque::new();
        let mut accumulator = CompletionAccumulator::default();

        for chunk in chunks {
            for frame in parser.push(chunk.as_ref().as_bytes())? {
                if let Some(parsed) = accumulator.ingest_frame(frame)? {
                    surfaced.push_back(parsed);
                }
            }
        }
        for frame in parser.finish()? {
            if let Some(parsed) = accumulator.ingest_frame(frame)? {
                surfaced.push_back(parsed);
            }
        }

        let final_completion = accumulator.finish()?;
        Ok(Self {
            metadata,
            completions: surfaced,
            final_completion,
        })
    }

    pub fn next_completion(&mut self) -> Option<Completion> {
        self.completions.pop_front()
    }

    pub async fn next_completion_async(&mut self) -> Option<Completion> {
        self.next_completion()
    }

    pub fn final_completion(&self) -> &Completion {
        &self.final_completion
    }

    pub fn metadata(&self) -> &ResponseMetadata {
        &self.metadata
    }
}

#[derive(Clone, Debug, Default)]
struct CompletionAccumulator {
    id: Option<String>,
    object: Option<String>,
    created: Option<i64>,
    model: Option<String>,
    system_fingerprint: Option<String>,
    choices: Vec<AccumulatedCompletionChoice>,
    usage: Option<Value>,
    seen_done: bool,
    seen_terminal_chunk: bool,
    parsed_completion: bool,
}

impl CompletionAccumulator {
    fn ingest_frame(&mut self, frame: SseFrame) -> Result<Option<Completion>, OpenAIError> {
        if frame.data.trim() == "[DONE]" {
            self.seen_done = true;
            return Ok(None);
        }

        let completion = serde_json::from_str::<Completion>(&frame.data).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to parse streamed legacy completion: {error}"),
            )
            .with_source(error)
        })?;
        self.apply_completion(&completion)?;
        Ok(Some(completion))
    }

    fn apply_completion(&mut self, completion: &Completion) -> Result<(), OpenAIError> {
        self.parsed_completion = true;
        if self.id.is_none() {
            self.id = Some(completion.id.clone());
        }
        if self.object.is_none() {
            self.object = Some(completion.object.clone());
        }
        if self.created.is_none() {
            self.created = Some(completion.created);
        }
        if self.model.is_none() {
            self.model = completion.model.clone();
        }
        if self.system_fingerprint.is_none() {
            self.system_fingerprint = completion.system_fingerprint.clone();
        }
        if completion.usage.is_some() {
            self.usage = completion.usage.clone();
        }

        for choice in &completion.choices {
            while self.choices.len() <= choice.index {
                self.choices.push(AccumulatedCompletionChoice::default());
            }
            let accumulated = self.choices.get_mut(choice.index).ok_or_else(|| {
                OpenAIError::new(
                    ErrorKind::Validation,
                    format!(
                        "missing accumulated legacy completion choice {}",
                        choice.index
                    ),
                )
            })?;
            accumulated.text.push_str(&choice.text);
            if choice.logprobs.is_some() {
                accumulated.logprobs = choice.logprobs.clone();
            }
            if let Some(finish_reason) = &choice.finish_reason {
                accumulated.finish_reason = Some(finish_reason.clone());
                self.seen_terminal_chunk = true;
            }
        }

        Ok(())
    }

    fn finish(self) -> Result<Completion, OpenAIError> {
        if !self.seen_done && !self.seen_terminal_chunk {
            return Err(OpenAIError::new(
                ErrorKind::Parse,
                "legacy completions stream ended without a [DONE] marker or terminal completion",
            ));
        }
        if !self.parsed_completion {
            return Err(OpenAIError::new(
                ErrorKind::Parse,
                "legacy completions stream ended without any parsed completion payload",
            ));
        }

        Ok(Completion {
            id: self.id.unwrap_or_default(),
            object: self
                .object
                .unwrap_or_else(|| String::from("text_completion")),
            created: self.created.unwrap_or_default(),
            model: self.model,
            choices: self
                .choices
                .into_iter()
                .enumerate()
                .map(|(index, choice)| CompletionChoice {
                    finish_reason: choice.finish_reason,
                    index,
                    logprobs: choice.logprobs,
                    text: choice.text,
                    extra: BTreeMap::new(),
                })
                .collect(),
            usage: self.usage,
            system_fingerprint: self.system_fingerprint,
            extra: BTreeMap::new(),
        })
    }
}

#[derive(Clone, Debug, Default)]
struct AccumulatedCompletionChoice {
    finish_reason: Option<String>,
    logprobs: Option<Value>,
    text: String,
}

fn map_live_transport_error(error: reqwest::Error) -> OpenAIError {
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Transport
    };
    OpenAIError::new(kind, error.to_string()).with_source(error)
}
