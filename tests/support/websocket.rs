#![allow(dead_code)]

use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};

/// Captured websocket transcript for one local session.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WebSocketTranscript {
    pub client_to_server: Vec<String>,
    pub server_to_client: Vec<String>,
}

/// Local websocket harness for transcript-driven tests.
pub struct LocalWebSocketHarness {
    url: String,
    expected_server_messages: usize,
    transcript_rx: oneshot::Receiver<Result<WebSocketTranscript, String>>,
    worker: JoinHandle<()>,
}

impl LocalWebSocketHarness {
    /// Spawns a websocket harness that replays scripted server text frames.
    pub async fn spawn(scripted_server_messages: Vec<String>) -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|err| err.to_string())?;
        let addr = listener.local_addr().map_err(|err| err.to_string())?;
        let expected_server_messages = scripted_server_messages.len();
        let (tx, rx) = oneshot::channel();
        let worker = tokio::spawn(async move {
            let transcript = async {
                let (stream, _) = listener.accept().await.map_err(|err| err.to_string())?;
                let websocket = accept_async(stream).await.map_err(|err| err.to_string())?;
                let (mut sink, mut stream) = websocket.split();

                for message in &scripted_server_messages {
                    sink.send(Message::Text(message.clone().into()))
                        .await
                        .map_err(|err| err.to_string())?;
                }
                sink.send(Message::Close(None))
                    .await
                    .map_err(|err| err.to_string())?;

                let mut client_to_server = Vec::new();
                while let Some(message) = stream.next().await {
                    match message.map_err(|err| err.to_string())? {
                        Message::Text(text) => client_to_server.push(text.to_string()),
                        Message::Close(_) => break,
                        _ => {}
                    }
                }

                Ok(WebSocketTranscript {
                    client_to_server,
                    server_to_client: scripted_server_messages,
                })
            }
            .await;

            let _ = tx.send(transcript);
        });

        Ok(Self {
            url: format!("ws://{addr}"),
            expected_server_messages,
            transcript_rx: rx,
            worker,
        })
    }

    /// Connects a client, exchanges text frames, and returns the transcript.
    pub async fn drive_text_session(
        self,
        client_messages: Vec<String>,
    ) -> Result<WebSocketTranscript, String> {
        let (mut socket, _) = connect_async(&self.url)
            .await
            .map_err(|err| err.to_string())?;

        let mut server_to_client = Vec::new();
        while server_to_client.len() < self.expected_server_messages {
            let Some(message) = socket.next().await else {
                break;
            };
            match message.map_err(|err| err.to_string())? {
                Message::Text(text) => server_to_client.push(text.to_string()),
                Message::Close(_) => break,
                _ => {}
            }
        }

        for message in client_messages {
            socket
                .send(Message::Text(message.into()))
                .await
                .map_err(|err| err.to_string())?;
        }
        socket.close(None).await.map_err(|err| err.to_string())?;

        let mut transcript = self.transcript_rx.await.map_err(|_| {
            String::from("websocket harness dropped before transcript was captured")
        })??;
        transcript.server_to_client = server_to_client;
        let _ = self.worker.await;
        Ok(transcript)
    }
}
