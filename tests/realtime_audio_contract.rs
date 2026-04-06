use openai_rust::realtime::{RealtimeEventState, RealtimeServerEvent, decode_server_event};
use serde_json::json;

#[test]
fn audio_buffer_commit_and_truncation_preserve_conversation_state() {
    let mut state = RealtimeEventState::default();

    for payload in [
        json!({
            "type": "conversation.item.created",
            "event_id": "evt_assistant_item",
            "item": {
                "id": "asst_1",
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [
                    {"type": "audio", "audio": "AAA=", "transcript": "full transcript"}
                ]
            }
        }),
        json!({
            "type": "input_audio_buffer.committed",
            "event_id": "evt_buffer_commit",
            "item_id": "user_audio_1",
            "previous_item_id": "asst_1",
            "sequence_number": 7
        }),
        json!({
            "type": "input_audio_buffer.speech_started",
            "event_id": "evt_speech_started",
            "item_id": "user_audio_1",
            "audio_start_ms": 120
        }),
        json!({
            "type": "conversation.item.created",
            "event_id": "evt_user_item",
            "previous_item_id": "asst_1",
            "item": {
                "id": "user_audio_1",
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_audio", "audio": "AQID", "transcript": "hello"}
                ]
            }
        }),
        json!({
            "type": "input_audio_buffer.speech_stopped",
            "event_id": "evt_speech_stopped",
            "item_id": "user_audio_1",
            "audio_end_ms": 420
        }),
        json!({
            "type": "conversation.item.truncated",
            "event_id": "evt_truncated",
            "item_id": "asst_1",
            "content_index": 0,
            "audio_end_ms": 240
        }),
        json!({
            "type": "input_audio_buffer.cleared",
            "event_id": "evt_buffer_cleared"
        }),
    ] {
        let event = decode_server_event(&payload).expect("event should decode");
        state.apply(&event).expect("event should apply");
    }

    let buffer = state.audio_buffer();
    assert_eq!(buffer.committed_item_id.as_deref(), Some("user_audio_1"));
    assert_eq!(buffer.previous_item_id.as_deref(), Some("asst_1"));
    assert_eq!(buffer.speech_started_ms, Some(120));
    assert_eq!(buffer.speech_stopped_ms, Some(420));
    assert!(buffer.cleared);

    let assistant = state.conversation_item("asst_1").expect("assistant item");
    assert_eq!(
        assistant.content[0].extra.get("audio_end_ms"),
        Some(&json!(240))
    );
    assert_eq!(assistant.content[0].transcript.as_deref(), Some(""));

    let user_item = state.conversation_item("user_audio_1").expect("user item");
    assert_eq!(user_item.content[0].audio.as_deref(), Some("AQID"));
    assert_eq!(user_item.content[0].transcript.as_deref(), Some("hello"));

    let additive = decode_server_event(&json!({
        "type": "input_audio_buffer.committed",
        "event_id": "evt_additive",
        "item_id": "user_audio_2",
        "previous_item_id": null,
        "future_field": {"ok": true}
    }))
    .expect("known events should accept additive fields");
    assert!(matches!(
        additive,
        RealtimeServerEvent::InputAudioBufferCommitted { .. }
    ));

    let unknown = decode_server_event(&json!({
        "type": "input_audio_buffer.future_mode",
        "event_id": "evt_unknown",
        "payload": {"ok": true}
    }))
    .expect("unknown events should stay lossless");
    assert!(matches!(unknown, RealtimeServerEvent::Unknown { .. }));
}
