use openai_rust::realtime::{RealtimeConversationItem, RealtimeServerEvent, decode_server_event};
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

#[test]
fn output_audio_done_decodes_as_a_first_class_lifecycle_event() {
    let done = decode_server_event(&json!({
        "type": "response.output_audio.done",
        "event_id": "evt_audio_done",
        "response_id": "resp_123",
        "item_id": "item_123",
        "output_index": 0,
        "content_index": 0
    }))
    .expect("response.output_audio.done should decode");
    assert!(matches!(
        done,
        RealtimeServerEvent::OutputAudioDone {
            ref event_id,
            ref response_id,
            ref item_id,
            output_index,
            content_index,
        } if event_id == "evt_audio_done"
            && response_id == "resp_123"
            && item_id == "item_123"
            && output_index == 0
            && content_index == 0
    ));
}

#[test]
fn output_item_events_derive_item_id_from_the_nested_item_payload() {
    let item_added = decode_server_event(&json!({
        "type": "response.output_item.added",
        "event_id": "evt_added",
        "response_id": "resp_123",
        "output_index": 0,
        "item": {
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": []
        }
    }))
    .expect("response.output_item.added should decode from item.id");
    assert!(matches!(
        item_added,
        RealtimeServerEvent::ResponseOutputItemAdded {
            ref item_id,
            item: RealtimeConversationItem { id: Some(ref nested_id), .. },
            ..
        } if item_id == "msg_123" && nested_id == "msg_123"
    ));

    let item_done = decode_server_event(&json!({
        "type": "response.output_item.done",
        "event_id": "evt_done",
        "response_id": "resp_123",
        "output_index": 0,
        "item": {
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": []
        }
    }))
    .expect("response.output_item.done should decode from item.id");
    assert!(matches!(
        item_done,
        RealtimeServerEvent::ResponseOutputItemDone {
            ref item_id,
            item: RealtimeConversationItem { id: Some(ref nested_id), .. },
            ..
        } if item_id == "msg_123" && nested_id == "msg_123"
    ));
}
