use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use openai_rust::resources::webhooks::{WebhookEvent, WebhookHeaders, WebhookVerificationOptions};
use openai_rust::{ErrorKind, OpenAI};
use sha2::Sha256;

static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

type HmacSha256 = Hmac<Sha256>;

#[test]
fn unwrap_verifies_the_raw_body_before_parsing_and_returns_typed_event() {
    let client = OpenAI::builder().webhook_secret("test-secret").build();
    let raw_body = br#"{
  "id": "evt_response_completed",
  "created_at": 1,
  "type": "response.completed",
  "data": { "id": "resp_123" }
}"#;
    let timestamp = now_seconds();
    let webhook_id = "wh_123";
    let headers = signed_headers("test-secret", webhook_id, timestamp, raw_body);

    let event = client.webhooks().unwrap(raw_body, &headers).unwrap();
    match event {
        WebhookEvent::ResponseCompleted(event) => {
            assert_eq!(event.id, "evt_response_completed");
            assert_eq!(event.data.id, "resp_123");
            assert_eq!(event.created_at, 1);
        }
        other => panic!("expected response.completed event, got {other:?}"),
    }

    let reparsed = serde_json::from_slice::<serde_json::Value>(raw_body).unwrap();
    let normalized = serde_json::to_vec(&reparsed).unwrap();
    let signature_error = client.webhooks().unwrap(&normalized, &headers).unwrap_err();
    assert_eq!(signature_error.kind, ErrorKind::WebhookSignature);
    assert!(signature_error.message.contains("does not match"));
}

#[test]
fn unwrap_surfaces_parse_failures_separately_from_signature_failures() {
    let client = OpenAI::builder().webhook_secret("test-secret").build();
    let raw_body = br#"{"id":"evt_bad","created_at":1,"type":"response.completed","data":{"id":}"#;
    let timestamp = now_seconds();
    let webhook_id = "wh_parse";
    let headers = signed_headers("test-secret", webhook_id, timestamp, raw_body);

    let error = client.webhooks().unwrap(raw_body, &headers).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Parse);
    assert!(
        error
            .message
            .contains("failed to parse verified webhook payload")
    );
}

#[test]
fn unwrap_uses_environment_webhook_secret_by_default() {
    with_env(&[("OPENAI_WEBHOOK_SECRET", Some("env-secret"))], || {
        let client = OpenAI::new();
        let raw_body =
            br#"{"id":"evt_env","created_at":2,"type":"response.failed","data":{"id":"resp_env"}}"#;
        let timestamp = now_seconds();
        let headers = signed_headers("env-secret", "wh_env", timestamp, raw_body);

        let event = client.webhooks().unwrap(raw_body, &headers).unwrap();
        assert!(matches!(event, WebhookEvent::ResponseFailed(_)));
    });
}

#[test]
fn unwrap_accepts_an_explicit_secret_override() {
    let client = OpenAI::builder().webhook_secret("default-secret").build();
    let raw_body = br#"{"id":"evt_override","created_at":3,"type":"batch.completed","data":{"id":"batch_123"}}"#;
    let timestamp = now_seconds();
    let headers = signed_headers("override-secret", "wh_override", timestamp, raw_body);

    let event = client
        .webhooks()
        .unwrap_with_options(
            raw_body,
            &headers,
            WebhookVerificationOptions::default().with_secret("override-secret"),
        )
        .unwrap();

    assert!(matches!(event, WebhookEvent::BatchCompleted(_)));
}

#[test]
fn unwrap_respects_custom_tolerance_for_signed_fixtures() {
    let client = OpenAI::builder().webhook_secret("test-secret").build();
    let raw_body = br#"{"id":"evt_tolerance","created_at":4,"type":"response.incomplete","data":{"id":"resp_incomplete"}}"#;
    let timestamp = now_seconds() - 301;
    let headers = signed_headers("test-secret", "wh_tol", timestamp, raw_body);

    let error = client.webhooks().unwrap(raw_body, &headers).unwrap_err();
    assert_eq!(error.kind, ErrorKind::WebhookSignature);
    assert!(error.message.contains("too old"));

    let event = client
        .webhooks()
        .unwrap_with_options(
            raw_body,
            &headers,
            WebhookVerificationOptions::default().with_tolerance(Duration::from_secs(400)),
        )
        .unwrap();
    assert!(matches!(event, WebhookEvent::ResponseIncomplete(_)));
}

fn signed_headers(secret: &str, webhook_id: &str, timestamp: i64, body: &[u8]) -> WebhookHeaders {
    WebhookHeaders::from_pairs([
        ("webhook-id", webhook_id.to_string()),
        ("webhook-timestamp", timestamp.to_string()),
        (
            "webhook-signature",
            format!(
                "v1,{}",
                compute_signature(secret, webhook_id, timestamp, body)
            ),
        ),
    ])
}

fn compute_signature(secret: &str, webhook_id: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(&decode_secret(secret)).unwrap();
    mac.update(format!("{webhook_id}.{timestamp}.").as_bytes());
    mac.update(body);
    STANDARD.encode(mac.finalize().into_bytes())
}

fn decode_secret(secret: &str) -> Vec<u8> {
    if let Some(encoded) = secret.strip_prefix("whsec_") {
        STANDARD.decode(encoded).unwrap()
    } else {
        secret.as_bytes().to_vec()
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn with_env(vars: &[(&str, Option<&str>)], test: impl FnOnce()) {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();

    let saved = vars
        .iter()
        .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
        .collect::<Vec<_>>();

    for (key, value) in vars {
        unsafe {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }

    test();

    for (key, value) in saved {
        unsafe {
            match value {
                Some(value) => std::env::set_var(&key, value),
                None => std::env::remove_var(&key),
            }
        }
    }
}
