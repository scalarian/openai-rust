use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use openai_rust::resources::webhooks::{WebhookHeaders, WebhookVerificationOptions};
use openai_rust::{ErrorKind, OpenAI};
use sha2::Sha256;

#[test]
fn signature_verification_handles_client_explicit_and_whsec_secrets() {
    let client = OpenAI::builder().webhook_secret("client-secret").build();
    let body =
        br#"{"id":"evt_sig","created_at":1,"type":"response.completed","data":{"id":"resp_sig"}}"#;
    let timestamp = now_seconds();

    let client_headers = signed_headers("client-secret", "wh_client", timestamp, body);
    client
        .webhooks()
        .verify_signature(body, &client_headers)
        .unwrap();

    let encoded_secret = format!("whsec_{}", STANDARD.encode("decoded-secret"));
    let explicit_headers = signed_headers(&encoded_secret, "wh_explicit", timestamp, body);
    client
        .webhooks()
        .verify_signature_with_options(
            body,
            &explicit_headers,
            WebhookVerificationOptions::default().with_secret(encoded_secret),
        )
        .unwrap();
}

#[test]
fn signature_verification_rejects_missing_secret_and_required_headers() {
    let client = OpenAI::builder().build();
    let body = br#"{"id":"evt_missing","created_at":1,"type":"response.completed","data":{"id":"resp_missing"}}"#;
    let missing_secret = client
        .webhooks()
        .verify_signature(body, &WebhookHeaders::default())
        .unwrap_err();
    assert_eq!(missing_secret.kind, ErrorKind::Configuration);
    assert!(missing_secret.message.contains("OPENAI_WEBHOOK_SECRET"));

    let missing_header = OpenAI::builder().webhook_secret("secret").build();
    let error = missing_header
        .webhooks()
        .verify_signature(
            body,
            &WebhookHeaders::from_pairs([
                ("webhook-id", "wh_missing".to_string()),
                ("webhook-timestamp", now_seconds().to_string()),
            ]),
        )
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::WebhookSignature);
    assert!(
        error
            .message
            .contains("Missing required header: webhook-signature")
    );
}

#[test]
fn signature_verification_rejects_invalid_stale_and_future_timestamps() {
    let client = OpenAI::builder().webhook_secret("secret").build();
    let body = br#"{"id":"evt_time","created_at":1,"type":"response.completed","data":{"id":"resp_time"}}"#;

    let invalid_timestamp = WebhookHeaders::from_pairs([
        ("webhook-id", "wh_invalid".to_string()),
        ("webhook-timestamp", "not-a-number".to_string()),
        ("webhook-signature", "v1,abc".to_string()),
    ]);
    let invalid = client
        .webhooks()
        .verify_signature(body, &invalid_timestamp)
        .unwrap_err();
    assert_eq!(invalid.kind, ErrorKind::WebhookSignature);
    assert!(invalid.message.contains("Invalid webhook timestamp format"));

    let old_timestamp = now_seconds() - 301;
    let old_headers = signed_headers("secret", "wh_old", old_timestamp, body);
    let old_error = client
        .webhooks()
        .verify_signature(body, &old_headers)
        .unwrap_err();
    assert_eq!(old_error.kind, ErrorKind::WebhookSignature);
    assert!(old_error.message.contains("too old"));

    let future_timestamp = now_seconds() + 301;
    let future_headers = signed_headers("secret", "wh_future", future_timestamp, body);
    let future_error = client
        .webhooks()
        .verify_signature(body, &future_headers)
        .unwrap_err();
    assert_eq!(future_error.kind, ErrorKind::WebhookSignature);
    assert!(future_error.message.contains("too new"));

    client
        .webhooks()
        .verify_signature_with_options(
            body,
            &future_headers,
            WebhookVerificationOptions::default().with_tolerance(Duration::from_secs(400)),
        )
        .unwrap();
}

#[test]
fn signature_verification_accepts_any_matching_signature_from_multi_value_headers() {
    let client = OpenAI::builder().webhook_secret("secret").build();
    let body = br#"{"id":"evt_multi","created_at":1,"type":"response.completed","data":{"id":"resp_multi"}}"#;
    let timestamp = now_seconds();
    let valid = compute_signature("secret", "wh_multi", timestamp, body);
    let headers = WebhookHeaders::from_pairs([
        ("webhook-id", "wh_multi".to_string()),
        ("webhook-timestamp", timestamp.to_string()),
        (
            "webhook-signature",
            format!("v1,invalid-base64 v1,{valid} v1,also-invalid"),
        ),
    ]);

    client.webhooks().verify_signature(body, &headers).unwrap();
}

#[test]
fn signature_verification_raises_a_typed_signature_error_on_mismatch() {
    let client = OpenAI::builder().webhook_secret("secret").build();
    let body = br#"{"id":"evt_mismatch","created_at":1,"type":"response.completed","data":{"id":"resp_mismatch"}}"#;
    let timestamp = now_seconds();
    let headers = WebhookHeaders::from_pairs([
        ("webhook-id", "wh_mismatch".to_string()),
        ("webhook-timestamp", timestamp.to_string()),
        ("webhook-signature", "v1,totally-wrong".to_string()),
    ]);

    let error = client
        .webhooks()
        .verify_signature(body, &headers)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::WebhookSignature);
    assert!(error.message.contains("does not match"));
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
    let mut mac = Hmac::<Sha256>::new_from_slice(&decode_secret(secret)).unwrap();
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
