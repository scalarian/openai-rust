#[path = "support/mock_http.rs"]
mod mock_http;

use std::{collections::BTreeMap, time::Duration};

use openai_rust::{
    ErrorKind, OpenAI,
    core::metadata::ResponseMetadata,
    resources::beta::{
        BetaAssistantCreateParams, BetaAssistantListParams, BetaAssistantResponseFormat,
        BetaAssistantResponseFormatJsonSchema, BetaAssistantStream, BetaAssistantTool,
        BetaAssistantToolChoice, BetaAssistantToolChoiceFunction, BetaAssistantUpdateParams,
        BetaQueryParams, BetaRunPollOptions, BetaThreadCreateAndRunParams, BetaThreadCreateParams,
        BetaThreadMessageAttachment, BetaThreadMessageAttachmentTool, BetaThreadMessageContent,
        BetaThreadMessageCreateParams, BetaThreadMessageListParams, BetaThreadMessageUpdateParams,
        BetaThreadRunAdditionalMessage, BetaThreadRunCreateParams, BetaThreadRunListParams,
        BetaThreadRunStepListParams, BetaThreadRunStepRetrieveParams,
        BetaThreadRunSubmitToolOutputsParams, BetaThreadRunToolOutput, BetaThreadRunUpdateParams,
        BetaThreadUpdateParams, BetaToolResourceFileSearchOverrides, BetaToolResourceOverrides,
        BetaToolResources, BetaToolResourcesCodeInterpreter, BetaTruncationStrategy,
    },
};
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
        .create(BetaAssistantCreateParams {
            model: String::from("gpt-4.1"),
            name: Some(String::from("Support analyst")),
            instructions: Some(String::from("Answer succinctly.")),
            reasoning_effort: Some(String::from("low")),
            tools: Some(vec![BetaAssistantTool::code_interpreter()]),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(assistant.output["id"], json!("asst_123"));

    assert_eq!(
        assistants.retrieve("asst_123").unwrap().output["object"],
        json!("assistant")
    );
    assert_eq!(
        assistants
            .update(
                "asst_123",
                BetaAssistantUpdateParams {
                    metadata: Some(BTreeMap::from([(
                        String::from("tier"),
                        String::from("gold"),
                    )])),
                    response_format: Some(BetaAssistantResponseFormat::JsonObject),
                    reasoning_effort: Some(String::from("minimal")),
                    ..Default::default()
                },
            )
            .unwrap()
            .output["id"],
        json!("asst_123")
    );
    assert_eq!(
        assistants
            .list(BetaAssistantListParams {
                after: Some(String::from("asst_after")),
                before: Some(String::from("asst_before")),
                limit: Some(2),
                order: Some(String::from("asc")),
            })
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
            .create(BetaThreadCreateParams {
                messages: Some(vec![BetaThreadMessageCreateParams {
                    role: String::from("user"),
                    content: BetaThreadMessageContent::from("Hello"),
                    attachments: None,
                    metadata: Some(BTreeMap::from([(
                        String::from("source"),
                        String::from("contract"),
                    )])),
                }]),
                metadata: Some(BTreeMap::from([(
                    String::from("case_id"),
                    String::from("case_123"),
                )])),
                tool_resources: Some(BetaToolResources {
                    code_interpreter: Some(BetaToolResourcesCodeInterpreter {
                        file_ids: vec![String::from("file_123")],
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            })
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
            .update(
                "thread_123",
                BetaThreadUpdateParams {
                    metadata: Some(BTreeMap::from([(
                        String::from("priority"),
                        String::from("high"),
                    )])),
                    tool_resources: Some(BetaToolResourceOverrides {
                        file_search: Some(BetaToolResourceFileSearchOverrides {
                            vector_store_ids: vec![String::from("vs_123")],
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                },
            )
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
            .create_and_run(BetaThreadCreateAndRunParams {
                assistant_id: String::from("asst_123"),
                max_prompt_tokens: Some(512),
                parallel_tool_calls: Some(true),
                response_format: Some(BetaAssistantResponseFormat::Text),
                tool_choice: Some(BetaAssistantToolChoice::Function(
                    BetaAssistantToolChoiceFunction {
                        name: String::from("lookup_case"),
                        ..Default::default()
                    },
                )),
                truncation_strategy: Some(BetaTruncationStrategy::last_messages(4)),
                thread: Some(BetaThreadCreateParams {
                    messages: Some(vec![BetaThreadMessageCreateParams {
                        role: String::from("user"),
                        content: BetaThreadMessageContent::from("Hello"),
                        attachments: None,
                        metadata: None,
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .unwrap()
            .output["id"],
        json!("run_created")
    );

    let messages = threads.messages();
    assert_eq!(
        messages
            .create(
                "thread_123",
                BetaThreadMessageCreateParams {
                    role: String::from("user"),
                    content: BetaThreadMessageContent::from("What is the status?"),
                    attachments: Some(vec![BetaThreadMessageAttachment {
                        file_id: Some(String::from("file_123")),
                        tools: vec![BetaThreadMessageAttachmentTool::CodeInterpreter],
                    }]),
                    metadata: Some(BTreeMap::from([(
                        String::from("source"),
                        String::from("customer"),
                    )])),
                },
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
                BetaThreadMessageUpdateParams {
                    metadata: Some(BTreeMap::from([(
                        String::from("seen"),
                        String::from("true"),
                    )])),
                },
            )
            .unwrap()
            .output["id"],
        json!("msg_123")
    );
    assert_eq!(
        messages
            .list(
                "thread_123",
                BetaThreadMessageListParams {
                    after: Some(String::from("msg_after")),
                    before: Some(String::from("msg_before")),
                    limit: Some(3),
                    order: Some(String::from("desc")),
                    run_id: Some(String::from("run_123")),
                }
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
            BetaThreadRunCreateParams {
                assistant_id: String::from("asst_123"),
                additional_instructions: Some(String::from("Use the support playbook.")),
                additional_messages: Some(vec![BetaThreadRunAdditionalMessage {
                    role: String::from("user"),
                    content: BetaThreadMessageContent::from("Any update?"),
                    attachments: None,
                    metadata: None,
                }]),
                max_completion_tokens: Some(256),
                parallel_tool_calls: Some(true),
                reasoning_effort: Some(String::from("low")),
                response_format: Some(BetaAssistantResponseFormat::JsonSchema(
                    BetaAssistantResponseFormatJsonSchema {
                        name: String::from("status_update"),
                        schema: json!({
                            "type": "object",
                            "properties": {
                                "status": {"type": "string"}
                            },
                            "required": ["status"]
                        }),
                        strict: Some(true),
                        ..Default::default()
                    },
                )),
                tool_choice: Some(BetaAssistantToolChoice::CodeInterpreter),
                tools: Some(vec![BetaAssistantTool::code_interpreter()]),
                truncation_strategy: Some(BetaTruncationStrategy::auto()),
                ..Default::default()
            },
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
            BetaThreadRunUpdateParams {
                metadata: Some(BTreeMap::from([(
                    String::from("owner"),
                    String::from("support"),
                )])),
            },
        )
        .unwrap()
        .output["id"],
        json!("run_123")
    );
    assert_eq!(
        runs.list(
            "thread_123",
            BetaThreadRunListParams {
                after: Some(String::from("run_after")),
                before: Some(String::from("run_before")),
                limit: Some(4),
                order: Some(String::from("asc")),
            }
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
            BetaThreadRunSubmitToolOutputsParams {
                tool_outputs: vec![BetaThreadRunToolOutput {
                    tool_call_id: Some(String::from("call_123")),
                    output: Some(String::from("done")),
                }],
                stream: None,
            },
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
                BetaThreadRunStepRetrieveParams {
                    include: Some(vec![String::from("step_details")]),
                },
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
                BetaThreadRunStepListParams {
                    after: Some(String::from("step_after")),
                    before: Some(String::from("step_before")),
                    include: Some(vec![String::from("step_details")]),
                    limit: Some(1),
                    order: Some(String::from("asc")),
                },
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
        "/v1/assistants?after=asst_after&before=asst_before&limit=2&order=asc"
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
        "/v1/threads/thread_123/messages?after=msg_after&before=msg_before&limit=3&order=desc&run_id=run_123"
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
        "/v1/threads/thread_123/runs?after=run_after&before=run_before&limit=4&order=asc"
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
        "/v1/threads/thread_123/runs/run_123/steps?after=step_after&before=step_before&limit=1&order=asc&include%5B%5D=step_details"
    );
    for request in &requests {
        assert_eq!(
            request.headers.get("openai-beta").map(String::as_str),
            Some("assistants=v2")
        );
    }

    let assistant_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(assistant_body["model"], json!("gpt-4.1"));
    assert_eq!(assistant_body["reasoning_effort"], json!("low"));
    assert_eq!(
        assistant_body["tools"][0]["type"],
        json!("code_interpreter")
    );
    let assistant_update_body: serde_json::Value =
        serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(assistant_update_body["metadata"]["tier"], json!("gold"));
    assert_eq!(assistant_update_body["reasoning_effort"], json!("minimal"));
    assert_eq!(
        assistant_update_body["response_format"]["type"],
        json!("json_object")
    );
    let empty_thread_body: serde_json::Value = serde_json::from_slice(&requests[5].body).unwrap();
    assert_eq!(empty_thread_body, json!({}));
    let thread_body: serde_json::Value = serde_json::from_slice(&requests[6].body).unwrap();
    assert_eq!(thread_body["messages"][0]["content"], json!("Hello"));
    assert_eq!(
        thread_body["tool_resources"]["code_interpreter"]["file_ids"][0],
        json!("file_123")
    );
    let thread_update_body: serde_json::Value = serde_json::from_slice(&requests[8].body).unwrap();
    assert_eq!(thread_update_body["metadata"]["priority"], json!("high"));
    assert_eq!(
        thread_update_body["tool_resources"]["file_search"]["vector_store_ids"][0],
        json!("vs_123")
    );
    let thread_run_body: serde_json::Value = serde_json::from_slice(&requests[10].body).unwrap();
    assert_eq!(thread_run_body["assistant_id"], json!("asst_123"));
    assert_eq!(thread_run_body["max_prompt_tokens"], json!(512));
    assert_eq!(thread_run_body["parallel_tool_calls"], json!(true));
    assert_eq!(thread_run_body["response_format"]["type"], json!("text"));
    assert_eq!(thread_run_body["tool_choice"]["type"], json!("function"));
    assert_eq!(
        thread_run_body["tool_choice"]["function"]["name"],
        json!("lookup_case")
    );
    assert_eq!(
        thread_run_body["truncation_strategy"]["type"],
        json!("last_messages")
    );
    assert_eq!(
        thread_run_body["truncation_strategy"]["last_messages"],
        json!(4)
    );
    assert_eq!(
        thread_run_body["thread"]["messages"][0]["content"],
        json!("Hello")
    );
    let message_body: serde_json::Value = serde_json::from_slice(&requests[11].body).unwrap();
    assert_eq!(message_body["role"], json!("user"));
    assert_eq!(message_body["attachments"][0]["file_id"], json!("file_123"));
    let message_update_body: serde_json::Value =
        serde_json::from_slice(&requests[13].body).unwrap();
    assert_eq!(message_update_body["metadata"]["seen"], json!("true"));
    let run_body: serde_json::Value = serde_json::from_slice(&requests[16].body).unwrap();
    assert_eq!(run_body["assistant_id"], json!("asst_123"));
    assert_eq!(run_body["parallel_tool_calls"], json!(true));
    assert_eq!(run_body["reasoning_effort"], json!("low"));
    assert_eq!(run_body["response_format"]["type"], json!("json_schema"));
    assert_eq!(
        run_body["response_format"]["json_schema"]["name"],
        json!("status_update")
    );
    assert_eq!(run_body["tool_choice"]["type"], json!("code_interpreter"));
    assert_eq!(run_body["tools"][0]["type"], json!("code_interpreter"));
    assert_eq!(run_body["truncation_strategy"]["type"], json!("auto"));
    let run_update_body: serde_json::Value = serde_json::from_slice(&requests[18].body).unwrap();
    assert_eq!(run_update_body["metadata"]["owner"], json!("support"));
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

#[test]
fn beta_assistants_stream_parser_preserves_raw_sse_events() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let mut stream = BetaAssistantStream::from_sse_chunks(
        metadata.clone(),
        [concat!(
            "event: thread.run.created\n",
            "data: {\"id\":\"run_stream\",\"status\":\"queued\"}\n\n",
            "data: [DONE]\n\n"
        )],
    )
    .expect("stream transcript");

    assert_eq!(stream.metadata().status_code(), 200);
    let event = stream
        .next_event()
        .expect("event read")
        .expect("first event");
    assert_eq!(event.event.as_deref(), Some("thread.run.created"));
    assert_eq!(event.data["id"], json!("run_stream"));
    assert_eq!(
        event.raw_data,
        "{\"id\":\"run_stream\",\"status\":\"queued\"}"
    );
    assert!(stream.next_event().expect("eof").is_none());

    let error = BetaAssistantStream::from_sse_chunks(
        metadata,
        ["data: {\"id\":\"missing_done\",\"status\":\"queued\"}\n\n"],
    )
    .expect_err("missing [DONE] marker should fail");
    assert_eq!(error.kind, ErrorKind::Transport);
}

#[test]
fn beta_assistants_stream_helpers_preserve_routes_headers_and_stream_bodies() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        sse_response(
            "req_run_stream",
            "thread.run.created",
            json!({"id": "run_stream", "status": "queued"}),
        ),
        sse_response(
            "req_thread_run_stream",
            "thread.run.created",
            json!({"id": "run_thread_stream", "status": "in_progress"}),
        ),
        sse_response(
            "req_tool_stream",
            "thread.run.requires_action",
            json!({"id": "run_tool_stream", "status": "requires_action"}),
        ),
    ])
    .unwrap();
    let client = client(&server.url());
    let threads = client.beta().threads();
    let runs = threads.runs();

    let mut run_stream = runs
        .create_and_stream("thread_123", json!({"assistant_id": "asst_123"}))
        .expect("run stream");
    assert_eq!(run_stream.metadata().request_id(), Some("req_run_stream"));
    assert_eq!(
        run_stream
            .next_event()
            .expect("run event")
            .expect("run event")
            .data["id"],
        json!("run_stream")
    );
    assert!(run_stream.next_event().expect("run eof").is_none());

    let mut thread_stream = threads
        .create_and_run_stream(BetaThreadCreateAndRunParams {
            assistant_id: String::from("asst_123"),
            thread: Some(BetaThreadCreateParams {
                messages: Some(vec![]),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("thread run stream");
    assert_eq!(
        thread_stream.metadata().request_id(),
        Some("req_thread_run_stream")
    );
    assert_eq!(
        thread_stream
            .next_event()
            .expect("thread event")
            .expect("thread event")
            .data["id"],
        json!("run_thread_stream")
    );
    assert!(thread_stream.next_event().expect("thread eof").is_none());

    let mut tool_stream = runs
        .submit_tool_outputs_stream(
            "thread_123",
            "run_123",
            json!({"tool_outputs": [{"tool_call_id": "call_123", "output": "done"}]}),
        )
        .expect("tool stream");
    assert_eq!(tool_stream.metadata().request_id(), Some("req_tool_stream"));
    assert_eq!(
        tool_stream
            .next_event()
            .expect("tool event")
            .expect("tool event")
            .data["status"],
        json!("requires_action")
    );
    assert!(tool_stream.next_event().expect("tool eof").is_none());

    let requests = server.captured_requests(3).unwrap();
    assert_methods(&requests, &["POST", "POST", "POST"]);
    assert_eq!(requests[0].path, "/v1/threads/thread_123/runs");
    assert_eq!(requests[1].path, "/v1/threads/runs");
    assert_eq!(
        requests[2].path,
        "/v1/threads/thread_123/runs/run_123/submit_tool_outputs"
    );
    assert_stream_headers(&requests[0], "threads.runs.create_and_stream");
    assert_stream_headers(&requests[1], "threads.create_and_run_stream");
    assert_stream_headers(&requests[2], "threads.runs.submit_tool_outputs_stream");

    let run_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(run_body["assistant_id"], json!("asst_123"));
    assert_eq!(run_body["stream"], json!(true));
    let thread_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(thread_body["thread"]["messages"], json!([]));
    assert_eq!(thread_body["stream"], json!(true));
    let tool_body: serde_json::Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(
        tool_body["tool_outputs"][0]["tool_call_id"],
        json!("call_123")
    );
    assert_eq!(tool_body["stream"], json!(true));
}

#[test]
fn beta_assistants_poll_helpers_preserve_routes_and_terminal_statuses() {
    let poll_options = BetaRunPollOptions {
        poll_interval: Some(Duration::from_millis(1)),
        max_wait: Duration::from_secs(1),
    };
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response_with_header(
            run_payload("run_poll", "queued"),
            "openai-poll-after-ms",
            "1",
        ),
        json_response(run_payload("run_poll", "completed")),
        json_response(run_payload("run_created_poll", "queued")),
        json_response(run_payload("run_created_poll", "completed")),
        json_response(run_payload("run_tool_poll", "queued")),
        json_response(run_payload("run_tool_poll", "completed")),
        json_response(run_payload("run_thread_poll", "queued")),
        json_response(run_payload("run_thread_poll", "requires_action")),
    ])
    .unwrap();
    let client = client(&server.url());
    let threads = client.beta().threads();
    let runs = threads.runs();

    let direct = runs
        .poll(
            "thread_123",
            "run_poll",
            BetaRunPollOptions {
                poll_interval: None,
                max_wait: Duration::from_secs(1),
            },
        )
        .expect("direct poll");
    assert_eq!(direct.output["status"], json!("completed"));

    let created = runs
        .create_and_poll(
            "thread_123",
            json!({"assistant_id": "asst_123"}),
            poll_options.clone(),
        )
        .expect("create and poll");
    assert_eq!(created.output["status"], json!("completed"));

    let submitted = runs
        .submit_tool_outputs_and_poll(
            "thread_123",
            "run_needs_tools",
            json!({"tool_outputs": [{"tool_call_id": "call_123", "output": "done"}]}),
            poll_options.clone(),
        )
        .expect("submit tool outputs and poll");
    assert_eq!(submitted.output["status"], json!("completed"));

    let thread_created = threads
        .create_and_run_poll(
            BetaThreadCreateAndRunParams {
                assistant_id: String::from("asst_123"),
                thread: Some(BetaThreadCreateParams {
                    messages: Some(vec![]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            poll_options,
        )
        .expect("create thread and poll");
    assert_eq!(thread_created.output["status"], json!("requires_action"));

    let requests = server.captured_requests(8).unwrap();
    assert_methods(
        &requests,
        &["GET", "GET", "POST", "GET", "POST", "GET", "POST", "GET"],
    );
    assert_eq!(requests[0].path, "/v1/threads/thread_123/runs/run_poll");
    assert_eq!(requests[1].path, "/v1/threads/thread_123/runs/run_poll");
    assert_eq!(requests[2].path, "/v1/threads/thread_123/runs");
    assert_eq!(
        requests[3].path,
        "/v1/threads/thread_123/runs/run_created_poll"
    );
    assert_eq!(
        requests[4].path,
        "/v1/threads/thread_123/runs/run_needs_tools/submit_tool_outputs"
    );
    assert_eq!(
        requests[5].path,
        "/v1/threads/thread_123/runs/run_tool_poll"
    );
    assert_eq!(requests[6].path, "/v1/threads/runs");
    assert_eq!(
        requests[7].path,
        "/v1/threads/thread_123/runs/run_thread_poll"
    );
    for request in &requests {
        assert_eq!(
            request.headers.get("openai-beta").map(String::as_str),
            Some("assistants=v2")
        );
    }
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

fn assert_stream_headers(request: &mock_http::CapturedRequest, helper: &str) {
    assert_eq!(
        request.headers.get("openai-beta").map(String::as_str),
        Some("assistants=v2")
    );
    assert_eq!(
        request.headers.get("accept").map(String::as_str),
        Some("text/event-stream")
    );
    assert_eq!(
        request
            .headers
            .get("x-stainless-stream-helper")
            .map(String::as_str),
        Some(helper)
    );
    assert_eq!(
        request
            .headers
            .get("x-stainless-custom-event-handler")
            .map(String::as_str),
        Some("false")
    );
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
    json_response_with_headers(body, Vec::new())
}

fn json_response_with_header(
    body: String,
    name: impl Into<String>,
    value: impl Into<String>,
) -> mock_http::ScriptedResponse {
    json_response_with_headers(body, vec![(name.into(), value.into())])
}

fn json_response_with_headers(
    body: String,
    extra_headers: Vec<(String, String)>,
) -> mock_http::ScriptedResponse {
    let mut headers = vec![
        (String::from("content-length"), body.len().to_string()),
        (
            String::from("content-type"),
            String::from("application/json"),
        ),
        (
            String::from("x-request-id"),
            String::from("req_beta_assistants"),
        ),
    ];
    headers.extend(extra_headers);

    mock_http::ScriptedResponse {
        headers,
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn sse_response(
    request_id: &str,
    event: &str,
    data: serde_json::Value,
) -> mock_http::ScriptedResponse {
    let body = format!("event: {event}\ndata: {data}\n\ndata: [DONE]\n\n");
    mock_http::ScriptedResponse {
        headers: vec![
            (
                String::from("content-type"),
                String::from("text/event-stream"),
            ),
            (String::from("x-request-id"), request_id.to_string()),
        ],
        body: body.into_bytes(),
        chunked: true,
        ..Default::default()
    }
}
