use std::time::{SystemTime, UNIX_EPOCH};

use openai_rust::{
    OpenAI,
    resources::conversations::{
        ConversationCreateParams, ConversationItemCreateParams, ConversationItemListParams,
        ConversationItemRetrieveParams,
    },
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAI::builder().build();
    let marker = unique_marker("items");

    let created_conversation = client.conversations().create(ConversationCreateParams {
        metadata: Some(json!({
            "smoke": "conversations_items",
            "marker": marker
        })),
        ..Default::default()
    })?;
    let conversation_id = created_conversation.output().id.clone();

    let created_items = client.conversations().items().create(
        &conversation_id,
        ConversationItemCreateParams {
            items: vec![json!({
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": format!("Conversation items smoke {marker}")}]
            })],
            ..Default::default()
        },
    )?;
    let created_item = created_items
        .output()
        .data
        .first()
        .ok_or("expected created item in response envelope")?;
    let item_id = created_item.id.clone().ok_or("expected created item id")?;

    let retrieved_item = client.conversations().items().retrieve(
        &conversation_id,
        &item_id,
        ConversationItemRetrieveParams::default(),
    )?;
    if retrieved_item.output().id.as_deref() != Some(item_id.as_str()) {
        return Err(format!("retrieved wrong item id: {:?}", retrieved_item.output().id).into());
    }

    let listed_before_delete = client.conversations().items().list(
        &conversation_id,
        ConversationItemListParams {
            order: Some(String::from("asc")),
            ..Default::default()
        },
    )?;
    let visible_before_delete = listed_before_delete
        .output()
        .data
        .iter()
        .any(|item| item.id.as_deref() == Some(item_id.as_str()));
    if !visible_before_delete {
        return Err(format!("created item {item_id} was not visible in ascending list").into());
    }

    let delete_item_response = client
        .conversations()
        .items()
        .delete(&conversation_id, &item_id)?;
    let listed_after_delete = client.conversations().items().list(
        &conversation_id,
        ConversationItemListParams {
            order: Some(String::from("asc")),
            ..Default::default()
        },
    )?;
    let visible_after_delete = listed_after_delete
        .output()
        .data
        .iter()
        .any(|item| item.id.as_deref() == Some(item_id.as_str()));
    if visible_after_delete {
        return Err(format!("deleted item {item_id} still appeared in subsequent list").into());
    }

    let deleted_conversation = client.conversations().delete(&conversation_id)?;

    println!(
        "conversation create request id: {}",
        created_conversation.request_id().unwrap_or("<missing>")
    );
    println!(
        "item create request id: {}",
        created_items.request_id().unwrap_or("<missing>")
    );
    println!(
        "item retrieve request id: {}",
        retrieved_item.request_id().unwrap_or("<missing>")
    );
    println!(
        "list-before-delete request id: {}",
        listed_before_delete.request_id().unwrap_or("<missing>")
    );
    println!(
        "item delete request id: {}",
        delete_item_response.request_id().unwrap_or("<missing>")
    );
    println!(
        "list-after-delete request id: {}",
        listed_after_delete.request_id().unwrap_or("<missing>")
    );
    println!(
        "conversation delete request id: {}",
        deleted_conversation.request_id().unwrap_or("<missing>")
    );
    println!("conversation id: {conversation_id}");
    println!("item id: {item_id}");
    println!("marker: {marker}");

    Ok(())
}

fn unique_marker(label: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{label}_{nanos}")
}
