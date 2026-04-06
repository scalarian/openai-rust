use std::{collections::BTreeMap, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError, core::runtime::ClientRuntime, error::ErrorKind, helpers::webhook::WebhookVerifier,
};

/// Webhook helper family.
#[derive(Clone, Debug)]
pub struct Webhooks {
    runtime: Arc<ClientRuntime>,
}

impl Webhooks {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Verifies the raw payload signature and parses the typed webhook event.
    pub fn unwrap(
        &self,
        body: impl AsRef<[u8]>,
        headers: &WebhookHeaders,
    ) -> Result<WebhookEvent, OpenAIError> {
        self.unwrap_with_options(body, headers, WebhookVerificationOptions::default())
    }

    /// Verifies the raw payload signature with explicit options before parsing.
    pub fn unwrap_with_options(
        &self,
        body: impl AsRef<[u8]>,
        headers: &WebhookHeaders,
        options: WebhookVerificationOptions,
    ) -> Result<WebhookEvent, OpenAIError> {
        let body = body.as_ref();
        self.verify_signature_with_options(body, headers, options)?;
        serde_json::from_slice(body).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to parse verified webhook payload: {error}"),
            )
            .with_source(error)
        })
    }

    /// Verifies the raw payload signature using the configured default secret.
    pub fn verify_signature(
        &self,
        body: impl AsRef<[u8]>,
        headers: &WebhookHeaders,
    ) -> Result<(), OpenAIError> {
        self.verify_signature_with_options(body, headers, WebhookVerificationOptions::default())
    }

    /// Verifies the raw payload signature with explicit options.
    pub fn verify_signature_with_options(
        &self,
        body: impl AsRef<[u8]>,
        headers: &WebhookHeaders,
        options: WebhookVerificationOptions,
    ) -> Result<(), OpenAIError> {
        let secret = options
            .secret
            .as_deref()
            .or(self.runtime.config().webhook_secret.as_deref())
            .map(str::trim)
            .filter(|secret| !secret.is_empty())
            .ok_or_else(|| {
                OpenAIError::new(
                    ErrorKind::Configuration,
                    "The webhook secret must either be set using the env var, OPENAI_WEBHOOK_SECRET, on the client builder, OpenAI::builder().webhook_secret(\"...\"), or passed to this function",
                )
            })?;

        WebhookVerifier::verify(body.as_ref(), headers, secret, &options)
    }
}

/// Optional overrides for webhook verification.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WebhookVerificationOptions {
    pub secret: Option<String>,
    pub tolerance: Option<Duration>,
}

impl WebhookVerificationOptions {
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    pub fn with_tolerance(mut self, tolerance: Duration) -> Self {
        self.tolerance = Some(tolerance);
        self
    }
}

/// Case-insensitive webhook headers wrapper used by the verification helpers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WebhookHeaders {
    values: BTreeMap<String, String>,
}

impl WebhookHeaders {
    pub fn from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut headers = Self::default();
        for (name, value) in pairs {
            headers.insert(name, value);
        }
        headers
    }

    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.values
            .insert(name.into().to_ascii_lowercase(), value.into());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub(crate) fn required(&self, name: &str) -> Result<&str, OpenAIError> {
        self.get(name).ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::WebhookSignature,
                format!("Missing required header: {name}"),
            )
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebhookEvent {
    #[serde(rename = "batch.cancelled")]
    BatchCancelled(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "batch.completed")]
    BatchCompleted(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "batch.expired")]
    BatchExpired(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "batch.failed")]
    BatchFailed(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "eval.run.canceled")]
    EvalRunCanceled(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "eval.run.failed")]
    EvalRunFailed(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "eval.run.succeeded")]
    EvalRunSucceeded(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "fine_tuning.job.cancelled")]
    FineTuningJobCancelled(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "fine_tuning.job.failed")]
    FineTuningJobFailed(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "fine_tuning.job.succeeded")]
    FineTuningJobSucceeded(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "realtime.call.incoming")]
    RealtimeCallIncoming(WebhookEnvelope<RealtimeCallIncomingData>),
    #[serde(rename = "response.cancelled")]
    ResponseCancelled(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "response.completed")]
    ResponseCompleted(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "response.failed")]
    ResponseFailed(WebhookEnvelope<WebhookEntityId>),
    #[serde(rename = "response.incomplete")]
    ResponseIncomplete(WebhookEnvelope<WebhookEntityId>),
}

impl WebhookEvent {
    /// Returns the webhook event identifier emitted by the producer surface.
    pub fn event_id(&self) -> &str {
        match self {
            Self::BatchCancelled(event)
            | Self::BatchCompleted(event)
            | Self::BatchExpired(event)
            | Self::BatchFailed(event)
            | Self::EvalRunCanceled(event)
            | Self::EvalRunFailed(event)
            | Self::EvalRunSucceeded(event)
            | Self::FineTuningJobCancelled(event)
            | Self::FineTuningJobFailed(event)
            | Self::FineTuningJobSucceeded(event)
            | Self::ResponseCancelled(event)
            | Self::ResponseCompleted(event)
            | Self::ResponseFailed(event)
            | Self::ResponseIncomplete(event) => event.id.as_str(),
            Self::RealtimeCallIncoming(event) => event.id.as_str(),
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            Self::BatchCancelled(_) => "batch.cancelled",
            Self::BatchCompleted(_) => "batch.completed",
            Self::BatchExpired(_) => "batch.expired",
            Self::BatchFailed(_) => "batch.failed",
            Self::EvalRunCanceled(_) => "eval.run.canceled",
            Self::EvalRunFailed(_) => "eval.run.failed",
            Self::EvalRunSucceeded(_) => "eval.run.succeeded",
            Self::FineTuningJobCancelled(_) => "fine_tuning.job.cancelled",
            Self::FineTuningJobFailed(_) => "fine_tuning.job.failed",
            Self::FineTuningJobSucceeded(_) => "fine_tuning.job.succeeded",
            Self::RealtimeCallIncoming(_) => "realtime.call.incoming",
            Self::ResponseCancelled(_) => "response.cancelled",
            Self::ResponseCompleted(_) => "response.completed",
            Self::ResponseFailed(_) => "response.failed",
            Self::ResponseIncomplete(_) => "response.incomplete",
        }
    }

    /// Returns the primary upstream resource identifier carried by the webhook payload.
    pub fn resource_id(&self) -> &str {
        match self {
            Self::BatchCancelled(event)
            | Self::BatchCompleted(event)
            | Self::BatchExpired(event)
            | Self::BatchFailed(event)
            | Self::EvalRunCanceled(event)
            | Self::EvalRunFailed(event)
            | Self::EvalRunSucceeded(event)
            | Self::FineTuningJobCancelled(event)
            | Self::FineTuningJobFailed(event)
            | Self::FineTuningJobSucceeded(event)
            | Self::ResponseCancelled(event)
            | Self::ResponseCompleted(event)
            | Self::ResponseFailed(event)
            | Self::ResponseIncomplete(event) => event.data.id.as_str(),
            Self::RealtimeCallIncoming(event) => event.data.call_id.as_str(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebhookEnvelope<T> {
    pub id: String,
    pub created_at: u64,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WebhookEntityId {
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeCallIncomingData {
    pub call_id: String,
    #[serde(default)]
    pub sip_headers: Vec<RealtimeSipHeader>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeSipHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WebhookRawEvent {
    pub id: String,
    pub created_at: u64,
    pub data: Value,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
}
