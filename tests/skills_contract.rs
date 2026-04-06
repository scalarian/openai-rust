#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use openai_rust::{
    DEFAULT_BASE_URL, ErrorKind, OpenAI,
    resources::skills::{
        SkillCreateParams, SkillDeleteResponse, SkillFileUpload, SkillFilesParam, SkillListParams,
        SkillOrder, SkillUpdateParams, SkillVersion, SkillVersionCreateParams,
        SkillVersionDeleteResponse, SkillVersionListParams,
    },
};
use serde_json::json;

#[test]
fn skills_crud_and_versioning_preserve_default_latest_and_binary_content_access() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(skill_payload("skill_alpha", "1", "2")),
        json_response(skill_payload("skill_alpha", "1", "2")),
        json_response(skill_payload("skill_alpha", "2", "2")),
        json_response(skill_list_payload()),
        binary_response(b"PK\x03\x04root-skill"),
        json_response(skill_version_payload("skill_alpha", "sv_2", "2")),
        json_response(skill_version_payload("skill_alpha", "sv_2", "2")),
        json_response(skill_versions_payload("skill_alpha")),
        binary_response(b"PK\x03\x04version-skill"),
        json_response(
            json!({
                "id": "skill_alpha",
                "object": "skill.version.deleted",
                "deleted": true,
                "version": "2"
            })
            .to_string(),
        ),
        json_response(
            json!({
                "id": "skill_alpha",
                "object": "skill.deleted",
                "deleted": true
            })
            .to_string(),
        ),
    ])
    .unwrap();
    let client = client(&server.url());

    let created = client
        .skills()
        .create(SkillCreateParams {
            files: Some(SkillFilesParam::Multiple(vec![
                SkillFileUpload::new("manifest.json", "application/json", br#"{"name":"zip"}"#),
                SkillFileUpload::new(
                    "bundle.zip",
                    "application/zip",
                    b"PK\x03\x04bundle".to_vec(),
                ),
            ])),
        })
        .unwrap();
    assert_eq!(created.output.id, "skill_alpha");
    assert_eq!(created.output.default_version, "1");
    assert_eq!(created.output.latest_version, "2");

    let retrieved = client.skills().retrieve("skill_alpha").unwrap();
    assert_eq!(retrieved.output.id, "skill_alpha");

    let updated = client
        .skills()
        .update(
            "skill_alpha",
            SkillUpdateParams {
                default_version: String::from("2"),
            },
        )
        .unwrap();
    assert_eq!(updated.output.default_version, "2");

    let listed = client
        .skills()
        .list(SkillListParams {
            after: Some(String::from("skill_prev")),
            limit: Some(2),
            order: Some(SkillOrder::Desc),
        })
        .unwrap();
    assert_eq!(listed.output.data.len(), 2);
    assert_eq!(listed.output.next_after(), Some("skill_beta"));
    assert!(listed.output.has_next_page());

    let content = client.skills().content().retrieve("skill_alpha").unwrap();
    assert_eq!(content.output, b"PK\x03\x04root-skill");

    let created_version = client
        .skills()
        .versions()
        .create(
            "skill_alpha",
            SkillVersionCreateParams {
                default: Some(true),
                files: Some(SkillFilesParam::Single(SkillFileUpload::new(
                    "version.zip",
                    "application/zip",
                    b"PK\x03\x04version".to_vec(),
                ))),
            },
        )
        .unwrap();
    assert_eq!(created_version.output.version, "2");

    let retrieved_version = client
        .skills()
        .versions()
        .retrieve("skill_alpha", "2")
        .unwrap();
    assert_eq!(retrieved_version.output.id, "sv_2");

    let versions = client
        .skills()
        .versions()
        .list(
            "skill_alpha",
            SkillVersionListParams {
                after: Some(String::from("sv_1")),
                limit: Some(2),
                order: Some(SkillOrder::Asc),
            },
        )
        .unwrap();
    assert_eq!(versions.output.data.len(), 2);
    assert_eq!(versions.output.next_after(), Some("sv_2"));
    assert_eq!(
        versions.output.data[1],
        SkillVersion {
            id: String::from("sv_2"),
            created_at: 1_717_171_818,
            description: String::from("skill version 2"),
            name: String::from("zip-skill"),
            object: String::from("skill.version"),
            skill_id: String::from("skill_alpha"),
            version: String::from("2"),
            extra: Default::default(),
        }
    );

    let version_content = client
        .skills()
        .versions()
        .content()
        .retrieve("skill_alpha", "2")
        .unwrap();
    assert_eq!(version_content.output, b"PK\x03\x04version-skill");

    let deleted_version = client
        .skills()
        .versions()
        .delete("skill_alpha", "2")
        .unwrap();
    assert_eq!(
        deleted_version.output,
        SkillVersionDeleteResponse {
            id: String::from("skill_alpha"),
            deleted: true,
            object: String::from("skill.version.deleted"),
            version: String::from("2"),
            extra: Default::default(),
        }
    );

    let deleted_skill = client.skills().delete("skill_alpha").unwrap();
    assert_eq!(
        deleted_skill.output,
        SkillDeleteResponse {
            id: String::from("skill_alpha"),
            deleted: true,
            object: String::from("skill.deleted"),
            extra: Default::default(),
        }
    );

    let requests = server.captured_requests(11).unwrap();
    assert_eq!(requests[0].path, "/v1/skills");
    let content_type = requests[0].headers.get("content-type").unwrap();
    assert!(content_type.starts_with("multipart/form-data; boundary="));
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    let multipart = multipart_support::parse_multipart(&requests[0].body, boundary).unwrap();
    assert_eq!(multipart.parts.len(), 2);
    assert_eq!(multipart.parts[0].name.as_deref(), Some("files"));
    assert_eq!(
        multipart.parts[0].filename.as_deref(),
        Some("manifest.json")
    );
    assert_eq!(multipart.parts[0].body, br#"{"name":"zip"}"#);
    assert_eq!(multipart.parts[1].filename.as_deref(), Some("bundle.zip"));
    assert_eq!(multipart.parts[1].body, b"PK\x03\x04bundle");

    assert_eq!(requests[1].path, "/v1/skills/skill_alpha");
    assert_eq!(requests[2].path, "/v1/skills/skill_alpha");
    let update_body: serde_json::Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(update_body, json!({"default_version": "2"}));
    assert_eq!(
        requests[3].path,
        "/v1/skills?after=skill_prev&limit=2&order=desc"
    );
    assert_eq!(requests[4].path, "/v1/skills/skill_alpha/content");
    assert_eq!(
        requests[4].headers.get("accept").map(String::as_str),
        Some("application/binary")
    );
    assert_eq!(requests[5].path, "/v1/skills/skill_alpha/versions");
    let version_content_type = requests[5].headers.get("content-type").unwrap();
    let version_boundary = version_content_type.split("boundary=").nth(1).unwrap();
    let version_multipart =
        multipart_support::parse_multipart(&requests[5].body, version_boundary).unwrap();
    assert_eq!(version_multipart.parts.len(), 2);
    assert_eq!(version_multipart.parts[0].name.as_deref(), Some("default"));
    assert_eq!(version_multipart.parts[0].body, b"true");
    assert_eq!(version_multipart.parts[1].name.as_deref(), Some("files"));
    assert_eq!(
        version_multipart.parts[1].filename.as_deref(),
        Some("version.zip")
    );
    assert_eq!(requests[6].path, "/v1/skills/skill_alpha/versions/2");
    assert_eq!(
        requests[7].path,
        "/v1/skills/skill_alpha/versions?after=sv_1&limit=2&order=asc"
    );
    assert_eq!(
        requests[8].path,
        "/v1/skills/skill_alpha/versions/2/content"
    );
    assert_eq!(requests[9].path, "/v1/skills/skill_alpha/versions/2");
    assert_eq!(requests[10].path, "/v1/skills/skill_alpha");

    let blank_skill = client.skills().retrieve(" ").unwrap_err();
    assert!(matches!(blank_skill.kind, ErrorKind::Validation));
    let blank_version = client
        .skills()
        .versions()
        .retrieve("skill_alpha", " ")
        .unwrap_err();
    assert!(matches!(blank_version.kind, ErrorKind::Validation));
}

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_skills_smoke_skips_explicitly_when_the_project_lacks_entitlement() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live skills client should resolve configuration");
    assert_eq!(resolved.base_url, DEFAULT_BASE_URL);

    match client.skills().list(SkillListParams::default()) {
        Ok(response) => {
            println!(
                "live skills list request id: {}",
                response.request_id().unwrap_or("<missing>")
            );
        }
        Err(error) => {
            if let Some(reason) = entitlement_skip_reason(&error) {
                println!("skills live smoke skipped: {reason}");
                return;
            }
            panic!("live skills smoke should succeed or skip explicitly: {error}");
        }
    }
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

fn skill_payload(id: &str, default_version: &str, latest_version: &str) -> String {
    json!({
        "id": id,
        "object": "skill",
        "created_at": 1_717_171_717u64,
        "default_version": default_version,
        "description": "zip skill",
        "latest_version": latest_version,
        "name": "zip-skill"
    })
    .to_string()
}

fn skill_list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&skill_payload("skill_alpha", "2", "2")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&skill_payload("skill_beta", "1", "1")).unwrap()
        ],
        "first_id": "skill_alpha",
        "last_id": "skill_beta",
        "has_more": true
    })
    .to_string()
}

fn skill_version_payload(skill_id: &str, id: &str, version: &str) -> String {
    json!({
        "id": id,
        "object": "skill.version",
        "created_at": 1_717_171_818u64,
        "description": format!("skill version {version}"),
        "name": "zip-skill",
        "skill_id": skill_id,
        "version": version
    })
    .to_string()
}

fn skill_versions_payload(skill_id: &str) -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&skill_version_payload(skill_id, "sv_1", "1")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&skill_version_payload(skill_id, "sv_2", "2")).unwrap()
        ],
        "first_id": "sv_1",
        "last_id": "sv_2",
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
            (String::from("x-request-id"), String::from("req_skills")),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn binary_response(body: &[u8]) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/binary"),
            ),
            (
                String::from("x-request-id"),
                String::from("req_skills_content"),
            ),
        ],
        body: body.to_vec(),
        ..Default::default()
    }
}
