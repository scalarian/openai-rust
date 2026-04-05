use openai_rust::{
    ErrorKind,
    core::metadata::ResponseMetadata,
    resources::chat::{ChatCompletionChunk, ChatCompletionStream},
};

#[test]
fn compatibility_stream_accumulates_legacy_function_and_tool_call_arguments() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let transcript = vec![
        concat!(
            r#"data: {"id":"chatcmpl_stream","object":"chat.completion.chunk","created":1,"model":"gpt-4.1-mini","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel","function_call":{"name":"lookup_weather","arguments":"{\"city\":\"Pa"},"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"lookup_weather","arguments":"{\"city\":\"Pa"}}]}}]}"#,
            "\n\n",
            r#"data: {"id":"chatcmpl_stream","object":"chat.completion.chunk","created":1,"model":"gpt-4.1-mini","choices":[{"index":0,"delta":{"content":"lo","function_call":{"arguments":"ris\"}"},"tool_calls":[{"index":0,"function":{"arguments":"ris\"}"}}]}}]}"#,
            "\n\n",
        ),
        concat!(
            "data: {\"id\":\"chatcmpl_stream\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-4.1-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n"
        ),
    ];

    let mut stream = ChatCompletionStream::from_sse_chunks(metadata, transcript)
        .expect("compatibility transcript should parse");

    assert!(matches!(
        stream.next_chunk(),
        Some(ChatCompletionChunk { .. })
    ));
    assert!(matches!(
        stream.next_chunk(),
        Some(ChatCompletionChunk { .. })
    ));
    assert!(matches!(
        stream.next_chunk(),
        Some(ChatCompletionChunk { .. })
    ));
    assert!(stream.next_chunk().is_none());

    let final_message = stream.final_message(0).expect("final message snapshot");
    assert_eq!(final_message.role.as_deref(), Some("assistant"));
    assert_eq!(final_message.content.as_deref(), Some("Hello"));
    assert_eq!(
        final_message
            .function_call
            .as_ref()
            .and_then(|call| call.name.as_deref()),
        Some("lookup_weather")
    );
    assert_eq!(
        final_message
            .function_call
            .as_ref()
            .and_then(|call| call.arguments.as_deref()),
        Some(r#"{"city":"Paris"}"#)
    );
    assert_eq!(final_message.tool_calls.len(), 1);
    assert_eq!(final_message.tool_calls[0].index, Some(0));
    assert_eq!(
        final_message.tool_calls[0].function.arguments.as_deref(),
        Some(r#"{"city":"Paris"}"#)
    );
}

#[test]
fn stream_requires_done_or_terminal_chunk() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let error = ChatCompletionStream::from_sse_chunks(
        metadata,
        [r#"data: {"id":"chatcmpl_stream","object":"chat.completion.chunk","created":1,"model":"gpt-4.1-mini","choices":[{"index":0,"delta":{"content":"partial"}}]}

"#],
    )
    .expect_err("missing done marker should fail");
    assert_eq!(error.kind, ErrorKind::Parse);
}
