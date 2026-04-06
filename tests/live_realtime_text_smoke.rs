use openai_rust::{
    OpenAI,
    realtime::{
        RealtimeAuth, RealtimeClientEvent, RealtimeConnectOptions, RealtimeConversationItem,
        RealtimeConversationMessageContentPart, RealtimeOutputModality, RealtimeServerEvent,
        RealtimeSessionConfig,
    },
};
use tokio::time::{Duration, Instant, timeout_at};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires live OpenAI credentials"]
async fn live_realtime_text_smoke_uses_client_secret_and_ga_text_events() {
    let client = OpenAI::builder().build();
    let client_secret = tokio::task::spawn_blocking({
        let client = client.clone();
        move || {
            client.realtime().client_secrets().create(
                openai_rust::realtime::RealtimeClientSecretCreateParams {
                    expires_after: Some(openai_rust::realtime::RealtimeSessionTTL {
                        anchor: String::from("created_at"),
                        seconds: 60,
                    }),
                    session: Some(RealtimeSessionConfig {
                        model: Some(String::from("gpt-realtime-mini")),
                        output_modalities: Some(vec![RealtimeOutputModality::Text]),
                        instructions: Some(String::from(
                            "You are a terse assistant. Reply with exactly `hi`.",
                        )),
                        ..Default::default()
                    }),
                },
            )
        }
    })
    .await
    .expect("blocking client-secret task should join")
    .expect("client secret should be created for the live smoke");

    let request_id = client_secret
        .request_id()
        .expect("client-secret creation should expose a request id");
    let model = client_secret
        .output()
        .session
        .model
        .clone()
        .unwrap_or_else(|| String::from("gpt-realtime-mini"));

    let mut connection = client
        .realtime()
        .connect(RealtimeConnectOptions {
            model: Some(model.clone()),
            auth: Some(RealtimeAuth::client_secret(
                client_secret.output().client_secret.value.clone(),
            )),
            ..Default::default()
        })
        .await
        .expect("live realtime websocket should connect");

    let bootstrap = connection
        .next_event()
        .await
        .expect("expected bootstrap event")
        .expect("bootstrap should decode");
    assert!(matches!(
        bootstrap,
        RealtimeServerEvent::SessionCreated { ref session, .. }
            if session.model.as_deref() == Some(model.as_str())
    ));

    connection
        .send(RealtimeClientEvent::session_update(RealtimeSessionConfig {
            instructions: Some(String::from("Reply with exactly `hi`.")),
            output_modalities: Some(vec![RealtimeOutputModality::Text]),
            ..Default::default()
        }))
        .await
        .expect("session.update should send");

    loop {
        let deadline = Instant::now() + Duration::from_secs(10);
        let Some(event) = timeout_at(deadline, connection.next_event())
            .await
            .expect("session.update response should arrive")
        else {
            panic!("websocket closed before session.updated");
        };
        match event.expect("session.update event should decode") {
            RealtimeServerEvent::SessionUpdated { .. } => break,
            RealtimeServerEvent::Error { error, .. } => {
                panic!(
                    "session.update returned a live realtime error: {}",
                    error.message
                )
            }
            _ => {}
        }
    }

    connection
        .send(RealtimeClientEvent::conversation_item_create(
            RealtimeConversationItem::user_message(vec![
                RealtimeConversationMessageContentPart::input_text("Reply with exactly hi."),
            ]),
        ))
        .await
        .expect("conversation.item.create should send");
    connection
        .send(RealtimeClientEvent::response_create(None))
        .await
        .expect("response.create should send");

    let mut text = String::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut saw_output_text = false;
    let mut saw_response_done = false;

    while Instant::now() < deadline {
        let Some(event) = timeout_at(deadline, connection.next_event())
            .await
            .expect("waiting for live realtime events timed out")
        else {
            break;
        };
        match event.expect("live event should decode") {
            RealtimeServerEvent::OutputTextDelta { delta, .. } => {
                text.push_str(&delta);
            }
            RealtimeServerEvent::OutputTextDone { text: done, .. } => {
                text = done;
                saw_output_text = true;
            }
            RealtimeServerEvent::ResponseDone { .. } => {
                saw_response_done = true;
                break;
            }
            RealtimeServerEvent::Error { error, .. } => {
                panic!("live realtime error: {}", error.message);
            }
            RealtimeServerEvent::Unknown { event_type, .. } => {
                assert!(
                    !matches!(
                        event_type.as_str(),
                        "response.text.delta" | "response.text.done"
                    ),
                    "stale beta aliases must not appear as the primary GA contract"
                );
            }
            _ => {}
        }
    }

    connection.close().await.expect("clean websocket close");

    assert!(
        saw_output_text,
        "expected at least one GA output_text completion event"
    );
    assert!(saw_response_done, "expected a terminal response.done event");
    assert!(
        !text.trim().is_empty(),
        "expected live realtime text smoke to produce non-empty text"
    );

    println!("live realtime client-secret request id: {request_id}");
    println!(
        "live realtime session id: {}",
        connection.session_id().unwrap_or("unknown")
    );
    println!("live realtime final text: {}", text.trim());
}
