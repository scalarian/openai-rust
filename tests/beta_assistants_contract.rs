#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{ErrorKind, OpenAI, resources::beta::BetaQueryParams};
use serde_json::json;

#[test]
fn beta_assistants_threads_runs_and_steps_preserve_routes_headers_and_bodies() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(assistant_payload("asst_123")),
        json_response(assistant_payload("asst_123")),
        json_response(assistant_payload("asst_123")),
        json_response(list_payload("assistant", "asst_123")),
        json_response(delete_payload("assistant", "asst_123")),
        json_response(thread_payload("thread_empty")),
        json_response(thread_payload("thread_123")),
        json_response(thread_payload("thread_123")),
        json_response(thread_payload("thread_123")),
        json_response(delete_payload("thread", "thread_123")),
        json_response(run_payload("run_created", "queued")),
        json_response(message_payload("msg_123")),
        json_response(message_payload("msg_123")),
        json_response(message_payload("msg_123")),
        json_response(list_payload("thread.message", "msg_123")),
        json_response(delete_payload("thread.message", "msg_123")),
        json_response(run_payload("run_123", "queued")),
        json_response(run_payload("run_123", "in_progress")),
        json_response(run_payload("run_123", "in_progress")),
        json_response(list_payload("thread.run", "run_123")),
        json_response(run_payload("run_123", "cancelled")),
        json_response(run_payload("run_123", "in_progress")),
        json_response(step_payload("step_123")),
        json_response(list_payload("thread.run.step", "step_123")),
    ])
    .unwrap();
    let client = client(&server.url());
    let beta = client.beta();
    let assistants = beta.assistants();
    let threads = beta.threads();

    let assistant = assistants
        .create(json!({
            "model": "gpt-4.1",
            "name": "Support analyst",
            "instructions": "Answer succinctly."
        }))
        .unwrap();
    assert_eq!(assistant.output["id"], json!("asst_123"));

    assert_eq!(
        assistants.retrieve("asst_123").unwrap().output["object"],
        json!("assistant")
    );
    assert_eq!(
        assistants
            .update("asst_123", json!({"metadata": {"tier": "gold"}}))
            .unwrap()
            .output["id"],
        json!("asst_123")
    );
    assert_eq!(
        assistants
            .list(
                BetaQueryParams::new()
                    .push("after", "asst_before")
                    .push("limit", 2)
                    .push("order", "asc")
            )
            .unwrap()
            .output["data"][0]["id"],
        json!("asst_123")
    );
    assert_eq!(
        assistants.delete("asst_123").unwrap().output["deleted"],
        json!(true)
    );

    assert_eq!(
        threads.create_empty().unwrap().output["id"],
        json!("thread_empty")
    );
    assert_eq!(
        threads
            .create(json!({"metadata": {"case_id": "case_123"}}))
            .unwrap()
            .output["id"],
        json!("thread_123")
    );
    assert_eq!(
        threads.retrieve("thread_123").unwrap().output["id"],
        json!("thread_123")
    );
    assert_eq!(
        threads
            .update("thread_123", json!({"metadata": {"priority": "high"}}))
            .unwrap()
            .output["id"],
        json!("thread_123")
    );
    assert_eq!(
        threads.delete("thread_123").unwrap().output["deleted"],
        json!(true)
    );
    assert_eq!(
        threads
            .create_and_run(json!({
                "assistant_id": "asst_123",
                "thread": {"messages": [{"role": "user", "content": "Hello"}]}
            }))
            .unwrap()
            .output["id"],
        json!("run_created")
    );

    let messages = threads.messages();
    assert_eq!(
        messages
            .create(
                "thread_123",
                json!({"role": "user", "content": "What is the status?"})
            )
            .unwrap()
            .output["id"],
        json!("msg_123")
    );
    assert_eq!(
        messages.retrieve("thread_123", "msg_123").unwrap().output["id"],
        json!("msg_123")
    );
    assert_eq!(
        messages
            .update(
                "thread_123",
                "msg_123",
                json!({"metadata": {"seen": "true"}})
            )
            .unwrap()
            .output["id"],
        json!("msg_123")
    );
    assert_eq!(
        messages
            .list(
                "thread_123",
                BetaQueryParams::new()
                    .push("after", "msg_after")
                    .push("limit", 3)
                    .push("order", "desc")
                    .push("run_id", "run_123")
            )
            .unwrap()
            .output["data"][0]["id"],
        json!("msg_123")
    );
    assert_eq!(
        messages.delete("thread_123", "msg_123").unwrap().output["deleted"],
        json!(true)
    );

    let runs = threads.runs();
    assert_eq!(
        runs.create_with_query(
            "thread_123",
            json!({"assistant_id": "asst_123"}),
            BetaQueryParams::new().push_array("include", ["run_details"])
        )
        .unwrap()
        .output["id"],
        json!("run_123")
    );
    assert_eq!(
        runs.retrieve("thread_123", "run_123").unwrap().output["status"],
        json!("in_progress")
    );
    assert_eq!(
        runs.update(
            "thread_123",
            "run_123",
            json!({"metadata": {"owner": "support"}})
        )
        .unwrap()
        .output["id"],
        json!("run_123")
    );
    assert_eq!(
        runs.list(
            "thread_123",
            BetaQueryParams::new()
                .push("after", "run_after")
                .push("limit", 4)
                .push("order", "asc")
        )
        .unwrap()
        .output["data"][0]["id"],
        json!("run_123")
    );
    assert_eq!(
        runs.cancel("thread_123", "run_123").unwrap().output["status"],
        json!("cancelled")
    );
    assert_eq!(
        runs.submit_tool_outputs(
            "thread_123",
            "run_123",
            json!({"tool_outputs": [{"tool_call_id": "call_123", "output": "done"}]})
        )
        .unwrap()
        .output["id"],
        json!("run_123")
    );

    let steps = runs.steps();
    assert_eq!(
        steps
            .retrieve_with_query(
                "thread_123",
                "run_123",
                "step_123",
                BetaQueryParams::new().push_array("include", ["step_details"])
            )
            .unwrap()
            .output["id"],
        json!("step_123")
    );
    assert_eq!(
        steps
            .list(
                "thread_123",
                "run_123",
                BetaQueryParams::new()
                    .push("after", "step_after")
                    .push("limit", 1)
                    .push("order", "asc")
                    .push_array("include", ["step_details"])
            )
            .unwrap()
            .output["data"][0]["id"],
        json!("step_123")
    );

    let requests = server.captured_requests(24).unwrap();
    assert_methods(
        &requests,
        &[
            "POST", "GET", "POST", "GET", "DELETE", "POST", "POST", "GET", "POST", "DELETE",
            "POST", "POST", "GET", "POST", "GET", "DELETE", "POST", "GET", "POST", "GET", "POST",
            "POST", "GET", "GET",
        ],
    );
    assert_eq!(requests[0].path, "/v1/assistants");
    assert_eq!(requests[1].path, "/v1/assistants/asst_123");
    assert_eq!(requests[2].path, "/v1/assistants/asst_123");
    assert_eq!(
        requests[3].path,
        "/v1/assistants?after=asst_before&limit=2&order=asc"
    );
    assert_eq!(requests[4].path, "/v1/assistants/asst_123");
    assert_eq!(requests[5].path, "/v1/threads");
    assert_eq!(requests[6].path, "/v1/threads");
    assert_eq!(requests[7].path, "/v1/threads/thread_123");
    assert_eq!(requests[8].path, "/v1/threads/thread_123");
    assert_eq!(requests[9].path, "/v1/threads/thread_123");
    assert_eq!(requests[10].path, "/v1/threads/runs");
    assert_eq!(requests[11].path, "/v1/threads/thread_123/messages");
    assert_eq!(requests[12].path, "/v1/threads/thread_123/messages/msg_123");
    assert_eq!(requests[13].path, "/v1/threads/thread_123/messages/msg_123");
    assert_eq!(
        requests[14].path,
        "/v1/threads/thread_123/messages?after=msg_after&limit=3&order=desc&run_id=run_123"
    );
    assert_eq!(requests[15].path, "/v1/threads/thread_123/messages/msg_123");
    assert_eq!(
        requests[16].path,
        "/v1/threads/thread_123/runs?include%5B%5D=run_details"
    );
    assert_eq!(requests[17].path, "/v1/threads/thread_123/runs/run_123");
    assert_eq!(requests[18].path, "/v1/threads/thread_123/runs/run_123");
    assert_eq!(
        requests[19].path,
        "/v1/threads/thread_123/runs?after=run_after&limit=4&order=asc"
    );
    assert_eq!(
        requests[20].path,
        "/v1/threads/thread_123/runs/run_123/cancel"
    );
    assert_eq!(
        requests[21].path,
        "/v1/threads/thread_123/runs/run_123/submit_tool_outputs"
    );
    assert_eq!(
        requests[22].path,
        "/v1/threads/thread_123/runs/run_123/steps/step_123?include%5B%5D=step_details"
    );
    assert_eq!(
        requests[23].path,
        "/v1/threads/thread_123/runs/run_123/steps?after=step_after&limit=1&order=asc&include%5B%5D=step_details"
    );
    for request in &requests {
        assert_eq!(
            request.headers.get("openai-beta").map(String::as_str),
            Some("assistants=v2")
        );
    }

    let assistant_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(assistant_body["model"], json!("gpt-4.1"));
    let empty_thread_body: serde_json::Value = serde_json::from_slice(&requests[5].body).unwrap();
    assert_eq!(empty_thread_body, json!({}));
    let message_body: serde_json::Value = serde_json::from_slice(&requests[11].body).unwrap();
    assert_eq!(message_body["role"], json!("user"));
    let run_body: serde_json::Value = serde_json::from_slice(&requests[16].body).unwrap();
    assert_eq!(run_body["assistant_id"], json!("asst_123"));
    let tool_outputs_body: serde_json::Value = serde_json::from_slice(&requests[21].body).unwrap();
    assert_eq!(
        tool_outputs_body["tool_outputs"][0]["tool_call_id"],
        json!("call_123")
    );

    assert!(matches!(
        assistants.retrieve(" ").unwrap_err().kind,
        ErrorKind::Validation
    ));
    assert!(matches!(
        messages.retrieve("thread_123", "").unwrap_err().kind,
        ErrorKind::Validation
    ));
    assert!(matches!(
        steps
            .retrieve("thread_123", "run_123", "\t")
            .unwrap_err()
            .kind,
        ErrorKind::Validation
    ));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn assert_methods(requests: &[mock_http::CapturedRequest], expected: &[&str]) {
    let methods: Vec<&str> = requests
        .iter()
        .map(|request| request.method.as_str())
        .collect();
    assert_eq!(methods, expected);
}

fn assistant_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "assistant",
        "created_at": 1_800_000_000u64,
        "model": "gpt-4.1",
        "name": "Support analyst"
    })
    .to_string()
}

fn thread_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "thread",
        "created_at": 1_800_000_001u64,
        "metadata": {"case_id": "case_123"}
    })
    .to_string()
}

fn message_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "thread.message",
        "created_at": 1_800_000_002u64,
        "thread_id": "thread_123",
        "role": "user",
        "content": [{"type": "text", "text": {"value": "hello"}}]
    })
    .to_string()
}

fn run_payload(id: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "thread.run",
        "created_at": 1_800_000_003u64,
        "thread_id": "thread_123",
        "assistant_id": "asst_123",
        "status": status
    })
    .to_string()
}

fn step_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "thread.run.step",
        "created_at": 1_800_000_004u64,
        "run_id": "run_123",
        "thread_id": "thread_123",
        "type": "message_creation",
        "status": "completed"
    })
    .to_string()
}

fn list_payload(object: &str, id: &str) -> String {
    json!({
        "object": "list",
        "data": [{
            "id": id,
            "object": object,
            "created_at": 1_800_000_010u64
        }],
        "first_id": id,
        "last_id": id,
        "has_more": false
    })
    .to_string()
}

fn delete_payload(object: &str, id: &str) -> String {
    json!({
        "id": id,
        "object": format!("{object}.deleted"),
        "deleted": true
    })
    .to_string()
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (
                String::from("x-request-id"),
                String::from("req_beta_assistants"),
            ),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}
