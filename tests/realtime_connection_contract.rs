use std::sync::{Arc, Mutex, mpsc};

use futures_util::{SinkExt, StreamExt};
use openai_rust::{
    ErrorKind, OpenAI,
    realtime::{
        RealtimeAuth, RealtimeClientEvent, RealtimeConnectOptions, RealtimeConversationItem,
        RealtimeConversationMessageContentPart, RealtimeFunctionTool, RealtimeMaxOutputTokens,
        RealtimeMcpAllowedTools, RealtimeMcpRequireApproval, RealtimeMcpTool,
        RealtimeOutputModality, RealtimeReasoning, RealtimeReasoningEffort,
        RealtimeResponseCreateParams, RealtimeServerEvent, RealtimeSessionConfig,
        RealtimeSessionType, RealtimeTool, RealtimeToolChoice, ResponsePrompt,
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

    let consumer_client = OpenAI::builder()
        .base_url("https://example.openai.invalid/v1")
        .organization("org_consumer")
        .project("proj_consumer")
        .user_agent("consumer-app/1.0")
        .build();
    let consumer_target = consumer_client
        .realtime()
        .prepare_ws_target(RealtimeConnectOptions {
            model: Some(String::from("gpt-realtime-mini")),
            auth: Some(RealtimeAuth::client_secret("ek_consumer_secret")),
            ..Default::default()
        })
        .expect("explicit client-secret auth should not require a server API key");
    assert_eq!(
        consumer_target.url,
        "wss://example.openai.invalid/v1/realtime?model=gpt-realtime-mini"
    );
    assert_eq!(
        consumer_target
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer ek_consumer_secret")
    );
    assert_eq!(
        consumer_target
            .headers
            .get("openai-organization")
            .map(String::as_str),
        Some("org_consumer")
    );
    assert_eq!(
        consumer_target
            .headers
            .get("openai-project")
            .map(String::as_str),
        Some("proj_consumer")
    );
    assert_eq!(
        consumer_target
            .headers
            .get("user-agent")
            .map(String::as_str),
        Some(concat!(
            "consumer-app/1.0 ",
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
    );

    let default_auth_requires_server_key = consumer_client
        .realtime()
        .prepare_ws_target(RealtimeConnectOptions {
            model: Some(String::from("gpt-realtime-mini")),
            ..Default::default()
        })
        .expect_err("default websocket auth should still require a server API key");
    assert_eq!(
        default_auth_requires_server_key.kind,
        ErrorKind::Configuration
    );

    let missing_target = client
        .realtime()
        .prepare_ws_target(RealtimeConnectOptions::default())
        .expect_err("connecting without a model or call id should be rejected");
    assert_eq!(missing_target.kind, ErrorKind::Validation);
}

#[test]
fn client_event_enum_serializes_all_upstream_event_types() {
    let events = vec![
        RealtimeClientEvent::session_update(RealtimeSessionConfig {
            instructions: Some(String::from("Be direct.")),
            ..Default::default()
        })
        .with_event_id("evt_update"),
        RealtimeClientEvent::response_create(Some(json!({"metadata": {"source": "enum"}}))),
        RealtimeClientEvent::response_cancel(Some(String::from("resp_123"))),
        RealtimeClientEvent::input_audio_buffer_append("AQID"),
        RealtimeClientEvent::input_audio_buffer_commit(),
        RealtimeClientEvent::input_audio_buffer_clear(),
        RealtimeClientEvent::conversation_item_create(RealtimeConversationItem::user_message(
            vec![RealtimeConversationMessageContentPart::input_text("Hello")],
        ))
        .with_previous_item_id("root"),
        RealtimeClientEvent::conversation_item_truncate("item_assistant", 0, 240),
        RealtimeClientEvent::conversation_item_retrieve("item_user"),
        RealtimeClientEvent::conversation_item_delete("item_user"),
        RealtimeClientEvent::output_audio_buffer_clear(),
    ];
    let serialized = events
        .iter()
        .map(RealtimeClientEvent::to_json_value)
        .collect::<Vec<_>>();
    let event_types = serialized
        .iter()
        .map(|event| event["type"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(
        event_types,
        vec![
            "session.update",
            "response.create",
            "response.cancel",
            "input_audio_buffer.append",
            "input_audio_buffer.commit",
            "input_audio_buffer.clear",
            "conversation.item.create",
            "conversation.item.truncate",
            "conversation.item.retrieve",
            "conversation.item.delete",
            "output_audio_buffer.clear",
        ]
    );
    assert_eq!(serialized[0]["event_id"], "evt_update");
    assert_eq!(serialized[1]["response"]["metadata"]["source"], "enum");
    assert_eq!(serialized[2]["response_id"], "resp_123");
    assert_eq!(serialized[3]["audio"], "AQID");
    assert_eq!(serialized[6]["previous_item_id"], "root");
    assert_eq!(serialized[7]["audio_end_ms"], 240);
    assert_eq!(serialized[8]["item_id"], "item_user");
    assert_eq!(serialized[9]["item_id"], "item_user");
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
        .send(
            RealtimeClientEvent::conversation_item_create(RealtimeConversationItem::user_message(
                vec![RealtimeConversationMessageContentPart::input_text(
                    "Hello from the client.",
                )],
            ))
            .with_previous_item_id("root"),
        )
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upstream_raw_realtime_aliases_round_trip() {
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
                    "event_id": "evt_raw_created",
                    "session": {
                        "id": "sess_raw",
                        "type": "realtime",
                        "model": "gpt-realtime-mini"
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();

        let raw = socket.next().await.unwrap().unwrap();
        captured_tx
            .send(raw.into_text().unwrap().to_string())
            .unwrap();

        socket
            .send(Message::Text(
                json!({
                    "type": "session.updated",
                    "event_id": "evt_raw_updated",
                    "session": {
                        "id": "sess_raw",
                        "type": "realtime",
                        "model": "gpt-realtime-mini",
                        "instructions": "raw"
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

    let bootstrap_bytes = connection.recv_bytes().await.unwrap().unwrap();
    let bootstrap = String::from_utf8(bootstrap_bytes).unwrap();
    assert!(bootstrap.contains("\"session.created\""));

    connection
        .send_raw(r#"{"type":"session.update","session":{"type":"realtime","instructions":"raw"}}"#)
        .await
        .unwrap();

    let updated = connection.recv().await.unwrap().unwrap();
    assert!(matches!(
        updated,
        RealtimeServerEvent::SessionUpdated { ref session, .. }
            if session.instructions.as_deref() == Some("raw")
    ));

    connection.close().await.unwrap();
    server.await.unwrap();

    let captured = captured_rx.recv().unwrap();
    assert_eq!(
        captured,
        r#"{"type":"session.update","session":{"type":"realtime","instructions":"raw"}}"#
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connection_convenience_resources_emit_upstream_client_events() {
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
                        "id": "sess_helpers",
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

        for _ in 0..11 {
            let message = socket.next().await.unwrap().unwrap();
            let text = message.into_text().unwrap().to_string();
            captured_tx.send(text).unwrap();
        }

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
    let bootstrap = connection.next_event().await.unwrap().unwrap();
    assert!(matches!(
        bootstrap,
        RealtimeServerEvent::SessionCreated { ref session, .. }
            if session.id.as_deref() == Some("sess_helpers")
    ));

    connection
        .session()
        .update(
            RealtimeSessionConfig {
                instructions: Some(String::from("Be direct.")),
                ..Default::default()
            },
            Some(String::from("evt_update")),
        )
        .await
        .unwrap();
    connection
        .response()
        .create_params(
            RealtimeResponseCreateParams {
                max_output_tokens: Some(RealtimeMaxOutputTokens::Inf),
                metadata: Some(json!({"source": "test"})),
                output_modalities: Some(vec![RealtimeOutputModality::Text]),
                prompt: Some(ResponsePrompt {
                    id: String::from("pmpt_response"),
                    variables: Some([(String::from("topic"), json!("response"))].into()),
                    ..Default::default()
                }),
                reasoning: Some(RealtimeReasoning {
                    effort: Some(RealtimeReasoningEffort::Minimal),
                    ..Default::default()
                }),
                tool_choice: Some(RealtimeToolChoice::function("lookup_weather")),
                tools: Some(vec![
                    RealtimeTool::Function(Box::new(RealtimeFunctionTool {
                        name: Some(String::from("lookup_weather")),
                        description: Some(String::from("Look up weather.")),
                        parameters: Some(json!({"type": "object"})),
                        ..Default::default()
                    })),
                    RealtimeTool::Mcp(Box::new(RealtimeMcpTool {
                        server_label: String::from("docs"),
                        allowed_tools: Some(RealtimeMcpAllowedTools::Names(vec![String::from(
                            "search",
                        )])),
                        require_approval: Some(RealtimeMcpRequireApproval::Never),
                        server_url: Some(String::from("https://mcp.example.test")),
                        ..Default::default()
                    })),
                ]),
                ..Default::default()
            },
            Some(String::from("evt_response")),
        )
        .await
        .unwrap();
    connection
        .response()
        .cancel(
            Some(String::from("resp_cancel")),
            Some(String::from("evt_cancel")),
        )
        .await
        .unwrap();
    connection
        .input_audio_buffer()
        .append("AQID", Some(String::from("evt_append")))
        .await
        .unwrap();
    connection
        .input_audio_buffer()
        .commit(Some(String::from("evt_commit")))
        .await
        .unwrap();
    connection
        .input_audio_buffer()
        .clear(Some(String::from("evt_input_clear")))
        .await
        .unwrap();
    connection
        .conversation()
        .item()
        .create(
            RealtimeConversationItem::user_message(vec![
                RealtimeConversationMessageContentPart::input_text("Hello"),
            ]),
            Some(String::from("root")),
            Some(String::from("evt_item_create")),
        )
        .await
        .unwrap();
    connection
        .conversation()
        .item()
        .truncate("item_assistant", 0, 240, Some(String::from("evt_truncate")))
        .await
        .unwrap();
    connection
        .conversation()
        .item()
        .retrieve("item_user", Some(String::from("evt_retrieve")))
        .await
        .unwrap();
    connection
        .conversation()
        .item()
        .delete("item_user", Some(String::from("evt_delete")))
        .await
        .unwrap();
    connection
        .output_audio_buffer()
        .clear(Some(String::from("evt_output_clear")))
        .await
        .unwrap();
    connection.close().await.unwrap();

    server.await.unwrap();

    let captured = captured_rx
        .try_iter()
        .map(|text| serde_json::from_str::<serde_json::Value>(&text).unwrap())
        .collect::<Vec<_>>();
    let event_types = captured
        .iter()
        .map(|event| event["type"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        event_types,
        vec![
            "session.update",
            "response.create",
            "response.cancel",
            "input_audio_buffer.append",
            "input_audio_buffer.commit",
            "input_audio_buffer.clear",
            "conversation.item.create",
            "conversation.item.truncate",
            "conversation.item.retrieve",
            "conversation.item.delete",
            "output_audio_buffer.clear",
        ]
    );
    assert_eq!(captured[0]["event_id"], "evt_update");
    assert_eq!(captured[0]["session"]["instructions"], "Be direct.");
    assert_eq!(captured[1]["response"]["metadata"]["source"], "test");
    assert_eq!(
        captured[1]["response"]["output_modalities"],
        json!(["text"])
    );
    assert_eq!(captured[1]["response"]["max_output_tokens"], "inf");
    assert_eq!(captured[1]["response"]["prompt"]["id"], "pmpt_response");
    assert_eq!(
        captured[1]["response"]["prompt"]["variables"]["topic"],
        "response"
    );
    assert_eq!(captured[1]["response"]["reasoning"]["effort"], "minimal");
    assert_eq!(captured[1]["response"]["tool_choice"]["type"], "function");
    assert_eq!(
        captured[1]["response"]["tool_choice"]["name"],
        "lookup_weather"
    );
    assert_eq!(captured[1]["response"]["tools"][0]["type"], "function");
    assert_eq!(
        captured[1]["response"]["tools"][0]["description"],
        "Look up weather."
    );
    assert_eq!(
        captured[1]["response"]["tools"][0]["parameters"]["type"],
        "object"
    );
    assert_eq!(captured[1]["response"]["tools"][1]["type"], "mcp");
    assert_eq!(captured[1]["response"]["tools"][1]["server_label"], "docs");
    assert_eq!(
        captured[1]["response"]["tools"][1]["allowed_tools"],
        json!(["search"])
    );
    assert_eq!(
        captured[1]["response"]["tools"][1]["require_approval"],
        "never"
    );
    assert_eq!(captured[2]["response_id"], "resp_cancel");
    assert_eq!(captured[3]["audio"], "AQID");
    assert_eq!(captured[6]["previous_item_id"], "root");
    assert_eq!(captured[6]["item"]["content"][0]["text"], "Hello");
    assert_eq!(captured[7]["item_id"], "item_assistant");
    assert_eq!(captured[7]["content_index"], 0);
    assert_eq!(captured[7]["audio_end_ms"], 240);
    assert_eq!(captured[8]["item_id"], "item_user");
    assert_eq!(captured[9]["item_id"], "item_user");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connection_callbacks_parse_and_dispatch_events() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();

        for event in [
            json!({
                "type": "session.created",
                "event_id": "evt_created",
                "session": {
                    "id": "sess_callbacks",
                    "type": "realtime",
                    "model": "gpt-realtime-mini",
                    "output_modalities": ["text"]
                }
            }),
            json!({
                "type": "response.output_text.delta",
                "event_id": "evt_delta_1",
                "response_id": "resp_123",
                "item_id": "item_123",
                "output_index": 0,
                "content_index": 0,
                "delta": "Hel"
            }),
            json!({
                "type": "response.output_text.delta",
                "event_id": "evt_delta_2",
                "response_id": "resp_123",
                "item_id": "item_123",
                "output_index": 0,
                "content_index": 0,
                "delta": "lo"
            }),
        ] {
            socket
                .send(Message::Text(event.to_string().into()))
                .await
                .unwrap();
        }

        socket.close(None).await.unwrap();
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

    let parsed = connection
        .parse_event(
            br#"{"type":"response.output_text.done","event_id":"evt_done","response_id":"resp_123","item_id":"item_123","output_index":0,"content_index":0,"text":"Hello"}"#,
        )
        .unwrap();
    assert!(matches!(
        parsed,
        RealtimeServerEvent::OutputTextDone { ref text, .. } if text == "Hello"
    ));

    let calls = Arc::new(Mutex::new(Vec::new()));
    let removed = {
        let calls = calls.clone();
        connection.on("session.created", move |_| {
            calls
                .lock()
                .unwrap()
                .push(String::from("removed:session.created"));
        })
    };
    assert!(connection.off("session.created", removed));

    {
        let calls = calls.clone();
        connection.on("session.created", move |event| {
            calls
                .lock()
                .unwrap()
                .push(format!("specific:{}", event.event_type()));
        });
    }
    {
        let calls = calls.clone();
        connection.once("response.output_text.delta", move |event| {
            if let RealtimeServerEvent::OutputTextDelta { delta, .. } = event {
                calls.lock().unwrap().push(format!("once:delta:{delta}"));
            }
        });
    }
    {
        let calls = calls.clone();
        connection.on("event", move |event| {
            calls
                .lock()
                .unwrap()
                .push(format!("generic:{}", event.event_type()));
        });
    }

    connection.dispatch_events().await.unwrap();
    server.await.unwrap();

    let calls = calls.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec![
            "specific:session.created",
            "generic:session.created",
            "once:delta:Hel",
            "generic:response.output_text.delta",
            "generic:response.output_text.delta",
        ]
    );
}
