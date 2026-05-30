#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::{admin::*, common::ListOrder},
};
use serde_json::{Value, json};

fn project_api_key_json(id: &str) -> Value {
    json!({
        "id": id,
        "created_at": 1_717_171_800u64,
        "last_used_at": null,
        "name": "project-key",
        "object": "organization.project.api_key",
        "owner": {
            "type": "user",
            "user": {
                "id": "user_project",
                "created_at": 1_717_171_500u64,
                "email": "project@example.com",
                "name": "Project User",
                "role": "owner"
            }
        },
        "redacted_value": "sk-project-...0"
    })
}

fn organization_data_retention_json(retention_type: &str) -> Value {
    json!({
        "object": "organization.data_retention",
        "type": retention_type
    })
}

fn project_data_retention_json(retention_type: &str) -> Value {
    json!({
        "object": "project.data_retention",
        "type": retention_type
    })
}

fn project_model_permissions_json() -> Value {
    json!({
        "mode": "allow_list",
        "model_ids": ["gpt-5"],
        "object": "project.model_permissions"
    })
}

fn project_hosted_tool_permissions_json() -> Value {
    json!({
        "code_interpreter": {"enabled": false},
        "file_search": {"enabled": false},
        "image_generation": {"enabled": false},
        "mcp": {"enabled": false},
        "web_search": {"enabled": true}
    })
}

fn organization_invite_json(id: &str) -> Value {
    json!({
        "id": id,
        "created_at": 1_717_171_900u64,
        "email": "new@example.com",
        "object": "organization.invite",
        "projects": [{"id": "proj_1", "role": "member"}],
        "role": "reader",
        "status": "pending",
        "accepted_at": null,
        "expires_at": 1_717_258_300u64
    })
}

fn organization_user_json(id: &str) -> Value {
    json!({
        "id": id,
        "added_at": 1_717_172_000u64,
        "object": "organization.user",
        "api_key_last_used_at": null,
        "created": 1_717_171_400u64,
        "developer_persona": "builder",
        "email": "ops@example.com",
        "is_default": false,
        "is_scale_tier_authorized_purchaser": false,
        "is_scim_managed": false,
        "is_service_account": false,
        "name": "Ops User",
        "projects": {
            "object": "list",
            "data": [{"id": "proj_1", "name": "Research", "role": "owner"}]
        },
        "role": "owner",
        "technical_level": "advanced",
        "user": {
            "id": id,
            "object": "user",
            "email": "ops@example.com",
            "enabled": true,
            "name": "Ops User"
        }
    })
}

#[test]
fn admin_organization_surface_matches_upstream_paths_and_payload_shapes() {
    let key_owner = json!({
        "id": "user_owner",
        "created_at": 1_717_171_600u64,
        "name": "Ops Owner",
        "object": "organization.user",
        "role": "owner",
        "type": "user"
    });
    let admin_key = json!({
        "id": "admin_0",
        "created_at": 1_717_171_700u64,
        "object": "organization.admin_api_key",
        "owner": key_owner,
        "redacted_value": "sk-admin-...0",
        "last_used_at": null,
        "name": "ops-key"
    });
    let mut responses = vec![
        json_response(
            json!({
                "id": "admin_0",
                "created_at": 1_717_171_700u64,
                "object": "organization.admin_api_key",
                "owner": {
                    "id": "user_owner",
                    "created_at": 1_717_171_600u64,
                    "name": "Ops Owner",
                    "object": "organization.user",
                    "role": "owner",
                    "type": "user"
                },
                "redacted_value": "sk-admin-...0",
                "value": "sk-admin-full",
                "name": "ops-key"
            })
            .to_string(),
        ),
        json_response(admin_key.to_string()),
        json_response(
            json!({
                "object": "list",
                "data": [admin_key],
                "has_more": false
            })
            .to_string(),
        ),
        json_response(
            json!({
                "id": "key_ops",
                "deleted": true,
                "object": "organization.admin_api_key.deleted"
            })
            .to_string(),
        ),
    ];
    responses.extend(
        (4..23).map(|index| json_response(json!({"id": format!("admin_{index}")}).to_string())),
    );
    responses[17] = json_response(project_api_key_json("key_project").to_string());
    responses[19] = json_response(
        json!({
            "deleted": true,
            "object": "project.model_permissions.deleted"
        })
        .to_string(),
    );
    responses[20] = json_response(project_hosted_tool_permissions_json().to_string());
    let server = mock_http::MockHttpServer::spawn_sequence(responses).unwrap();
    let client = client(&server.url());
    let org = client.admin().organization();

    let created_key = org
        .admin_api_keys()
        .create(AdminApiKeyCreateParams {
            name: String::from("ops-key"),
        })
        .unwrap();
    assert_eq!(created_key.output.id, "admin_0");
    assert_eq!(
        created_key.output.object,
        AdminApiKeyObject::OrganizationAdminApiKey
    );
    assert_eq!(created_key.output.value, "sk-admin-full");
    assert_eq!(created_key.output.owner.owner_type.as_deref(), Some("user"));
    let retrieved_key = org.admin_api_keys().retrieve("key_ops").unwrap();
    assert_eq!(retrieved_key.output.redacted_value, "sk-admin-...0");
    let listed_keys = org
        .admin_api_keys()
        .list(AdminApiKeyListParams {
            after: Some(String::from("key_prev")),
            limit: Some(2),
            order: Some(ListOrder::Asc),
        })
        .unwrap();
    assert_eq!(listed_keys.output.data[0].id, "admin_0");
    assert_eq!(listed_keys.output.has_more, Some(false));
    assert_eq!(listed_keys.output.next_after(), None);
    let deleted_key = org.admin_api_keys().delete("key_ops").unwrap();
    assert!(deleted_key.output.deleted);
    assert_eq!(
        deleted_key.output.object,
        AdminApiKeyDeletedObject::OrganizationAdminApiKeyDeleted
    );

    org.usage()
        .completions(AdminUsageCompletionsParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            models: Some(vec![String::from("gpt-5.5"), String::from("gpt-5-mini")]),
            ..Default::default()
        })
        .unwrap();
    org.usage()
        .costs(AdminUsageCostsParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageCostsBucketWidth::OneDay),
            group_by: Some(vec![
                AdminUsageCostsGroupBy::ProjectId,
                AdminUsageCostsGroupBy::LineItem,
            ]),
            ..Default::default()
        })
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
    projects
        .create(AdminProjectCreateParams {
            name: String::from("research"),
            external_key_id: Some(String::from("key_ext")),
            geography: Some(String::from("us")),
        })
        .unwrap();
    projects
        .update(
            "proj_research",
            AdminProjectUpdateParams {
                name: Some(String::from("research-prod")),
                ..Default::default()
            },
        )
        .unwrap();
    projects
        .list(AdminProjectListParams {
            after: Some(String::from("proj_prev")),
            include_archived: Some(true),
            limit: Some(10),
        })
        .unwrap();
    projects.archive("proj_research").unwrap();
    projects
        .users()
        .create(
            "proj_research",
            AdminProjectUserCreateParams {
                role: String::from("owner"),
                email: None,
                user_id: Some(String::from("user_admin")),
            },
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
            AdminProjectRoleCreateParams {
                description: None,
                permissions: vec![String::from("logs.read")],
                role_name: String::from("auditor"),
            },
        )
        .unwrap();
    projects
        .groups()
        .roles()
        .list(
            "proj_research",
            "grp_eng",
            AdminProjectGroupRoleListParams {
                limit: Some(3),
                ..Default::default()
            },
        )
        .unwrap();
    let project_key = projects
        .api_keys()
        .retrieve("proj_research", "key_project")
        .unwrap();
    assert_eq!(
        project_key.output.object,
        ProjectApiKeyObject::OrganizationProjectApiKey
    );
    assert_eq!(
        project_key.output.owner.owner_type,
        Some(ProjectApiKeyOwnerType::User)
    );
    projects
        .rate_limits()
        .update_rate_limit(
            "proj_research",
            "rl_gpt_5",
            AdminProjectRateLimitUpdateParams {
                max_requests_per_1_minute: Some(120),
                ..Default::default()
            },
        )
        .unwrap();
    let deleted_model_permissions = projects
        .model_permissions()
        .delete("proj_research")
        .unwrap();
    assert!(deleted_model_permissions.output.deleted);
    assert_eq!(
        deleted_model_permissions.output.object,
        AdminProjectModelPermissionsDeletedObject::ProjectModelPermissionsDeleted
    );
    let hosted_tool_permissions = projects
        .hosted_tool_permissions()
        .update(
            "proj_research",
            AdminProjectHostedToolPermissionUpdateParams {
                web_search: Some(AdminHostedToolPermission { enabled: true }),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(hosted_tool_permissions.output.web_search.enabled);
    projects
        .groups()
        .retrieve(
            "proj_research",
            "grp_eng",
            AdminProjectGroupRetrieveParams {
                group_type: Some(AdminGroupType::Group),
            },
        )
        .unwrap();
    projects
        .certificates()
        .deactivate(
            "proj_research",
            AdminProjectCertificateIdsParams {
                certificate_ids: vec![String::from("cert_project")],
            },
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
    let hosted_tool_body: AdminValue = serde_json::from_slice(&requests[20].body).unwrap();
    assert_eq!(hosted_tool_body["web_search"], json!({"enabled": true}));

    let blank_project = projects.retrieve(" ").unwrap_err();
    assert!(matches!(blank_project.kind, ErrorKind::Validation));
}

#[test]
fn admin_organization_typed_params_preserve_queries_and_bodies() {
    let mut responses = (0..24)
        .map(|index| json_response(json!({"id": format!("typed_admin_{index}")}).to_string()))
        .collect::<Vec<_>>();
    responses[1] = json_response(organization_invite_json("invite_new").to_string());
    responses[2] = json_response(
        json!({
            "data": [organization_invite_json("invite_new")],
            "has_more": true,
            "last_id": "invite_new"
        })
        .to_string(),
    );
    responses[3] = json_response(
        json!({
            "data": [organization_user_json("user_admin")],
            "has_more": true,
            "last_id": "user_admin"
        })
        .to_string(),
    );
    responses[4] = json_response(organization_user_json("user_admin").to_string());
    responses[15] =
        json_response(organization_data_retention_json("zero_data_retention").to_string());
    let server = mock_http::MockHttpServer::spawn_sequence(responses).unwrap();
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
            event_types: Some(vec![AdminAuditLogEventType::ProjectCreated]),
            limit: Some(5),
            project_ids: Some(vec![String::from("proj_1")]),
            resource_ids: Some(vec![String::from("res_1")]),
        })
        .unwrap();

    let invite = org
        .invites()
        .create(AdminInviteCreateParams {
            email: String::from("new@example.com"),
            role: AdminInviteRole::Reader,
            projects: Some(vec![AdminInviteProject {
                id: String::from("proj_1"),
                role: AdminProjectMembershipRole::Member,
            }]),
        })
        .unwrap();
    assert_eq!(invite.output.object, AdminInviteObject::OrganizationInvite);
    assert_eq!(invite.output.role, AdminInviteRole::Reader);
    assert_eq!(invite.output.status, AdminInviteStatus::Pending);
    assert_eq!(
        invite.output.projects[0].role,
        AdminProjectMembershipRole::Member
    );
    let invites = org
        .invites()
        .list(AdminInviteListParams {
            after: Some(String::from("invite_after")),
            limit: Some(6),
        })
        .unwrap();
    assert_eq!(invites.output.data[0].id, "invite_new");
    assert_eq!(invites.output.next_after(), Some("invite_new"));

    let users = org
        .users()
        .list(AdminUserListParams {
            after: Some(String::from("user_after")),
            emails: Some(vec![String::from("ops@example.com")]),
            limit: Some(7),
        })
        .unwrap();
    assert_eq!(
        users.output.data[0].object,
        AdminOrganizationUserObject::OrganizationUser
    );
    assert_eq!(users.output.next_after(), Some("user_admin"));
    let updated_user = org
        .users()
        .update(
            "user_admin",
            AdminUserUpdateParams {
                role_id: Some(String::from("role_owner")),
                technical_level: Some(String::from("advanced")),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(updated_user.output.role.as_deref(), Some("owner"));
    assert_eq!(
        updated_user.output.technical_level.as_deref(),
        Some("advanced")
    );
    org.users()
        .roles()
        .list(
            "user_admin",
            AdminUserRoleListParams {
                after: Some(String::from("user_role_after")),
                limit: Some(8),
                order: Some(ListOrder::Desc),
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
            order: Some(ListOrder::Asc),
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
                order: Some(ListOrder::Asc),
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
            order: Some(ListOrder::Desc),
        })
        .unwrap();

    let organization_retention = org
        .data_retention()
        .update(AdminDataRetentionUpdateParams {
            retention_type: AdminOrganizationDataRetentionType::ZeroDataRetention,
        })
        .unwrap();
    assert_eq!(
        organization_retention.output.object,
        AdminOrganizationDataRetentionObject::OrganizationDataRetention
    );
    assert_eq!(
        organization_retention.output.retention_type,
        AdminOrganizationDataRetentionType::ZeroDataRetention
    );

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
                include: Some(vec![AdminCertificateInclude::Content]),
            },
        )
        .unwrap();
    org.certificates()
        .list(AdminCertificateListParams {
            after: Some(String::from("cert_after")),
            limit: Some(12),
            order: Some(ListOrder::Asc),
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
            currency: AdminSpendAlertCurrency::Usd,
            interval: AdminSpendAlertInterval::Month,
            notification_channel: AdminSpendAlertNotificationChannel {
                recipients: vec![String::from("ops@example.com")],
                kind: AdminSpendAlertNotificationType::Email,
                subject_prefix: None,
            },
            threshold_amount: 10_000,
        })
        .unwrap();
    org.spend_alerts()
        .list(AdminSpendAlertListParams {
            after: Some(String::from("alert_after")),
            before: Some(String::from("alert_before")),
            limit: Some(13),
            order: Some(ListOrder::Desc),
        })
        .unwrap();
    org.spend_alerts()
        .update(
            "alert_ops",
            AdminSpendAlertUpdateParams {
                currency: AdminSpendAlertCurrency::Usd,
                interval: AdminSpendAlertInterval::Month,
                notification_channel: AdminSpendAlertNotificationChannel {
                    recipients: vec![String::from("sec@example.com")],
                    kind: AdminSpendAlertNotificationType::Email,
                    subject_prefix: None,
                },
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
    assert_eq!(invite_body["role"], json!("reader"));
    assert_eq!(invite_body["projects"][0]["role"], json!("member"));
    let user_update_body: AdminValue = serde_json::from_slice(&requests[4].body).unwrap();
    assert_eq!(user_update_body["role_id"], json!("role_owner"));
    let role_body: AdminValue = serde_json::from_slice(&requests[12].body).unwrap();
    assert_eq!(role_body["permissions"], json!(["logs.read"]));
    let spend_alert_body: AdminValue = serde_json::from_slice(&requests[21].body).unwrap();
    assert_eq!(spend_alert_body["threshold_amount"], json!(10_000));
    assert_eq!(
        spend_alert_body["notification_channel"],
        json!({"type": "email", "recipients": ["ops@example.com"]})
    );
}

#[test]
fn admin_project_typed_params_preserve_queries_and_bodies() {
    let mut responses = (0..21)
        .map(|index| json_response(json!({"id": format!("typed_project_{index}")}).to_string()))
        .collect::<Vec<_>>();
    responses[7] = json_response(
        json!({
            "data": [project_api_key_json("key_project")],
            "has_more": true,
            "last_id": "key_project"
        })
        .to_string(),
    );
    responses[9] = json_response(project_model_permissions_json().to_string());
    responses[15] = json_response(project_data_retention_json("organization_default").to_string());
    let server = mock_http::MockHttpServer::spawn_sequence(responses).unwrap();
    let client = client(&server.url());
    let projects = client.admin().organization().projects();

    projects
        .users()
        .list(
            "proj_research",
            AdminProjectUserListParams {
                after: Some(String::from("project_user_after")),
                limit: Some(2),
            },
        )
        .unwrap();
    projects
        .users()
        .update(
            "proj_research",
            "user_admin",
            AdminProjectUserUpdateParams {
                role: Some(String::from("member")),
            },
        )
        .unwrap();
    projects
        .users()
        .roles()
        .create(
            "proj_research",
            "user_admin",
            AdminProjectUserRoleCreateParams {
                role_id: String::from("role_project_viewer"),
            },
        )
        .unwrap();
    projects
        .users()
        .roles()
        .list(
            "proj_research",
            "user_admin",
            AdminProjectUserRoleListParams {
                after: Some(String::from("project_user_role_after")),
                limit: Some(3),
                order: Some(ListOrder::Asc),
            },
        )
        .unwrap();

    projects
        .service_accounts()
        .create(
            "proj_research",
            AdminProjectServiceAccountCreateParams {
                name: String::from("robot"),
            },
        )
        .unwrap();
    projects
        .service_accounts()
        .update(
            "proj_research",
            "svc_robot",
            AdminProjectServiceAccountUpdateParams {
                name: Some(String::from("robot-prod")),
                role: Some(AdminProjectMembershipRole::Owner),
            },
        )
        .unwrap();
    projects
        .service_accounts()
        .list(
            "proj_research",
            AdminProjectServiceAccountListParams {
                after: Some(String::from("svc_after")),
                limit: Some(4),
            },
        )
        .unwrap();

    let project_keys = projects
        .api_keys()
        .list(
            "proj_research",
            AdminProjectApiKeyListParams {
                after: Some(String::from("key_after")),
                limit: Some(5),
            },
        )
        .unwrap();
    assert_eq!(project_keys.output.data[0].id, "key_project");
    assert_eq!(project_keys.output.next_after(), Some("key_project"));
    projects
        .rate_limits()
        .list_rate_limits(
            "proj_research",
            AdminProjectRateLimitListParams {
                after: Some(String::from("rate_after")),
                before: Some(String::from("rate_before")),
                limit: Some(6),
            },
        )
        .unwrap();
    let model_permissions = projects
        .model_permissions()
        .update(
            "proj_research",
            AdminProjectModelPermissionUpdateParams {
                mode: Some(AdminProjectModelPermissionMode::AllowList),
                model_ids: Some(vec![String::from("gpt-5")]),
            },
        )
        .unwrap();
    assert_eq!(
        model_permissions.output.object,
        AdminProjectModelPermissionsObject::ProjectModelPermissions
    );
    assert_eq!(
        model_permissions.output.mode,
        AdminProjectModelPermissionMode::AllowList
    );
    assert_eq!(
        model_permissions.output.model_ids,
        vec![String::from("gpt-5")]
    );

    projects
        .groups()
        .create(
            "proj_research",
            AdminProjectGroupCreateParams {
                group_id: String::from("grp_eng"),
                role: String::from("member"),
            },
        )
        .unwrap();
    projects
        .groups()
        .list(
            "proj_research",
            AdminProjectGroupListParams {
                after: Some(String::from("project_group_after")),
                limit: Some(7),
                order: Some(ListOrder::Desc),
            },
        )
        .unwrap();
    projects
        .groups()
        .roles()
        .create(
            "proj_research",
            "grp_eng",
            AdminProjectGroupRoleCreateParams {
                role_id: String::from("role_group_viewer"),
            },
        )
        .unwrap();

    projects
        .roles()
        .update(
            "proj_research",
            "role_audit",
            AdminProjectRoleUpdateParams {
                description: Some(String::from("Project audit reader")),
                permissions: vec![String::from("logs.read")],
                role_name: Some(String::from("project_audit_reader")),
            },
        )
        .unwrap();
    projects
        .roles()
        .list(
            "proj_research",
            AdminProjectRoleListParams {
                after: Some(String::from("project_role_after")),
                limit: Some(8),
                order: Some(ListOrder::Asc),
            },
        )
        .unwrap();

    let project_retention = projects
        .data_retention()
        .update(
            "proj_research",
            AdminProjectDataRetentionUpdateParams {
                retention_type: AdminProjectDataRetentionType::OrganizationDefault,
            },
        )
        .unwrap();
    assert_eq!(
        project_retention.output.object,
        AdminProjectDataRetentionObject::ProjectDataRetention
    );
    assert_eq!(
        project_retention.output.retention_type,
        AdminProjectDataRetentionType::OrganizationDefault
    );
    projects
        .spend_alerts()
        .create(
            "proj_research",
            AdminProjectSpendAlertCreateParams {
                currency: AdminSpendAlertCurrency::Usd,
                interval: AdminSpendAlertInterval::Month,
                notification_channel: AdminSpendAlertNotificationChannel {
                    recipients: vec![String::from("ops@example.com")],
                    kind: AdminSpendAlertNotificationType::Email,
                    subject_prefix: None,
                },
                threshold_amount: 30_000,
            },
        )
        .unwrap();
    projects
        .spend_alerts()
        .list(
            "proj_research",
            AdminProjectSpendAlertListParams {
                after: Some(String::from("project_alert_after")),
                before: Some(String::from("project_alert_before")),
                limit: Some(9),
                order: Some(ListOrder::Desc),
            },
        )
        .unwrap();
    projects
        .spend_alerts()
        .update(
            "proj_research",
            "alert_project",
            AdminProjectSpendAlertUpdateParams {
                currency: AdminSpendAlertCurrency::Usd,
                interval: AdminSpendAlertInterval::Month,
                notification_channel: AdminSpendAlertNotificationChannel {
                    recipients: vec![String::from("sec@example.com")],
                    kind: AdminSpendAlertNotificationType::Email,
                    subject_prefix: None,
                },
                threshold_amount: 40_000,
            },
        )
        .unwrap();

    projects
        .certificates()
        .list(
            "proj_research",
            AdminProjectCertificateListParams {
                after: Some(String::from("project_cert_after")),
                limit: Some(10),
                order: Some(ListOrder::Asc),
            },
        )
        .unwrap();
    projects
        .certificates()
        .activate(
            "proj_research",
            AdminProjectCertificateIdsParams {
                certificate_ids: vec![String::from("cert_project")],
            },
        )
        .unwrap();

    let requests = server.captured_requests(21).unwrap();
    let paths = requests
        .iter()
        .map(|request| request.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            "/v1/organization/projects/proj_research/users?after=project_user_after&limit=2",
            "/v1/organization/projects/proj_research/users/user_admin",
            "/v1/projects/proj_research/users/user_admin/roles",
            "/v1/projects/proj_research/users/user_admin/roles?after=project_user_role_after&limit=3&order=asc",
            "/v1/organization/projects/proj_research/service_accounts",
            "/v1/organization/projects/proj_research/service_accounts/svc_robot",
            "/v1/organization/projects/proj_research/service_accounts?after=svc_after&limit=4",
            "/v1/organization/projects/proj_research/api_keys?after=key_after&limit=5",
            "/v1/organization/projects/proj_research/rate_limits?after=rate_after&before=rate_before&limit=6",
            "/v1/organization/projects/proj_research/model_permissions",
            "/v1/organization/projects/proj_research/groups",
            "/v1/organization/projects/proj_research/groups?after=project_group_after&limit=7&order=desc",
            "/v1/projects/proj_research/groups/grp_eng/roles",
            "/v1/projects/proj_research/roles/role_audit",
            "/v1/projects/proj_research/roles?after=project_role_after&limit=8&order=asc",
            "/v1/organization/projects/proj_research/data_retention",
            "/v1/organization/projects/proj_research/spend_alerts",
            "/v1/organization/projects/proj_research/spend_alerts?after=project_alert_after&before=project_alert_before&limit=9&order=desc",
            "/v1/organization/projects/proj_research/spend_alerts/alert_project",
            "/v1/organization/projects/proj_research/certificates?after=project_cert_after&limit=10&order=asc",
            "/v1/organization/projects/proj_research/certificates/activate",
        ]
    );

    let user_role_body: AdminValue = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(user_role_body["role_id"], json!("role_project_viewer"));
    let service_account_body: AdminValue = serde_json::from_slice(&requests[4].body).unwrap();
    assert_eq!(service_account_body["name"], json!("robot"));
    let model_permissions_body: AdminValue = serde_json::from_slice(&requests[9].body).unwrap();
    assert_eq!(model_permissions_body["mode"], json!("allow_list"));
    assert_eq!(model_permissions_body["model_ids"], json!(["gpt-5"]));
    let group_body: AdminValue = serde_json::from_slice(&requests[10].body).unwrap();
    assert_eq!(group_body["group_id"], json!("grp_eng"));
    let project_alert_body: AdminValue = serde_json::from_slice(&requests[16].body).unwrap();
    assert_eq!(project_alert_body["threshold_amount"], json!(30_000));
    assert_eq!(
        project_alert_body["notification_channel"],
        json!({"type": "email", "recipients": ["ops@example.com"]})
    );
}

#[test]
fn admin_project_api_key_delete_returns_typed_confirmation() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![json_response(
        json!({
            "id": "key_project",
            "deleted": true,
            "object": "organization.project.api_key.deleted"
        })
        .to_string(),
    )])
    .unwrap();
    let client = client(&server.url());

    let deleted = client
        .admin()
        .organization()
        .projects()
        .api_keys()
        .delete("proj_research", "key_project")
        .unwrap();

    assert!(deleted.output.deleted);
    assert_eq!(
        deleted.output.object,
        ProjectApiKeyDeletedObject::OrganizationProjectApiKeyDeleted
    );
    let requests = server.captured_requests(1).unwrap();
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(
        requests[0].path,
        "/v1/organization/projects/proj_research/api_keys/key_project"
    );
}

#[test]
fn admin_invite_retrieve_and_delete_return_typed_responses() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(organization_invite_json("invite_new").to_string()),
        json_response(
            json!({
                "id": "invite_new",
                "deleted": true,
                "object": "organization.invite.deleted"
            })
            .to_string(),
        ),
    ])
    .unwrap();
    let client = client(&server.url());
    let invites = client.admin().organization().invites();

    let invite = invites.retrieve("invite_new").unwrap();
    assert_eq!(invite.output.object, AdminInviteObject::OrganizationInvite);
    assert_eq!(invite.output.status, AdminInviteStatus::Pending);

    let deleted = invites.delete("invite_new").unwrap();
    assert!(deleted.output.deleted);
    assert_eq!(
        deleted.output.object,
        AdminInviteDeletedObject::OrganizationInviteDeleted
    );

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(requests[0].path, "/v1/organization/invites/invite_new");
    assert_eq!(requests[1].method, "DELETE");
    assert_eq!(requests[1].path, "/v1/organization/invites/invite_new");
}

#[test]
fn admin_user_retrieve_and_delete_return_typed_responses() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(organization_user_json("user_admin").to_string()),
        json_response(
            json!({
                "id": "user_admin",
                "deleted": true,
                "object": "organization.user.deleted"
            })
            .to_string(),
        ),
    ])
    .unwrap();
    let client = client(&server.url());
    let users = client.admin().organization().users();

    let user = users.retrieve("user_admin").unwrap();
    assert_eq!(
        user.output.object,
        AdminOrganizationUserObject::OrganizationUser
    );
    assert_eq!(
        user.output.user.as_ref().map(|inner| &inner.object),
        Some(&AdminUserObject::User)
    );

    let deleted = users.delete("user_admin").unwrap();
    assert!(deleted.output.deleted);
    assert_eq!(
        deleted.output.object,
        AdminOrganizationUserDeletedObject::OrganizationUserDeleted
    );

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(requests[0].path, "/v1/organization/users/user_admin");
    assert_eq!(requests[1].method, "DELETE");
    assert_eq!(requests[1].path, "/v1/organization/users/user_admin");
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

    usage
        .audio_speeches(AdminUsageAudioSpeechesParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            ..Default::default()
        })
        .unwrap();
    usage
        .audio_transcriptions(AdminUsageAudioTranscriptionsParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            ..Default::default()
        })
        .unwrap();
    usage
        .code_interpreter_sessions(AdminUsageCodeInterpreterSessionsParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            ..Default::default()
        })
        .unwrap();
    usage
        .completions(AdminUsageCompletionsParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            ..Default::default()
        })
        .unwrap();
    usage
        .embeddings(AdminUsageEmbeddingsParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            ..Default::default()
        })
        .unwrap();
    usage
        .file_search_calls(AdminUsageFileSearchCallsParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            ..Default::default()
        })
        .unwrap();
    usage
        .images(AdminUsageImagesParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            sizes: Some(vec![AdminUsageImagesSize::Size1024x1024]),
            sources: Some(vec![AdminUsageImagesSource::Generation]),
            ..Default::default()
        })
        .unwrap();
    usage
        .moderations(AdminUsageModerationsParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            ..Default::default()
        })
        .unwrap();
    usage
        .vector_stores(AdminUsageVectorStoresParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            ..Default::default()
        })
        .unwrap();
    usage
        .web_search_calls(AdminUsageWebSearchCallsParams {
            start_time: Some(1_717_171_700),
            bucket_width: Some(AdminUsageBucketWidth::OneDay),
            context_levels: Some(vec![AdminUsageWebSearchContextLevel::High]),
            limit: Some(1),
            page: Some(String::from("cursor_123")),
            ..Default::default()
        })
        .unwrap();

    let requests = server.captured_requests(10).unwrap();
    let paths = requests
        .iter()
        .map(|request| request.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            "/v1/organization/usage/audio_speeches?start_time=1717171700&bucket_width=1d&limit=1&page=cursor_123",
            "/v1/organization/usage/audio_transcriptions?start_time=1717171700&bucket_width=1d&limit=1&page=cursor_123",
            "/v1/organization/usage/code_interpreter_sessions?start_time=1717171700&bucket_width=1d&limit=1&page=cursor_123",
            "/v1/organization/usage/completions?start_time=1717171700&bucket_width=1d&limit=1&page=cursor_123",
            "/v1/organization/usage/embeddings?start_time=1717171700&bucket_width=1d&limit=1&page=cursor_123",
            "/v1/organization/usage/file_search_calls?start_time=1717171700&bucket_width=1d&limit=1&page=cursor_123",
            "/v1/organization/usage/images?start_time=1717171700&bucket_width=1d&limit=1&page=cursor_123&sizes=1024x1024&sources=image.generation",
            "/v1/organization/usage/moderations?start_time=1717171700&bucket_width=1d&limit=1&page=cursor_123",
            "/v1/organization/usage/vector_stores?start_time=1717171700&bucket_width=1d&limit=1&page=cursor_123",
            "/v1/organization/usage/web_search_calls?start_time=1717171700&bucket_width=1d&context_levels=high&limit=1&page=cursor_123",
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
