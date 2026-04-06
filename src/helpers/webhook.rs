use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{
    OpenAIError,
    error::ErrorKind,
    resources::webhooks::{WebhookHeaders, WebhookVerificationOptions},
};

type HmacSha256 = Hmac<Sha256>;

/// Shared webhook verification helper.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WebhookVerifier;

impl WebhookVerifier {
    /// Verifies the raw payload against the supplied webhook headers and secret.
    pub fn verify(
        body: &[u8],
        headers: &WebhookHeaders,
        secret: &str,
        options: &WebhookVerificationOptions,
    ) -> Result<(), OpenAIError> {
        let webhook_id = headers.required("webhook-id")?;
        let timestamp = headers.required("webhook-timestamp")?;
        let signature_header = headers.required("webhook-signature")?;

        let timestamp_seconds = timestamp.parse::<i64>().map_err(|_| {
            OpenAIError::new(
                ErrorKind::WebhookSignature,
                "Invalid webhook timestamp format",
            )
        })?;

        let now_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let tolerance = options
            .tolerance
            .unwrap_or(Duration::from_secs(300))
            .as_secs() as i64;

        if now_seconds - timestamp_seconds > tolerance {
            return Err(OpenAIError::new(
                ErrorKind::WebhookSignature,
                "Webhook timestamp is too old",
            ));
        }

        if timestamp_seconds > now_seconds + tolerance {
            return Err(OpenAIError::new(
                ErrorKind::WebhookSignature,
                "Webhook timestamp is too new",
            ));
        }

        let secret_bytes = decode_secret(secret)?;
        let signatures = signature_header
            .split_whitespace()
            .map(|part| part.strip_prefix("v1,").unwrap_or(part));

        for signature in signatures {
            let Ok(candidate) = STANDARD.decode(signature) else {
                continue;
            };
            let mut mac = HmacSha256::new_from_slice(&secret_bytes).map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Configuration,
                    format!("invalid webhook secret: {error}"),
                )
                .with_source(error)
            })?;
            mac.update(format!("{webhook_id}.{timestamp}.").as_bytes());
            mac.update(body);
            if mac.verify_slice(&candidate).is_ok() {
                return Ok(());
            }
        }

        Err(OpenAIError::new(
            ErrorKind::WebhookSignature,
            "The given webhook signature does not match the expected signature",
        ))
    }
}

fn decode_secret(secret: &str) -> Result<Vec<u8>, OpenAIError> {
    if let Some(encoded) = secret.strip_prefix("whsec_") {
        STANDARD.decode(encoded).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Configuration,
                format!("invalid webhook secret encoding: {error}"),
            )
            .with_source(error)
        })
    } else {
        Ok(secret.as_bytes().to_vec())
    }
}
