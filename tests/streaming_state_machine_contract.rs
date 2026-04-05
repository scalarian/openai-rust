use openai_rust::{core::metadata::ResponseMetadata, resources::responses::ResponseStream};

#[test]
fn interleaved_output_and_content_indices_stay_isolated() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_interleaved\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[],\"usage\":{}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"output_index\":0,\"item\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"output_index\":1,\"item\":{\"id\":\"fc_1\",\"type\":\"function_call\",\"name\":\"math\",\"call_id\":\"call_math\",\"arguments\":\"\",\"status\":\"in_progress\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"output_index\":2,\"item\":{\"id\":\"msg_2\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"item_id\":\"msg_2\",\"output_index\":2,\"content_index\":0,\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Alpha\"}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"item_id\":\"fc_1\",\"output_index\":1,\"delta\":\"{\\\"x\\\":1}\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":2,\"content_index\":0,\"delta\":\"Beta\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_interleaved\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Alpha\"}]},{\"id\":\"fc_1\",\"type\":\"function_call\",\"name\":\"math\",\"call_id\":\"call_math\",\"arguments\":\"{\\\"x\\\":1}\",\"status\":\"completed\"},{\"id\":\"msg_2\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Beta\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );

    let mut stream = ResponseStream::from_sse_chunks(metadata, vec![transcript]).expect("stream");
    for _ in 0..7 {
        stream.next_event();
    }
    let snapshot = stream.current_response().unwrap();
    assert_eq!(snapshot.output[0].content[0].text.as_deref(), Some("Alpha"));
    assert_eq!(snapshot.output[1].arguments.as_deref(), Some(""));
    assert_eq!(snapshot.output[2].content[0].text.as_deref(), Some(""));

    stream.next_event();
    let snapshot = stream.current_response().unwrap();
    assert_eq!(snapshot.output[0].content[0].text.as_deref(), Some("Alpha"));
    assert_eq!(snapshot.output[1].arguments.as_deref(), Some("{\"x\":1}"));
    assert_eq!(snapshot.output[2].content[0].text.as_deref(), Some(""));

    stream.next_event();
    let snapshot = stream.current_response().unwrap();
    assert_eq!(snapshot.output[0].content[0].text.as_deref(), Some("Alpha"));
    assert_eq!(snapshot.output[1].arguments.as_deref(), Some("{\"x\":1}"));
    assert_eq!(snapshot.output[2].content[0].text.as_deref(), Some("Beta"));
    assert_eq!(snapshot.output_text(), "AlphaBeta");
}
