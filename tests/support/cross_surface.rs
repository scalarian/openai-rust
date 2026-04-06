#![allow(dead_code)]

use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NormalizedCrossSurfaceEntry {
    pub surface: String,
    pub status_class: String,
    pub request_metadata_shape: String,
    pub terminal_state: String,
    pub event_ordering: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NormalizedCrossSurfaceReport {
    pub entries: Vec<NormalizedCrossSurfaceEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PairedCrossSurfaceReport {
    pub mock_baseline: NormalizedCrossSurfaceReport,
    pub live_report: NormalizedCrossSurfaceReport,
}

pub fn normalized_entry(
    surface: impl Into<String>,
    status_class: impl Into<String>,
    request_metadata_shape: impl Into<String>,
    terminal_state: impl Into<String>,
    event_ordering: impl IntoIterator<Item = impl Into<String>>,
) -> NormalizedCrossSurfaceEntry {
    NormalizedCrossSurfaceEntry {
        surface: surface.into(),
        status_class: status_class.into(),
        request_metadata_shape: request_metadata_shape.into(),
        terminal_state: terminal_state.into(),
        event_ordering: event_ordering.into_iter().map(Into::into).collect(),
    }
}

pub fn expected_publish_ready_equivalence_baseline() -> NormalizedCrossSurfaceReport {
    NormalizedCrossSurfaceReport {
        entries: vec![
            normalized_entry(
                "responses.create",
                "success",
                "request_id:present",
                "completed",
                Vec::<String>::new(),
            ),
            normalized_entry(
                "chat.completions.create",
                "success",
                "request_id:present",
                "completed",
                Vec::<String>::new(),
            ),
            normalized_entry(
                "files.create",
                "success",
                "request_id:present",
                "ready_or_processing",
                Vec::<String>::new(),
            ),
            normalized_entry(
                "realtime.client_secrets.create + ws bootstrap",
                "success",
                "request_id:present",
                "session_created",
                [
                    "rest.client_secrets.create",
                    "ws.session.created",
                    "ws.close",
                ],
            ),
        ],
    }
}

pub fn normalize_live_publish_ready_report<T: Serialize>(
    report: &T,
) -> NormalizedCrossSurfaceReport {
    let value = serde_json::to_value(report).expect("serialize live cross-surface report");
    let entries = value["entries"]
        .as_array()
        .expect("live cross-surface report should contain entries")
        .iter()
        .map(|entry| {
            let surface = entry["surface"]
                .as_str()
                .expect("report entry surface")
                .to_string();
            let status_class = entry["status_class"]
                .as_str()
                .expect("report entry status_class")
                .to_string();
            let request_id = entry["request_id"].as_str().unwrap_or_default();
            normalized_entry(
                surface.clone(),
                status_class,
                if request_id.is_empty() || request_id == "<missing>" {
                    "request_id:missing"
                } else {
                    "request_id:present"
                },
                normalize_terminal_state(&surface, entry["terminal_interpretation"].as_str()),
                normalize_event_ordering(&surface),
            )
        })
        .collect();
    NormalizedCrossSurfaceReport { entries }
}

fn normalize_terminal_state(surface: &str, interpretation: Option<&str>) -> String {
    match surface {
        "responses.create" => String::from("completed"),
        "chat.completions.create" => match interpretation.unwrap_or_default() {
            "stop" => String::from("completed"),
            other if other.trim().is_empty() => String::from("missing_finish_reason"),
            other => other.to_string(),
        },
        "files.create" => match interpretation.unwrap_or_default() {
            "uploaded" | "processed" => String::from("ready_or_processing"),
            other if other.trim().is_empty() => String::from("missing_status"),
            other => other.to_string(),
        },
        "realtime.client_secrets.create + ws bootstrap" => {
            if interpretation
                .unwrap_or_default()
                .contains("session.created")
            {
                String::from("session_created")
            } else {
                interpretation.unwrap_or("missing bootstrap").to_string()
            }
        }
        _ => interpretation.unwrap_or_default().to_string(),
    }
}

fn normalize_event_ordering(surface: &str) -> Vec<String> {
    if surface == "realtime.client_secrets.create + ws bootstrap" {
        vec![
            String::from("rest.client_secrets.create"),
            String::from("ws.session.created"),
            String::from("ws.close"),
        ]
    } else {
        Vec::new()
    }
}
