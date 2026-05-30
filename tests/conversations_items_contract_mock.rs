use openai_rust::{
    ErrorKind, OpenAI,
    resources::{
        common::SearchContextSize,
        responses::{
            ResponseApplyPatchOperation, ResponseComputerAction, ResponseInputContentPart,
            ResponseInputItem, ResponseInputText, ResponseItemAction, ResponseItemEnvironment,
            ResponseItemOutput, ResponseShellOutputOutcome, ResponseTool,
        },
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn routes_and_pagination() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(items_envelope(vec![
            message_item("item_1", "hello"),
            reasoning_item("rs_1"),
        ])),
        json_response(message_item("item_1", "hello").to_string()),
        json_response(items_envelope(vec![
            message_item("item_1", "hello"),
            message_item("item_2", "world"),
        ])),
        json_response(conversation_payload(
            "conv_123",
            json!({"phase": "after_delete"}),
        )),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();
    let conversation_id = "conv_123/tenant a?x=y";
    let item_id = "item_1/seed a?x=y";

    let created = client
        .conversations()
        .items()
        .create(
            conversation_id,
            openai_rust::resources::conversations::ConversationItemCreateParams {
                items: vec![
                    ResponseInputItem::message(
                        "user",
                        vec![ResponseInputContentPart::Text(ResponseInputText::new(
                            "hello",
                        ))],
                    ),
                    ResponseInputItem {
                        item_type: Some(String::from("reasoning")),
                        extra: BTreeMap::from([(String::from("summary"), json!([]))]),
                        ..Default::default()
                    },
                ],
                include: vec![
                    String::from("message.output_text.logprobs"),
                    String::from("reasoning.encrypted_content/preview?x=y"),
                ],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(created.output().object, "list");
    assert_eq!(created.output().data.len(), 2);
    assert_eq!(created.output().data[0].id.as_deref(), Some("item_1"));
    assert_eq!(created.output().data[1].item_type, "reasoning");
    assert_eq!(created.output().data[1].summary.len(), 1);
    assert_eq!(
        created.output().data[1].encrypted_content.as_deref(),
        Some("encrypted_reasoning")
    );
    assert!(created.output().has_more);
    assert_eq!(created.output().next_after(), Some("rs_1"));

    let retrieved = client
        .conversations()
        .items()
        .retrieve(
            conversation_id,
            item_id,
            openai_rust::resources::conversations::ConversationItemRetrieveParams {
                include: vec![String::from("message.output_text.logprobs/with space")],
            },
        )
        .unwrap();
    assert_eq!(retrieved.output().id.as_deref(), Some("item_1"));
    assert_eq!(retrieved.output().item_type, "message");

    let listed = client
        .conversations()
        .items()
        .list(
            conversation_id,
            openai_rust::resources::conversations::ConversationItemListParams {
                after: Some(String::from("item_1")),
                include: vec![String::from("message.output_text.logprobs/with space")],
                limit: Some(2),
                order: Some(String::from("asc")),
            },
        )
        .unwrap();
    assert_eq!(listed.output().data.len(), 2);
    assert_eq!(listed.output().first_id.as_deref(), Some("item_1"));
    assert_eq!(listed.output().last_id.as_deref(), Some("item_2"));
    assert!(listed.output().has_next_page());
    assert_eq!(listed.output().next_after(), Some("item_2"));

    let deleted = client
        .conversations()
        .items()
        .delete(conversation_id, item_id)
        .unwrap();
    assert_eq!(deleted.output().id, "conv_123");
    assert_eq!(deleted.output().object, "conversation");
    assert_eq!(deleted.output().metadata["phase"], "after_delete");

    let requests = server.captured_requests(4).expect("captured requests");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].path,
        "/v1/conversations/conv_123%2Ftenant%20a%3Fx%3Dy/items?include=message.output_text.logprobs&include=reasoning.encrypted_content%2Fpreview%3Fx%3Dy"
    );
    let create_body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["items"][0]["content"][0]["text"], "hello");
    assert_eq!(create_body["items"][1]["type"], "reasoning");

    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].path,
        "/v1/conversations/conv_123%2Ftenant%20a%3Fx%3Dy/items/item_1%2Fseed%20a%3Fx%3Dy?include=message.output_text.logprobs%2Fwith+space"
    );

    assert_eq!(requests[2].method, "GET");
    assert_eq!(
        requests[2].path,
        "/v1/conversations/conv_123%2Ftenant%20a%3Fx%3Dy/items?after=item_1&include=message.output_text.logprobs%2Fwith+space&limit=2&order=asc"
    );

    assert_eq!(requests[3].method, "DELETE");
    assert_eq!(
        requests[3].path,
        "/v1/conversations/conv_123%2Ftenant%20a%3Fx%3Dy/items/item_1%2Fseed%20a%3Fx%3Dy"
    );

    let invalid_create = client
        .conversations()
        .items()
        .create(
            "   ",
            openai_rust::resources::conversations::ConversationItemCreateParams {
                items: vec![],
                include: vec![],
                ..Default::default()
            },
        )
        .expect_err("blank conversation id should be rejected locally");
    assert_eq!(invalid_create.kind, ErrorKind::Validation);

    for invalid in ["", "   "] {
        let retrieve_error = client
            .conversations()
            .items()
            .retrieve(
                invalid,
                "item_1",
                openai_rust::resources::conversations::ConversationItemRetrieveParams::default(),
            )
            .expect_err("blank conversation id should be rejected locally");
        assert_eq!(retrieve_error.kind, ErrorKind::Validation);

        let list_error = client
            .conversations()
            .items()
            .list(
                invalid,
                openai_rust::resources::conversations::ConversationItemListParams::default(),
            )
            .expect_err("blank conversation id should be rejected locally");
        assert_eq!(list_error.kind, ErrorKind::Validation);

        let delete_error = client
            .conversations()
            .items()
            .delete(invalid, "item_1")
            .expect_err("blank conversation id should be rejected locally");
        assert_eq!(delete_error.kind, ErrorKind::Validation);
    }

    for invalid in ["", "   "] {
        let retrieve_error = client
            .conversations()
            .items()
            .retrieve(
                "conv_123",
                invalid,
                openai_rust::resources::conversations::ConversationItemRetrieveParams::default(),
            )
            .expect_err("blank item id should be rejected locally");
        assert_eq!(retrieve_error.kind, ErrorKind::Validation);

        let delete_error = client
            .conversations()
            .items()
            .delete("conv_123", invalid)
            .expect_err("blank item id should be rejected locally");
        assert_eq!(delete_error.kind, ErrorKind::Validation);
    }
}

#[test]
fn typed_known_fields_are_not_lost() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(items_envelope(vec![
            function_call_item("fc_1"),
            refusal_message_item("msg_refusal"),
            mcp_list_tools_item("mcp_tools_1"),
            computer_call_item("computer_1"),
            computer_call_output_item("computer_output_1"),
            custom_tool_call_output_item("custom_output_1"),
            shell_call_output_item("shell_output_1"),
            local_shell_call_item("local_shell_1"),
            shell_call_item("shell_1"),
            apply_patch_call_item("patch_1"),
            tool_search_output_item("tool_search_output_1"),
        ])),
        json_response(function_call_item("fc_1").to_string()),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let listed = client
        .conversations()
        .items()
        .list("conv_123", Default::default())
        .unwrap();
    let retrieved = client
        .conversations()
        .items()
        .retrieve("conv_123", "fc_1", Default::default())
        .unwrap();

    let function_call = &listed.output().data[0];
    assert_eq!(function_call.item_type, "function_call");
    assert_eq!(function_call.name.as_deref(), Some("lookup_weather"));
    assert_eq!(
        function_call.arguments.as_deref(),
        Some(r#"{"city":"Paris"}"#)
    );
    assert_eq!(function_call.call_id.as_deref(), Some("call_123"));
    assert_eq!(function_call.status.as_deref(), Some("completed"));
    assert_eq!(function_call.created_by.as_deref(), Some("model"));
    assert_eq!(
        function_call.arguments_json.as_ref(),
        Some(&json!({"city": "Paris"}))
    );
    assert!(!function_call.extra.contains_key("name"));
    assert!(!function_call.extra.contains_key("arguments"));
    assert!(!function_call.extra.contains_key("call_id"));

    let refusal_message = &listed.output().data[1];
    let refusal_part = refusal_message
        .content
        .iter()
        .find(|part| part.content_type == "refusal")
        .expect("refusal content");
    assert_eq!(
        refusal_part.refusal.as_deref(),
        Some("I can't help with that")
    );
    assert!(!refusal_part.extra.contains_key("refusal"));
    let image_part = refusal_message
        .content
        .iter()
        .find(|part| part.content_type == "input_image")
        .expect("image content");
    assert_eq!(image_part.detail.as_deref(), Some("high"));
    assert_eq!(image_part.file_id.as_deref(), Some("file_img"));
    assert_eq!(
        image_part.image_url.as_deref(),
        Some("https://example.com/cat.png")
    );
    let output_text = refusal_message
        .content
        .iter()
        .find(|part| part.content_type == "output_text")
        .expect("output text content");
    assert_eq!(output_text.annotations.len(), 1);
    assert_eq!(output_text.logprobs.as_ref().unwrap().len(), 1);

    let mcp_tools = &listed.output().data[2];
    assert_eq!(mcp_tools.item_type, "mcp_list_tools");
    assert_eq!(mcp_tools.server_label.as_deref(), Some("deepwiki"));
    let mcp_tool = mcp_tools.tools[0].as_mcp_list().expect("mcp list tool");
    assert_eq!(mcp_tool.name, "search_docs");
    assert_eq!(mcp_tool.input_schema["type"], "object");
    assert_eq!(mcp_tool.annotations.as_ref().unwrap()["readOnlyHint"], true);
    assert_eq!(mcp_tool.description.as_deref(), Some("Search docs"));

    let computer_call = &listed.output().data[3];
    assert_eq!(computer_call.item_type, "computer_call");
    assert_eq!(computer_call.call_id.as_deref(), Some("call_computer"));
    assert_eq!(
        computer_call.pending_safety_checks[0].id,
        "safety_pending_1"
    );
    assert_eq!(
        computer_call.pending_safety_checks[0].code.as_deref(),
        Some("unsafe_browser")
    );
    assert_eq!(
        computer_call.pending_safety_checks[0].message.as_deref(),
        Some("Browser confirmation required")
    );
    assert!(matches!(
        computer_call.action.as_ref(),
        Some(ResponseItemAction::Computer(ResponseComputerAction::Click(action)))
            if action.button == "left" && action.x == 10 && action.y == 20
    ));
    assert!(matches!(
        computer_call.actions.as_ref().unwrap().as_slice(),
        [
            ResponseComputerAction::Keypress(_),
            ResponseComputerAction::Type(_),
            ResponseComputerAction::Wait
        ]
    ));

    let computer_output = &listed.output().data[4];
    assert_eq!(computer_output.item_type, "computer_call_output");
    assert_eq!(
        computer_output.acknowledged_safety_checks[0].id,
        "safety_ack_1"
    );
    assert_eq!(
        computer_output.acknowledged_safety_checks[0]
            .message
            .as_deref(),
        Some("Acknowledged")
    );
    assert!(matches!(
        computer_output.output.as_ref(),
        Some(ResponseItemOutput::ComputerScreenshot(screenshot))
            if screenshot.image_url.as_deref() == Some("data:image/png;base64,AA==")
    ));

    let custom_output = &listed.output().data[5];
    assert_eq!(custom_output.item_type, "custom_tool_call_output");
    assert!(matches!(
        custom_output.output.as_ref(),
        Some(ResponseItemOutput::ContentList(parts))
            if parts.len() == 1
                && parts[0].content_type == "input_text"
                && parts[0].text.as_deref() == Some("custom payload")
    ));

    let shell_output = &listed.output().data[6];
    assert_eq!(shell_output.item_type, "shell_call_output");
    assert_eq!(shell_output.max_output_length, Some(4096));
    assert!(matches!(
        shell_output.output.as_ref(),
        Some(ResponseItemOutput::Shell(outputs))
            if outputs.len() == 1
                && outputs[0].stdout == "ok\n"
                && matches!(
                    &outputs[0].outcome,
                    ResponseShellOutputOutcome::Exit(outcome) if outcome.exit_code == 0
                )
    ));

    let local_shell_call = &listed.output().data[7];
    assert_eq!(local_shell_call.item_type, "local_shell_call");
    assert!(matches!(
        local_shell_call.action.as_ref(),
        Some(ResponseItemAction::LocalShell(action))
            if action.command == ["ls", "-la"]
                && action.env.get("LC_ALL").map(String::as_str) == Some("C")
    ));

    let shell_call = &listed.output().data[8];
    assert_eq!(shell_call.item_type, "shell_call");
    assert!(matches!(
        shell_call.action.as_ref(),
        Some(ResponseItemAction::Shell(action))
            if action.commands == ["echo ok"] && action.max_output_length == Some(2048)
    ));
    assert!(matches!(
        shell_call.environment.as_ref(),
        Some(ResponseItemEnvironment::Local(_))
    ));

    let apply_patch_call = &listed.output().data[9];
    assert_eq!(apply_patch_call.item_type, "apply_patch_call");
    assert!(matches!(
        apply_patch_call.operation.as_ref(),
        Some(ResponseApplyPatchOperation::CreateFile(operation))
            if operation.path == "src/new.rs" && operation.diff == "@@ +hello"
    ));

    let tool_search = listed
        .output()
        .data
        .iter()
        .find(|item| item.item_type == "tool_search_output")
        .expect("tool search output item");
    assert_eq!(tool_search.call_id.as_deref(), Some("call_tool_search"));
    assert_eq!(tool_search.execution.as_deref(), Some("server"));
    assert!(matches!(
        tool_search.tools[0].as_definition(),
        Some(ResponseTool::Function(tool))
            if tool.name == "lookup_weather"
                && tool.parameters["type"] == "object"
                && tool.description.as_deref() == Some("Lookup weather")
    ));
    assert!(matches!(
        tool_search.tools[1].as_definition(),
        Some(ResponseTool::WebSearchPreview(tool))
            if tool.search_context_size.as_ref().map(SearchContextSize::as_str) == Some("low")
    ));

    let retrieved_function_call = retrieved.output();
    assert_eq!(
        retrieved_function_call.name.as_deref(),
        Some("lookup_weather")
    );
    assert_eq!(
        retrieved_function_call.arguments.as_deref(),
        Some(r#"{"city":"Paris"}"#)
    );
    assert_eq!(retrieved_function_call.call_id.as_deref(), Some("call_123"));
    assert_eq!(retrieved_function_call.status.as_deref(), Some("completed"));
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn items_envelope(items: Vec<Value>) -> String {
    let first_id = items
        .first()
        .and_then(|item| item.get("id"))
        .cloned()
        .unwrap_or(Value::Null);
    let last_id = items
        .last()
        .and_then(|item| item.get("id"))
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "object": "list",
        "data": items,
        "first_id": first_id,
        "last_id": last_id,
        "has_more": true
    })
    .to_string()
}

fn message_item(id: &str, text: &str) -> Value {
    json!({
        "id": id,
        "type": "message",
        "role": "user",
        "status": "completed",
        "content": [{"type": "input_text", "text": text}]
    })
}

fn reasoning_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "reasoning",
        "status": "completed",
        "summary": [{"type": "summary_text", "text": "Plan"}],
        "encrypted_content": "encrypted_reasoning"
    })
}

fn function_call_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "function_call",
        "name": "lookup_weather",
        "arguments": {"city": "Paris"},
        "call_id": "call_123",
        "status": "completed",
        "created_by": "model"
    })
}

fn refusal_message_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "status": "completed",
        "content": [
            {"type": "refusal", "refusal": "I can't help with that"},
            {
                "type": "input_image",
                "detail": "high",
                "file_id": "file_img",
                "image_url": "https://example.com/cat.png"
            },
            {
                "type": "output_text",
                "text": "See source",
                "annotations": [{"type": "url_citation", "url": "https://example.com"}],
                "logprobs": [{"token": "See", "bytes": [83, 101, 101], "logprob": -0.1, "top_logprobs": []}]
            }
        ]
    })
}

fn mcp_list_tools_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "mcp_list_tools",
        "server_label": "deepwiki",
        "tools": [{
            "name": "search_docs",
            "input_schema": {"type": "object"},
            "annotations": {"readOnlyHint": true},
            "description": "Search docs"
        }]
    })
}

fn tool_search_output_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "tool_search_output",
        "call_id": "call_tool_search",
        "execution": "server",
        "status": "completed",
        "created_by": "assistant",
        "tools": [
            {
                "type": "function",
                "name": "lookup_weather",
                "parameters": {"type": "object"},
                "description": "Lookup weather"
            },
            {
                "type": "web_search_preview",
                "search_context_size": "low"
            }
        ]
    })
}

fn computer_call_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "computer_call",
        "call_id": "call_computer",
        "status": "completed",
        "action": {"type": "click", "button": "left", "x": 10, "y": 20},
        "actions": [
            {"type": "keypress", "keys": ["CTRL", "L"]},
            {"type": "type", "text": "openai.com"},
            {"type": "wait"}
        ],
        "pending_safety_checks": [{
            "id": "safety_pending_1",
            "code": "unsafe_browser",
            "message": "Browser confirmation required"
        }]
    })
}

fn computer_call_output_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "computer_call_output",
        "call_id": "call_computer",
        "status": "completed",
        "output": {"type": "computer_screenshot", "image_url": "data:image/png;base64,AA=="},
        "acknowledged_safety_checks": [{
            "id": "safety_ack_1",
            "code": "unsafe_browser",
            "message": "Acknowledged"
        }]
    })
}

fn custom_tool_call_output_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "custom_tool_call_output",
        "call_id": "call_custom",
        "status": "completed",
        "output": [{"type": "input_text", "text": "custom payload"}]
    })
}

fn shell_call_output_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "shell_call_output",
        "call_id": "call_shell",
        "status": "completed",
        "max_output_length": 4096,
        "output": [{
            "stdout": "ok\n",
            "stderr": "",
            "outcome": {"type": "exit", "exit_code": 0}
        }]
    })
}

fn local_shell_call_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "local_shell_call",
        "call_id": "call_local_shell",
        "status": "completed",
        "action": {
            "type": "exec",
            "command": ["ls", "-la"],
            "env": {"LC_ALL": "C"}
        }
    })
}

fn shell_call_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "shell_call",
        "call_id": "call_shell",
        "status": "completed",
        "action": {"commands": ["echo ok"], "max_output_length": 2048},
        "environment": {"type": "local"}
    })
}

fn apply_patch_call_item(id: &str) -> Value {
    json!({
        "id": id,
        "type": "apply_patch_call",
        "call_id": "call_patch",
        "status": "completed",
        "operation": {"type": "create_file", "path": "src/new.rs", "diff": "@@ +hello"}
    })
}

fn conversation_payload(id: &str, metadata: Value) -> String {
    json!({
        "id": id,
        "object": "conversation",
        "created_at": 1,
        "metadata": metadata
    })
    .to_string()
}
