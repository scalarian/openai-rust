use openai_rust::realtime::{RealtimeServerEvent, decode_server_event};
use serde_json::json;

#[test]
fn ga_event_names_are_canonical_and_beta_aliases_stay_non_primary() {
    let delta = decode_server_event(&json!({
        "type": "response.output_text.delta",
        "event_id": "evt_delta",
        "response_id": "resp_123",
        "item_id": "item_123",
        "output_index": 0,
        "content_index": 0,
        "delta": "Hel"
    }))
    .unwrap();
    assert!(matches!(
        delta,
        RealtimeServerEvent::OutputTextDelta { ref delta, .. } if delta == "Hel"
    ));

    let done = decode_server_event(&json!({
        "type": "response.output_text.done",
        "event_id": "evt_done",
        "response_id": "resp_123",
        "item_id": "item_123",
        "output_index": 0,
        "content_index": 0,
        "text": "Hello"
    }))
    .unwrap();
    assert!(matches!(
        done,
        RealtimeServerEvent::OutputTextDone { ref text, .. } if text == "Hello"
    ));

    let audio_started = decode_server_event(&json!({
        "type": "output_audio_buffer.started",
        "event_id": "evt_audio",
        "response_id": "resp_123"
    }))
    .unwrap();
    assert!(matches!(
        audio_started,
        RealtimeServerEvent::OutputAudioBufferStarted { .. }
    ));

    let beta_alias = decode_server_event(&json!({
        "type": "response.text.delta",
        "event_id": "evt_beta",
        "delta": "legacy"
    }))
    .unwrap();
    assert!(matches!(
        beta_alias,
        RealtimeServerEvent::Unknown { ref event_type, .. } if event_type == "response.text.delta"
    ));

    let additive_unknown = decode_server_event(&json!({
        "type": "response.future.added",
        "event_id": "evt_future",
        "payload": {"ok": true}
    }))
    .unwrap();
    assert!(matches!(
        additive_unknown,
        RealtimeServerEvent::Unknown { ref event_type, .. } if event_type == "response.future.added"
    ));
}
