#[path = "support/cross_surface.rs"]
mod cross_surface;

use openai_rust::{
    OpenAI,
    realtime::{
        RealtimeAuth, RealtimeConnectOptions, RealtimeServerEvent, RealtimeSessionConfig,
        RealtimeSessionTTL,
    },
    resources::{
        chat::ChatCompletionCreateParams,
        files::{FileCreateParams, FilePurpose, FileUpload},
        responses::ResponseCreateParams,
    },
};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ReportEntry {
    surface: &'static str,
    status_class: &'static str,
    request_id: String,
    terminal_interpretation: String,
}

#[derive(Debug, Serialize)]
struct CrossSurfaceReport {
    entries: Vec<ReportEntry>,
}

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_cross_surface_smoke_proves_env_only_multi_surface_and_realtime_bootstrap() {
    let client = OpenAI::new();

    let response = client
        .responses()
        .create(ResponseCreateParams {
            model: String::from("gpt-4.1-mini"),
            input: Some(serde_json::json!("Reply with exactly hi.")),
            ..Default::default()
        })
        .expect("live responses call should succeed");

    let chat = client
        .chat()
        .completions()
        .create(ChatCompletionCreateParams {
            model: String::from("gpt-4.1-mini"),
            messages: vec![serde_json::json!({
                "role": "user",
                "content": "Reply with exactly hi."
            })],
            ..Default::default()
        })
        .expect("live compatibility chat call should succeed");

    let file = client
        .files()
        .create(FileCreateParams {
            file: FileUpload::new(
                "cross-surface-live.txt",
                "text/plain",
                b"cross-surface live smoke".to_vec(),
            ),
            purpose: FilePurpose::UserData,
            expires_after: None,
        })
        .expect("live file create should succeed");

    let realtime_secret = client
        .realtime()
        .client_secrets()
        .create(openai_rust::realtime::RealtimeClientSecretCreateParams {
            expires_after: Some(RealtimeSessionTTL {
                anchor: String::from("created_at"),
                seconds: 60,
            }),
            session: Some(RealtimeSessionConfig {
                model: Some(String::from("gpt-realtime-mini")),
                ..Default::default()
            }),
        })
        .expect("live realtime client secret should succeed");

    let realtime_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    realtime_runtime.block_on(async {
        let mut connection = client
            .realtime()
            .connect(RealtimeConnectOptions {
                model: Some(String::from("gpt-realtime-mini")),
                auth: Some(RealtimeAuth::client_secret(
                    realtime_secret.output.client_secret.value.clone(),
                )),
                ..Default::default()
            })
            .await
            .expect("live realtime websocket should connect");

        let bootstrap = connection
            .next_event()
            .await
            .expect("expected realtime bootstrap event")
            .expect("bootstrap should decode");
        assert!(matches!(
            bootstrap,
            RealtimeServerEvent::SessionCreated { .. }
        ));
        connection
            .close()
            .await
            .expect("realtime close should succeed");
    });

    let deleted = client
        .files()
        .delete(&file.output.id)
        .expect("live cleanup delete should succeed");
    assert!(deleted.output.deleted);

    let report = CrossSurfaceReport {
        entries: vec![
            ReportEntry {
                surface: "responses.create",
                status_class: "success",
                request_id: response.request_id().unwrap_or("<missing>").to_string(),
                terminal_interpretation: String::from("completed response object"),
            },
            ReportEntry {
                surface: "chat.completions.create",
                status_class: "success",
                request_id: chat.request_id().unwrap_or("<missing>").to_string(),
                terminal_interpretation: chat.output.choices[0]
                    .finish_reason
                    .as_deref()
                    .unwrap_or("missing finish_reason")
                    .to_string(),
            },
            ReportEntry {
                surface: "files.create",
                status_class: "success",
                request_id: file.request_id().unwrap_or("<missing>").to_string(),
                terminal_interpretation: file
                    .output
                    .status
                    .as_ref()
                    .map(|status| match status {
                        openai_rust::resources::files::FileStatus::Uploaded => {
                            String::from("uploaded")
                        }
                        openai_rust::resources::files::FileStatus::Processed => {
                            String::from("processed")
                        }
                        openai_rust::resources::files::FileStatus::Error => String::from("error"),
                        openai_rust::resources::files::FileStatus::Deleted => {
                            String::from("deleted")
                        }
                        openai_rust::resources::files::FileStatus::Unknown => {
                            String::from("unknown")
                        }
                    })
                    .unwrap_or_else(|| String::from("missing status")),
            },
            ReportEntry {
                surface: "realtime.client_secrets.create + ws bootstrap",
                status_class: "success",
                request_id: realtime_secret
                    .request_id()
                    .unwrap_or("<missing>")
                    .to_string(),
                terminal_interpretation: String::from("session.created observed"),
            },
        ],
    };

    let normalized = cross_surface::normalize_live_publish_ready_report(&report);
    assert_eq!(
        normalized,
        cross_surface::expected_publish_ready_equivalence_baseline()
    );

    let paired = cross_surface::PairedCrossSurfaceReport {
        mock_baseline: cross_surface::expected_publish_ready_equivalence_baseline(),
        live_report: normalized,
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&paired)
            .expect("serialize paired live cross-surface report")
    );
    println!(
        "live file cleanup delete request id: {}",
        deleted.request_id().unwrap_or("<missing>")
    );
}
