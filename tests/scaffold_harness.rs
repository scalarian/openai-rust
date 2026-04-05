use std::io::Write;
use std::net::{Shutdown, TcpStream};

use openai_rust::OpenAI;

mod support;

#[test]
fn client_scaffold_exposes_primary_resource_families() {
    let client = OpenAI::new();

    let _ = client.responses();
    let _ = client.conversations();
    let _ = client.chat();
    let _ = client.images();
    let _ = client.audio();
    let _ = client.files();
    let _ = client.uploads();
    let _ = client.vector_stores();
    let _ = client.batches();
    let _ = client.webhooks();
    let _ = client.fine_tuning();
    let _ = client.evals();
    let _ = client.containers();
    let _ = client.skills();
    let _ = client.videos();
    let _ = client.realtime();
}

#[test]
fn mock_http_harness_captures_request_bytes() {
    let server =
        support::mock_http::MockHttpServer::spawn(support::mock_http::ScriptedResponse::default())
            .unwrap();

    let mut stream = TcpStream::connect(server.url().trim_start_matches("http://")).unwrap();
    stream
        .write_all(
            b"POST /v1/files HTTP/1.1\r\nHost: localhost\r\nContent-Length: 7\r\n\r\npayload",
        )
        .unwrap();
    stream.shutdown(Shutdown::Write).unwrap();

    let captured = server.captured_request().unwrap();
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/v1/files");
    assert_eq!(captured.body, b"payload");
}

#[test]
fn sse_helpers_preserve_event_payloads_across_fragmentation() {
    let transcript = support::sse::SseTranscript::from_events(vec![
        support::sse::SseEvent::named("response.created").json(r#"{"id":"resp_123"}"#),
        support::sse::SseEvent::named("response.completed").json(r#"{"id":"resp_123"}"#),
    ]);

    let encoded = transcript.encode();
    let fragments = transcript.fragment(&[5, 3, 8]);

    assert_eq!(fragments.concat(), encoded.as_bytes());
    assert!(encoded.contains("response.created"));
    assert!(encoded.contains("response.completed"));
}

#[test]
fn multipart_helper_extracts_named_parts() {
    let body = concat!(
        "--boundary\r\n",
        "Content-Disposition: form-data; name=\"purpose\"\r\n\r\n",
        "responses\r\n",
        "--boundary\r\n",
        "Content-Disposition: form-data; name=\"file\"; filename=\"input.jsonl\"\r\n",
        "Content-Type: application/jsonl\r\n\r\n",
        "{}\n",
        "--boundary--\r\n"
    );

    let multipart = support::multipart::parse_multipart(body.as_bytes(), "boundary").unwrap();

    assert_eq!(multipart.parts.len(), 2);
    assert_eq!(multipart.parts[0].name.as_deref(), Some("purpose"));
    assert_eq!(multipart.parts[1].filename.as_deref(), Some("input.jsonl"));
}

#[test]
fn multipart_binary_parts_round_trip() {
    let mut body = Vec::new();
    body.extend_from_slice(b"--boundary\r\n");
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"blob.bin\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");

    let binary = [0_u8, 159, 255, b'\r', b'\n', b'-', b'-', b'x'];
    body.extend_from_slice(&binary);
    body.extend_from_slice(b"\r\n--boundary--\r\n");

    let multipart = support::multipart::parse_multipart(&body, "boundary").unwrap();

    assert_eq!(multipart.parts.len(), 1);
    assert_eq!(multipart.parts[0].filename.as_deref(), Some("blob.bin"));
    assert_eq!(multipart.parts[0].body, binary);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_harness_records_bidirectional_frames() {
    let server =
        support::websocket::LocalWebSocketHarness::spawn(vec![String::from("server->client")])
            .await
            .unwrap();

    let transcript = server
        .drive_text_session(vec![String::from("client->server")])
        .await
        .unwrap();

    assert_eq!(transcript.client_to_server, vec!["client->server"]);
    assert_eq!(transcript.server_to_client, vec!["server->client"]);
}
