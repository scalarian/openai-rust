#![allow(clippy::result_large_err)]

use std::{collections::BTreeMap, sync::mpsc};

use futures_util::{SinkExt, StreamExt};
use openai_rust::{
    OpenAI,
    resources::responses::{ResponsesConnectOptions, ResponsesConnectionEvent},
};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        Message,
        handshake::server::{Request, Response},
    },
};

#[test]
fn responses_websocket_target_preserves_upstream_path_query_and_auth_headers() {
    let client = OpenAI::builder()
        .api_key("sk-test")
        .base_url("https://example.openai.invalid/v1")
        .organization("org_ws")
        .project("proj_ws")
        .user_agent("ws-test/1.0")
        .build();

    let target = client
        .responses()
        .prepare_ws_target(
            ResponsesConnectOptions::new()
                .query("conversation", "conv_123")
                .query("include[]", "response.output_text.delta")
                .header("X-Trace-Id", "trace_123"),
        )
        .expect("prepared websocket target");

    assert_eq!(
        target.url,
        "wss://example.openai.invalid/v1/responses?conversation=conv_123&include%5B%5D=response.output_text.delta"
    );
    assert_eq!(
        target.headers.get("authorization").map(String::as_str),
        Some("Bearer sk-test")
    );
    assert_eq!(
        target
            .headers
            .get("openai-organization")
            .map(String::as_str),
        Some("org_ws")
    );
    assert_eq!(
        target.headers.get("openai-project").map(String::as_str),
        Some("proj_ws")
    );
    assert_eq!(
        target.headers.get("user-agent").map(String::as_str),
        Some(concat!(
            "ws-test/1.0 ",
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
    );
    assert_eq!(
        target.headers.get("x-trace-id").map(String::as_str),
        Some("trace_123")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_can_exchange_flexible_json_events() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (request_tx, request_rx) = mpsc::channel();
    let (message_tx, message_rx) = mpsc::channel();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let callback = |request: &Request, response: Response| {
            let headers = request
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    Some((
                        name.as_str().to_ascii_lowercase(),
                        value.to_str().ok()?.to_string(),
                    ))
                })
                .collect::<BTreeMap<_, _>>();
            request_tx
                .send((request.uri().to_string(), headers))
                .unwrap();
            Ok(response)
        };
        let mut socket = accept_hdr_async(stream, callback).await.unwrap();
        socket
            .send(Message::Text(
                json!({
                    "type": "response.created",
                    "response": {
                        "id": "resp_ws",
                        "status": "in_progress"
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();

        let client_message = socket.next().await.unwrap().unwrap();
        message_tx
            .send(client_message.into_text().unwrap().to_string())
            .unwrap();
        socket.close(None).await.unwrap();
    });

    let client = OpenAI::builder()
        .api_key("sk-test")
        .base_url(format!("http://{addr}/v1"))
        .build();
    let mut connection = client
        .responses()
        .connect(ResponsesConnectOptions::new().query("trace", "yes"))
        .await
        .expect("responses websocket connection");

    let event = connection
        .recv()
        .await
        .expect("server event")
        .expect("parsed event");
    assert_eq!(
        event,
        ResponsesConnectionEvent {
            event_type: Some(String::from("response.created")),
            payload: json!({
                "type": "response.created",
                "response": {
                    "id": "resp_ws",
                    "status": "in_progress"
                }
            }),
        }
    );

    connection
        .send(json!({
            "type": "response.create",
            "response": {
                "model": "gpt-5.5",
                "input": "Say hello"
            }
        }))
        .await
        .expect("send response.create event");
    assert!(connection.recv().await.is_none());
    connection.close().await.expect("idempotent close");

    let (request_path, request_headers) = request_rx.recv().unwrap();
    assert_eq!(request_path, "/v1/responses?trace=yes");
    assert_eq!(
        request_headers.get("authorization").map(String::as_str),
        Some("Bearer sk-test")
    );
    assert_eq!(
        message_rx.recv().unwrap(),
        json!({
            "type": "response.create",
            "response": {
                "model": "gpt-5.5",
                "input": "Say hello"
            }
        })
        .to_string()
    );

    server.await.unwrap();
}
