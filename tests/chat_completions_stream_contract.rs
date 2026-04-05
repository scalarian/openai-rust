use openai_rust::{
    ErrorKind, OpenAI,
    core::metadata::ResponseMetadata,
    resources::chat::{ChatCompletionChunk, ChatCompletionCreateParams, ChatCompletionStream},
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
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

#[test]
fn done_without_terminal_chunk_is_rejected() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let error = ChatCompletionStream::from_sse_chunks(
        metadata,
        [concat!(
            r#"data: {"id":"chatcmpl_stream","object":"chat.completion.chunk","created":1,"model":"gpt-4.1-mini","choices":[{"index":0,"delta":{"role":"assistant","content":"partial"}}]}"#,
            "\n\n",
            "data: [DONE]\n\n"
        )],
    )
    .expect_err("[DONE]-only transcript should fail");
    assert_eq!(error.kind, ErrorKind::Parse);
}

#[test]
fn stream_yields_incremental_chunks_before_terminal_tail_arrives() {
    let server = IncrementalSseServer::spawn(
        concat!(
            "data: {\"id\":\"chatcmpl_stream\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-4.1-mini\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hel\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl_stream\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-4.1-mini\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"}}]}\n\n",
        ),
        concat!(
            "data: {\"id\":\"chatcmpl_stream\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-4.1-mini\",\"choices\":[{\"index\":0,\"delta\":{\"function_call\":{\"name\":\"lookup_weather\",\"arguments\":\"{\\\"city\\\":\\\"Pa\"},\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"lookup_weather\",\"arguments\":\"{\\\"city\\\":\\\"Pa\"}}]}}]}\n\n",
            "data: {\"id\":\"chatcmpl_stream\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-4.1-mini\",\"choices\":[{\"index\":0,\"delta\":{\"function_call\":{\"arguments\":\"ris\\\"}\"},\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"ris\\\"}\"}}]}}]}\n\n",
            "data: {\"id\":\"chatcmpl_stream\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-4.1-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n"
        ),
        Duration::from_millis(400),
    )
    .expect("incremental chat server");
    let client = OpenAI::builder()
        .api_key("sk-test")
        .base_url(server.url())
        .build();

    let started = Instant::now();
    let mut stream = client
        .chat()
        .completions()
        .stream(ChatCompletionCreateParams {
            model: String::from("gpt-4.1-mini"),
            messages: vec![json!({"role": "user", "content": "hello"})],
            ..Default::default()
        })
        .expect("stream should start");

    assert!(matches!(
        stream.next_chunk(),
        Some(ChatCompletionChunk { choices, .. })
            if choices.first().and_then(|choice| choice.delta.content.as_deref()) == Some("Hel")
    ));
    assert!(matches!(
        stream.next_chunk(),
        Some(ChatCompletionChunk { choices, .. })
            if choices.first().and_then(|choice| choice.delta.content.as_deref()) == Some("lo")
    ));
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "expected early chunks before delayed tail, got {:?}",
        started.elapsed()
    );

    let final_message = stream.final_message(0).expect("terminal message");
    assert_eq!(final_message.content.as_deref(), Some("Hello"));
    assert_eq!(
        final_message
            .function_call
            .as_ref()
            .and_then(|call| call.arguments.as_deref()),
        Some(r#"{"city":"Paris"}"#)
    );
    assert_eq!(
        final_message.tool_calls[0].function.arguments.as_deref(),
        Some(r#"{"city":"Paris"}"#)
    );
}

struct IncrementalSseServer {
    addr: std::net::SocketAddr,
    worker: Option<thread::JoinHandle<()>>,
}

impl IncrementalSseServer {
    fn spawn(
        first_chunk: &'static str,
        second_chunk: &'static str,
        delay: Duration,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let worker = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = read_request_headers(&mut stream);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
                );
                let _ = stream.write_all(first_chunk.as_bytes());
                let _ = stream.flush();
                thread::sleep(delay);
                let _ = stream.write_all(second_chunk.as_bytes());
                let _ = stream.flush();
                let _ = stream.shutdown(Shutdown::Both);
            }
        });
        Ok(Self {
            addr,
            worker: Some(worker),
        })
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for IncrementalSseServer {
    fn drop(&mut self) {
        let _ = TcpStream::connect(self.addr);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_request_headers(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0_u8; 1024];
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(());
        }
    }
}
