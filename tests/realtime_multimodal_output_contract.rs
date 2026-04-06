use openai_rust::realtime::{RealtimeEventState, decode_server_event};
use serde_json::json;

#[test]
fn output_items_reconcile_at_response_done() {
    let mut state = RealtimeEventState::default();
    let transcript = vec![
        json!({
            "type": "response.created",
            "event_id": "evt_created",
            "response": {
                "id": "resp_mm",
                "object": "realtime.response",
                "status": "in_progress",
                "output": []
            }
        }),
        json!({
            "type": "response.output_item.added",
            "event_id": "evt_item_added",
            "response_id": "resp_mm",
            "output_index": 0,
            "item_id": "msg_1",
            "item": {
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "status": "in_progress",
                "content": []
            }
        }),
        json!({
            "type": "response.content_part.added",
            "event_id": "evt_part_audio",
            "response_id": "resp_mm",
            "item_id": "msg_1",
            "output_index": 0,
            "content_index": 0,
            "part": {
                "type": "audio",
                "audio": "",
                "transcript": ""
            }
        }),
        json!({
            "type": "response.output_audio.delta",
            "event_id": "evt_audio_delta",
            "response_id": "resp_mm",
            "item_id": "msg_1",
            "output_index": 0,
            "content_index": 0,
            "delta": "YWJj"
        }),
        json!({
            "type": "response.output_audio.delta",
            "event_id": "evt_audio_delta_2",
            "response_id": "resp_mm",
            "item_id": "msg_1",
            "output_index": 0,
            "content_index": 0,
            "delta": "ZGVm"
        }),
        json!({
            "type": "response.output_audio_transcript.delta",
            "event_id": "evt_audio_tx_delta",
            "response_id": "resp_mm",
            "item_id": "msg_1",
            "output_index": 0,
            "content_index": 0,
            "delta": "spoken"
        }),
        json!({
            "type": "response.output_audio.done",
            "event_id": "evt_audio_done",
            "response_id": "resp_mm",
            "item_id": "msg_1",
            "output_index": 0,
            "content_index": 0
        }),
        json!({
            "type": "response.output_audio_transcript.done",
            "event_id": "evt_audio_tx_done",
            "response_id": "resp_mm",
            "item_id": "msg_1",
            "output_index": 0,
            "content_index": 0,
            "transcript": "spoken hi"
        }),
        json!({
            "type": "response.content_part.added",
            "event_id": "evt_part_text",
            "response_id": "resp_mm",
            "item_id": "msg_1",
            "output_index": 0,
            "content_index": 1,
            "part": {
                "type": "text",
                "text": ""
            }
        }),
        json!({
            "type": "response.output_text.delta",
            "event_id": "evt_text_delta",
            "response_id": "resp_mm",
            "item_id": "msg_1",
            "output_index": 0,
            "content_index": 1,
            "delta": "Hi"
        }),
        json!({
            "type": "response.output_text.done",
            "event_id": "evt_text_done",
            "response_id": "resp_mm",
            "item_id": "msg_1",
            "output_index": 0,
            "content_index": 1,
            "text": "Hi"
        }),
        json!({
            "type": "response.done",
            "event_id": "evt_done",
            "response": {
                "id": "resp_mm",
                "object": "realtime.response",
                "status": "completed",
                "output": [
                    {
                        "id": "msg_1",
                        "type": "message",
                        "role": "assistant",
                        "status": "completed",
                        "content": [
                            {"type": "audio", "audio": "YWJjZGVm", "transcript": "spoken hi"},
                            {"type": "text", "text": "Hi"}
                        ]
                    }
                ]
            }
        }),
    ];

    for payload in transcript {
        let event = decode_server_event(&payload).expect("event should decode");
        if payload["type"] == json!("response.output_audio.done") {
            assert_eq!(event.event_type(), "response.output_audio.done");
        }
        state.apply(&event).expect("event should apply");
    }

    let current = state.current_response().expect("current response");
    assert_eq!(current.output_text(), "Hi");
    assert_eq!(
        current.output[0].content[0].audio.as_deref(),
        Some("YWJjZGVm")
    );
    assert_eq!(
        current.output[0].content[0].transcript.as_deref(),
        Some("spoken hi")
    );
    assert_eq!(current.output[0].content[1].text.as_deref(), Some("Hi"));

    let terminal = state.terminal_response().expect("terminal response");
    assert_eq!(terminal.response["status"], json!("completed"));
    assert_eq!(terminal.output_text(), "Hi");
}
