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

#[test]
fn newer_realtime_server_events_decode_as_first_class_variants() {
    let conversation = decode_server_event(&json!({
        "type": "conversation.created",
        "event_id": "evt_conversation",
        "conversation": {"id": "conv_123", "object": "realtime.conversation"}
    }))
    .expect("conversation.created should decode");
    assert!(matches!(
        conversation,
        RealtimeServerEvent::ConversationCreated { ref conversation, .. }
            if conversation["id"] == "conv_123"
    ));

    let item_added = decode_server_event(&json!({
        "type": "conversation.item.added",
        "event_id": "evt_item_added",
        "previous_item_id": "item_prev",
        "item": {"id": "item_123", "type": "message", "role": "user", "content": []}
    }))
    .expect("conversation.item.added should decode");
    assert!(matches!(
        item_added,
        RealtimeServerEvent::ConversationItemAdded {
            previous_item_id: Some(ref previous_item_id),
            item: RealtimeConversationItem { id: Some(ref item_id), .. },
            ..
        } if previous_item_id == "item_prev" && item_id == "item_123"
    ));

    let item_done = decode_server_event(&json!({
        "type": "conversation.item.done",
        "event_id": "evt_item_done",
        "item": {"id": "item_123", "type": "message", "role": "user", "content": []}
    }))
    .expect("conversation.item.done should decode");
    assert_eq!(item_done.event_type(), "conversation.item.done");

    let item_deleted = decode_server_event(&json!({
        "type": "conversation.item.deleted",
        "event_id": "evt_item_deleted",
        "item_id": "item_123"
    }))
    .expect("conversation.item.deleted should decode");
    assert!(matches!(
        item_deleted,
        RealtimeServerEvent::ConversationItemDeleted { ref item_id, .. } if item_id == "item_123"
    ));

    let retrieved = decode_server_event(&json!({
        "type": "conversation.item.retrieved",
        "event_id": "evt_item_retrieved",
        "item": {"id": "item_123", "type": "message", "role": "user", "content": []}
    }))
    .expect("conversation.item.retrieved should decode");
    assert_eq!(retrieved.event_type(), "conversation.item.retrieved");

    let transcription_delta = decode_server_event(&json!({
        "type": "conversation.item.input_audio_transcription.delta",
        "event_id": "evt_transcript_delta",
        "item_id": "item_123",
        "content_index": 0,
        "delta": "hel",
        "logprobs": [{"token": "hel"}]
    }))
    .expect("transcription delta should decode");
    assert!(matches!(
        transcription_delta,
        RealtimeServerEvent::ConversationItemInputAudioTranscriptionDelta {
            content_index: Some(0),
            delta: Some(ref delta),
            logprobs: Some(_),
            ..
        } if delta == "hel"
    ));

    let transcription_completed = decode_server_event(&json!({
        "type": "conversation.item.input_audio_transcription.completed",
        "event_id": "evt_transcript_completed",
        "item_id": "item_123",
        "content_index": 0,
        "transcript": "hello",
        "usage": {"type": "tokens", "input_tokens": 1, "output_tokens": 1, "total_tokens": 2}
    }))
    .expect("transcription completed should decode");
    assert!(matches!(
        transcription_completed,
        RealtimeServerEvent::ConversationItemInputAudioTranscriptionCompleted {
            ref transcript,
            ref usage,
            ..
        } if transcript == "hello" && usage["type"] == "tokens"
    ));

    let transcription_failed = decode_server_event(&json!({
        "type": "conversation.item.input_audio_transcription.failed",
        "event_id": "evt_transcript_failed",
        "item_id": "item_123",
        "content_index": 0,
        "error": {"message": "bad audio"}
    }))
    .expect("transcription failed should decode");
    assert_eq!(
        transcription_failed.event_type(),
        "conversation.item.input_audio_transcription.failed"
    );

    let segment = decode_server_event(&json!({
        "type": "conversation.item.input_audio_transcription.segment",
        "event_id": "evt_segment",
        "item_id": "item_123",
        "content_index": 0,
        "id": "seg_1",
        "start": 0.0,
        "end": 0.4,
        "speaker": "speaker_0",
        "text": "hello"
    }))
    .expect("transcription segment should decode");
    assert!(matches!(
        segment,
        RealtimeServerEvent::ConversationItemInputAudioTranscriptionSegment {
            ref id,
            ref speaker,
            ..
        } if id == "seg_1" && speaker == "speaker_0"
    ));

    let dtmf = decode_server_event(&json!({
        "type": "input_audio_buffer.dtmf_event_received",
        "event": "1",
        "received_at": 123
    }))
    .expect("dtmf event should decode");
    assert!(matches!(
        dtmf,
        RealtimeServerEvent::InputAudioBufferDtmfEventReceived {
            ref event,
            received_at: 123
        } if event == "1"
    ));

    let timeout = decode_server_event(&json!({
        "type": "input_audio_buffer.timeout_triggered",
        "event_id": "evt_timeout",
        "item_id": "item_123",
        "audio_start_ms": 10,
        "audio_end_ms": 90
    }))
    .expect("timeout event should decode");
    assert_eq!(timeout.event_type(), "input_audio_buffer.timeout_triggered");

    let rate_limits = decode_server_event(&json!({
        "type": "rate_limits.updated",
        "event_id": "evt_rate_limits",
        "rate_limits": [{"name": "requests", "limit": 100, "remaining": 99, "reset_seconds": 1.0}]
    }))
    .expect("rate limits should decode");
    assert!(matches!(
        rate_limits,
        RealtimeServerEvent::RateLimitsUpdated { ref rate_limits, .. }
            if rate_limits[0]["name"] == "requests"
    ));

    for event_type in [
        "mcp_list_tools.in_progress",
        "mcp_list_tools.completed",
        "mcp_list_tools.failed",
    ] {
        let status = decode_server_event(&json!({
            "type": event_type,
            "event_id": format!("evt_{event_type}"),
            "item_id": "mcp_123"
        }))
        .expect("mcp list tools status should decode");
        assert!(matches!(
            status,
            RealtimeServerEvent::McpListToolsStatus {
                ref item_id,
                ..
            } if item_id == "mcp_123"
        ));
        assert_eq!(status.event_type(), event_type);
    }
}
