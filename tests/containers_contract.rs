#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    DEFAULT_BASE_URL, ErrorKind, OpenAI,
    resources::containers::{
        ContainerCreateParams, ContainerExpiresAfter, ContainerInlineSkill,
        ContainerInlineSkillSource, ContainerListParams, ContainerMemoryLimit,
        ContainerNetworkPolicy, ContainerOrder, ContainerReadNetworkPolicy, ContainerSkill,
        ContainerSkillReference, ContainerStatus, DomainSecret,
    },
};
use serde_json::json;

#[test]
fn containers_crud_preserves_execution_policy_fields_without_requiring_tool_execution() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(container_payload("cntr_created")),
        json_response(container_payload("cntr_created")),
        json_response(container_list_payload()),
        empty_response(200),
    ])
    .unwrap();
    let client = client(&server.url());

    let created = client
        .containers()
        .create(ContainerCreateParams {
            name: String::from("code-interpreter"),
            expires_after: Some(ContainerExpiresAfter {
                anchor: String::from("last_active_at"),
                minutes: 20,
            }),
            file_ids: Some(vec![String::from("file_alpha"), String::from("file_beta")]),
            memory_limit: Some(ContainerMemoryLimit::G4),
            network_policy: Some(ContainerNetworkPolicy::Allowlist {
                allowed_domains: vec![String::from("api.buildkite.com")],
                domain_secrets: Some(vec![DomainSecret {
                    domain: String::from("api.buildkite.com"),
                    name: String::from("BUILDKITE_TOKEN"),
                    value: String::from("secret-value"),
                }]),
            }),
            skills: Some(vec![
                ContainerSkill::Reference({
                    let mut reference = ContainerSkillReference::new("skill_123");
                    reference.version = Some(String::from("latest"));
                    reference
                }),
                ContainerSkill::Inline(ContainerInlineSkill::new(
                    "zip-skill",
                    "inline bundle",
                    ContainerInlineSkillSource::new("UEsDBAoAAAAAA"),
                )),
            ]),
        })
        .unwrap();

    assert_eq!(created.output.id, "cntr_created");
    assert_eq!(created.output.name, "code-interpreter");
    assert_eq!(created.output.memory_limit, Some(ContainerMemoryLimit::G4));
    assert_eq!(created.output.status, Some(ContainerStatus::Running));
    match created.output.network_policy.as_ref().unwrap() {
        ContainerReadNetworkPolicy::Allowlist { allowed_domains } => {
            assert_eq!(allowed_domains, &vec![String::from("api.buildkite.com")]);
        }
        other => panic!("expected allowlist policy, got {other:?}"),
    }

    let retrieved = client.containers().retrieve("cntr_created").unwrap();
    assert_eq!(retrieved.output.id, "cntr_created");
    assert_eq!(retrieved.output.expires_after.as_ref().unwrap().minutes, 20);

    let listed = client
        .containers()
        .list(ContainerListParams {
            after: Some(String::from("cntr_prev")),
            limit: Some(2),
            name: Some(String::from("code")),
            order: Some(ContainerOrder::Asc),
        })
        .unwrap();
    assert_eq!(listed.output.data.len(), 2);
    assert_eq!(listed.output.next_after(), Some("cntr_list_two"));
    assert!(listed.output.has_next_page());
    assert!(matches!(
        listed.output.data[0].status,
        Some(ContainerStatus::Running)
    ));

    let deleted = client.containers().delete("cntr_created").unwrap();
    assert_eq!(deleted.output, ());

    let requests = server.captured_requests(4).unwrap();
    assert_eq!(requests[0].path, "/v1/containers");
    assert_eq!(requests[1].path, "/v1/containers/cntr_created");
    assert_eq!(
        requests[2].path,
        "/v1/containers?after=cntr_prev&limit=2&name=code&order=asc"
    );
    assert_eq!(requests[3].path, "/v1/containers/cntr_created");
    assert_eq!(
        requests[3].headers.get("accept").map(String::as_str),
        Some("*/*")
    );

    let create_body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(create_body["name"], json!("code-interpreter"));
    assert_eq!(
        create_body["expires_after"]["anchor"],
        json!("last_active_at")
    );
    assert_eq!(create_body["file_ids"], json!(["file_alpha", "file_beta"]));
    assert_eq!(create_body["memory_limit"], json!("4g"));
    assert_eq!(create_body["network_policy"]["type"], json!("allowlist"));
    assert_eq!(
        create_body["network_policy"]["domain_secrets"][0]["domain"],
        json!("api.buildkite.com")
    );
    assert_eq!(create_body["skills"][0]["type"], json!("skill_reference"));
    assert_eq!(create_body["skills"][1]["type"], json!("inline"));
    assert_eq!(
        create_body["skills"][1]["source"]["media_type"],
        json!("application/zip")
    );

    let blank_id = client.containers().retrieve(" ").unwrap_err();
    assert!(matches!(blank_id.kind, ErrorKind::Validation));
}

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_containers_smoke_records_entitlement_failures_explicitly() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live containers client should resolve configuration");
    assert_eq!(resolved.base_url, DEFAULT_BASE_URL);

    let create_result = client.containers().create(ContainerCreateParams {
        name: String::from("live-containers-smoke"),
        expires_after: Some(ContainerExpiresAfter {
            anchor: String::from("last_active_at"),
            minutes: 20,
        }),
        file_ids: None,
        memory_limit: Some(ContainerMemoryLimit::G1),
        network_policy: Some(ContainerNetworkPolicy::Disabled),
        skills: None,
    });

    let created = match create_result {
        Ok(response) => response,
        Err(error) => {
            if let Some(reason) = entitlement_skip_reason(&error) {
                println!("containers live smoke skipped: {reason}");
                return;
            }
            panic!("live containers create should succeed or skip explicitly: {error}");
        }
    };

    let container_id = created.output.id.clone();
    let create_request_id = created
        .request_id()
        .expect("live containers create should expose a request id");
    assert!(!create_request_id.trim().is_empty());

    let uploaded = client
        .containers()
        .files()
        .create(
            &container_id,
            openai_rust::resources::containers::ContainerFileCreateParams::Upload(
                openai_rust::resources::containers::ContainerFileUpload::new(
                    "live-container.txt",
                    "text/plain",
                    b"live container bytes".to_vec(),
                ),
            ),
        )
        .expect("live container file upload should succeed once containers are available");
    let container_file_id = uploaded.output.id.clone();

    let retrieved = client
        .containers()
        .files()
        .retrieve(&container_id, &container_file_id)
        .expect("live container file retrieve should succeed");
    assert_eq!(retrieved.output.id, container_file_id);

    let content = client
        .containers()
        .files()
        .content(&container_id, &container_file_id)
        .expect("live container file content should succeed");
    assert_eq!(content.output, b"live container bytes");

    let listed = client
        .containers()
        .files()
        .list(
            &container_id,
            openai_rust::resources::containers::ContainerFileListParams::default(),
        )
        .expect("live container file list should succeed");
    assert!(
        listed
            .output
            .data
            .iter()
            .any(|file| file.id == container_file_id)
    );

    let deleted_file = client
        .containers()
        .files()
        .delete(&container_id, &container_file_id)
        .expect("live container file delete should succeed");
    assert!(deleted_file.output.deleted);

    client
        .containers()
        .delete(&container_id)
        .expect("live container delete should succeed");

    println!("live container id: {container_id}");
    println!("live container create request id: {create_request_id}");
    println!(
        "live container file upload request id: {}",
        uploaded.request_id().unwrap_or("<missing>")
    );
    println!(
        "live container file retrieve request id: {}",
        retrieved.request_id().unwrap_or("<missing>")
    );
    println!(
        "live container file content request id: {}",
        content.request_id().unwrap_or("<missing>")
    );
    println!(
        "live container file delete request id: {}",
        deleted_file.request_id().unwrap_or("<missing>")
    );
}

fn entitlement_skip_reason(error: &openai_rust::OpenAIError) -> Option<String> {
    let status = error.status_code()?;
    let payload = error.api_error()?;
    let code = payload.code.as_deref().unwrap_or("<missing-code>");
    let message = payload.message.trim();
    match status {
        403 | 404 => Some(format!("status={status}, code={code}, message={message}")),
        _ => None,
    }
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn container_payload(id: &str) -> String {
    json!({
        "id": id,
        "object": "container",
        "created_at": 1_717_171_717,
        "status": "running",
        "name": "code-interpreter",
        "expires_after": {
            "anchor": "last_active_at",
            "minutes": 20
        },
        "last_active_at": 1_717_171_817u64,
        "memory_limit": "4g",
        "network_policy": {
            "type": "allowlist",
            "allowed_domains": ["api.buildkite.com"]
        }
    })
    .to_string()
}

fn container_list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&container_payload("cntr_list_one")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&container_payload("cntr_list_two")).unwrap()
        ],
        "has_more": true
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
            (String::from("x-request-id"), String::from("req_containers")),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn empty_response(status_code: u16) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        status_code,
        reason: "OK",
        headers: vec![
            (String::from("content-length"), String::from("0")),
            (
                String::from("x-request-id"),
                String::from("req_container_delete"),
            ),
        ],
        body: Vec::new(),
        ..Default::default()
    }
}
