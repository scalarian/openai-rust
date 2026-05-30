#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::admin::{AdminQueryParams, AdminValue},
};
use serde_json::json;

#[test]
fn admin_organization_surface_matches_upstream_paths_and_payload_shapes() {
    let server = mock_http::MockHttpServer::spawn_sequence(
        (0..23)
            .map(|index| json_response(json!({"id": format!("admin_{index}")}).to_string()))
            .collect(),
    )
    .unwrap();
    let client = client(&server.url());
    let org = client.admin().organization();

    let created_key = org
        .admin_api_keys()
        .create(json!({"name": "ops-key"}))
        .unwrap();
    assert_eq!(created_key.output["id"], json!("admin_0"));
    org.admin_api_keys().retrieve("key_ops").unwrap();
    org.admin_api_keys()
        .list(
            AdminQueryParams::new()
                .push("after", "key_prev")
                .push("limit", 2),
        )
        .unwrap();
    org.admin_api_keys().delete("key_ops").unwrap();

    org.usage()
        .completions(
            AdminQueryParams::new()
                .push("start_time", 1_717_171_700)
                .push("bucket_width", "1d")
                .push_repeated("models", ["gpt-5.5", "gpt-5-mini"]),
        )
        .unwrap();
    org.usage()
        .costs(
            AdminQueryParams::new()
                .push("start_time", 1_717_171_700)
                .push("bucket_width", "1d")
                .push_repeated("group_by", ["project_id", "line_item"]),
        )
        .unwrap();

    org.users()
        .roles()
        .create("user_admin", json!({"role_id": "role_viewer"}))
        .unwrap();
    org.groups()
        .users()
        .list("grp_eng", AdminQueryParams::new().push("limit", 100))
        .unwrap();
    org.certificates()
        .activate(json!({"certificate_ids": ["cert_org"]}))
        .unwrap();

    let projects = org.projects();
    projects.create(json!({"name": "research"})).unwrap();
    projects
        .update("proj_research", json!({"name": "research-prod"}))
        .unwrap();
    projects
        .list(
            AdminQueryParams::new()
                .push("after", "proj_prev")
                .push("include_archived", true)
                .push("limit", 10),
        )
        .unwrap();
    projects.archive("proj_research").unwrap();
    projects
        .users()
        .create(
            "proj_research",
            json!({"user_id": "user_admin", "role": "owner"}),
        )
        .unwrap();
    projects
        .users()
        .roles()
        .delete("proj_research", "user_admin", "role_owner")
        .unwrap();
    projects
        .roles()
        .create(
            "proj_research",
            json!({"role_name": "auditor", "permissions": ["logs.read"]}),
        )
        .unwrap();
    projects
        .groups()
        .roles()
        .list(
            "proj_research",
            "grp_eng",
            AdminQueryParams::new().push("limit", 3),
        )
        .unwrap();
    projects
        .api_keys()
        .retrieve("proj_research", "key_project")
        .unwrap();
    projects
        .rate_limits()
        .update_rate_limit(
            "proj_research",
            "rl_gpt_5",
            json!({"max_requests_per_1_minute": 120}),
        )
        .unwrap();
    projects
        .model_permissions()
        .delete("proj_research")
        .unwrap();
    projects
        .hosted_tool_permissions()
        .update("proj_research", json!({"web_search": {"mode": "enabled"}}))
        .unwrap();
    projects
        .groups()
        .retrieve(
            "proj_research",
            "grp_eng",
            AdminQueryParams::new().push("group_type", "group"),
        )
        .unwrap();
    projects
        .certificates()
        .deactivate(
            "proj_research",
            json!({"certificate_ids": ["cert_project"]}),
        )
        .unwrap();

    let requests = server.captured_requests(23).unwrap();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/organization/admin_api_keys");
    assert_eq!(requests[1].path, "/v1/organization/admin_api_keys/key_ops");
    assert_eq!(
        requests[2].path,
        "/v1/organization/admin_api_keys?after=key_prev&limit=2"
    );
    assert_eq!(requests[3].method, "DELETE");
    assert_eq!(requests[3].path, "/v1/organization/admin_api_keys/key_ops");
    assert_eq!(
        requests[4].path,
        "/v1/organization/usage/completions?start_time=1717171700&bucket_width=1d&models=gpt-5.5&models=gpt-5-mini"
    );
    assert_eq!(
        requests[5].path,
        "/v1/organization/costs?start_time=1717171700&bucket_width=1d&group_by=project_id&group_by=line_item"
    );
    assert_eq!(requests[6].path, "/v1/organization/users/user_admin/roles");
    assert_eq!(
        requests[7].path,
        "/v1/organization/groups/grp_eng/users?limit=100"
    );
    assert_eq!(requests[8].path, "/v1/organization/certificates/activate");
    assert_eq!(requests[9].path, "/v1/organization/projects");
    assert_eq!(requests[10].path, "/v1/organization/projects/proj_research");
    assert_eq!(
        requests[11].path,
        "/v1/organization/projects?after=proj_prev&include_archived=true&limit=10"
    );
    assert_eq!(
        requests[12].path,
        "/v1/organization/projects/proj_research/archive"
    );
    assert_eq!(
        requests[13].path,
        "/v1/organization/projects/proj_research/users"
    );
    assert_eq!(
        requests[14].path,
        "/v1/projects/proj_research/users/user_admin/roles/role_owner"
    );
    assert_eq!(requests[15].path, "/v1/projects/proj_research/roles");
    assert_eq!(
        requests[16].path,
        "/v1/projects/proj_research/groups/grp_eng/roles?limit=3"
    );
    assert_eq!(
        requests[17].path,
        "/v1/organization/projects/proj_research/api_keys/key_project"
    );
    assert_eq!(
        requests[18].path,
        "/v1/organization/projects/proj_research/rate_limits/rl_gpt_5"
    );
    assert_eq!(
        requests[19].path,
        "/v1/organization/projects/proj_research/model_permissions"
    );
    assert_eq!(
        requests[20].path,
        "/v1/organization/projects/proj_research/hosted_tool_permissions"
    );
    assert_eq!(
        requests[21].path,
        "/v1/organization/projects/proj_research/groups/grp_eng?group_type=group"
    );
    assert_eq!(
        requests[22].path,
        "/v1/organization/projects/proj_research/certificates/deactivate"
    );

    let key_body: AdminValue = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(key_body["name"], json!("ops-key"));
    let certificate_body: AdminValue = serde_json::from_slice(&requests[8].body).unwrap();
    assert_eq!(certificate_body["certificate_ids"], json!(["cert_org"]));
    let project_user_body: AdminValue = serde_json::from_slice(&requests[13].body).unwrap();
    assert_eq!(project_user_body["role"], json!("owner"));
    let project_role_body: AdminValue = serde_json::from_slice(&requests[15].body).unwrap();
    assert_eq!(project_role_body["role_name"], json!("auditor"));
    let rate_limit_body: AdminValue = serde_json::from_slice(&requests[18].body).unwrap();
    assert_eq!(rate_limit_body["max_requests_per_1_minute"], json!(120));

    let blank_project = projects.retrieve(" ").unwrap_err();
    assert!(matches!(blank_project.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("x-request-id"), String::from("req_admin")),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}
