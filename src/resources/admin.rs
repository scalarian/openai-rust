use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    resources::files::{encode_path_id, validate_path_id},
};

/// JSON value returned by the flexible admin endpoint surface.
pub type AdminValue = Value;

/// Query parameters for admin endpoints.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminQueryParams {
    pairs: Vec<(String, String)>,
}

impl AdminQueryParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.append(key, value);
        self
    }

    pub fn append(&mut self, key: impl Into<String>, value: impl ToString) {
        self.pairs.push((key.into(), value.to_string()));
    }

    pub fn push_repeated<I, V>(mut self, key: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: ToString,
    {
        let key = key.into();
        for value in values {
            self.pairs.push((key.clone(), value.to_string()));
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn pairs(&self) -> &[(String, String)] {
        &self.pairs
    }
}

impl<K, V, const N: usize> From<[(K, V); N]> for AdminQueryParams
where
    K: Into<String>,
    V: ToString,
{
    fn from(value: [(K, V); N]) -> Self {
        Self {
            pairs: value
                .into_iter()
                .map(|(key, value)| (key.into(), value.to_string()))
                .collect(),
        }
    }
}

impl From<Vec<(String, String)>> for AdminQueryParams {
    fn from(pairs: Vec<(String, String)>) -> Self {
        Self { pairs }
    }
}

/// Admin API family.
#[derive(Clone, Debug)]
pub struct Admin {
    runtime: Arc<ClientRuntime>,
}

impl Admin {
    pub(crate) fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    /// Returns organization-scoped admin operations.
    pub fn organization(&self) -> Organization {
        Organization::new(self.runtime.clone())
    }
}

/// Organization-scoped admin operations.
#[derive(Clone, Debug)]
pub struct Organization {
    runtime: Arc<ClientRuntime>,
}

impl Organization {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn audit_logs(&self) -> AuditLogs {
        AuditLogs::new(self.runtime.clone())
    }

    pub fn admin_api_keys(&self) -> AdminApiKeys {
        AdminApiKeys::new(self.runtime.clone())
    }

    pub fn usage(&self) -> OrganizationUsage {
        OrganizationUsage::new(self.runtime.clone())
    }

    pub fn invites(&self) -> OrganizationInvites {
        OrganizationInvites::new(self.runtime.clone())
    }

    pub fn users(&self) -> OrganizationUsers {
        OrganizationUsers::new(self.runtime.clone())
    }

    pub fn groups(&self) -> OrganizationGroups {
        OrganizationGroups::new(self.runtime.clone())
    }

    pub fn roles(&self) -> OrganizationRoles {
        OrganizationRoles::new(self.runtime.clone())
    }

    pub fn data_retention(&self) -> OrganizationDataRetention {
        OrganizationDataRetention::new(self.runtime.clone())
    }

    pub fn spend_alerts(&self) -> OrganizationSpendAlerts {
        OrganizationSpendAlerts::new(self.runtime.clone())
    }

    pub fn certificates(&self) -> OrganizationCertificates {
        OrganizationCertificates::new(self.runtime.clone())
    }

    pub fn projects(&self) -> OrganizationProjects {
        OrganizationProjects::new(self.runtime.clone())
    }
}

/// Organization audit log endpoint.
#[derive(Clone, Debug)]
pub struct AuditLogs {
    runtime: Arc<ClientRuntime>,
}

impl AuditLogs {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/audit_logs", params)
    }
}

/// Organization admin API key endpoints.
#[derive(Clone, Debug)]
pub struct AdminApiKeys {
    runtime: Arc<ClientRuntime>,
}

impl AdminApiKeys {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        post_body(&self.runtime, "/organization/admin_api_keys", params)
    }

    pub fn retrieve(&self, key_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let key_id = path_id("key_id", key_id)?;
        get(
            &self.runtime,
            format!("/organization/admin_api_keys/{key_id}"),
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/admin_api_keys", params)
    }

    pub fn delete(&self, key_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let key_id = path_id("key_id", key_id)?;
        delete(
            &self.runtime,
            format!("/organization/admin_api_keys/{key_id}"),
        )
    }
}

/// Organization usage endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationUsage {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationUsage {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn audio_speeches(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("audio_speeches", params)
    }

    pub fn audio_transcriptions(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("audio_transcriptions", params)
    }

    pub fn code_interpreter_sessions(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("code_interpreter_sessions", params)
    }

    pub fn completions(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("completions", params)
    }

    pub fn costs(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("costs", params)
    }

    pub fn embeddings(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("embeddings", params)
    }

    pub fn file_search_calls(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("file_search_calls", params)
    }

    pub fn images(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("images", params)
    }

    pub fn moderations(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("moderations", params)
    }

    pub fn vector_stores(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("vector_stores", params)
    }

    pub fn web_search_calls(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        self.usage("web_search_calls", params)
    }

    fn usage(
        &self,
        name: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, format!("/organization/usage/{name}"), params)
    }
}

/// Organization invite endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationInvites {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationInvites {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        post_body(&self.runtime, "/organization/invites", params)
    }

    pub fn retrieve(&self, invite_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let invite_id = path_id("invite_id", invite_id)?;
        get(&self.runtime, format!("/organization/invites/{invite_id}"))
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/invites", params)
    }

    pub fn delete(&self, invite_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let invite_id = path_id("invite_id", invite_id)?;
        delete(&self.runtime, format!("/organization/invites/{invite_id}"))
    }
}

/// Organization user endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationUsers {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationUsers {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn roles(&self) -> OrganizationUserRoles {
        OrganizationUserRoles::new(self.runtime.clone())
    }

    pub fn retrieve(&self, user_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        get(&self.runtime, format!("/organization/users/{user_id}"))
    }

    pub fn update<B: Serialize>(
        &self,
        user_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        post_body(
            &self.runtime,
            format!("/organization/users/{user_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/users", params)
    }

    pub fn delete(&self, user_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        delete(&self.runtime, format!("/organization/users/{user_id}"))
    }
}

/// Organization user role endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationUserRoles {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationUserRoles {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(
        &self,
        user_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        post_body(
            &self.runtime,
            format!("/organization/users/{user_id}/roles"),
            params,
        )
    }

    pub fn retrieve(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        let role_id = path_id("role_id", role_id)?;
        get(
            &self.runtime,
            format!("/organization/users/{user_id}/roles/{role_id}"),
        )
    }

    pub fn list(
        &self,
        user_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        get_query(
            &self.runtime,
            format!("/organization/users/{user_id}/roles"),
            params,
        )
    }

    pub fn delete(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        let role_id = path_id("role_id", role_id)?;
        delete(
            &self.runtime,
            format!("/organization/users/{user_id}/roles/{role_id}"),
        )
    }
}

/// Organization group endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationGroups {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationGroups {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn users(&self) -> OrganizationGroupUsers {
        OrganizationGroupUsers::new(self.runtime.clone())
    }

    pub fn roles(&self) -> OrganizationGroupRoles {
        OrganizationGroupRoles::new(self.runtime.clone())
    }

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        post_body(&self.runtime, "/organization/groups", params)
    }

    pub fn retrieve(&self, group_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        get(&self.runtime, format!("/organization/groups/{group_id}"))
    }

    pub fn update<B: Serialize>(
        &self,
        group_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        post_body(
            &self.runtime,
            format!("/organization/groups/{group_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/groups", params)
    }

    pub fn delete(&self, group_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        delete(&self.runtime, format!("/organization/groups/{group_id}"))
    }
}

/// Organization group user endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationGroupUsers {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationGroupUsers {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(
        &self,
        group_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        post_body(
            &self.runtime,
            format!("/organization/groups/{group_id}/users"),
            params,
        )
    }

    pub fn retrieve(
        &self,
        group_id: &str,
        user_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        let user_id = path_id("user_id", user_id)?;
        get(
            &self.runtime,
            format!("/organization/groups/{group_id}/users/{user_id}"),
        )
    }

    pub fn list(
        &self,
        group_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        get_query(
            &self.runtime,
            format!("/organization/groups/{group_id}/users"),
            params,
        )
    }

    pub fn delete(
        &self,
        group_id: &str,
        user_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        let user_id = path_id("user_id", user_id)?;
        delete(
            &self.runtime,
            format!("/organization/groups/{group_id}/users/{user_id}"),
        )
    }
}

/// Organization group role endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationGroupRoles {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationGroupRoles {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(
        &self,
        group_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        post_body(
            &self.runtime,
            format!("/organization/groups/{group_id}/roles"),
            params,
        )
    }

    pub fn retrieve(
        &self,
        group_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        let role_id = path_id("role_id", role_id)?;
        get(
            &self.runtime,
            format!("/organization/groups/{group_id}/roles/{role_id}"),
        )
    }

    pub fn list(
        &self,
        group_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        get_query(
            &self.runtime,
            format!("/organization/groups/{group_id}/roles"),
            params,
        )
    }

    pub fn delete(
        &self,
        group_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        let role_id = path_id("role_id", role_id)?;
        delete(
            &self.runtime,
            format!("/organization/groups/{group_id}/roles/{role_id}"),
        )
    }
}

/// Organization role endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationRoles {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationRoles {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        post_body(&self.runtime, "/organization/roles", params)
    }

    pub fn retrieve(&self, role_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let role_id = path_id("role_id", role_id)?;
        get(&self.runtime, format!("/organization/roles/{role_id}"))
    }

    pub fn update<B: Serialize>(
        &self,
        role_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let role_id = path_id("role_id", role_id)?;
        post_body(
            &self.runtime,
            format!("/organization/roles/{role_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/roles", params)
    }

    pub fn delete(&self, role_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let role_id = path_id("role_id", role_id)?;
        delete(&self.runtime, format!("/organization/roles/{role_id}"))
    }
}

/// Organization data-retention endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationDataRetention {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationDataRetention {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn retrieve(&self) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get(&self.runtime, "/organization/data_retention")
    }

    pub fn update<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        post_body(&self.runtime, "/organization/data_retention", params)
    }
}

/// Organization spend-alert endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationSpendAlerts {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationSpendAlerts {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        post_body(&self.runtime, "/organization/spend_alerts", params)
    }

    pub fn update<B: Serialize>(
        &self,
        alert_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let alert_id = path_id("alert_id", alert_id)?;
        post_body(
            &self.runtime,
            format!("/organization/spend_alerts/{alert_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/spend_alerts", params)
    }

    pub fn delete(&self, alert_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let alert_id = path_id("alert_id", alert_id)?;
        delete(
            &self.runtime,
            format!("/organization/spend_alerts/{alert_id}"),
        )
    }
}

/// Organization certificate endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationCertificates {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationCertificates {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        post_body(&self.runtime, "/organization/certificates", params)
    }

    pub fn retrieve(
        &self,
        certificate_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let certificate_id = path_id("certificate_id", certificate_id)?;
        get_query(
            &self.runtime,
            format!("/organization/certificates/{certificate_id}"),
            params,
        )
    }

    pub fn update<B: Serialize>(
        &self,
        certificate_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let certificate_id = path_id("certificate_id", certificate_id)?;
        post_body(
            &self.runtime,
            format!("/organization/certificates/{certificate_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/certificates", params)
    }

    pub fn delete(&self, certificate_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let certificate_id = path_id("certificate_id", certificate_id)?;
        delete(
            &self.runtime,
            format!("/organization/certificates/{certificate_id}"),
        )
    }

    pub fn activate<B: Serialize>(
        &self,
        certificate_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let certificate_id = path_id("certificate_id", certificate_id)?;
        post_body(
            &self.runtime,
            format!("/organization/certificates/{certificate_id}/activate"),
            params,
        )
    }

    pub fn deactivate<B: Serialize>(
        &self,
        certificate_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let certificate_id = path_id("certificate_id", certificate_id)?;
        post_body(
            &self.runtime,
            format!("/organization/certificates/{certificate_id}/deactivate"),
            params,
        )
    }
}

/// Organization project endpoints.
#[derive(Clone, Debug)]
pub struct OrganizationProjects {
    runtime: Arc<ClientRuntime>,
}

impl OrganizationProjects {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn users(&self) -> ProjectUsers {
        ProjectUsers::new(self.runtime.clone())
    }

    pub fn service_accounts(&self) -> ProjectServiceAccounts {
        ProjectServiceAccounts::new(self.runtime.clone())
    }

    pub fn api_keys(&self) -> ProjectApiKeys {
        ProjectApiKeys::new(self.runtime.clone())
    }

    pub fn rate_limits(&self) -> ProjectRateLimits {
        ProjectRateLimits::new(self.runtime.clone())
    }

    pub fn model_permissions(&self) -> ProjectModelPermissions {
        ProjectModelPermissions::new(self.runtime.clone())
    }

    pub fn hosted_tool_permissions(&self) -> ProjectHostedToolPermissions {
        ProjectHostedToolPermissions::new(self.runtime.clone())
    }

    pub fn groups(&self) -> ProjectGroups {
        ProjectGroups::new(self.runtime.clone())
    }

    pub fn roles(&self) -> ProjectRoles {
        ProjectRoles::new(self.runtime.clone())
    }

    pub fn data_retention(&self) -> ProjectDataRetention {
        ProjectDataRetention::new(self.runtime.clone())
    }

    pub fn spend_alerts(&self) -> ProjectSpendAlerts {
        ProjectSpendAlerts::new(self.runtime.clone())
    }

    pub fn certificates(&self) -> ProjectCertificates {
        ProjectCertificates::new(self.runtime.clone())
    }

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        post_body(&self.runtime, "/organization/projects", params)
    }

    pub fn retrieve(&self, project_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}"),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/projects", params)
    }

    pub fn archive(&self, project_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post(
            &self.runtime,
            format!("/organization/projects/{project_id}/archive"),
        )
    }
}

/// Project user endpoints.
#[derive(Clone, Debug)]
pub struct ProjectUsers {
    runtime: Arc<ClientRuntime>,
}

impl ProjectUsers {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn roles(&self) -> ProjectUserRoles {
        ProjectUserRoles::new(self.runtime.clone())
    }

    pub fn create<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/users"),
            params,
        )
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}/users/{user_id}"),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        user_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/users/{user_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/users"),
            params,
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        delete(
            &self.runtime,
            format!("/organization/projects/{project_id}/users/{user_id}"),
        )
    }
}

/// Project user role endpoints.
#[derive(Clone, Debug)]
pub struct ProjectUserRoles {
    runtime: Arc<ClientRuntime>,
}

impl ProjectUserRoles {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(
        &self,
        project_id: &str,
        user_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/users/{user_id}/roles"),
            params,
        )
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        user_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        let role_id = path_id("role_id", role_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}/users/{user_id}/roles/{role_id}"),
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        user_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/users/{user_id}/roles"),
            params,
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        user_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        let role_id = path_id("role_id", role_id)?;
        delete(
            &self.runtime,
            format!("/organization/projects/{project_id}/users/{user_id}/roles/{role_id}"),
        )
    }
}

/// Project service account endpoints.
#[derive(Clone, Debug)]
pub struct ProjectServiceAccounts {
    runtime: Arc<ClientRuntime>,
}

impl ProjectServiceAccounts {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/service_accounts"),
            params,
        )
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        service_account_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let service_account_id = path_id("service_account_id", service_account_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}/service_accounts/{service_account_id}"),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        service_account_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let service_account_id = path_id("service_account_id", service_account_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/service_accounts/{service_account_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/service_accounts"),
            params,
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        service_account_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let service_account_id = path_id("service_account_id", service_account_id)?;
        delete(
            &self.runtime,
            format!("/organization/projects/{project_id}/service_accounts/{service_account_id}"),
        )
    }
}

/// Project API key endpoints.
#[derive(Clone, Debug)]
pub struct ProjectApiKeys {
    runtime: Arc<ClientRuntime>,
}

impl ProjectApiKeys {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        api_key_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let api_key_id = path_id("api_key_id", api_key_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}/api_keys/{api_key_id}"),
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/api_keys"),
            params,
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        api_key_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let api_key_id = path_id("api_key_id", api_key_id)?;
        delete(
            &self.runtime,
            format!("/organization/projects/{project_id}/api_keys/{api_key_id}"),
        )
    }
}

/// Project rate-limit endpoints.
#[derive(Clone, Debug)]
pub struct ProjectRateLimits {
    runtime: Arc<ClientRuntime>,
}

impl ProjectRateLimits {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn list_rate_limits(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/rate_limits"),
            params,
        )
    }

    pub fn update_rate_limit<B: Serialize>(
        &self,
        project_id: &str,
        rate_limit_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let rate_limit_id = path_id("rate_limit_id", rate_limit_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/rate_limits/{rate_limit_id}"),
            params,
        )
    }
}

/// Project model-permission endpoints.
#[derive(Clone, Debug)]
pub struct ProjectModelPermissions {
    runtime: Arc<ClientRuntime>,
}

impl ProjectModelPermissions {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn retrieve(&self, project_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}/model_permissions"),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/model_permissions"),
            params,
        )
    }

    pub fn delete(&self, project_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        delete(
            &self.runtime,
            format!("/organization/projects/{project_id}/model_permissions"),
        )
    }
}

/// Project hosted-tool permission endpoints.
#[derive(Clone, Debug)]
pub struct ProjectHostedToolPermissions {
    runtime: Arc<ClientRuntime>,
}

impl ProjectHostedToolPermissions {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn retrieve(&self, project_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}/hosted_tool_permissions"),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/hosted_tool_permissions"),
            params,
        )
    }
}

/// Project group endpoints.
#[derive(Clone, Debug)]
pub struct ProjectGroups {
    runtime: Arc<ClientRuntime>,
}

impl ProjectGroups {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn roles(&self) -> ProjectGroupRoles {
        ProjectGroupRoles::new(self.runtime.clone())
    }

    pub fn create<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/groups"),
            params,
        )
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        group_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let group_id = path_id("group_id", group_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/groups/{group_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/groups"),
            params,
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        group_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let group_id = path_id("group_id", group_id)?;
        delete(
            &self.runtime,
            format!("/organization/projects/{project_id}/groups/{group_id}"),
        )
    }
}

/// Project group role endpoints.
#[derive(Clone, Debug)]
pub struct ProjectGroupRoles {
    runtime: Arc<ClientRuntime>,
}

impl ProjectGroupRoles {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(
        &self,
        project_id: &str,
        group_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let group_id = path_id("group_id", group_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/groups/{group_id}/roles"),
            params,
        )
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        group_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let group_id = path_id("group_id", group_id)?;
        let role_id = path_id("role_id", role_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}/groups/{group_id}/roles/{role_id}"),
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        group_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let group_id = path_id("group_id", group_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/groups/{group_id}/roles"),
            params,
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        group_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let group_id = path_id("group_id", group_id)?;
        let role_id = path_id("role_id", role_id)?;
        delete(
            &self.runtime,
            format!("/organization/projects/{project_id}/groups/{group_id}/roles/{role_id}"),
        )
    }
}

/// Project role endpoints.
#[derive(Clone, Debug)]
pub struct ProjectRoles {
    runtime: Arc<ClientRuntime>,
}

impl ProjectRoles {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/roles"),
            params,
        )
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let role_id = path_id("role_id", role_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}/roles/{role_id}"),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        role_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let role_id = path_id("role_id", role_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/roles/{role_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/roles"),
            params,
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let role_id = path_id("role_id", role_id)?;
        delete(
            &self.runtime,
            format!("/organization/projects/{project_id}/roles/{role_id}"),
        )
    }
}

/// Project data-retention endpoints.
#[derive(Clone, Debug)]
pub struct ProjectDataRetention {
    runtime: Arc<ClientRuntime>,
}

impl ProjectDataRetention {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn retrieve(&self, project_id: &str) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get(
            &self.runtime,
            format!("/organization/projects/{project_id}/data_retention"),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/data_retention"),
            params,
        )
    }
}

/// Project spend-alert endpoints.
#[derive(Clone, Debug)]
pub struct ProjectSpendAlerts {
    runtime: Arc<ClientRuntime>,
}

impl ProjectSpendAlerts {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn create<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/spend_alerts"),
            params,
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        alert_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let alert_id = path_id("alert_id", alert_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/spend_alerts/{alert_id}"),
            params,
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/spend_alerts"),
            params,
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        alert_id: &str,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let alert_id = path_id("alert_id", alert_id)?;
        delete(
            &self.runtime,
            format!("/organization/projects/{project_id}/spend_alerts/{alert_id}"),
        )
    }
}

/// Project certificate endpoints.
#[derive(Clone, Debug)]
pub struct ProjectCertificates {
    runtime: Arc<ClientRuntime>,
}

impl ProjectCertificates {
    fn new(runtime: Arc<ClientRuntime>) -> Self {
        Self { runtime }
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        get_query(
            &self.runtime,
            format!("/organization/projects/{project_id}/certificates"),
            params,
        )
    }

    pub fn activate<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/certificates/activate"),
            params,
        )
    }

    pub fn deactivate<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        post_body(
            &self.runtime,
            format!("/organization/projects/{project_id}/certificates/deactivate"),
            params,
        )
    }
}

fn path_id(name: &str, value: &str) -> Result<String, OpenAIError> {
    Ok(encode_path_id(validate_path_id(name, value)?))
}

fn path_with_query(base: impl Into<String>, params: impl Into<AdminQueryParams>) -> String {
    let base = base.into();
    let params = params.into();
    if params.is_empty() {
        return base;
    }

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params.pairs() {
        serializer.append_pair(key, value);
    }
    format!("{base}?{}", serializer.finish())
}

fn get(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
) -> Result<ApiResponse<AdminValue>, OpenAIError> {
    runtime.execute_json("GET", path, RequestOptions::default())
}

fn get_query(
    runtime: &ClientRuntime,
    base: impl Into<String>,
    params: impl Into<AdminQueryParams>,
) -> Result<ApiResponse<AdminValue>, OpenAIError> {
    get(runtime, path_with_query(base, params))
}

fn post(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
) -> Result<ApiResponse<AdminValue>, OpenAIError> {
    runtime.execute_json("POST", path, RequestOptions::default())
}

fn post_body<B: Serialize>(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
    body: B,
) -> Result<ApiResponse<AdminValue>, OpenAIError> {
    runtime.execute_json_with_body("POST", path, &body, RequestOptions::default())
}

fn delete(
    runtime: &ClientRuntime,
    path: impl AsRef<str>,
) -> Result<ApiResponse<AdminValue>, OpenAIError> {
    runtime.execute_json("DELETE", path, RequestOptions::default())
}
