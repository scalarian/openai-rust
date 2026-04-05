use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

use openai_rust::{ErrorKind, OpenAI};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn images_generate_and_edit_stream_surface_partial_and_completed_events() {
    let generation_stream_body = concat!(
        "event: image_generation.partial_image\n",
        "data: {\"type\":\"image_generation.partial_image\",\"partial_image_index\":0,\"b64_json\":\"cGFydGlhbA==\",\"created_at\":1717171717,\"background\":\"transparent\",\"output_format\":\"png\",\"quality\":\"high\",\"size\":\"1024x1024\"}\n\n",
        "event: image_generation.completed\n",
        "data: {\"type\":\"image_generation.completed\",\"b64_json\":\"ZmluYWw=\",\"created_at\":1717171718,\"background\":\"transparent\",\"output_format\":\"png\",\"quality\":\"high\",\"size\":\"1024x1024\",\"usage\":{\"input_tokens\":4,\"input_tokens_details\":{\"text_tokens\":2,\"image_tokens\":2},\"output_tokens\":16,\"total_tokens\":20}}\n\n",
        "data: [DONE]\n\n"
    );
    let edit_stream_body = concat!(
        "event: image_edit.partial_image\n",
        "data: {\"type\":\"image_edit.partial_image\",\"partial_image_index\":1,\"b64_json\":\"ZWRpdC1wYXJ0aWFs\",\"created_at\":1818181818,\"background\":\"opaque\",\"output_format\":\"jpeg\",\"quality\":\"medium\",\"size\":\"1024x1536\"}\n\n",
        "event: image_edit.completed\n",
        "data: {\"type\":\"image_edit.completed\",\"b64_json\":\"ZWRpdC1maW5hbA==\",\"created_at\":1818181819,\"background\":\"opaque\",\"output_format\":\"jpeg\",\"quality\":\"medium\",\"size\":\"1024x1536\",\"usage\":{\"input_tokens\":5,\"input_tokens_details\":{\"text_tokens\":3,\"image_tokens\":2},\"output_tokens\":18,\"total_tokens\":23}}\n\n",
        "data: [DONE]\n\n"
    );

    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        sse_response(generation_stream_body),
        sse_response(edit_stream_body),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let mut generation_stream = client
        .images()
        .generate_stream(openai_rust::resources::images::ImageGenerateParams {
            prompt: String::from("Render a paper lantern"),
            model: Some(String::from("gpt-image-1")),
            partial_images: Some(1),
            output_format: Some(String::from("png")),
            ..Default::default()
        })
        .unwrap();
    let first_generation_event = generation_stream.next_event().expect("partial event");
    match first_generation_event {
        openai_rust::resources::images::ImageGenerationStreamEvent::PartialImage(event) => {
            assert_eq!(event.partial_image_index, 0);
            assert_eq!(event.b64_json, "cGFydGlhbA==");
        }
        _ => panic!("expected generation partial image event"),
    }
    match generation_stream.next_event().expect("completed event") {
        openai_rust::resources::images::ImageGenerationStreamEvent::Completed(event) => {
            assert_eq!(event.b64_json, "ZmluYWw=");
            assert_eq!(event.usage.total_tokens, 20);
        }
        _ => panic!("expected generation completed event"),
    }
    assert!(generation_stream.next_event().is_none());
    assert_eq!(
        generation_stream.final_completed().unwrap().b64_json,
        "ZmluYWw="
    );

    let mut edit_stream = client
        .images()
        .edit_stream(openai_rust::resources::images::ImageEditParams {
            images: vec![openai_rust::resources::images::ImageInput::new(
                "edit-source.png",
                "image/png",
                vec![1, 3, 3, 7],
            )],
            prompt: String::from("Add soft reflections"),
            partial_images: Some(2),
            output_format: Some(String::from("jpeg")),
            ..Default::default()
        })
        .unwrap();
    match edit_stream.next_event().expect("edit partial event") {
        openai_rust::resources::images::ImageEditStreamEvent::PartialImage(event) => {
            assert_eq!(event.partial_image_index, 1);
            assert_eq!(event.output_format, "jpeg");
        }
        _ => panic!("expected edit partial image event"),
    }
    match edit_stream.next_event().expect("edit completed event") {
        openai_rust::resources::images::ImageEditStreamEvent::Completed(event) => {
            assert_eq!(event.b64_json, "ZWRpdC1maW5hbA==");
            assert_eq!(event.usage.output_tokens, 18);
        }
        _ => panic!("expected edit completed event"),
    }
    assert!(edit_stream.next_event().is_none());
    assert_eq!(
        edit_stream.final_completed().unwrap().b64_json,
        "ZWRpdC1maW5hbA=="
    );

    let requests = server.captured_requests(2).expect("captured requests");
    let generation_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(generation_body["stream"], true);
    assert_eq!(generation_body["partial_images"], 1);
    assert_eq!(generation_body["output_format"], "png");

    let content_type = requests[1]
        .headers
        .get("content-type")
        .expect("content-type");
    assert!(content_type.starts_with("multipart/form-data; boundary="));
}

#[test]
fn image_stream_requires_terminal_completed_event() {
    let body = concat!(
        "event: image_generation.partial_image\n",
        "data: {\"type\":\"image_generation.partial_image\",\"partial_image_index\":0,\"b64_json\":\"cGFydGlhbA==\",\"created_at\":1717171717,\"background\":\"transparent\",\"output_format\":\"png\",\"quality\":\"high\",\"size\":\"1024x1024\"}\n\n",
        "data: [DONE]\n\n"
    );

    let server = mock_http::MockHttpServer::spawn(sse_response(body)).unwrap();
    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let mut stream = client
        .images()
        .generate_stream(openai_rust::resources::images::ImageGenerateParams {
            prompt: String::from("Render a paper lantern"),
            ..Default::default()
        })
        .expect("stream should start before terminal validation");

    assert!(matches!(
        stream.next_event(),
        Some(openai_rust::resources::images::ImageGenerationStreamEvent::PartialImage(_))
    ));
    assert!(stream.next_event().is_none());
    let error = stream
        .final_completed()
        .expect_err("missing terminal event should fail");

    assert!(matches!(error.kind, ErrorKind::Parse));
    assert!(error.to_string().contains("terminal completed event"));
}

#[test]
fn image_stream_rejects_eof_truncated_transcripts_without_completed_event() {
    let body = concat!(
        "event: image_generation.partial_image\n",
        "data: {\"type\":\"image_generation.partial_image\",\"partial_image_index\":0,\"b64_json\":\"cGFydGlhbA==\",\"created_at\":1717171717,\"background\":\"transparent\",\"output_format\":\"png\",\"quality\":\"high\",\"size\":\"1024x1024\"}\n\n"
    );

    let server = mock_http::MockHttpServer::spawn(sse_response(body)).unwrap();
    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let mut stream = client
        .images()
        .generate_stream(openai_rust::resources::images::ImageGenerateParams {
            prompt: String::from("Render a paper lantern"),
            ..Default::default()
        })
        .expect("stream should start before EOF validation");

    assert!(matches!(
        stream.next_event(),
        Some(openai_rust::resources::images::ImageGenerationStreamEvent::PartialImage(_))
    ));
    assert!(stream.next_event().is_none());
    let error = stream
        .final_completed()
        .expect_err("EOF-truncated stream should fail");

    assert!(matches!(error.kind, ErrorKind::Parse));
    assert!(error.to_string().contains("terminal completed event"));
}

#[test]
fn image_stream_yields_incremental_generation_events_before_terminal_tail_arrives() {
    let server = IncrementalSseServer::spawn(
        concat!(
            "event: image_generation.partial_image\n",
            "data: {\"type\":\"image_generation.partial_image\",\"partial_image_index\":0,\"b64_json\":\"cGFydGlhbA==\",\"created_at\":1717171717,\"background\":\"transparent\",\"output_format\":\"png\",\"quality\":\"high\",\"size\":\"1024x1024\"}\n\n"
        ),
        concat!(
            "event: image_generation.completed\n",
            "data: {\"type\":\"image_generation.completed\",\"b64_json\":\"ZmluYWw=\",\"created_at\":1717171718,\"background\":\"transparent\",\"output_format\":\"png\",\"quality\":\"high\",\"size\":\"1024x1024\",\"usage\":{\"input_tokens\":4,\"input_tokens_details\":{\"text_tokens\":2,\"image_tokens\":2},\"output_tokens\":16,\"total_tokens\":20}}\n\n",
            "data: [DONE]\n\n"
        ),
        Duration::from_millis(400),
    )
    .expect("incremental image server");
    let client = OpenAI::builder()
        .api_key("sk-test")
        .base_url(server.url())
        .build();

    let started = Instant::now();
    let mut stream = client
        .images()
        .generate_stream(openai_rust::resources::images::ImageGenerateParams {
            prompt: String::from("Render a paper lantern"),
            partial_images: Some(1),
            ..Default::default()
        })
        .expect("stream should start");

    assert!(matches!(
        stream.next_event(),
        Some(openai_rust::resources::images::ImageGenerationStreamEvent::PartialImage(ref event))
            if event.b64_json == "cGFydGlhbA=="
    ));
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "expected partial image before delayed tail, got {:?}",
        started.elapsed()
    );

    assert!(matches!(
        stream.next_event(),
        Some(openai_rust::resources::images::ImageGenerationStreamEvent::Completed(ref event))
            if event.b64_json == "ZmluYWw="
    ));
    assert_eq!(stream.final_completed().unwrap().usage.total_tokens, 20);
}

#[test]
fn image_edit_stream_rejects_eof_truncated_transcripts_without_completed_event() {
    let body = concat!(
        "event: image_edit.partial_image\n",
        "data: {\"type\":\"image_edit.partial_image\",\"partial_image_index\":1,\"b64_json\":\"ZWRpdC1wYXJ0aWFs\",\"created_at\":1818181818,\"background\":\"opaque\",\"output_format\":\"jpeg\",\"quality\":\"medium\",\"size\":\"1024x1536\"}\n\n"
    );

    let server = mock_http::MockHttpServer::spawn(sse_response(body)).unwrap();
    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let mut stream = client
        .images()
        .edit_stream(openai_rust::resources::images::ImageEditParams {
            images: vec![openai_rust::resources::images::ImageInput::new(
                "edit-source.png",
                "image/png",
                vec![1, 3, 3, 7],
            )],
            prompt: String::from("Add soft reflections"),
            partial_images: Some(2),
            output_format: Some(String::from("jpeg")),
            ..Default::default()
        })
        .expect("stream should start before EOF validation");

    assert!(matches!(
        stream.next_event(),
        Some(openai_rust::resources::images::ImageEditStreamEvent::PartialImage(_))
    ));
    assert!(stream.next_event().is_none());
    let error = stream
        .final_completed()
        .expect_err("EOF-truncated edit stream should fail");

    assert!(matches!(error.kind, ErrorKind::Parse));
    assert!(error.to_string().contains("terminal completed event"));
}

fn sse_response(body: &str) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("text/event-stream"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body: body.as_bytes().to_vec(),
        ..Default::default()
    }
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
