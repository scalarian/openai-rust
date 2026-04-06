use openai_rust::realtime::{RealtimeEventState, decode_server_event};
use serde_json::json;

#[test]
fn tool_and_mcp_events_finalize_against_terminal_response() {
    let mut state = RealtimeEventState::default();
    let transcript = vec![
        json!({
            "type": "response.created",
            "event_id": "evt_created",
            "response": {
                "id": "resp_tools",
                "object": "realtime.response",
                "status": "in_progress",
                "output": [
                    {
                        "id": "fc_1",
                        "type": "function_call",
                        "name": "weather",
                        "arguments": "",
                        "status": "in_progress"
                    },
                    {
                        "id": "mcp_1",
                        "type": "mcp_call",
                        "name": "docs.lookup",
                        "arguments": "",
                        "status": "in_progress"
                    }
                ]
            }
        }),
        json!({
            "type": "response.function_call_arguments.delta",
            "event_id": "evt_fc_delta",
            "response_id": "resp_tools",
            "item_id": "fc_1",
            "output_index": 0,
            "delta": "{\"city\":"
        }),
        json!({
            "type": "response.function_call_arguments.done",
            "event_id": "evt_fc_done",
            "response_id": "resp_tools",
            "item_id": "fc_1",
            "output_index": 0,
            "arguments": "{\"city\":\"Paris\"}",
            "name": "weather"
        }),
        json!({
            "type": "response.mcp_call_arguments.delta",
            "event_id": "evt_mcp_delta",
            "response_id": "resp_tools",
            "item_id": "mcp_1",
            "output_index": 1,
            "delta": "{\"server\":\"docs\"}",
            "obfuscation": null
        }),
        json!({
            "type": "response.mcp_call.in_progress",
            "event_id": "evt_mcp_progress",
            "item_id": "mcp_1",
            "output_index": 1
        }),
        json!({
            "type": "response.mcp_call_arguments.done",
            "event_id": "evt_mcp_done",
            "response_id": "resp_tools",
            "item_id": "mcp_1",
            "output_index": 1,
            "arguments": "{\"server\":\"docs\"}"
        }),
        json!({
            "type": "response.mcp_call.completed",
            "event_id": "evt_mcp_completed",
            "item_id": "mcp_1",
            "output_index": 1
        }),
        json!({
            "type": "response.done",
            "event_id": "evt_done",
            "response": {
                "id": "resp_tools",
                "object": "realtime.response",
                "status": "completed",
                "output": [
                    {
                        "id": "fc_1",
                        "type": "function_call",
                        "name": "weather",
                        "arguments": "{\"city\":\"Paris\"}",
                        "status": "completed"
                    },
                    {
                        "id": "mcp_1",
                        "type": "mcp_call",
                        "name": "docs.lookup",
                        "arguments": "{\"server\":\"docs\"}",
                        "status": "completed"
                    }
                ]
            }
        }),
    ];

    for payload in transcript {
        let event = decode_server_event(&payload).expect("event should decode");
        state.apply(&event).expect("event should apply");
    }

    let current = state.current_response().expect("current response");
    assert_eq!(
        current.output[0].arguments.as_deref(),
        Some("{\"city\":\"Paris\"}")
    );
    assert_eq!(
        current.output[1].arguments.as_deref(),
        Some("{\"server\":\"docs\"}")
    );
    assert_eq!(current.output[1].status.as_deref(), Some("completed"));

    let terminal = state.terminal_response().expect("terminal response");
    assert_eq!(terminal.response["status"], json!("completed"));
    assert_eq!(
        terminal.output[0].arguments.as_deref(),
        Some("{\"city\":\"Paris\"}")
    );
    assert_eq!(terminal.output[1].status.as_deref(), Some("completed"));
}
