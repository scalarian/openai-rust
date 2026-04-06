use std::sync::mpsc;

use futures_util::{SinkExt, StreamExt};
use openai_rust::{
    ErrorKind, OpenAI,
    realtime::{
        RealtimeAuth, RealtimeClientEvent, RealtimeConnectOptions, RealtimeConversationItem,
        RealtimeConversationMessageContentPart, RealtimeOutputModality, RealtimeServerEvent,
        RealtimeSessionConfig, RealtimeSessionType,
    },
};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[test]
fn websocket_target_builds_ws_urls_and_safe_auth_inputs() {
    let client = OpenAI::builder()
        .api_key("sk_server")
        .base_url("https://example.openai.invalid/v1")
        .build();

    let target = client
        .realtime()
        .prepare_ws_target(RealtimeConnectOptions {
            model: Some(String::from("gpt-realtime-mini")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        target.url,
        "wss://example.openai.invalid/v1/realtime?model=gpt-realtime-mini"
    );
    assert_eq!(
        target.headers.get("authorization").map(String::as_str),
        Some("Bearer sk_server")
    );

    let client_secret_target = client
        .realtime()
        .prepare_ws_target(RealtimeConnectOptions {
            call_id: Some(String::from("call_123")),
            auth: Some(RealtimeAuth::client_secret("ek_test_secret")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        client_secret_target.url,
        "wss://example.openai.invalid/v1/realtime?call_id=call_123"
    );
    assert_eq!(
        client_secret_target
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer ek_test_secret")
    );

    let missing_target = client
        .realtime()
        .prepare_ws_target(RealtimeConnectOptions::default())
        .expect_err("connecting without a model or call id should be rejected");
    assert_eq!(missing_target.kind, ErrorKind::Validation);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bootstrap_and_clean_close() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (captured_tx, captured_rx) = mpsc::channel();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();

        socket
            .send(Message::Text(
                json!({
                    "type": "session.created",
                    "event_id": "evt_created",
                    "session": {
                        "id": "sess_123",
                        "type": "realtime",
                        "model": "gpt-realtime-mini",
                        "output_modalities": ["text"]
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();

        let update = socket.next().await.unwrap().unwrap();
        let update_text = update.into_text().unwrap().to_string();
        captured_tx.send(update_text.clone()).unwrap();
        let update_json: serde_json::Value = serde_json::from_str(&update_text).unwrap();
        assert_eq!(update_json["type"], "session.update");
        assert_eq!(update_json["session"]["instructions"], "");
        assert_eq!(update_json["session"]["output_modalities"][0], "text");

        socket
            .send(Message::Text(
                json!({
                    "type": "session.updated",
                    "event_id": "evt_updated",
                    "session": {
                        "id": "sess_123",
                        "type": "realtime",
                        "model": "gpt-realtime-mini",
                        "instructions": "",
                        "output_modalities": ["text"]
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();

        let item_create = socket.next().await.unwrap().unwrap();
        let item_text = item_create.into_text().unwrap().to_string();
        captured_tx.send(item_text.clone()).unwrap();
        let item_json: serde_json::Value = serde_json::from_str(&item_text).unwrap();
        assert_eq!(item_json["type"], "conversation.item.create");
        assert_eq!(item_json["previous_item_id"], "root");
        assert_eq!(item_json["item"]["type"], "message");
        assert_eq!(item_json["item"]["role"], "user");

        socket
            .send(Message::Text(
                json!({
                    "type": "conversation.item.created",
                    "event_id": "evt_item_created",
                    "previous_item_id": "root",
                    "item": {
                        "id": "item_123",
                        "type": "message",
                        "role": "user",
                        "content": [
                            {
                                "type": "input_text",
                                "text": "Hello from the client."
                            }
                        ]
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();

        match socket.next().await.unwrap().unwrap() {
            Message::Close(_) => {}
            other => panic!("expected close frame, got {other:?}"),
        }
    });

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(format!("http://{addr}/v1"))
        .build();

    let mut connection = client
        .realtime()
        .connect(RealtimeConnectOptions {
            model: Some(String::from("gpt-realtime-mini")),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(connection.session_id(), Some("sess_123"));

    let created = connection.next_event().await.unwrap().unwrap();
    assert!(matches!(
        created,
        RealtimeServerEvent::SessionCreated { ref session, .. }
            if session.id.as_deref() == Some("sess_123")
    ));

    connection
        .send(RealtimeClientEvent::session_update(RealtimeSessionConfig {
            session_type: RealtimeSessionType::Realtime,
            instructions: Some(String::new()),
            output_modalities: Some(vec![RealtimeOutputModality::Text]),
            ..Default::default()
        }))
        .await
        .unwrap();

    let updated = connection.next_event().await.unwrap().unwrap();
    assert!(matches!(
        updated,
        RealtimeServerEvent::SessionUpdated { ref session, .. }
            if session.instructions.as_deref() == Some("")
    ));

    connection
        .send(RealtimeClientEvent::conversation_item_create(
            RealtimeConversationItem::user_message(vec![
                RealtimeConversationMessageContentPart::input_text("Hello from the client."),
            ]),
        )
        .with_previous_item_id("root"))
        .await
        .unwrap();

    let item_created = connection.next_event().await.unwrap().unwrap();
    assert!(matches!(
        item_created,
        RealtimeServerEvent::ConversationItemCreated {
            ref previous_item_id,
            ref item,
            ..
        } if previous_item_id.as_deref() == Some("root")
            && item.id.as_deref() == Some("item_123")
    ));

    connection.close().await.unwrap();
    assert!(connection.next_event().await.is_none());

    server.await.unwrap();

    let captured = captured_rx.try_iter().collect::<Vec<_>>();
    assert_eq!(captured.len(), 2);
    assert!(captured[0].contains("\"session.update\""));
    assert!(captured[1].contains("\"conversation.item.create\""));
}
