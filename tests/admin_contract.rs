#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::admin::{
        AdminApiKeyCreateParams, AdminApiKeyListParams, AdminAuditLogEffectiveAtParams,
        AdminAuditLogListParams, AdminCertificateCreateParams, AdminCertificateIdsParams,
        AdminCertificateListParams, AdminCertificateRetrieveParams, AdminCertificateUpdateParams,
        AdminDataRetentionUpdateParams, AdminGroupCreateParams, AdminGroupListParams,
        AdminGroupRoleCreateParams, AdminGroupRoleListParams, AdminGroupUpdateParams,
        AdminGroupUserCreateParams, AdminGroupUserListParams, AdminInviteCreateParams,
        AdminInviteListParams, AdminQueryParams, AdminRoleCreateParams, AdminRoleListParams,
        AdminRoleUpdateParams, AdminSpendAlertCreateParams, AdminSpendAlertListParams,
        AdminSpendAlertUpdateParams, AdminUserListParams, AdminUserRoleCreateParams,
        AdminUserRoleListParams, AdminUserUpdateParams, AdminValue,
    },
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
        .create(AdminApiKeyCreateParams {
            name: String::from("ops-key"),
        })
        .unwrap();
    assert_eq!(created_key.output["id"], json!("admin_0"));
    org.admin_api_keys().retrieve("key_ops").unwrap();
    org.admin_api_keys()
        .list(AdminApiKeyListParams {
            after: Some(String::from("key_prev")),
            limit: Some(2),
            order: Some(String::from("asc")),
        })
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
        .create(
            "user_admin",
            AdminUserRoleCreateParams {
                role_id: String::from("role_viewer"),
            },
        )
        .unwrap();
    org.groups()
        .users()
        .list(
            "grp_eng",
            AdminGroupUserListParams {
                limit: Some(100),
                ..Default::default()
            },
        )
        .unwrap();
    org.certificates()
        .activate(AdminCertificateIdsParams {
            certificate_ids: vec![String::from("cert_org")],
        })
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
        "/v1/organization/admin_api_keys?after=key_prev&limit=2&order=asc"
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

#[test]
fn admin_organization_typed_params_preserve_queries_and_bodies() {
    let server = mock_http::MockHttpServer::spawn_sequence(
        (0..24)
            .map(|index| json_response(json!({"id": format!("typed_admin_{index}")}).to_string()))
            .collect(),
    )
    .unwrap();
    let client = client(&server.url());
    let org = client.admin().organization();

    org.audit_logs()
        .list(AdminAuditLogListParams {
            actor_emails: Some(vec![
                String::from("ops@example.com"),
                String::from("sec@example.com"),
            ]),
            actor_ids: Some(vec![String::from("actor_1")]),
            after: Some(String::from("audit_after")),
            before: Some(String::from("audit_before")),
            effective_at: Some(AdminAuditLogEffectiveAtParams {
                gte: Some(100),
                lte: Some(200),
                ..Default::default()
            }),
            event_types: Some(vec![String::from("project.created")]),
            limit: Some(5),
            project_ids: Some(vec![String::from("proj_1")]),
            resource_ids: Some(vec![String::from("res_1")]),
        })
        .unwrap();

    org.invites()
        .create(AdminInviteCreateParams {
            email: String::from("new@example.com"),
            role: String::from("reader"),
            projects: Some(vec![json!({"id": "proj_1", "role": "member"})]),
        })
        .unwrap();
    org.invites()
        .list(AdminInviteListParams {
            after: Some(String::from("invite_after")),
            limit: Some(6),
        })
        .unwrap();

    org.users()
        .list(AdminUserListParams {
            after: Some(String::from("user_after")),
            emails: Some(vec![String::from("ops@example.com")]),
            limit: Some(7),
        })
        .unwrap();
    org.users()
        .update(
            "user_admin",
            AdminUserUpdateParams {
                role_id: Some(String::from("role_owner")),
                technical_level: Some(String::from("advanced")),
                ..Default::default()
            },
        )
        .unwrap();
    org.users()
        .roles()
        .list(
            "user_admin",
            AdminUserRoleListParams {
                after: Some(String::from("user_role_after")),
                limit: Some(8),
                order: Some(String::from("desc")),
            },
        )
        .unwrap();

    org.groups()
        .create(AdminGroupCreateParams {
            name: String::from("Engineering"),
        })
        .unwrap();
    org.groups()
        .update(
            "grp_eng",
            AdminGroupUpdateParams {
                name: String::from("Product Engineering"),
            },
        )
        .unwrap();
    org.groups()
        .list(AdminGroupListParams {
            after: Some(String::from("group_after")),
            limit: Some(9),
            order: Some(String::from("asc")),
        })
        .unwrap();
    org.groups()
        .users()
        .create(
            "grp_eng",
            AdminGroupUserCreateParams {
                user_id: String::from("user_admin"),
            },
        )
        .unwrap();
    org.groups()
        .roles()
        .create(
            "grp_eng",
            AdminGroupRoleCreateParams {
                role_id: String::from("role_viewer"),
            },
        )
        .unwrap();
    org.groups()
        .roles()
        .list(
            "grp_eng",
            AdminGroupRoleListParams {
                limit: Some(10),
                order: Some(String::from("asc")),
                ..Default::default()
            },
        )
        .unwrap();

    org.roles()
        .create(AdminRoleCreateParams {
            description: Some(String::from("Read audit logs")),
            permissions: vec![String::from("logs.read")],
            role_name: String::from("audit_reader"),
        })
        .unwrap();
    org.roles()
        .update(
            "role_audit",
            AdminRoleUpdateParams {
                description: Some(String::from("Read security audit logs")),
                permissions: vec![String::from("logs.read")],
                role_name: Some(String::from("audit_reader")),
            },
        )
        .unwrap();
    org.roles()
        .list(AdminRoleListParams {
            after: Some(String::from("role_after")),
            limit: Some(11),
            order: Some(String::from("desc")),
        })
        .unwrap();

    org.data_retention()
        .update(AdminDataRetentionUpdateParams {
            retention_type: String::from("default"),
        })
        .unwrap();

    org.certificates()
        .create(AdminCertificateCreateParams {
            certificate: String::from("-----BEGIN CERTIFICATE-----"),
            name: String::from("org-cert"),
        })
        .unwrap();
    org.certificates()
        .retrieve(
            "cert_org",
            AdminCertificateRetrieveParams {
                include: Some(vec![String::from("content")]),
            },
        )
        .unwrap();
    org.certificates()
        .list(AdminCertificateListParams {
            after: Some(String::from("cert_after")),
            limit: Some(12),
            order: Some(String::from("asc")),
        })
        .unwrap();
    org.certificates()
        .update(
            "cert_org",
            AdminCertificateUpdateParams {
                name: String::from("org-cert-renamed"),
            },
        )
        .unwrap();
    org.certificates()
        .deactivate(AdminCertificateIdsParams {
            certificate_ids: vec![String::from("cert_org")],
        })
        .unwrap();

    org.spend_alerts()
        .create(AdminSpendAlertCreateParams {
            currency: String::from("USD"),
            interval: String::from("month"),
            notification_channel: json!({
                "type": "email",
                "recipients": ["ops@example.com"]
            }),
            threshold_amount: 10_000,
        })
        .unwrap();
    org.spend_alerts()
        .list(AdminSpendAlertListParams {
            after: Some(String::from("alert_after")),
            before: Some(String::from("alert_before")),
            limit: Some(13),
            order: Some(String::from("desc")),
        })
        .unwrap();
    org.spend_alerts()
        .update(
            "alert_ops",
            AdminSpendAlertUpdateParams {
                currency: String::from("USD"),
                interval: String::from("month"),
                notification_channel: json!({
                    "type": "email",
                    "recipients": ["sec@example.com"]
                }),
                threshold_amount: 20_000,
            },
        )
        .unwrap();

    let requests = server.captured_requests(24).unwrap();
    let paths = requests
        .iter()
        .map(|request| request.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            "/v1/organization/audit_logs?actor_emails=ops%40example.com&actor_emails=sec%40example.com&actor_ids=actor_1&after=audit_after&before=audit_before&event_types=project.created&limit=5&project_ids=proj_1&resource_ids=res_1&effective_at%5Bgte%5D=100&effective_at%5Blte%5D=200",
            "/v1/organization/invites",
            "/v1/organization/invites?after=invite_after&limit=6",
            "/v1/organization/users?after=user_after&emails=ops%40example.com&limit=7",
            "/v1/organization/users/user_admin",
            "/v1/organization/users/user_admin/roles?after=user_role_after&limit=8&order=desc",
            "/v1/organization/groups",
            "/v1/organization/groups/grp_eng",
            "/v1/organization/groups?after=group_after&limit=9&order=asc",
            "/v1/organization/groups/grp_eng/users",
            "/v1/organization/groups/grp_eng/roles",
            "/v1/organization/groups/grp_eng/roles?limit=10&order=asc",
            "/v1/organization/roles",
            "/v1/organization/roles/role_audit",
            "/v1/organization/roles?after=role_after&limit=11&order=desc",
            "/v1/organization/data_retention",
            "/v1/organization/certificates",
            "/v1/organization/certificates/cert_org?include=content",
            "/v1/organization/certificates?after=cert_after&limit=12&order=asc",
            "/v1/organization/certificates/cert_org",
            "/v1/organization/certificates/deactivate",
            "/v1/organization/spend_alerts",
            "/v1/organization/spend_alerts?after=alert_after&before=alert_before&limit=13&order=desc",
            "/v1/organization/spend_alerts/alert_ops",
        ]
    );

    let invite_body: AdminValue = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(invite_body["email"], json!("new@example.com"));
    assert_eq!(invite_body["projects"][0]["role"], json!("member"));
    let user_update_body: AdminValue = serde_json::from_slice(&requests[4].body).unwrap();
    assert_eq!(user_update_body["role_id"], json!("role_owner"));
    let role_body: AdminValue = serde_json::from_slice(&requests[12].body).unwrap();
    assert_eq!(role_body["permissions"], json!(["logs.read"]));
    let spend_alert_body: AdminValue = serde_json::from_slice(&requests[21].body).unwrap();
    assert_eq!(spend_alert_body["threshold_amount"], json!(10_000));
}

#[test]
fn admin_organization_usage_categories_match_upstream_paths() {
    let server = mock_http::MockHttpServer::spawn_sequence(
        (0..10)
            .map(|index| json_response(json!({"id": format!("usage_{index}")}).to_string()))
            .collect(),
    )
    .unwrap();
    let client = client(&server.url());
    let usage = client.admin().organization().usage();
    let params = AdminQueryParams::new()
        .push("start_time", 1_717_171_700)
        .push("bucket_width", "1d")
        .push("page", "cursor_123")
        .push("limit", 1);

    usage.audio_speeches(params.clone()).unwrap();
    usage.audio_transcriptions(params.clone()).unwrap();
    usage.code_interpreter_sessions(params.clone()).unwrap();
    usage.completions(params.clone()).unwrap();
    usage.embeddings(params.clone()).unwrap();
    usage.file_search_calls(params.clone()).unwrap();
    usage.images(params.clone()).unwrap();
    usage.moderations(params.clone()).unwrap();
    usage.vector_stores(params.clone()).unwrap();
    usage.web_search_calls(params).unwrap();

    let requests = server.captured_requests(10).unwrap();
    let paths = requests
        .iter()
        .map(|request| request.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            "/v1/organization/usage/audio_speeches?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
            "/v1/organization/usage/audio_transcriptions?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
            "/v1/organization/usage/code_interpreter_sessions?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
            "/v1/organization/usage/completions?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
            "/v1/organization/usage/embeddings?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
            "/v1/organization/usage/file_search_calls?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
            "/v1/organization/usage/images?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
            "/v1/organization/usage/moderations?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
            "/v1/organization/usage/vector_stores?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
            "/v1/organization/usage/web_search_calls?start_time=1717171700&bucket_width=1d&page=cursor_123&limit=1",
        ]
    );
    assert!(requests.iter().all(|request| request.method == "GET"));
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
