use std::collections::BTreeMap;

use openai_rust::{
    ApiErrorKind, ErrorKind, OpenAI,
    resources::{
        common::{
            PromptCacheRetention, ReasoningEffort, SearchContextSize, ServiceTier, Truncation,
        },
        containers::{ContainerMemoryLimit, ContainerNetworkPolicy},
        responses::{
            ResponseApplyPatchOperation, ResponseCodeInterpreterContainer,
            ResponseCodeInterpreterOutput, ResponseCodeInterpreterTool, ResponseCompactServiceTier,
            ResponseComputerAction, ResponseContextManagement, ResponseConversation,
            ResponseConversationObject, ResponseCustomTool, ResponseCustomToolGrammar,
            ResponseCustomToolGrammarSyntax, ResponseCustomToolInputFormat,
            ResponseFileSearchAttributeValue, ResponseFileSearchFilter,
            ResponseFileSearchFilterValue, ResponseFileSearchRanker, ResponseFormatTextConfig,
            ResponseIncludable, ResponseInput, ResponseInputContentPart, ResponseInputItem,
            ResponseInputText, ResponseInstructions, ResponseItemAction, ResponseItemEnvironment,
            ResponseItemOutput, ResponseItemRole, ResponseItemStatus, ResponseItemType,
            ResponseMcpAllowedTools, ResponseMcpApprovalFilter, ResponseMcpRequireApproval,
            ResponseMcpTool, ResponseMcpToolFilter, ResponseMessagePhase, ResponsePrompt,
            ResponseReasoning, ResponseShellEnvironment, ResponseShellOutputOutcome,
            ResponseShellTool, ResponseStreamOptions, ResponseTextAnnotation, ResponseTool,
            ResponseToolChoice, ResponseWebSearchContentType, ResponseWebSearchPreviewTool,
        },
    },
};
use serde_json::{Value, json};

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn create_populates_output_text_helper() {
    let server = mock_http::MockHttpServer::spawn(json_response(response_payload(
        "resp_create",
        Some(true),
        Some("resp_prev"),
        Some(json!("conv_123")),
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            background: Some(true),
            context_management: vec![ResponseContextManagement {
                context_type: String::from("compaction"),
                compact_threshold: Some(2048),
                extra: BTreeMap::new(),
            }],
            include: vec![ResponseIncludable::MessageOutputTextLogprobs],
            input: Some(ResponseInput::text("hello")),
            instructions: Some(String::from("Be concise.")),
            max_output_tokens: Some(512),
            max_tool_calls: Some(4),
            metadata: Some(BTreeMap::from([(
                String::from("trace"),
                String::from("resp_create"),
            )])),
            parallel_tool_calls: Some(false),
            previous_response_id: Some("resp_prev".into()),
            conversation: Some(ResponseConversation::Id(String::from("conv_123"))),
            prompt: Some(ResponsePrompt {
                id: String::from("pmpt_123"),
                variables: Some(BTreeMap::from([(String::from("topic"), json!("Rust"))])),
                ..Default::default()
            }),
            prompt_cache_key: Some(String::from("cache-key")),
            prompt_cache_retention: Some(PromptCacheRetention::TwentyFourHours),
            reasoning: Some(ResponseReasoning {
                effort: Some(ReasoningEffort::Low),
                ..Default::default()
            }),
            safety_identifier: Some(String::from("user_hash")),
            service_tier: Some(ServiceTier::Priority),
            store: Some(true),
            stream: Some(false),
            stream_options: Some(ResponseStreamOptions {
                include_obfuscation: Some(false),
                extra: BTreeMap::new(),
            }),
            temperature: Some(0.2),
            tool_choice: Some(ResponseToolChoice::Function {
                name: String::from("lookup_weather"),
            }),
            top_logprobs: Some(2),
            top_p: Some(0.8),
            truncation: Some(Truncation::Auto),
            user: Some(String::from("legacy-user")),
            tools: vec![
                ResponseTool::WebSearchPreview(ResponseWebSearchPreviewTool {
                    search_content_types: vec![
                        ResponseWebSearchContentType::Text,
                        ResponseWebSearchContentType::Image,
                    ],
                    search_context_size: Some(SearchContextSize::Low),
                    user_location: None,
                    extra: BTreeMap::new(),
                }),
                ResponseTool::CodeInterpreter(ResponseCodeInterpreterTool {
                    container: ResponseCodeInterpreterContainer::Auto {
                        file_ids: vec![String::from("file_code")],
                        memory_limit: Some(ContainerMemoryLimit::G4),
                        network_policy: Some(ContainerNetworkPolicy::Disabled),
                        extra: BTreeMap::new(),
                    },
                    extra: BTreeMap::new(),
                }),
                ResponseTool::Mcp(ResponseMcpTool {
                    server_label: String::from("deepwiki"),
                    allowed_tools: Some(ResponseMcpAllowedTools::Filter(ResponseMcpToolFilter {
                        read_only: Some(true),
                        tool_names: vec![String::from("search_docs")],
                        extra: BTreeMap::new(),
                    })),
                    authorization: None,
                    connector_id: None,
                    defer_loading: Some(false),
                    headers: Some(BTreeMap::from([(
                        String::from("x-tenant"),
                        String::from("docs"),
                    )])),
                    require_approval: Some(ResponseMcpRequireApproval::Filter(
                        ResponseMcpApprovalFilter {
                            always: Some(ResponseMcpToolFilter {
                                read_only: Some(false),
                                tool_names: vec![String::from("write_docs")],
                                extra: BTreeMap::new(),
                            }),
                            never: Some(ResponseMcpToolFilter {
                                read_only: Some(true),
                                tool_names: Vec::new(),
                                extra: BTreeMap::new(),
                            }),
                            extra: BTreeMap::new(),
                        },
                    )),
                    server_description: Some(String::from("Docs MCP")),
                    server_url: Some(String::from("https://mcp.example.test")),
                    extra: BTreeMap::new(),
                }),
                ResponseTool::Shell(ResponseShellTool {
                    environment: Some(ResponseShellEnvironment::ContainerAuto {
                        file_ids: vec![String::from("file_shell")],
                        memory_limit: Some(ContainerMemoryLimit::G1),
                        network_policy: Some(ContainerNetworkPolicy::Allowlist {
                            allowed_domains: vec![String::from("api.example.com")],
                            domain_secrets: None,
                        }),
                        skills: Vec::new(),
                        extra: BTreeMap::new(),
                    }),
                }),
                ResponseTool::Custom(ResponseCustomTool {
                    name: String::from("query_parser"),
                    defer_loading: Some(true),
                    description: Some(String::from("Parse search query")),
                    format: Some(ResponseCustomToolInputFormat::Grammar(
                        ResponseCustomToolGrammar {
                            definition: String::from("start: WORD+"),
                            syntax: ResponseCustomToolGrammarSyntax::Lark,
                            extra: BTreeMap::new(),
                        },
                    )),
                    extra: BTreeMap::new(),
                }),
            ],
            ..Default::default()
        })
        .unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/responses");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["model"], "gpt-4.1-nano");
    assert_eq!(body["background"], true);
    assert_eq!(body["context_management"][0]["type"], "compaction");
    assert_eq!(body["context_management"][0]["compact_threshold"], 2048);
    assert_eq!(body["include"], json!(["message.output_text.logprobs"]));
    assert_eq!(body["input"], "hello");
    assert_eq!(body["instructions"], "Be concise.");
    assert_eq!(body["max_output_tokens"], 512);
    assert_eq!(body["max_tool_calls"], 4);
    assert_eq!(body["metadata"]["trace"], "resp_create");
    assert_eq!(body["parallel_tool_calls"], false);
    assert_eq!(body["previous_response_id"], "resp_prev");
    assert_eq!(body["conversation"], "conv_123");
    assert_eq!(body["prompt"]["id"], "pmpt_123");
    assert_eq!(body["prompt"]["variables"]["topic"], "Rust");
    assert_eq!(body["prompt_cache_key"], "cache-key");
    assert_eq!(body["prompt_cache_retention"], "24h");
    assert_eq!(body["reasoning"]["effort"], "low");
    assert_eq!(body["safety_identifier"], "user_hash");
    assert_eq!(body["service_tier"], "priority");
    assert_eq!(body["store"], true);
    assert_eq!(body["stream_options"]["include_obfuscation"], false);
    assert_eq!(body["temperature"], 0.2);
    assert_eq!(body["tool_choice"]["type"], "function");
    assert_eq!(body["tool_choice"]["name"], "lookup_weather");
    assert_eq!(body["stream"], false);
    assert_eq!(body["top_logprobs"], 2);
    assert_eq!(body["top_p"], 0.8);
    assert_eq!(body["truncation"], "auto");
    assert_eq!(body["user"], "legacy-user");
    assert_eq!(body["tools"][0]["type"], "web_search_preview");
    assert_eq!(
        body["tools"][0]["search_content_types"],
        json!(["text", "image"])
    );
    assert_eq!(body["tools"][0]["search_context_size"], "low");
    assert_eq!(body["tools"][1]["type"], "code_interpreter");
    assert_eq!(body["tools"][1]["container"]["type"], "auto");
    assert_eq!(
        body["tools"][1]["container"]["file_ids"],
        json!(["file_code"])
    );
    assert_eq!(body["tools"][1]["container"]["memory_limit"], "4g");
    assert_eq!(
        body["tools"][1]["container"]["network_policy"]["type"],
        "disabled"
    );
    assert_eq!(body["tools"][2]["type"], "mcp");
    assert_eq!(body["tools"][2]["server_label"], "deepwiki");
    assert_eq!(body["tools"][2]["server_url"], "https://mcp.example.test");
    assert_eq!(body["tools"][2]["server_description"], "Docs MCP");
    assert_eq!(body["tools"][2]["defer_loading"], false);
    assert_eq!(body["tools"][2]["headers"]["x-tenant"], "docs");
    assert_eq!(body["tools"][2]["allowed_tools"]["read_only"], true);
    assert_eq!(
        body["tools"][2]["allowed_tools"]["tool_names"],
        json!(["search_docs"])
    );
    assert_eq!(
        body["tools"][2]["require_approval"]["always"]["tool_names"],
        json!(["write_docs"])
    );
    assert_eq!(
        body["tools"][2]["require_approval"]["never"]["read_only"],
        true
    );
    assert_eq!(body["tools"][3]["type"], "shell");
    assert_eq!(body["tools"][3]["environment"]["type"], "container_auto");
    assert_eq!(
        body["tools"][3]["environment"]["file_ids"],
        json!(["file_shell"])
    );
    assert_eq!(body["tools"][3]["environment"]["memory_limit"], "1g");
    assert_eq!(
        body["tools"][3]["environment"]["network_policy"]["type"],
        "allowlist"
    );
    assert_eq!(
        body["tools"][3]["environment"]["network_policy"]["allowed_domains"],
        json!(["api.example.com"])
    );
    assert_eq!(body["tools"][4]["type"], "custom");
    assert_eq!(body["tools"][4]["name"], "query_parser");
    assert_eq!(body["tools"][4]["defer_loading"], true);
    assert_eq!(body["tools"][4]["format"]["type"], "grammar");
    assert_eq!(body["tools"][4]["format"]["syntax"], "lark");
    assert_eq!(body["tools"][4]["format"]["definition"], "start: WORD+");
    assert_eq!(response.output().id, "resp_create");
    assert_eq!(response.output().object, "response");
    assert_eq!(response.output().created_at, 1.25);
    assert_eq!(response.output().status.as_deref(), Some("completed"));
    assert_eq!(response.output().model.as_deref(), Some("gpt-4.1-nano"));
    assert_eq!(
        response.output().instructions,
        Some(ResponseInstructions::Text(String::from(
            "Server instructions"
        )))
    );
    assert_eq!(response.output().parallel_tool_calls, Some(true));
    assert_eq!(
        response.output().previous_response_id.as_deref(),
        Some("resp_prev")
    );
    assert_eq!(
        response.output().conversation,
        Some(ResponseConversation::Id(String::from("conv_123")))
    );
    assert_eq!(response.output().store, Some(true));
    assert_eq!(response.output().background, Some(false));
    assert_eq!(response.output().completed_at, Some(2.5));
    assert_eq!(response.output().max_output_tokens, Some(512));
    assert_eq!(response.output().max_tool_calls, Some(4));
    let prompt = response.output().prompt.as_ref().unwrap();
    assert_eq!(prompt.id, "pmpt_response");
    assert_eq!(prompt.variables.as_ref().unwrap()["topic"], json!("Rust"));
    assert_eq!(
        response.output().prompt_cache_key.as_deref(),
        Some("response-cache-key")
    );
    assert_eq!(
        response
            .output()
            .prompt_cache_retention
            .as_ref()
            .map(PromptCacheRetention::as_str),
        Some("24h")
    );
    assert_eq!(
        response
            .output()
            .reasoning
            .as_ref()
            .and_then(|reasoning| reasoning.effort.as_ref())
            .map(ReasoningEffort::as_str),
        Some("low")
    );
    assert_eq!(
        response.output().safety_identifier.as_deref(),
        Some("response_user_hash")
    );
    assert_eq!(
        response
            .output()
            .service_tier
            .as_ref()
            .map(ServiceTier::as_str),
        Some("priority")
    );
    assert_eq!(response.output().temperature, Some(0.2));
    assert_eq!(
        response
            .output()
            .text
            .as_ref()
            .and_then(|text| text.format.as_ref()),
        Some(&ResponseFormatTextConfig::Text)
    );
    assert_eq!(
        response.output().tool_choice,
        Some(ResponseToolChoice::Auto)
    );
    assert_eq!(response.output().tools.len(), 6);
    assert_eq!(
        response.output().tools[0],
        ResponseTool::WebSearchPreview(ResponseWebSearchPreviewTool {
            search_content_types: Vec::new(),
            search_context_size: None,
            user_location: None,
            extra: BTreeMap::new(),
        })
    );
    let ResponseTool::FileSearch(response_file_search_tool) = &response.output().tools[1] else {
        panic!("expected response file search tool");
    };
    assert_eq!(
        response_file_search_tool.vector_store_ids,
        vec![String::from("vs_123")]
    );
    let Some(ResponseFileSearchFilter::And { filters }) =
        response_file_search_tool.filters.as_ref()
    else {
        panic!("expected compound file search filter");
    };
    assert!(matches!(
        &filters[0],
        ResponseFileSearchFilter::Eq { key, value: ResponseFileSearchFilterValue::String(value) }
            if key == "section" && value == "intro"
    ));
    assert!(matches!(
        &filters[1],
        ResponseFileSearchFilter::Gte { key, value: ResponseFileSearchFilterValue::Number(value) }
            if key == "score" && (*value - 0.7).abs() < f64::EPSILON
    ));
    assert_eq!(
        response_file_search_tool
            .ranking_options
            .as_ref()
            .and_then(|options| options.ranker.as_ref()),
        Some(&ResponseFileSearchRanker::Auto)
    );
    let ResponseTool::CodeInterpreter(response_code_tool) = &response.output().tools[2] else {
        panic!("expected response code interpreter tool");
    };
    assert!(matches!(
        &response_code_tool.container,
        ResponseCodeInterpreterContainer::Id(id) if id == "cntr_response"
    ));
    let ResponseTool::Shell(response_shell_tool) = &response.output().tools[3] else {
        panic!("expected response shell tool");
    };
    assert!(matches!(
        response_shell_tool.environment.as_ref(),
        Some(ResponseShellEnvironment::ContainerReference(environment))
            if environment.container_id == "cntr_shell"
    ));
    let ResponseTool::Custom(response_custom_tool) = &response.output().tools[4] else {
        panic!("expected response custom tool");
    };
    assert_eq!(response_custom_tool.name, "freeform");
    assert!(matches!(
        response_custom_tool.format.as_ref(),
        Some(ResponseCustomToolInputFormat::Text)
    ));
    let ResponseTool::Mcp(response_mcp_tool) = &response.output().tools[5] else {
        panic!("expected response mcp tool");
    };
    assert_eq!(response_mcp_tool.server_label, "deepwiki");
    assert!(matches!(
        response_mcp_tool.allowed_tools.as_ref(),
        Some(ResponseMcpAllowedTools::Names(names))
            if names.len() == 1 && names[0] == "search_docs"
    ));
    assert!(matches!(
        response_mcp_tool.require_approval.as_ref(),
        Some(ResponseMcpRequireApproval::Never)
    ));
    let mcp_list = response
        .output()
        .output
        .iter()
        .find(|item| item.item_type == "mcp_list_tools")
        .expect("mcp list tools item");
    assert_eq!(mcp_list.server_label.as_deref(), Some("deepwiki"));
    let mcp_tool = mcp_list.tools[0].as_mcp_list().expect("mcp list tool");
    assert_eq!(mcp_tool.name, "search_docs");
    assert_eq!(mcp_tool.input_schema["type"], "object");
    assert_eq!(mcp_tool.annotations.as_ref().unwrap()["readOnlyHint"], true);
    assert_eq!(mcp_tool.description.as_deref(), Some("Search docs"));
    let tool_search = response
        .output()
        .output
        .iter()
        .find(|item| item.item_type == "tool_search_output")
        .expect("tool search output item");
    assert_eq!(tool_search.call_id.as_deref(), Some("call_tool_search"));
    assert_eq!(tool_search.execution.as_deref(), Some("server"));
    assert_eq!(tool_search.created_by.as_deref(), Some("assistant"));
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
    let computer_call = response
        .output()
        .output
        .iter()
        .find(|item| item.item_type == "computer_call")
        .expect("computer call item");
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
    let computer_output = response
        .output()
        .output
        .iter()
        .find(|item| item.item_type == "computer_call_output")
        .expect("computer output item");
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
    assert_eq!(response.output().top_logprobs, Some(2));
    assert_eq!(response.output().top_p, Some(0.8));
    assert_eq!(
        response
            .output()
            .truncation
            .as_ref()
            .map(Truncation::as_str),
        Some("auto")
    );
    assert_eq!(response.output().user.as_deref(), Some("legacy-user"));
    assert_eq!(
        response.output().metadata,
        Some(BTreeMap::from([(
            String::from("trace"),
            String::from("response_payload")
        )]))
    );
    let usage = response.output().usage.as_ref().unwrap();
    assert_eq!(usage.input_tokens, Some(1));
    assert_eq!(usage.output_tokens, Some(2));
    assert_eq!(usage.total_tokens, Some(3));
    assert_eq!(
        usage.input_tokens_details.as_ref().unwrap().cached_tokens,
        Some(1)
    );
    assert_eq!(
        usage
            .output_tokens_details
            .as_ref()
            .unwrap()
            .reasoning_tokens,
        Some(1)
    );
    assert_eq!(response.output().output_text(), "Hello world!");
}

#[test]
fn retrieve_round_trips_output_text_and_query() {
    let server = mock_http::MockHttpServer::spawn(json_response(response_payload(
        "resp_store",
        Some(true),
        None,
        None,
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .responses()
        .retrieve(
            "resp_store",
            openai_rust::resources::responses::ResponseRetrieveParams {
                include: vec![ResponseIncludable::MessageOutputTextLogprobs],
                include_obfuscation: Some(true),
                starting_after: Some(7),
                stream: Some(false),
            },
        )
        .unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.path,
        "/v1/responses/resp_store?include=message.output_text.logprobs&include_obfuscation=true&starting_after=7&stream=false"
    );
    assert_eq!(response.output().id, "resp_store");
    assert_eq!(response.output().output_text(), "Hello world!");

    let error = client
        .responses()
        .retrieve("   ", Default::default())
        .expect_err("blank response id should be rejected locally");
    assert_eq!(error.kind, ErrorKind::Validation);
}

#[test]
fn response_instructions_decode_input_items() {
    let server = mock_http::MockHttpServer::spawn(json_response(
        response_payload_with_instruction_items("resp_instruction_items"),
    ))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .responses()
        .retrieve("resp_instruction_items", Default::default())
        .unwrap();

    let Some(ResponseInstructions::Items(items)) = &response.output().instructions else {
        panic!("expected typed instruction items");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_type, "message");
    assert_eq!(items[0].role.as_deref(), Some("developer"));
    assert_eq!(items[0].content[0].content_type, "input_text");
    assert_eq!(items[0].content[0].text.as_deref(), Some("Use plain text."));
}

#[test]
fn response_input_item_literals_serialize_as_typed_values() {
    let value = serde_json::to_value(ResponseInputItem {
        item_type: Some(ResponseItemType::Message),
        role: Some(ResponseItemRole::Developer),
        content: Some(
            vec![ResponseInputContentPart::Text(ResponseInputText::new(
                "Use terse answers.",
            ))]
            .into(),
        ),
        status: Some(ResponseItemStatus::InProgress),
        phase: Some(ResponseMessagePhase::Commentary),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(value["type"], "message");
    assert_eq!(value["role"], "developer");
    assert_eq!(value["status"], "in_progress");
    assert_eq!(value["phase"], "commentary");
    assert_eq!(value["content"][0]["type"], "input_text");
}

#[test]
fn delete_returns_unit() {
    let server = mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse {
        headers: vec![(String::from("content-length"), String::from("0"))],
        ..Default::default()
    })
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    client.responses().delete("resp_delete").unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.path, "/v1/responses/resp_delete");
    assert_eq!(
        request.headers.get("accept").map(String::as_str),
        Some("*/*")
    );

    let error = client
        .responses()
        .delete("")
        .expect_err("blank response id should be rejected locally");
    assert_eq!(error.kind, ErrorKind::Validation);
}

#[test]
fn cancel_posts_to_background_endpoint() {
    let server = mock_http::MockHttpServer::spawn(json_response(response_payload(
        "resp_bg",
        Some(true),
        None,
        None,
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client.responses().cancel("resp_bg").unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/responses/resp_bg/cancel");
    assert_eq!(response.output().id, "resp_bg");
    assert_eq!(response.output().output_text(), "Hello world!");

    let error = client
        .responses()
        .cancel("   ")
        .expect_err("blank response id should be rejected locally");
    assert_eq!(error.kind, ErrorKind::Validation);
}

#[test]
fn tool_and_refusal_fields_round_trip() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(response_payload_with_tool_and_refusal("resp_create")),
        json_response(response_payload_with_tool_and_refusal("resp_retrieve")),
        json_response(response_payload_with_tool_and_refusal("resp_cancel")),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let created = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            input: Some(ResponseInput::text("hello")),
            ..Default::default()
        })
        .unwrap();
    let retrieved = client
        .responses()
        .retrieve("resp_retrieve", Default::default())
        .unwrap();
    let cancelled = client.responses().cancel("resp_cancel").unwrap();

    for response in [created.output(), retrieved.output(), cancelled.output()] {
        let function_call = response
            .output
            .iter()
            .find(|item| item.item_type == "function_call")
            .expect("function_call item");
        assert_eq!(function_call.name.as_deref(), Some("lookup_weather"));
        assert_eq!(
            function_call.arguments.as_deref(),
            Some(r#"{"city":"Paris"}"#)
        );
        assert_eq!(
            function_call.arguments_json,
            Some(json!(r#"{"city":"Paris"}"#))
        );
        assert_eq!(function_call.call_id.as_deref(), Some("call_123"));
        assert_eq!(function_call.status.as_deref(), Some("completed"));
        assert_eq!(function_call.namespace.as_deref(), Some("weather"));
        assert_eq!(function_call.created_by.as_deref(), Some("assistant"));

        let text_message = response
            .output
            .iter()
            .find(|item| item.id.as_deref() == Some("msg_text"))
            .expect("text message");
        assert_eq!(text_message.status.as_deref(), Some("completed"));
        assert_eq!(text_message.phase.as_deref(), Some("final_answer"));
        assert!(matches!(
            text_message.content[0].annotations.as_slice(),
            [ResponseTextAnnotation::UrlCitation(annotation)]
                if annotation.start_index == Some(0)
                    && annotation.end_index == Some(5)
                    && annotation.title.as_deref() == Some("Weather")
                    && annotation.url.as_deref() == Some("https://example.com/weather")
        ));
        let logprobs = text_message.content[0].logprobs.as_ref().unwrap();
        assert_eq!(logprobs[0].token, "Hello");
        assert_eq!(logprobs[0].bytes, vec![72, 101, 108, 108, 111]);
        assert_eq!(logprobs[0].logprob, -0.01);
        assert!(logprobs[0].top_logprobs.is_empty());

        let reasoning = response
            .output
            .iter()
            .find(|item| item.item_type == "reasoning")
            .expect("reasoning item");
        assert_eq!(reasoning.summary[0].summary_type, "summary_text");
        assert_eq!(
            reasoning.summary[0].text.as_deref(),
            Some("Checked weather")
        );
        assert_eq!(
            reasoning.encrypted_content.as_deref(),
            Some("enc_reasoning")
        );
        assert_eq!(reasoning.content[0].content_type, "reasoning_text");

        let tool_search = response
            .output
            .iter()
            .find(|item| item.item_type == "tool_search_call")
            .expect("tool_search_call item");
        assert_eq!(tool_search.execution.as_deref(), Some("server"));
        assert_eq!(
            tool_search.arguments_json,
            Some(json!({"query": "weather"}))
        );
        assert_eq!(
            tool_search.arguments.as_deref(),
            Some(r#"{"query":"weather"}"#)
        );

        let file_search = response
            .output
            .iter()
            .find(|item| item.item_type == "file_search_call")
            .expect("file_search_call item");
        assert_eq!(file_search.queries, vec![String::from("docs")]);
        let file_search_results = file_search.results.as_ref().unwrap();
        assert_eq!(file_search_results[0].file_id.as_deref(), Some("file_1"));
        assert_eq!(file_search_results[0].filename.as_deref(), Some("guide.md"));
        assert_eq!(file_search_results[0].score, Some(0.9));
        assert_eq!(
            file_search_results[0].text.as_deref(),
            Some("Relevant guide excerpt")
        );
        assert!(matches!(
            file_search_results[0]
                .attributes
                .as_ref()
                .unwrap()
                .get("section"),
            Some(ResponseFileSearchAttributeValue::String(section)) if section == "intro"
        ));

        let code_call = response
            .output
            .iter()
            .find(|item| item.item_type == "code_interpreter_call")
            .expect("code_interpreter_call item");
        assert_eq!(code_call.container_id.as_deref(), Some("cntr_123"));
        let code_outputs = code_call.outputs.as_ref().unwrap();
        assert!(matches!(
            &code_outputs[0],
            ResponseCodeInterpreterOutput::Logs(logs) if logs.logs == "ok"
        ));
        assert!(matches!(
            &code_outputs[1],
            ResponseCodeInterpreterOutput::Image(image)
                if image.url == "https://example.com/output.png"
        ));

        let mcp_call = response
            .output
            .iter()
            .find(|item| item.item_type == "mcp_call")
            .expect("mcp_call item");
        assert_eq!(mcp_call.server_label.as_deref(), Some("weather_mcp"));
        assert_eq!(
            mcp_call.approval_request_id.as_deref(),
            Some("approval_123")
        );
        assert_eq!(
            mcp_call.output,
            Some(ResponseItemOutput::Text(String::from("sunny")))
        );

        let custom_output = response
            .output
            .iter()
            .find(|item| item.item_type == "custom_tool_call_output")
            .expect("custom_tool_call_output item");
        assert!(matches!(
            custom_output.output.as_ref(),
            Some(ResponseItemOutput::ContentList(parts))
                if parts.len() == 2
                    && parts[0].content_type == "input_text"
                    && parts[0].text.as_deref() == Some("custom payload")
                    && parts[1].content_type == "input_file"
                    && parts[1].filename.as_deref() == Some("result.txt")
        ));

        let local_shell_call = response
            .output
            .iter()
            .find(|item| item.item_type == "local_shell_call")
            .expect("local_shell_call item");
        assert!(matches!(
            local_shell_call.action.as_ref(),
            Some(ResponseItemAction::LocalShell(action))
                if action.command == ["ls", "-la"]
                    && action.env.get("LC_ALL").map(String::as_str) == Some("C")
                    && action.working_directory.as_deref() == Some("/workspace")
        ));

        let shell_call = response
            .output
            .iter()
            .find(|item| item.item_type == "shell_call")
            .expect("shell_call item");
        assert!(matches!(
            shell_call.action.as_ref(),
            Some(ResponseItemAction::Shell(action))
                if action.commands == ["echo ok"]
                    && action.max_output_length == Some(2048)
                    && action.timeout_ms == Some(1000)
        ));
        assert!(matches!(
            shell_call.environment.as_ref(),
            Some(ResponseItemEnvironment::ContainerReference(environment))
                if environment.container_id == "cntr_123"
        ));

        let apply_patch_call = response
            .output
            .iter()
            .find(|item| item.item_type == "apply_patch_call")
            .expect("apply_patch_call item");
        assert!(matches!(
            apply_patch_call.operation.as_ref(),
            Some(ResponseApplyPatchOperation::UpdateFile(operation))
                if operation.path == "src/lib.rs" && operation.diff == "@@ -1 +1"
        ));

        let shell_output = response
            .output
            .iter()
            .find(|item| item.item_type == "shell_call_output")
            .expect("shell_call_output item");
        assert_eq!(shell_output.max_output_length, Some(4096));
        assert!(matches!(
            shell_output.output.as_ref(),
            Some(ResponseItemOutput::Shell(outputs))
                if outputs.len() == 1
                    && outputs[0].stdout == "ok\n"
                    && outputs[0].stderr.is_empty()
                    && matches!(
                        &outputs[0].outcome,
                        ResponseShellOutputOutcome::Exit(outcome)
                            if outcome.exit_code == 0
                    )
        ));

        let image_call = response
            .output
            .iter()
            .find(|item| item.item_type == "image_generation_call")
            .expect("image_generation_call item");
        assert_eq!(image_call.result.as_deref(), Some("aW1n"));

        let refusal_message = response
            .output
            .iter()
            .find(|item| item.id.as_deref() == Some("msg_refusal"))
            .expect("refusal message");
        let refusal_part = refusal_message
            .content
            .iter()
            .find(|part| part.content_type == "refusal")
            .expect("refusal content");
        assert_eq!(
            refusal_part.refusal.as_deref(),
            Some("I can't help with that")
        );
        assert_eq!(response.refusal_text(), Some("I can't help with that"));
        assert_eq!(response.output_text(), "Hello world!");
    }
}

#[test]
fn compact_returns_compaction_object() {
    let body = json!({
        "id": "cmp_123",
        "object": "response.compaction",
        "created_at": 1,
        "output": [
            {
                "id": "msg_user",
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "Original prompt"}
                ]
            },
            {
                "id": "cmp_item",
                "type": "compaction",
                "encrypted_content": "enc_compacted_context"
            }
        ],
        "usage": {
            "input_tokens": 12,
            "output_tokens": 3,
            "total_tokens": 15
        }
    });
    let server = mock_http::MockHttpServer::spawn(json_value_response(body)).unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let response = client
        .responses()
        .compact(openai_rust::resources::responses::ResponseCompactParams {
            model: "gpt-4.1-nano".into(),
            input: Some(ResponseInput::text("follow-up")),
            previous_response_id: Some("resp_prev".into()),
            prompt_cache_key: Some(String::from("compact-cache")),
            prompt_cache_retention: Some(PromptCacheRetention::InMemory),
            service_tier: Some(ResponseCompactServiceTier::Flex),
            ..Default::default()
        })
        .unwrap();

    let request = server.captured_request().expect("captured request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/responses/compact");
    let request_body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(request_body["model"], "gpt-4.1-nano");
    assert_eq!(request_body["input"], "follow-up");
    assert_eq!(request_body["previous_response_id"], "resp_prev");
    assert_eq!(request_body["prompt_cache_key"], "compact-cache");
    assert_eq!(request_body["prompt_cache_retention"], "in_memory");
    assert_eq!(request_body["service_tier"], "flex");
    assert_eq!(response.output().object, "response.compaction");
    assert_eq!(response.output().output.len(), 2);
    assert_eq!(response.output().output[0].item_type, "message");
    assert_eq!(response.output().output[1].item_type, "compaction");
    assert_eq!(
        response.output().output[1].encrypted_content.as_deref(),
        Some("enc_compacted_context")
    );
    assert_eq!(
        response.output().usage.as_ref().unwrap().total_tokens,
        Some(15)
    );
}

#[test]
fn continuity_fields_round_trip() {
    let server = mock_http::MockHttpServer::spawn(json_response(response_payload(
        "resp_conflict",
        Some(true),
        Some("resp_prev"),
        Some(json!({"id": "conv_123"})),
    )))
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            previous_response_id: Some("resp_prev".into()),
            conversation: Some(ResponseConversation::Object(ResponseConversationObject {
                id: String::from("conv_123"),
                extra: BTreeMap::new(),
            })),
            ..Default::default()
        })
        .unwrap();

    let request = server.captured_request().expect("captured request");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["previous_response_id"], "resp_prev");
    assert_eq!(body["conversation"], json!({"id": "conv_123"}));
}

#[test]
fn store_flag_pass_through() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(response_payload("resp_stored", Some(true), None, None)),
        json_response(response_payload("resp_ephemeral", Some(false), None, None)),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let stored = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            store: Some(true),
            ..Default::default()
        })
        .unwrap();
    let ephemeral = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            store: Some(false),
            ..Default::default()
        })
        .unwrap();

    let requests = server.captured_requests(2).expect("captured requests");
    let first: Value = serde_json::from_slice(&requests[0].body).unwrap();
    let second: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(first["store"], true);
    assert_eq!(second["store"], false);
    assert_eq!(stored.output().store, Some(true));
    assert_eq!(ephemeral.output().store, Some(false));
}

#[test]
fn conflicting_state_api_failures_surface_cleanly() {
    let body = br#"{"error":{"message":"previous_response_id cannot be used with conversation","type":"invalid_request_error","code":"conflict_state"}}"#.to_vec();
    let server = mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse {
        status_code: 400,
        reason: "Bad Request",
        headers: vec![
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("x-request-id"),
                String::from("req_conflict_state"),
            ),
        ],
        body,
        ..Default::default()
    })
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let error = client
        .responses()
        .create(openai_rust::resources::responses::ResponseCreateParams {
            model: "gpt-4.1-nano".into(),
            previous_response_id: Some("resp_prev".into()),
            conversation: Some(ResponseConversation::Id(String::from("conv_123"))),
            ..Default::default()
        })
        .expect_err("conflicting continuity modes should surface API failure");

    assert_eq!(error.kind, ErrorKind::Api(ApiErrorKind::BadRequest));
    assert_eq!(error.request_id(), Some("req_conflict_state"));
    assert_eq!(
        error.api_error().unwrap().code.as_deref(),
        Some("conflict_state")
    );
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

fn json_value_response(body: Value) -> mock_http::ScriptedResponse {
    json_response(body.to_string())
}

fn response_payload(
    id: &str,
    store: Option<bool>,
    previous_response_id: Option<&str>,
    conversation: Option<Value>,
) -> String {
    let tools = response_payload_tools();
    json!({
        "id": id,
        "object": "response",
        "created_at": 1.25,
        "status": "completed",
        "background": false,
        "completed_at": 2.5,
        "error": null,
        "incomplete_details": null,
        "instructions": "Server instructions",
        "metadata": {"trace": "response_payload"},
        "model": "gpt-4.1-nano",
        "max_output_tokens": 512,
        "max_tool_calls": 4,
        "output": [
            {
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "output_text", "text": "Hello "},
                    {"type": "refusal", "text": "ignored"}
                ]
            },
            {
                "id": "reasoning_1",
                "type": "reasoning",
                "summary": []
            },
            {
                "id": "mcp_tools_1",
                "type": "mcp_list_tools",
                "server_label": "deepwiki",
                "tools": [{
                    "name": "search_docs",
                    "input_schema": {"type": "object"},
                    "annotations": {"readOnlyHint": true},
                    "description": "Search docs"
                }]
            },
            {
                "id": "tool_search_output_1",
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
            },
            {
                "id": "computer_1",
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
            },
            {
                "id": "computer_output_1",
                "type": "computer_call_output",
                "call_id": "call_computer",
                "status": "completed",
                "output": {"type": "computer_screenshot", "image_url": "data:image/png;base64,AA=="},
                "acknowledged_safety_checks": [{
                    "id": "safety_ack_1",
                    "code": "unsafe_browser",
                    "message": "Acknowledged"
                }]
            },
            {
                "id": "msg_2",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "output_text", "text": "world!"}
                ]
            }
        ],
        "parallel_tool_calls": true,
        "previous_response_id": previous_response_id,
        "conversation": conversation,
        "store": store,
        "prompt": {"id": "pmpt_response", "variables": {"topic": "Rust"}},
        "prompt_cache_key": "response-cache-key",
        "prompt_cache_retention": "24h",
        "reasoning": {"effort": "low"},
        "safety_identifier": "response_user_hash",
        "service_tier": "priority",
        "temperature": 0.2,
        "text": {"format": {"type": "text"}},
        "tool_choice": "auto",
        "tools": tools,
        "top_logprobs": 2,
        "top_p": 0.8,
        "truncation": "auto",
        "user": "legacy-user",
        "usage": {
            "input_tokens": 1,
            "input_tokens_details": {"cached_tokens": 1},
            "output_tokens": 2,
            "output_tokens_details": {"reasoning_tokens": 1},
            "total_tokens": 3
        }
    })
    .to_string()
}

fn response_payload_with_instruction_items(id: &str) -> String {
    let mut payload: Value =
        serde_json::from_str(&response_payload(id, Some(true), None, None)).unwrap();
    payload["instructions"] = json!([
        {
            "type": "message",
            "role": "developer",
            "content": "Use plain text."
        }
    ]);
    payload.to_string()
}

fn response_payload_tools() -> Value {
    json!([
        {"type": "web_search_preview"},
        {
            "type": "file_search",
            "vector_store_ids": ["vs_123"],
            "filters": {
                "type": "and",
                "filters": [
                    {"type": "eq", "key": "section", "value": "intro"},
                    {"type": "gte", "key": "score", "value": 0.7}
                ]
            },
            "ranking_options": {
                "ranker": "auto",
                "score_threshold": 0.4,
                "hybrid_search": {"embedding_weight": 0.75, "text_weight": 0.25}
            }
        },
        {
            "type": "code_interpreter",
            "container": "cntr_response"
        },
        {
            "type": "shell",
            "environment": {
                "type": "container_reference",
                "container_id": "cntr_shell"
            }
        },
        {
            "type": "custom",
            "name": "freeform",
            "format": {"type": "text"}
        },
        {
            "type": "mcp",
            "server_label": "deepwiki",
            "allowed_tools": ["search_docs"],
            "require_approval": "never",
            "server_url": "https://mcp.example.test"
        }
    ])
}

fn response_payload_with_tool_and_refusal(id: &str) -> String {
    json!({
        "id": id,
        "object": "response",
        "created_at": 1,
        "status": "completed",
        "background": false,
        "error": null,
        "incomplete_details": null,
        "model": "gpt-4.1-nano",
        "output": [
            {
                "id": "msg_text",
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "phase": "final_answer",
                "content": [
                    {
                        "type": "output_text",
                        "text": "Hello ",
                        "annotations": [{
                            "type": "url_citation",
                            "start_index": 0,
                            "end_index": 5,
                            "title": "Weather",
                            "url": "https://example.com/weather"
                        }],
                        "logprobs": [{
                            "token": "Hello",
                            "bytes": [72, 101, 108, 108, 111],
                            "logprob": -0.01,
                            "top_logprobs": []
                        }]
                    },
                    {"type": "output_text", "text": "world!"}
                ]
            },
            {
                "id": "reasoning_1",
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "Checked weather"}],
                "content": [{"type": "reasoning_text", "text": "Need weather lookup"}],
                "encrypted_content": "enc_reasoning",
                "status": "completed"
            },
            {
                "id": "fc_123",
                "type": "function_call",
                "name": "lookup_weather",
                "arguments": "{\"city\":\"Paris\"}",
                "call_id": "call_123",
                "status": "completed",
                "namespace": "weather",
                "created_by": "assistant"
            },
            {
                "id": "fs_123",
                "type": "file_search_call",
                "queries": ["docs"],
                "status": "completed",
                "results": [{
                    "file_id": "file_1",
                    "filename": "guide.md",
                    "score": 0.9,
                    "text": "Relevant guide excerpt",
                    "attributes": {"section": "intro", "rank": 1}
                }]
            },
            {
                "id": "ci_123",
                "type": "code_interpreter_call",
                "container_id": "cntr_123",
                "code": "print('ok')",
                "outputs": [
                    {"type": "logs", "logs": "ok"},
                    {"type": "image", "url": "https://example.com/output.png"}
                ],
                "status": "completed"
            },
            {
                "id": "mcp_123",
                "type": "mcp_call",
                "arguments": "{\"city\":\"Paris\"}",
                "name": "weather",
                "server_label": "weather_mcp",
                "approval_request_id": "approval_123",
                "output": "sunny",
                "status": "completed"
            },
            {
                "id": "custom_output_123",
                "type": "custom_tool_call_output",
                "call_id": "call_custom",
                "status": "completed",
                "created_by": "tool",
                "output": [
                    {"type": "input_text", "text": "custom payload"},
                    {
                        "type": "input_file",
                        "filename": "result.txt",
                        "file_data": "Zm9v"
                    }
                ]
            },
            {
                "id": "local_shell_123",
                "type": "local_shell_call",
                "call_id": "call_local_shell",
                "status": "completed",
                "action": {
                    "type": "exec",
                    "command": ["ls", "-la"],
                    "env": {"LC_ALL": "C"},
                    "timeout_ms": 1000,
                    "user": "sandbox",
                    "working_directory": "/workspace"
                }
            },
            {
                "id": "shell_123",
                "type": "shell_call",
                "call_id": "call_shell",
                "status": "completed",
                "created_by": "assistant",
                "action": {
                    "commands": ["echo ok"],
                    "max_output_length": 2048,
                    "timeout_ms": 1000
                },
                "environment": {
                    "type": "container_reference",
                    "container_id": "cntr_123"
                }
            },
            {
                "id": "patch_123",
                "type": "apply_patch_call",
                "call_id": "call_patch",
                "status": "completed",
                "created_by": "assistant",
                "operation": {
                    "type": "update_file",
                    "path": "src/lib.rs",
                    "diff": "@@ -1 +1"
                }
            },
            {
                "id": "shell_output_123",
                "type": "shell_call_output",
                "call_id": "call_shell",
                "status": "completed",
                "created_by": "tool",
                "max_output_length": 4096,
                "output": [{
                    "stdout": "ok\n",
                    "stderr": "",
                    "created_by": "runner",
                    "outcome": {"type": "exit", "exit_code": 0}
                }]
            },
            {
                "id": "tool_search_123",
                "type": "tool_search_call",
                "arguments": {"query": "weather"},
                "execution": "server",
                "status": "completed"
            },
            {
                "id": "image_123",
                "type": "image_generation_call",
                "result": "aW1n",
                "status": "completed"
            },
            {
                "id": "msg_refusal",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "refusal", "refusal": "I can't help with that"}
                ]
            }
        ],
        "usage": {"input_tokens": 1, "output_tokens": 2, "total_tokens": 3}
    })
    .to_string()
}
