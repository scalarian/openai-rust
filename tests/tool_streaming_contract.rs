use openai_rust::{
    core::metadata::ResponseMetadata,
    resources::responses::{FunctionTool, ResponseStream},
};
use serde_json::json;

#[test]
fn function_and_custom_tool_inputs_accumulate_until_completion() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_tools\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"fc_1\",\"type\":\"function_call\",\"name\":\"weather\",\"call_id\":\"call_weather\",\"arguments\":\"\",\"status\":\"in_progress\"},{\"id\":\"ct_1\",\"type\":\"custom_tool_call\",\"name\":\"browser\",\"call_id\":\"call_browser\",\"input\":\"\"}],\"usage\":{}}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"item_id\":\"fc_1\",\"output_index\":0,\"delta\":\"{\\\"city\\\":\\\"Pa\"}\n\n",
        "event: response.custom_tool_call_input.delta\n",
        "data: {\"item_id\":\"ct_1\",\"output_index\":1,\"delta\":\"look\"}\n\n",
        "event: response.function_call_arguments.done\n",
        "data: {\"item_id\":\"fc_1\",\"output_index\":0,\"name\":\"weather\",\"arguments\":\"{\\\"city\\\":\\\"Paris\\\"}\"}\n\n",
        "event: response.custom_tool_call_input.done\n",
        "data: {\"item_id\":\"ct_1\",\"output_index\":1,\"input\":\"lookup weather\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_tools\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"fc_1\",\"type\":\"function_call\",\"name\":\"weather\",\"call_id\":\"call_weather\",\"arguments\":\"{\\\"city\\\":\\\"Paris\\\"}\",\"status\":\"completed\"},{\"id\":\"ct_1\",\"type\":\"custom_tool_call\",\"name\":\"browser\",\"call_id\":\"call_browser\",\"input\":\"lookup weather\"}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );

    let mut stream = ResponseStream::from_sse_chunks(metadata, vec![transcript]).expect("stream");
    stream.next_event();
    stream.next_event();
    assert_eq!(
        stream.current_response().unwrap().output[0]
            .arguments
            .as_deref(),
        Some("{\"city\":\"Pa")
    );
    assert!(
        stream.current_response().unwrap().output[0]
            .parsed_arguments
            .is_none()
    );

    stream.next_event();
    assert_eq!(
        stream.current_response().unwrap().output[1]
            .input
            .as_deref(),
        Some("look")
    );

    stream.next_event();
    assert_eq!(
        stream.current_response().unwrap().output[0]
            .arguments
            .as_deref(),
        Some("{\"city\":\"Paris\"}")
    );
    assert!(
        stream.current_response().unwrap().output[0]
            .parsed_arguments
            .is_none()
    );

    stream.next_event();
    assert_eq!(
        stream.current_response().unwrap().output[1]
            .input
            .as_deref(),
        Some("lookup weather")
    );

    let parsed = stream
        .parse_final::<serde_json::Value>(
            None,
            &[FunctionTool {
                name: String::from("weather"),
                parameters: json!({"type": "object"}),
                strict: Some(true),
                description: None,
                defer_loading: None,
            }],
        )
        .expect("final parse");
    assert_eq!(
        parsed.output[0].parsed_arguments.as_ref(),
        Some(&json!({"city": "Paris"}))
    );
    assert_eq!(parsed.output[1].input.as_deref(), Some("lookup weather"));
}
