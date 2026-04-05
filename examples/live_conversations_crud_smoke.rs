use std::time::{SystemTime, UNIX_EPOCH};

use openai_rust::{
    OpenAI,
    resources::conversations::{ConversationCreateParams, ConversationUpdateParams},
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAI::builder().build();
    let marker = unique_marker("crud");

    let created = client.conversations().create(ConversationCreateParams {
        metadata: Some(json!({
            "smoke": "conversations_crud",
            "marker": marker,
            "phase": "created"
        })),
        items: vec![json!({
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "Conversation CRUD smoke"}]
        })],
        ..Default::default()
    })?;
    let conversation_id = created.output().id.clone();

    let retrieved = client.conversations().retrieve(&conversation_id)?;
    if retrieved.output().metadata["marker"] != marker {
        return Err(format!(
            "retrieved metadata marker mismatch: expected {marker:?}, got {:?}",
            retrieved.output().metadata
        )
        .into());
    }

    let updated = client.conversations().update(
        &conversation_id,
        ConversationUpdateParams {
            metadata: json!({
                "smoke": "conversations_crud",
                "marker": marker,
                "phase": "updated"
            }),
            ..Default::default()
        },
    )?;
    if updated.output().metadata["phase"] != "updated" {
        return Err(format!(
            "expected updated phase metadata, got {:?}",
            updated.output().metadata
        )
        .into());
    }

    let deleted = client.conversations().delete(&conversation_id)?;
    if !deleted.output().deleted {
        return Err(format!("expected deleted=true for {}", deleted.output().id).into());
    }

    println!(
        "conversation create request id: {}",
        created.request_id().unwrap_or("<missing>")
    );
    println!(
        "conversation retrieve request id: {}",
        retrieved.request_id().unwrap_or("<missing>")
    );
    println!(
        "conversation update request id: {}",
        updated.request_id().unwrap_or("<missing>")
    );
    println!(
        "conversation delete request id: {}",
        deleted.request_id().unwrap_or("<missing>")
    );
    println!("conversation id: {conversation_id}");
    println!("conversation marker: {marker}");

    Ok(())
}

fn unique_marker(label: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{label}_{nanos}")
}
