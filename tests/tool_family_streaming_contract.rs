use openai_rust::{core::metadata::ResponseMetadata, resources::responses::ResponseStream};
use serde_json::json;

#[test]
fn tool_family_events_reconcile_with_final_state() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_tool_family\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"fs_1\",\"type\":\"file_search_call\",\"status\":\"in_progress\",\"queries\":[\"docs\"],\"results\":[]},{\"id\":\"ws_1\",\"type\":\"web_search_call\",\"status\":\"in_progress\",\"action\":{\"type\":\"search\",\"query\":\"weather\",\"sources\":[]}}, {\"id\":\"ci_1\",\"type\":\"code_interpreter_call\",\"status\":\"in_progress\",\"code\":\"\",\"outputs\":[]},{\"id\":\"mcp_1\",\"type\":\"mcp_call\",\"status\":\"in_progress\",\"arguments\":\"\"}],\"usage\":{}}\n\n",
        "event: response.file_search_call.searching\n",
        "data: {\"item_id\":\"fs_1\",\"output_index\":0}\n\n",
        "event: response.web_search_call.searching\n",
        "data: {\"item_id\":\"ws_1\",\"output_index\":1}\n\n",
        "event: response.code_interpreter_call_code.delta\n",
        "data: {\"item_id\":\"ci_1\",\"output_index\":2,\"delta\":\"print(1)\"}\n\n",
        "event: response.mcp_call_arguments.delta\n",
        "data: {\"item_id\":\"mcp_1\",\"output_index\":3,\"delta\":\"{\\\"server\\\":\\\"docs\\\"}\"}\n\n",
        "event: response.file_search_call.completed\n",
        "data: {\"item_id\":\"fs_1\",\"output_index\":0}\n\n",
        "event: response.web_search_call.completed\n",
        "data: {\"item_id\":\"ws_1\",\"output_index\":1}\n\n",
        "event: response.code_interpreter_call.completed\n",
        "data: {\"item_id\":\"ci_1\",\"output_index\":2}\n\n",
        "event: response.mcp_call_arguments.done\n",
        "data: {\"item_id\":\"mcp_1\",\"output_index\":3,\"arguments\":\"{\\\"server\\\":\\\"docs\\\"}\"}\n\n",
        "event: response.mcp_call.completed\n",
        "data: {\"item_id\":\"mcp_1\",\"output_index\":3}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_tool_family\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"fs_1\",\"type\":\"file_search_call\",\"status\":\"completed\",\"queries\":[\"docs\"],\"results\":[{\"file_id\":\"file_1\",\"filename\":\"guide.md\",\"score\":0.9}]},{\"id\":\"ws_1\",\"type\":\"web_search_call\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"query\":\"weather\",\"sources\":[{\"type\":\"url\",\"url\":\"https://example.com\"}]}},{\"id\":\"ci_1\",\"type\":\"code_interpreter_call\",\"status\":\"completed\",\"code\":\"print(1)\",\"outputs\":[{\"type\":\"logs\",\"logs\":\"1\"}]},{\"id\":\"mcp_1\",\"type\":\"mcp_call\",\"status\":\"completed\",\"arguments\":\"{\\\"server\\\":\\\"docs\\\"}\"}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );

    let mut stream = ResponseStream::from_sse_chunks(metadata, vec![transcript]).expect("stream");
    for _ in 0..5 {
        stream.next_event();
    }
    let snapshot = stream.current_response().unwrap();
    assert_eq!(snapshot.output[0].status.as_deref(), Some("searching"));
    assert_eq!(snapshot.output[1].status.as_deref(), Some("searching"));
    assert_eq!(snapshot.output[2].code.as_deref(), Some("print(1)"));
    assert_eq!(
        snapshot.output[3].arguments.as_deref(),
        Some("{\"server\":\"docs\"}")
    );

    for _ in 0..5 {
        stream.next_event();
    }
    let snapshot = stream.current_response().unwrap();
    assert_eq!(snapshot.output[0].status.as_deref(), Some("completed"));
    assert_eq!(snapshot.output[1].status.as_deref(), Some("completed"));
    assert_eq!(snapshot.output[2].status.as_deref(), Some("completed"));
    assert_eq!(snapshot.output[3].status.as_deref(), Some("completed"));

    let final_response = stream.final_response().expect("final response");
    assert_eq!(final_response.output[2].code.as_deref(), Some("print(1)"));
    assert_eq!(
        final_response.output[1].extra.get("action"),
        Some(&json!({
            "type": "search",
            "query": "weather",
            "sources": [{"type": "url", "url": "https://example.com"}]
        }))
    );
}
