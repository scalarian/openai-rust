use openai_rust::{core::metadata::ResponseMetadata, resources::responses::ResponseStream};
use serde_json::json;

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

#[test]
fn interleaved_reasoning_text_and_tool_indices_stay_isolated() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_reasoning\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[],\"usage\":{}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"output_index\":0,\"item\":{\"id\":\"msg_reasoning\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"output_index\":1,\"item\":{\"id\":\"fc_reasoning\",\"type\":\"function_call\",\"name\":\"math\",\"call_id\":\"call_reasoning\",\"arguments\":\"\",\"status\":\"in_progress\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"output_index\":2,\"item\":{\"id\":\"msg_text\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"item_id\":\"msg_reasoning\",\"output_index\":0,\"content_index\":0,\"part\":{\"type\":\"reasoning_text\",\"text\":\"\"}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"item_id\":\"msg_reasoning\",\"output_index\":0,\"content_index\":1,\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"item_id\":\"msg_text\",\"output_index\":2,\"content_index\":0,\"part\":{\"type\":\"reasoning_text\",\"text\":\"\"}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"item_id\":\"msg_text\",\"output_index\":2,\"content_index\":1,\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
        "event: response.reasoning_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Think\"}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"item_id\":\"fc_reasoning\",\"output_index\":1,\"delta\":\"{\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":1,\"delta\":\"Answer\"}\n\n",
        "event: response.reasoning_text.delta\n",
        "data: {\"output_index\":2,\"content_index\":0,\"delta\":\"Plan\"}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"item_id\":\"fc_reasoning\",\"output_index\":1,\"delta\":\"\\\"x\\\":1}\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":2,\"content_index\":1,\"delta\":\"Beta\"}\n\n",
        "event: response.reasoning_text.done\n",
        "data: {\"output_index\":0,\"content_index\":0,\"text\":\"Thinking\"}\n\n",
        "event: response.output_text.done\n",
        "data: {\"output_index\":2,\"content_index\":1,\"text\":\"Beta!\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_reasoning\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_reasoning\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"reasoning_text\",\"text\":\"Thinking\"},{\"type\":\"output_text\",\"text\":\"Answer\"}]},{\"id\":\"fc_reasoning\",\"type\":\"function_call\",\"name\":\"math\",\"call_id\":\"call_reasoning\",\"arguments\":\"{\\\"x\\\":1}\",\"status\":\"completed\"},{\"id\":\"msg_text\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"reasoning_text\",\"text\":\"Plan\"},{\"type\":\"output_text\",\"text\":\"Beta!\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );

    let mut stream = ResponseStream::from_sse_chunks(metadata, vec![transcript]).expect("stream");
    for _ in 0..9 {
        stream.next_event();
    }
    let snapshot = stream
        .current_response()
        .expect("snapshot after first reasoning delta");
    assert_eq!(snapshot.output[0].content[0].text.as_deref(), Some("Think"));
    assert_eq!(snapshot.output[0].content[1].text.as_deref(), Some(""));
    assert_eq!(snapshot.output[1].arguments.as_deref(), Some(""));
    assert_eq!(snapshot.output[2].content[0].text.as_deref(), Some(""));
    assert_eq!(snapshot.output[2].content[1].text.as_deref(), Some(""));
    assert_eq!(snapshot.output_text(), "");

    for _ in 0..5 {
        stream.next_event();
    }
    let snapshot = stream
        .current_response()
        .expect("snapshot after interleaving");
    assert_eq!(snapshot.output[0].content[0].text.as_deref(), Some("Think"));
    assert_eq!(
        snapshot.output[0].content[1].text.as_deref(),
        Some("Answer")
    );
    assert_eq!(snapshot.output[1].arguments.as_deref(), Some("{\"x\":1}"));
    assert_eq!(snapshot.output[2].content[0].text.as_deref(), Some("Plan"));
    assert_eq!(snapshot.output[2].content[1].text.as_deref(), Some("Beta"));
    assert_eq!(snapshot.output_text(), "AnswerBeta");

    stream.next_event();
    stream.next_event();
    let snapshot = stream
        .current_response()
        .expect("snapshot after done events");
    assert_eq!(
        snapshot.output[0].content[0].text.as_deref(),
        Some("Thinking")
    );
    assert_eq!(
        snapshot.output[0].content[1].text.as_deref(),
        Some("Answer")
    );
    assert_eq!(snapshot.output[1].arguments.as_deref(), Some("{\"x\":1}"));
    assert_eq!(snapshot.output[2].content[0].text.as_deref(), Some("Plan"));
    assert_eq!(snapshot.output[2].content[1].text.as_deref(), Some("Beta!"));
    assert_eq!(snapshot.output_text(), "AnswerBeta!");
}

#[test]
fn multimodal_content_parts_remain_isolated_until_completion() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_multimodal\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[],\"usage\":{}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"output_index\":0,\"item\":{\"id\":\"msg_audio\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"output_index\":1,\"item\":{\"id\":\"msg_text\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"item_id\":\"msg_audio\",\"output_index\":0,\"content_index\":0,\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"item_id\":\"msg_audio\",\"output_index\":0,\"content_index\":1,\"part\":{\"type\":\"output_audio\",\"audio\":{\"id\":\"aud_1\",\"data\":\"pcm-chunk-1\",\"transcript\":\"Hello\",\"format\":\"wav\"}}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"item_id\":\"msg_text\",\"output_index\":1,\"content_index\":0,\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Alpha\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":1,\"content_index\":0,\"delta\":\"Beta\"}\n\n",
        "event: response.content_part.done\n",
        "data: {\"item_id\":\"msg_audio\",\"output_index\":0,\"content_index\":1,\"part\":{\"type\":\"output_audio\",\"audio\":{\"id\":\"aud_1\",\"data\":\"pcm-final\",\"transcript\":\"Hello there\",\"format\":\"wav\"},\"sequence\":2}}\n\n",
        "event: response.output_text.done\n",
        "data: {\"output_index\":1,\"content_index\":0,\"text\":\"Beta!\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_multimodal\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_audio\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Alpha\"},{\"type\":\"output_audio\",\"audio\":{\"id\":\"aud_1\",\"data\":\"pcm-final\",\"transcript\":\"Hello there\",\"format\":\"wav\"},\"sequence\":2}]},{\"id\":\"msg_text\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Beta!\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );

    let mut stream = ResponseStream::from_sse_chunks(metadata, vec![transcript]).expect("stream");
    for _ in 0..8 {
        stream.next_event();
    }
    let snapshot = stream
        .current_response()
        .expect("snapshot before audio completion");
    assert_eq!(snapshot.output_text(), "AlphaBeta");
    assert_eq!(snapshot.output[0].content[0].text.as_deref(), Some("Alpha"));
    assert_eq!(snapshot.output[1].content[0].text.as_deref(), Some("Beta"));
    assert_eq!(snapshot.output[0].content[1].content_type, "output_audio");
    assert_eq!(
        snapshot.output[0].content[1].extra.get("audio"),
        Some(&json!({
            "id": "aud_1",
            "data": "pcm-chunk-1",
            "transcript": "Hello",
            "format": "wav"
        }))
    );

    stream.next_event();
    let snapshot = stream
        .current_response()
        .expect("snapshot after audio completion");
    assert_eq!(snapshot.output_text(), "AlphaBeta");
    assert_eq!(snapshot.output[0].content[0].text.as_deref(), Some("Alpha"));
    assert_eq!(snapshot.output[1].content[0].text.as_deref(), Some("Beta"));
    assert_eq!(snapshot.output[0].content[1].content_type, "output_audio");
    assert_eq!(
        snapshot.output[0].content[1].extra.get("audio"),
        Some(&json!({
            "id": "aud_1",
            "data": "pcm-final",
            "transcript": "Hello there",
            "format": "wav"
        }))
    );
    assert_eq!(
        snapshot.output[0].content[1].extra.get("sequence"),
        Some(&json!(2))
    );

    stream.next_event();
    let snapshot = stream
        .current_response()
        .expect("snapshot after sibling text completion");
    assert_eq!(snapshot.output_text(), "AlphaBeta!");
    assert_eq!(snapshot.output[0].content[0].text.as_deref(), Some("Alpha"));
    assert_eq!(snapshot.output[1].content[0].text.as_deref(), Some("Beta!"));
    assert_eq!(
        snapshot.output[0].content[1].extra.get("audio"),
        Some(&json!({
            "id": "aud_1",
            "data": "pcm-final",
            "transcript": "Hello there",
            "format": "wav"
        }))
    );
    assert_eq!(
        snapshot.output[0].content[1].extra.get("sequence"),
        Some(&json!(2))
    );
}
