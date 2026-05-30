use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OpenAIError,
    core::{request::RequestOptions, response::ApiResponse, runtime::ClientRuntime},
    resources::{
        common::ListOrder,
        files::{encode_path_id, validate_path_id},
    },
};

/// JSON value returned by the flexible admin endpoint surface.
pub type AdminValue = Value;

macro_rules! admin_string_literal_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident => $literal:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant,)+
            Unknown(String),
        }

        impl $name {
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $literal,)+
                    Self::Unknown(value) => value.as_str(),
                }
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::Unknown(String::new())
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                match value {
                    $($literal => Self::$variant,)+
                    _ => Self::Unknown(value.to_string()),
                }
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                match value.as_str() {
                    $($literal => Self::$variant,)+
                    _ => Self::Unknown(value),
                }
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }

        impl PartialEq<$name> for &str {
            fn eq(&self, other: &$name) -> bool {
                *self == other.as_str()
            }
        }

        impl PartialEq<String> for $name {
            fn eq(&self, other: &String) -> bool {
                self.as_str() == other.as_str()
            }
        }

        impl PartialEq<$name> for String {
            fn eq(&self, other: &$name) -> bool {
                self.as_str() == other.as_str()
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Ok(Self::from(value))
            }
        }
    };
}

admin_string_literal_enum! {
    /// Organization admin API key object type.
    pub enum AdminApiKeyObject {
        OrganizationAdminApiKey => "organization.admin_api_key",
    }
}

admin_string_literal_enum! {
    /// Organization admin API key deletion object type.
    pub enum AdminApiKeyDeletedObject {
        OrganizationAdminApiKeyDeleted => "organization.admin_api_key.deleted",
    }
}

admin_string_literal_enum! {
    /// Project API key object type.
    pub enum ProjectApiKeyObject {
        OrganizationProjectApiKey => "organization.project.api_key",
    }
}

admin_string_literal_enum! {
    /// Project API key deletion object type.
    pub enum ProjectApiKeyDeletedObject {
        OrganizationProjectApiKeyDeleted => "organization.project.api_key.deleted",
    }
}

admin_string_literal_enum! {
    /// Owner type for a project API key.
    pub enum ProjectApiKeyOwnerType {
        User => "user",
        ServiceAccount => "service_account",
    }
}

admin_string_literal_enum! {
    /// Organization data retention object type.
    pub enum AdminOrganizationDataRetentionObject {
        OrganizationDataRetention => "organization.data_retention",
    }
}

admin_string_literal_enum! {
    /// Project data retention object type.
    pub enum AdminProjectDataRetentionObject {
        ProjectDataRetention => "project.data_retention",
    }
}

admin_string_literal_enum! {
    /// Organization invite role.
    pub enum AdminInviteRole {
        Reader => "reader",
        Owner => "owner",
    }
}

admin_string_literal_enum! {
    /// Organization invite object type.
    pub enum AdminInviteObject {
        OrganizationInvite => "organization.invite",
    }
}

admin_string_literal_enum! {
    /// Organization invite deletion object type.
    pub enum AdminInviteDeletedObject {
        OrganizationInviteDeleted => "organization.invite.deleted",
    }
}

admin_string_literal_enum! {
    /// Organization invite status.
    pub enum AdminInviteStatus {
        Accepted => "accepted",
        Expired => "expired",
        Pending => "pending",
    }
}

admin_string_literal_enum! {
    /// Organization user object type.
    pub enum AdminOrganizationUserObject {
        OrganizationUser => "organization.user",
    }
}

admin_string_literal_enum! {
    /// Nested user object type.
    pub enum AdminUserObject {
        User => "user",
    }
}

admin_string_literal_enum! {
    /// Organization user deletion object type.
    pub enum AdminOrganizationUserDeletedObject {
        OrganizationUserDeleted => "organization.user.deleted",
    }
}

admin_string_literal_enum! {
    /// Project membership role used by invite project grants and service accounts.
    pub enum AdminProjectMembershipRole {
        Member => "member",
        Owner => "owner",
    }
}

admin_string_literal_enum! {
    /// Organization-level data retention mode.
    pub enum AdminOrganizationDataRetentionType {
        ZeroDataRetention => "zero_data_retention",
        ModifiedAbuseMonitoring => "modified_abuse_monitoring",
        EnhancedZeroDataRetention => "enhanced_zero_data_retention",
        EnhancedModifiedAbuseMonitoring => "enhanced_modified_abuse_monitoring",
    }
}

admin_string_literal_enum! {
    /// Project-level data retention mode.
    pub enum AdminProjectDataRetentionType {
        OrganizationDefault => "organization_default",
        None => "none",
        ZeroDataRetention => "zero_data_retention",
        ModifiedAbuseMonitoring => "modified_abuse_monitoring",
        EnhancedZeroDataRetention => "enhanced_zero_data_retention",
        EnhancedModifiedAbuseMonitoring => "enhanced_modified_abuse_monitoring",
    }
}

admin_string_literal_enum! {
    /// Currency supported by spend-alert thresholds.
    pub enum AdminSpendAlertCurrency {
        Usd => "USD",
    }
}

admin_string_literal_enum! {
    /// Spend-alert evaluation interval.
    pub enum AdminSpendAlertInterval {
        Month => "month",
    }
}

admin_string_literal_enum! {
    /// Spend-alert notification channel type.
    pub enum AdminSpendAlertNotificationType {
        Email => "email",
    }
}

admin_string_literal_enum! {
    /// Organization spend-alert object type.
    pub enum AdminOrganizationSpendAlertObject {
        OrganizationSpendAlert => "organization.spend_alert",
    }
}

admin_string_literal_enum! {
    /// Organization spend-alert deletion object type.
    pub enum AdminOrganizationSpendAlertDeletedObject {
        OrganizationSpendAlertDeleted => "organization.spend_alert.deleted",
    }
}

admin_string_literal_enum! {
    /// Project spend-alert object type.
    pub enum AdminProjectSpendAlertObject {
        ProjectSpendAlert => "project.spend_alert",
    }
}

admin_string_literal_enum! {
    /// Project spend-alert deletion object type.
    pub enum AdminProjectSpendAlertDeletedObject {
        ProjectSpendAlertDeleted => "project.spend_alert.deleted",
    }
}

admin_string_literal_enum! {
    /// Organization project object type.
    pub enum AdminProjectObject {
        OrganizationProject => "organization.project",
    }
}

admin_string_literal_enum! {
    /// Project user object type.
    pub enum AdminProjectUserObject {
        OrganizationProjectUser => "organization.project.user",
    }
}

admin_string_literal_enum! {
    /// Project user deletion object type.
    pub enum AdminProjectUserDeletedObject {
        OrganizationProjectUserDeleted => "organization.project.user.deleted",
    }
}

admin_string_literal_enum! {
    /// Project service-account object type.
    pub enum AdminProjectServiceAccountObject {
        OrganizationProjectServiceAccount => "organization.project.service_account",
    }
}

admin_string_literal_enum! {
    /// Project service-account API key object type.
    pub enum AdminProjectServiceAccountApiKeyObject {
        OrganizationProjectServiceAccountApiKey => "organization.project.service_account.api_key",
    }
}

admin_string_literal_enum! {
    /// Project service-account deletion object type.
    pub enum AdminProjectServiceAccountDeletedObject {
        OrganizationProjectServiceAccountDeleted => "organization.project.service_account.deleted",
    }
}

admin_string_literal_enum! {
    /// Project model-permission mode.
    pub enum AdminProjectModelPermissionMode {
        AllowList => "allow_list",
        DenyList => "deny_list",
    }
}

admin_string_literal_enum! {
    /// Project model permissions object type.
    pub enum AdminProjectModelPermissionsObject {
        ProjectModelPermissions => "project.model_permissions",
    }
}

admin_string_literal_enum! {
    /// Project model permissions deletion object type.
    pub enum AdminProjectModelPermissionsDeletedObject {
        ProjectModelPermissionsDeleted => "project.model_permissions.deleted",
    }
}

admin_string_literal_enum! {
    /// Organization group type.
    pub enum AdminGroupType {
        Group => "group",
        TenantGroup => "tenant_group",
    }
}

admin_string_literal_enum! {
    /// Organization group deletion object type.
    pub enum AdminGroupDeletedObject {
        GroupDeleted => "group.deleted",
    }
}

admin_string_literal_enum! {
    /// Organization group object type.
    pub enum AdminGroupObject {
        Group => "group",
    }
}

admin_string_literal_enum! {
    /// Organization group-user membership object type.
    pub enum AdminGroupUserObject {
        GroupUser => "group.user",
    }
}

admin_string_literal_enum! {
    /// Organization group-user membership deletion object type.
    pub enum AdminGroupUserDeletedObject {
        GroupUserDeleted => "group.user.deleted",
    }
}

admin_string_literal_enum! {
    /// User type returned from organization group membership lookups.
    pub enum AdminGroupUserType {
        User => "user",
        TenantUser => "tenant_user",
    }
}

admin_string_literal_enum! {
    /// Organization role object type.
    pub enum AdminRoleObject {
        Role => "role",
    }
}

admin_string_literal_enum! {
    /// Organization role deletion object type.
    pub enum AdminRoleDeletedObject {
        RoleDeleted => "role.deleted",
    }
}

admin_string_literal_enum! {
    /// Organization group-role assignment object type.
    pub enum AdminGroupRoleObject {
        GroupRole => "group.role",
    }
}

admin_string_literal_enum! {
    /// Organization user-role assignment object type.
    pub enum AdminUserRoleObject {
        UserRole => "user.role",
    }
}

admin_string_literal_enum! {
    /// Include fields accepted by organization certificate retrieve.
    pub enum AdminCertificateInclude {
        Content => "content",
    }
}

admin_string_literal_enum! {
    /// Certificate object type.
    pub enum AdminCertificateObject {
        Certificate => "certificate",
        OrganizationCertificate => "organization.certificate",
        OrganizationProjectCertificate => "organization.project.certificate",
    }
}

admin_string_literal_enum! {
    /// Certificate deletion object type.
    pub enum AdminCertificateDeletedObject {
        CertificateDeleted => "certificate.deleted",
    }
}

admin_string_literal_enum! {
    /// Audit-log event type filter.
    pub enum AdminAuditLogEventType {
        ApiKeyCreated => "api_key.created",
        ApiKeyUpdated => "api_key.updated",
        ApiKeyDeleted => "api_key.deleted",
        CertificateCreated => "certificate.created",
        CertificateUpdated => "certificate.updated",
        CertificateDeleted => "certificate.deleted",
        CertificatesActivated => "certificates.activated",
        CertificatesDeactivated => "certificates.deactivated",
        CheckpointPermissionCreated => "checkpoint.permission.created",
        CheckpointPermissionDeleted => "checkpoint.permission.deleted",
        ExternalKeyRegistered => "external_key.registered",
        ExternalKeyRemoved => "external_key.removed",
        GroupCreated => "group.created",
        GroupUpdated => "group.updated",
        GroupDeleted => "group.deleted",
        InviteSent => "invite.sent",
        InviteAccepted => "invite.accepted",
        InviteDeleted => "invite.deleted",
        IpAllowlistCreated => "ip_allowlist.created",
        IpAllowlistUpdated => "ip_allowlist.updated",
        IpAllowlistDeleted => "ip_allowlist.deleted",
        IpAllowlistConfigActivated => "ip_allowlist.config.activated",
        IpAllowlistConfigDeactivated => "ip_allowlist.config.deactivated",
        LoginSucceeded => "login.succeeded",
        LoginFailed => "login.failed",
        LogoutSucceeded => "logout.succeeded",
        LogoutFailed => "logout.failed",
        OrganizationUpdated => "organization.updated",
        ProjectCreated => "project.created",
        ProjectUpdated => "project.updated",
        ProjectArchived => "project.archived",
        ProjectDeleted => "project.deleted",
        RateLimitUpdated => "rate_limit.updated",
        RateLimitDeleted => "rate_limit.deleted",
        ResourceDeleted => "resource.deleted",
        TunnelCreated => "tunnel.created",
        TunnelUpdated => "tunnel.updated",
        TunnelDeleted => "tunnel.deleted",
        RoleCreated => "role.created",
        RoleUpdated => "role.updated",
        RoleDeleted => "role.deleted",
        RoleAssignmentCreated => "role.assignment.created",
        RoleAssignmentDeleted => "role.assignment.deleted",
        ScimEnabled => "scim.enabled",
        ScimDisabled => "scim.disabled",
        ServiceAccountCreated => "service_account.created",
        ServiceAccountUpdated => "service_account.updated",
        ServiceAccountDeleted => "service_account.deleted",
        UserAdded => "user.added",
        UserUpdated => "user.updated",
        UserDeleted => "user.deleted",
    }
}

admin_string_literal_enum! {
    /// Bucket width accepted by most organization usage endpoints.
    pub enum AdminUsageBucketWidth {
        OneMinute => "1m",
        OneHour => "1h",
        OneDay => "1d",
    }
}

admin_string_literal_enum! {
    /// Bucket width accepted by the costs usage endpoint.
    pub enum AdminUsageCostsBucketWidth {
        OneDay => "1d",
    }
}

admin_string_literal_enum! {
    /// Group-by dimensions for audio, embeddings, and moderations usage.
    pub enum AdminUsageStandardGroupBy {
        ProjectId => "project_id",
        UserId => "user_id",
        ApiKeyId => "api_key_id",
        Model => "model",
    }
}

admin_string_literal_enum! {
    /// Group-by dimensions for code-interpreter session and vector-store usage.
    pub enum AdminUsageProjectGroupBy {
        ProjectId => "project_id",
    }
}

admin_string_literal_enum! {
    /// Group-by dimensions for completions usage.
    pub enum AdminUsageCompletionsGroupBy {
        ProjectId => "project_id",
        UserId => "user_id",
        ApiKeyId => "api_key_id",
        Model => "model",
        Batch => "batch",
        ServiceTier => "service_tier",
    }
}

admin_string_literal_enum! {
    /// Group-by dimensions for costs usage.
    pub enum AdminUsageCostsGroupBy {
        ProjectId => "project_id",
        LineItem => "line_item",
        ApiKeyId => "api_key_id",
    }
}

admin_string_literal_enum! {
    /// Group-by dimensions for file-search-call usage.
    pub enum AdminUsageFileSearchCallsGroupBy {
        ProjectId => "project_id",
        UserId => "user_id",
        ApiKeyId => "api_key_id",
        VectorStoreId => "vector_store_id",
    }
}

admin_string_literal_enum! {
    /// Group-by dimensions for images usage.
    pub enum AdminUsageImagesGroupBy {
        ProjectId => "project_id",
        UserId => "user_id",
        ApiKeyId => "api_key_id",
        Model => "model",
        Size => "size",
        Source => "source",
    }
}

admin_string_literal_enum! {
    /// Image size filters for images usage.
    pub enum AdminUsageImagesSize {
        Size256x256 => "256x256",
        Size512x512 => "512x512",
        Size1024x1024 => "1024x1024",
        Size1792x1792 => "1792x1792",
        Size1024x1792 => "1024x1792",
    }
}

admin_string_literal_enum! {
    /// Image source filters for images usage.
    pub enum AdminUsageImagesSource {
        Generation => "image.generation",
        Edit => "image.edit",
        Variation => "image.variation",
    }
}

admin_string_literal_enum! {
    /// Group-by dimensions for web-search-call usage.
    pub enum AdminUsageWebSearchCallsGroupBy {
        ProjectId => "project_id",
        UserId => "user_id",
        ApiKeyId => "api_key_id",
        Model => "model",
        ContextLevel => "context_level",
    }
}

admin_string_literal_enum! {
    /// Context-level filters for web-search-call usage.
    pub enum AdminUsageWebSearchContextLevel {
        Low => "low",
        Medium => "medium",
        High => "high",
    }
}

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

    pub fn push_opt<T: ToString>(mut self, key: impl Into<String>, value: Option<T>) -> Self {
        if let Some(value) = value {
            self.append(key, value);
        }
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

    pub fn push_repeated_opt<I, V>(self, key: impl Into<String>, values: Option<I>) -> Self
    where
        I: IntoIterator<Item = V>,
        V: ToString,
    {
        match values {
            Some(values) => self.push_repeated(key, values),
            None => self,
        }
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

/// Organization audit-log effective-time filter.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminAuditLogEffectiveAtParams {
    pub gt: Option<i64>,
    pub gte: Option<i64>,
    pub lt: Option<i64>,
    pub lte: Option<i64>,
}

/// Organization audit-log list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminAuditLogListParams {
    pub actor_emails: Option<Vec<String>>,
    pub actor_ids: Option<Vec<String>>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub effective_at: Option<AdminAuditLogEffectiveAtParams>,
    pub event_types: Option<Vec<AdminAuditLogEventType>>,
    pub limit: Option<u32>,
    pub project_ids: Option<Vec<String>>,
    pub resource_ids: Option<Vec<String>>,
}

impl From<AdminAuditLogListParams> for AdminQueryParams {
    fn from(value: AdminAuditLogListParams) -> Self {
        let mut params = AdminQueryParams::new()
            .push_repeated_opt("actor_emails", value.actor_emails)
            .push_repeated_opt("actor_ids", value.actor_ids)
            .push_opt("after", value.after)
            .push_opt("before", value.before)
            .push_repeated_opt("event_types", value.event_types)
            .push_opt("limit", value.limit)
            .push_repeated_opt("project_ids", value.project_ids)
            .push_repeated_opt("resource_ids", value.resource_ids);
        if let Some(effective_at) = value.effective_at {
            params = params
                .push_opt("effective_at[gt]", effective_at.gt)
                .push_opt("effective_at[gte]", effective_at.gte)
                .push_opt("effective_at[lt]", effective_at.lt)
                .push_opt("effective_at[lte]", effective_at.lte);
        }
        params
    }
}

/// Organization admin API key creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminApiKeyCreateParams {
    pub name: String,
}

/// Organization admin API key list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminApiKeyListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminApiKeyListParams> for AdminQueryParams {
    fn from(value: AdminApiKeyListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Owner information attached to an organization admin API key.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AdminApiKeyOwner {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub created_at: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default, rename = "type")]
    pub owner_type: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Represents an individual organization admin API key.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminApiKey {
    pub id: String,
    pub created_at: u64,
    pub object: AdminApiKeyObject,
    pub owner: AdminApiKeyOwner,
    pub redacted_value: String,
    #[serde(default)]
    pub last_used_at: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Organization admin API key creation response.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminApiKeyCreateResponse {
    pub id: String,
    pub created_at: u64,
    pub object: AdminApiKeyObject,
    pub owner: AdminApiKeyOwner,
    pub redacted_value: String,
    pub value: String,
    #[serde(default)]
    pub last_used_at: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Organization admin API key deletion response.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminApiKeyDeleteResponse {
    pub id: String,
    pub deleted: bool,
    pub object: AdminApiKeyDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Cursor page returned by admin list endpoints.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AdminCursorPage<T> {
    #[serde(default)]
    pub object: Option<String>,
    pub data: Vec<T>,
    #[serde(default)]
    pub has_more: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl AdminCursorPage<AdminApiKey> {
    pub fn has_next_page(&self) -> bool {
        self.has_more != Some(false) && !self.data.is_empty()
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_next_page() {
            self.data.last().map(|key| key.id.as_str())
        } else {
            None
        }
    }
}

pub type AdminApiKeyListResponse = AdminCursorPage<AdminApiKey>;

/// Conversation-style cursor page returned by admin list endpoints.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AdminConversationCursorPage<T> {
    #[serde(default)]
    pub object: Option<String>,
    pub data: Vec<T>,
    #[serde(default)]
    pub has_more: Option<bool>,
    #[serde(default)]
    pub last_id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl<T> AdminConversationCursorPage<T> {
    pub fn has_next_page(&self) -> bool {
        self.has_more != Some(false) && self.last_id.is_some()
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_next_page() {
            self.last_id.as_deref()
        } else {
            None
        }
    }
}

/// Next-token cursor page returned by admin list endpoints.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AdminNextCursorPage<T> {
    #[serde(default)]
    pub object: Option<String>,
    pub data: Vec<T>,
    #[serde(default)]
    pub has_more: Option<bool>,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl<T> AdminNextCursorPage<T> {
    pub fn has_next_page(&self) -> bool {
        self.has_more != Some(false) && self.next.is_some()
    }

    pub fn next_after(&self) -> Option<&str> {
        if self.has_next_page() {
            self.next.as_deref()
        } else {
            None
        }
    }
}

/// Non-paginated list page returned by admin bulk action endpoints.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AdminPage<T> {
    pub data: Vec<T>,
    pub object: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Organization invite creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminInviteCreateParams {
    pub email: String,
    pub role: AdminInviteRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projects: Option<Vec<AdminInviteProject>>,
}

/// Project membership granted when an organization invite is accepted.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminInviteProject {
    pub id: String,
    pub role: AdminProjectMembershipRole,
}

/// Organization invite list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminInviteListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
}

impl From<AdminInviteListParams> for AdminQueryParams {
    fn from(value: AdminInviteListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
    }
}

/// Project membership granted by an organization invite response.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminInviteProjectGrant {
    pub id: String,
    pub role: AdminProjectMembershipRole,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Represents an individual invite to the organization.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminInvite {
    pub id: String,
    pub created_at: u64,
    pub email: String,
    pub object: AdminInviteObject,
    pub projects: Vec<AdminInviteProjectGrant>,
    pub role: AdminInviteRole,
    pub status: AdminInviteStatus,
    #[serde(default)]
    pub accepted_at: Option<u64>,
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Organization invite deletion response.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminInviteDeleteResponse {
    pub id: String,
    pub deleted: bool,
    pub object: AdminInviteDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminInviteListResponse = AdminConversationCursorPage<AdminInvite>;

/// Organization user list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUserListParams {
    pub after: Option<String>,
    pub emails: Option<Vec<String>>,
    pub limit: Option<u32>,
}

impl From<AdminUserListParams> for AdminQueryParams {
    fn from(value: AdminUserListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_repeated_opt("emails", value.emails)
            .push_opt("limit", value.limit)
    }
}

/// Organization user update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminUserUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_persona: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_level: Option<String>,
}

/// Project summary associated with an organization user.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AdminOrganizationUserProject {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Projects associated with an organization user, if included.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AdminOrganizationUserProjects {
    #[serde(default)]
    pub data: Vec<AdminOrganizationUserProject>,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Nested user details inside an organization user response.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminOrganizationUserDetails {
    pub id: String,
    pub object: AdminUserObject,
    #[serde(default)]
    pub banned: Option<bool>,
    #[serde(default)]
    pub banned_at: Option<u64>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Represents an individual user within an organization.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminOrganizationUser {
    pub id: String,
    pub added_at: u64,
    pub object: AdminOrganizationUserObject,
    #[serde(default)]
    pub api_key_last_used_at: Option<u64>,
    #[serde(default)]
    pub created: Option<u64>,
    #[serde(default)]
    pub developer_persona: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub is_default: Option<bool>,
    #[serde(default)]
    pub is_scale_tier_authorized_purchaser: Option<bool>,
    #[serde(default)]
    pub is_scim_managed: Option<bool>,
    #[serde(default)]
    pub is_service_account: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub projects: Option<AdminOrganizationUserProjects>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub technical_level: Option<String>,
    #[serde(default)]
    pub user: Option<AdminOrganizationUserDetails>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Organization user deletion response.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminOrganizationUserDeleteResponse {
    pub id: String,
    pub deleted: bool,
    pub object: AdminOrganizationUserDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminOrganizationUserListResponse = AdminConversationCursorPage<AdminOrganizationUser>;

/// Organization role creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminRoleCreateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub permissions: Vec<String>,
    pub role_name: String,
}

/// Organization role update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminRoleUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub permissions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
}

/// Organization role list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminRoleListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminRoleListParams> for AdminQueryParams {
    fn from(value: AdminRoleListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Details about a role that can be assigned through the public Roles API.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminRole {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    pub name: String,
    pub object: AdminRoleObject,
    pub permissions: Vec<String>,
    pub predefined_role: bool,
    pub resource_type: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after deleting a role.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminRoleDeleteResponse {
    pub id: String,
    pub deleted: bool,
    pub object: AdminRoleDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminRoleListResponse = AdminNextCursorPage<AdminRole>;

/// Organization user-role creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminUserRoleCreateParams {
    pub role_id: String,
}

/// Role assignment linking a user to a role.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminUserRoleCreateResponse {
    pub object: AdminUserRoleObject,
    pub role: AdminRole,
    pub user: AdminOrganizationUser,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Organization user-role list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUserRoleListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminUserRoleListParams> for AdminQueryParams {
    fn from(value: AdminUserRoleListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

pub type AdminUserRoleRetrieveResponse = AdminRoleAssignment;
pub type AdminUserRoleListResponse = AdminNextCursorPage<AdminRoleAssignment>;

/// Organization group creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminGroupCreateParams {
    pub name: String,
}

/// Organization group update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminGroupUpdateParams {
    pub name: String,
}

/// Organization group list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminGroupListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminGroupListParams> for AdminQueryParams {
    fn from(value: AdminGroupListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Details about an organization group.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminGroup {
    pub id: String,
    pub created_at: u64,
    pub group_type: AdminGroupType,
    pub is_scim_managed: bool,
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Response returned after updating an organization group.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminGroupUpdateResponse {
    pub id: String,
    pub created_at: u64,
    pub is_scim_managed: bool,
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after deleting an organization group.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminGroupDeleteResponse {
    pub id: String,
    pub deleted: bool,
    pub object: AdminGroupDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminGroupListResponse = AdminNextCursorPage<AdminGroup>;

/// Organization group-user creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminGroupUserCreateParams {
    pub user_id: String,
}

/// Organization group-user list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminGroupUserListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminGroupUserListParams> for AdminQueryParams {
    fn from(value: AdminGroupUserListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Confirmation payload returned after adding a user to a group.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminGroupUserCreateResponse {
    pub group_id: String,
    pub object: AdminGroupUserObject,
    pub user_id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Represents an individual user returned when inspecting group membership.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminGroupUser {
    pub id: String,
    #[serde(default)]
    pub email: Option<String>,
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Details about a user returned from an organization group membership lookup.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminGroupUserRetrieveResponse {
    pub id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub is_service_account: Option<bool>,
    pub name: String,
    #[serde(default)]
    pub picture: Option<String>,
    pub user_type: AdminGroupUserType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after removing a user from a group.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminGroupUserDeleteResponse {
    pub deleted: bool,
    pub object: AdminGroupUserDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminGroupUserListResponse = AdminNextCursorPage<AdminGroupUser>;

/// Organization group-role creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminGroupRoleCreateParams {
    pub role_id: String,
}

/// Organization group-role list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminGroupRoleListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminGroupRoleListParams> for AdminQueryParams {
    fn from(value: AdminGroupRoleListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Summary information about a group returned in role assignment responses.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminGroupRoleGroup {
    pub id: String,
    pub created_at: u64,
    pub name: String,
    pub object: AdminGroupObject,
    pub scim_managed: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Role assignment linking a group to a role.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminGroupRoleCreateResponse {
    pub group: AdminGroupRoleGroup,
    pub object: AdminGroupRoleObject,
    pub role: AdminRole,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Principal from which a role assignment is inherited.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminRoleAssignmentSource {
    pub principal_id: String,
    pub principal_type: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Detailed information about a role assignment entry returned when listing assignments.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminRoleAssignment {
    pub id: String,
    #[serde(default)]
    pub assignment_sources: Option<Vec<AdminRoleAssignmentSource>>,
    #[serde(default)]
    pub created_at: Option<u64>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub created_by_user_obj: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, Value>>,
    pub name: String,
    pub permissions: Vec<String>,
    pub predefined_role: bool,
    pub resource_type: String,
    #[serde(default)]
    pub updated_at: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after unassigning a role.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminRoleAssignmentDeleteResponse {
    pub deleted: bool,
    pub object: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminGroupRoleRetrieveResponse = AdminRoleAssignment;
pub type AdminGroupRoleListResponse = AdminNextCursorPage<AdminRoleAssignment>;

/// Organization certificate creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminCertificateCreateParams {
    pub certificate: String,
    pub name: String,
}

/// Organization certificate update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminCertificateUpdateParams {
    pub name: String,
}

/// Organization certificate activation/deactivation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminCertificateIdsParams {
    pub certificate_ids: Vec<String>,
}

/// Organization certificate retrieve query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminCertificateRetrieveParams {
    pub include: Option<Vec<AdminCertificateInclude>>,
}

impl From<AdminCertificateRetrieveParams> for AdminQueryParams {
    fn from(value: AdminCertificateRetrieveParams) -> Self {
        AdminQueryParams::new().push_repeated_opt("include", value.include)
    }
}

/// Organization certificate list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminCertificateListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminCertificateListParams> for AdminQueryParams {
    fn from(value: AdminCertificateListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Certificate validity and optional PEM details.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AdminCertificateDetails {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub valid_at: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Represents an individual certificate uploaded to the organization.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminCertificate {
    pub id: String,
    #[serde(default)]
    pub active: Option<bool>,
    pub certificate_details: AdminCertificateDetails,
    pub created_at: u64,
    #[serde(default)]
    pub name: Option<String>,
    pub object: AdminCertificateObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after deleting a certificate.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminCertificateDeleteResponse {
    pub id: String,
    pub object: AdminCertificateDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminCertificateListResponse = AdminConversationCursorPage<AdminCertificate>;
pub type AdminCertificateActionResponse = AdminPage<AdminCertificate>;

/// Organization data-retention update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminDataRetentionUpdateParams {
    pub retention_type: AdminOrganizationDataRetentionType,
}

/// Organization data retention control setting.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminOrganizationDataRetention {
    pub object: AdminOrganizationDataRetentionObject,
    #[serde(rename = "type")]
    pub retention_type: AdminOrganizationDataRetentionType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Organization spend-alert creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminSpendAlertCreateParams {
    pub currency: AdminSpendAlertCurrency,
    pub interval: AdminSpendAlertInterval,
    pub notification_channel: AdminSpendAlertNotificationChannel,
    pub threshold_amount: i64,
}

/// Organization spend-alert update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminSpendAlertUpdateParams {
    pub currency: AdminSpendAlertCurrency,
    pub interval: AdminSpendAlertInterval,
    pub notification_channel: AdminSpendAlertNotificationChannel,
    pub threshold_amount: i64,
}

/// Email notification settings for an organization or project spend alert.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdminSpendAlertNotificationChannel {
    pub recipients: Vec<String>,
    #[serde(rename = "type")]
    pub kind: AdminSpendAlertNotificationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_prefix: Option<String>,
}

/// Organization spend-alert list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminSpendAlertListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminSpendAlertListParams> for AdminQueryParams {
    fn from(value: AdminSpendAlertListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("before", value.before)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Represents a spend alert configured at the organization level.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminOrganizationSpendAlert {
    pub id: String,
    pub currency: AdminSpendAlertCurrency,
    pub interval: AdminSpendAlertInterval,
    pub notification_channel: AdminSpendAlertNotificationChannel,
    pub object: AdminOrganizationSpendAlertObject,
    pub threshold_amount: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after deleting an organization spend alert.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminOrganizationSpendAlertDeleted {
    pub id: String,
    pub deleted: bool,
    pub object: AdminOrganizationSpendAlertDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminOrganizationSpendAlertListResponse =
    AdminConversationCursorPage<AdminOrganizationSpendAlert>;

/// Organization project creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectCreateParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geography: Option<String>,
}

/// Organization project update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geography: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Organization project list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectListParams {
    pub after: Option<String>,
    pub include_archived: Option<bool>,
    pub limit: Option<u32>,
}

impl From<AdminProjectListParams> for AdminQueryParams {
    fn from(value: AdminProjectListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("include_archived", value.include_archived)
            .push_opt("limit", value.limit)
    }
}

/// Represents an individual project.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProject {
    pub id: String,
    pub created_at: u64,
    pub object: AdminProjectObject,
    #[serde(default)]
    pub archived_at: Option<u64>,
    #[serde(default)]
    pub external_key_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminProjectListResponse = AdminConversationCursorPage<AdminProject>;

/// Represents an individual user in a project.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectUser {
    pub id: String,
    pub added_at: u64,
    pub object: AdminProjectUserObject,
    pub role: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after deleting a project user.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectUserDeleteResponse {
    pub id: String,
    pub deleted: bool,
    pub object: AdminProjectUserDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminProjectUserListResponse = AdminConversationCursorPage<AdminProjectUser>;

/// Project user creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectUserCreateParams {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Project user update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectUserUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Project user list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectUserListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
}

impl From<AdminProjectUserListParams> for AdminQueryParams {
    fn from(value: AdminProjectUserListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
    }
}

/// Project user-role creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectUserRoleCreateParams {
    pub role_id: String,
}

/// Project user-role list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectUserRoleListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminProjectUserRoleListParams> for AdminQueryParams {
    fn from(value: AdminProjectUserRoleListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

pub type AdminProjectUserRoleCreateResponse = AdminUserRoleCreateResponse;
pub type AdminProjectUserRoleRetrieveResponse = AdminRoleAssignment;
pub type AdminProjectUserRoleListResponse = AdminNextCursorPage<AdminRoleAssignment>;
pub type AdminProjectUserRoleDeleteResponse = AdminRoleAssignmentDeleteResponse;

/// API key returned when creating a project service account.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectServiceAccountApiKey {
    pub id: String,
    pub created_at: u64,
    pub name: String,
    pub object: AdminProjectServiceAccountApiKeyObject,
    pub value: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Represents an individual service account in a project.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectServiceAccount {
    pub id: String,
    pub created_at: u64,
    pub name: String,
    pub object: AdminProjectServiceAccountObject,
    pub role: AdminProjectMembershipRole,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Response returned when creating a project service account.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectServiceAccountCreateResponse {
    pub id: String,
    #[serde(default)]
    pub api_key: Option<AdminProjectServiceAccountApiKey>,
    pub created_at: u64,
    pub name: String,
    pub object: AdminProjectServiceAccountObject,
    pub role: AdminProjectMembershipRole,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after deleting a project service account.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectServiceAccountDeleteResponse {
    pub id: String,
    pub deleted: bool,
    pub object: AdminProjectServiceAccountDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminProjectServiceAccountListResponse =
    AdminConversationCursorPage<AdminProjectServiceAccount>;

/// Project service-account creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectServiceAccountCreateParams {
    pub name: String,
}

/// Project service-account update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectServiceAccountUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<AdminProjectMembershipRole>,
}

/// Project service-account list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectServiceAccountListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
}

impl From<AdminProjectServiceAccountListParams> for AdminQueryParams {
    fn from(value: AdminProjectServiceAccountListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
    }
}

/// Project API key list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectApiKeyListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
}

impl From<AdminProjectApiKeyListParams> for AdminQueryParams {
    fn from(value: AdminProjectApiKeyListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
    }
}

/// Service account that owns a project API key.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProjectApiKeyOwnerServiceAccount {
    pub id: String,
    pub created_at: u64,
    pub name: String,
    pub role: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// User that owns a project API key.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProjectApiKeyOwnerUser {
    pub id: String,
    pub created_at: u64,
    pub email: String,
    pub name: String,
    pub role: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Owner information attached to a project API key.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ProjectApiKeyOwner {
    #[serde(default)]
    pub service_account: Option<ProjectApiKeyOwnerServiceAccount>,
    #[serde(default, rename = "type")]
    pub owner_type: Option<ProjectApiKeyOwnerType>,
    #[serde(default)]
    pub user: Option<ProjectApiKeyOwnerUser>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Represents an individual API key in a project.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProjectApiKey {
    pub id: String,
    pub created_at: u64,
    #[serde(default)]
    pub last_used_at: Option<u64>,
    pub name: String,
    pub object: ProjectApiKeyObject,
    pub owner: ProjectApiKeyOwner,
    pub redacted_value: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Project API key deletion response.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProjectApiKeyDeleteResponse {
    pub id: String,
    pub deleted: bool,
    pub object: ProjectApiKeyDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type ProjectApiKeyListResponse = AdminConversationCursorPage<ProjectApiKey>;

/// Project rate-limit list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectRateLimitListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
}

impl From<AdminProjectRateLimitListParams> for AdminQueryParams {
    fn from(value: AdminProjectRateLimitListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("before", value.before)
            .push_opt("limit", value.limit)
    }
}

/// Project rate-limit update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectRateLimitUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_1_day_max_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_audio_megabytes_per_1_minute: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_images_per_1_minute: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_requests_per_1_day: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_requests_per_1_minute: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens_per_1_minute: Option<u64>,
}

/// Project model-permission update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectModelPermissionUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<AdminProjectModelPermissionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_ids: Option<Vec<String>>,
}

/// Project model allowlist or denylist policy.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectModelPermissions {
    pub mode: AdminProjectModelPermissionMode,
    pub model_ids: Vec<String>,
    pub object: AdminProjectModelPermissionsObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after deleting project model permissions.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectModelPermissionsDeleted {
    pub deleted: bool,
    pub object: AdminProjectModelPermissionsDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Project hosted-tool permission update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectHostedToolPermissionUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_interpreter: Option<AdminHostedToolPermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_search: Option<AdminHostedToolPermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_generation: Option<AdminHostedToolPermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<AdminHostedToolPermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search: Option<AdminHostedToolPermission>,
}

/// Permission update for a single project hosted tool.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminHostedToolPermission {
    pub enabled: bool,
}

/// Permission state for a single project hosted tool.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminHostedToolPermissionState {
    pub enabled: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Hosted tool permissions configured for a project.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectHostedToolPermissions {
    pub code_interpreter: AdminHostedToolPermissionState,
    pub file_search: AdminHostedToolPermissionState,
    pub image_generation: AdminHostedToolPermissionState,
    pub mcp: AdminHostedToolPermissionState,
    pub web_search: AdminHostedToolPermissionState,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Project group creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectGroupCreateParams {
    pub group_id: String,
    pub role: String,
}

/// Project group retrieve query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectGroupRetrieveParams {
    pub group_type: Option<AdminGroupType>,
}

impl From<AdminProjectGroupRetrieveParams> for AdminQueryParams {
    fn from(value: AdminProjectGroupRetrieveParams) -> Self {
        AdminQueryParams::new().push_opt("group_type", value.group_type)
    }
}

/// Project group list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectGroupListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminProjectGroupListParams> for AdminQueryParams {
    fn from(value: AdminProjectGroupListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Project group-role creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectGroupRoleCreateParams {
    pub role_id: String,
}

/// Project group-role list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectGroupRoleListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminProjectGroupRoleListParams> for AdminQueryParams {
    fn from(value: AdminProjectGroupRoleListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Project role creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectRoleCreateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub permissions: Vec<String>,
    pub role_name: String,
}

/// Project role update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectRoleUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub permissions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
}

/// Project role list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectRoleListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminProjectRoleListParams> for AdminQueryParams {
    fn from(value: AdminProjectRoleListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Project data-retention update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectDataRetentionUpdateParams {
    pub retention_type: AdminProjectDataRetentionType,
}

/// Project data retention control setting.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectDataRetention {
    pub object: AdminProjectDataRetentionObject,
    #[serde(rename = "type")]
    pub retention_type: AdminProjectDataRetentionType,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Project spend-alert creation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectSpendAlertCreateParams {
    pub currency: AdminSpendAlertCurrency,
    pub interval: AdminSpendAlertInterval,
    pub notification_channel: AdminSpendAlertNotificationChannel,
    pub threshold_amount: i64,
}

/// Project spend-alert update body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectSpendAlertUpdateParams {
    pub currency: AdminSpendAlertCurrency,
    pub interval: AdminSpendAlertInterval,
    pub notification_channel: AdminSpendAlertNotificationChannel,
    pub threshold_amount: i64,
}

/// Project spend-alert list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectSpendAlertListParams {
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminProjectSpendAlertListParams> for AdminQueryParams {
    fn from(value: AdminProjectSpendAlertListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("before", value.before)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

/// Represents a spend alert configured at the project level.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectSpendAlert {
    pub id: String,
    pub currency: AdminSpendAlertCurrency,
    pub interval: AdminSpendAlertInterval,
    pub notification_channel: AdminSpendAlertNotificationChannel,
    pub object: AdminProjectSpendAlertObject,
    pub threshold_amount: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Confirmation payload returned after deleting a project spend alert.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AdminProjectSpendAlertDeleted {
    pub id: String,
    pub deleted: bool,
    pub object: AdminProjectSpendAlertDeletedObject,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type AdminProjectSpendAlertListResponse = AdminConversationCursorPage<AdminProjectSpendAlert>;

/// Project certificate activation/deactivation body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AdminProjectCertificateIdsParams {
    pub certificate_ids: Vec<String>,
}

/// Project certificate list query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProjectCertificateListParams {
    pub after: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<ListOrder>,
}

impl From<AdminProjectCertificateListParams> for AdminQueryParams {
    fn from(value: AdminProjectCertificateListParams) -> Self {
        AdminQueryParams::new()
            .push_opt("after", value.after)
            .push_opt("limit", value.limit)
            .push_opt("order", value.order)
    }
}

pub type AdminProjectCertificateListResponse = AdminConversationCursorPage<AdminCertificate>;
pub type AdminProjectCertificateActionResponse = AdminPage<AdminCertificate>;

/// Organization audio speeches usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageAudioSpeechesParams {
    pub start_time: Option<i64>,
    pub api_key_ids: Option<Vec<String>>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageStandardGroupBy>>,
    pub limit: Option<u32>,
    pub models: Option<Vec<String>>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
    pub user_ids: Option<Vec<String>>,
}

impl From<AdminUsageAudioSpeechesParams> for AdminQueryParams {
    fn from(value: AdminUsageAudioSpeechesParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_repeated_opt("api_key_ids", value.api_key_ids)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_repeated_opt("models", value.models)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
            .push_repeated_opt("user_ids", value.user_ids)
    }
}

/// Organization audio transcriptions usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageAudioTranscriptionsParams {
    pub start_time: Option<i64>,
    pub api_key_ids: Option<Vec<String>>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageStandardGroupBy>>,
    pub limit: Option<u32>,
    pub models: Option<Vec<String>>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
    pub user_ids: Option<Vec<String>>,
}

impl From<AdminUsageAudioTranscriptionsParams> for AdminQueryParams {
    fn from(value: AdminUsageAudioTranscriptionsParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_repeated_opt("api_key_ids", value.api_key_ids)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_repeated_opt("models", value.models)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
            .push_repeated_opt("user_ids", value.user_ids)
    }
}

/// Organization code interpreter sessions usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageCodeInterpreterSessionsParams {
    pub start_time: Option<i64>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageProjectGroupBy>>,
    pub limit: Option<u32>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
}

impl From<AdminUsageCodeInterpreterSessionsParams> for AdminQueryParams {
    fn from(value: AdminUsageCodeInterpreterSessionsParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
    }
}

/// Organization completions usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageCompletionsParams {
    pub start_time: Option<i64>,
    pub api_key_ids: Option<Vec<String>>,
    pub batch: Option<bool>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageCompletionsGroupBy>>,
    pub limit: Option<u32>,
    pub models: Option<Vec<String>>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
    pub user_ids: Option<Vec<String>>,
}

impl From<AdminUsageCompletionsParams> for AdminQueryParams {
    fn from(value: AdminUsageCompletionsParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_repeated_opt("api_key_ids", value.api_key_ids)
            .push_opt("batch", value.batch)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_repeated_opt("models", value.models)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
            .push_repeated_opt("user_ids", value.user_ids)
    }
}

/// Organization costs usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageCostsParams {
    pub start_time: Option<i64>,
    pub api_key_ids: Option<Vec<String>>,
    pub bucket_width: Option<AdminUsageCostsBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageCostsGroupBy>>,
    pub limit: Option<u32>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
}

impl From<AdminUsageCostsParams> for AdminQueryParams {
    fn from(value: AdminUsageCostsParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_repeated_opt("api_key_ids", value.api_key_ids)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
    }
}

/// Organization embeddings usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageEmbeddingsParams {
    pub start_time: Option<i64>,
    pub api_key_ids: Option<Vec<String>>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageStandardGroupBy>>,
    pub limit: Option<u32>,
    pub models: Option<Vec<String>>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
    pub user_ids: Option<Vec<String>>,
}

impl From<AdminUsageEmbeddingsParams> for AdminQueryParams {
    fn from(value: AdminUsageEmbeddingsParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_repeated_opt("api_key_ids", value.api_key_ids)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_repeated_opt("models", value.models)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
            .push_repeated_opt("user_ids", value.user_ids)
    }
}

/// Organization file search calls usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageFileSearchCallsParams {
    pub start_time: Option<i64>,
    pub api_key_ids: Option<Vec<String>>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageFileSearchCallsGroupBy>>,
    pub limit: Option<u32>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
    pub user_ids: Option<Vec<String>>,
    pub vector_store_ids: Option<Vec<String>>,
}

impl From<AdminUsageFileSearchCallsParams> for AdminQueryParams {
    fn from(value: AdminUsageFileSearchCallsParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_repeated_opt("api_key_ids", value.api_key_ids)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
            .push_repeated_opt("user_ids", value.user_ids)
            .push_repeated_opt("vector_store_ids", value.vector_store_ids)
    }
}

/// Organization images usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageImagesParams {
    pub start_time: Option<i64>,
    pub api_key_ids: Option<Vec<String>>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageImagesGroupBy>>,
    pub limit: Option<u32>,
    pub models: Option<Vec<String>>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
    pub sizes: Option<Vec<AdminUsageImagesSize>>,
    pub sources: Option<Vec<AdminUsageImagesSource>>,
    pub user_ids: Option<Vec<String>>,
}

impl From<AdminUsageImagesParams> for AdminQueryParams {
    fn from(value: AdminUsageImagesParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_repeated_opt("api_key_ids", value.api_key_ids)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_repeated_opt("models", value.models)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
            .push_repeated_opt("sizes", value.sizes)
            .push_repeated_opt("sources", value.sources)
            .push_repeated_opt("user_ids", value.user_ids)
    }
}

/// Organization moderations usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageModerationsParams {
    pub start_time: Option<i64>,
    pub api_key_ids: Option<Vec<String>>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageStandardGroupBy>>,
    pub limit: Option<u32>,
    pub models: Option<Vec<String>>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
    pub user_ids: Option<Vec<String>>,
}

impl From<AdminUsageModerationsParams> for AdminQueryParams {
    fn from(value: AdminUsageModerationsParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_repeated_opt("api_key_ids", value.api_key_ids)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_repeated_opt("models", value.models)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
            .push_repeated_opt("user_ids", value.user_ids)
    }
}

/// Organization vector stores usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageVectorStoresParams {
    pub start_time: Option<i64>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageProjectGroupBy>>,
    pub limit: Option<u32>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
}

impl From<AdminUsageVectorStoresParams> for AdminQueryParams {
    fn from(value: AdminUsageVectorStoresParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_opt("bucket_width", value.bucket_width)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
    }
}

/// Organization web search calls usage query parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUsageWebSearchCallsParams {
    pub start_time: Option<i64>,
    pub api_key_ids: Option<Vec<String>>,
    pub bucket_width: Option<AdminUsageBucketWidth>,
    pub context_levels: Option<Vec<AdminUsageWebSearchContextLevel>>,
    pub end_time: Option<i64>,
    pub group_by: Option<Vec<AdminUsageWebSearchCallsGroupBy>>,
    pub limit: Option<u32>,
    pub models: Option<Vec<String>>,
    pub page: Option<String>,
    pub project_ids: Option<Vec<String>>,
    pub user_ids: Option<Vec<String>>,
}

impl From<AdminUsageWebSearchCallsParams> for AdminQueryParams {
    fn from(value: AdminUsageWebSearchCallsParams) -> Self {
        AdminQueryParams::new()
            .push_opt("start_time", value.start_time)
            .push_repeated_opt("api_key_ids", value.api_key_ids)
            .push_opt("bucket_width", value.bucket_width)
            .push_repeated_opt("context_levels", value.context_levels)
            .push_opt("end_time", value.end_time)
            .push_repeated_opt("group_by", value.group_by)
            .push_opt("limit", value.limit)
            .push_repeated_opt("models", value.models)
            .push_opt("page", value.page)
            .push_repeated_opt("project_ids", value.project_ids)
            .push_repeated_opt("user_ids", value.user_ids)
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

    pub fn create<B: Serialize>(
        &self,
        params: B,
    ) -> Result<ApiResponse<AdminApiKeyCreateResponse>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/admin_api_keys",
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(&self, key_id: &str) -> Result<ApiResponse<AdminApiKey>, OpenAIError> {
        let key_id = path_id("key_id", key_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/admin_api_keys/{key_id}"),
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminApiKeyListResponse>, OpenAIError> {
        self.runtime.execute_json(
            "GET",
            path_with_query("/organization/admin_api_keys", params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        key_id: &str,
    ) -> Result<ApiResponse<AdminApiKeyDeleteResponse>, OpenAIError> {
        let key_id = path_id("key_id", key_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/admin_api_keys/{key_id}"),
            RequestOptions::default(),
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
        get_query(&self.runtime, "/organization/usage/audio_speeches", params)
    }

    pub fn audio_transcriptions(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(
            &self.runtime,
            "/organization/usage/audio_transcriptions",
            params,
        )
    }

    pub fn code_interpreter_sessions(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(
            &self.runtime,
            "/organization/usage/code_interpreter_sessions",
            params,
        )
    }

    pub fn completions(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/usage/completions", params)
    }

    pub fn costs(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/costs", params)
    }

    pub fn embeddings(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/usage/embeddings", params)
    }

    pub fn file_search_calls(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(
            &self.runtime,
            "/organization/usage/file_search_calls",
            params,
        )
    }

    pub fn images(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/usage/images", params)
    }

    pub fn moderations(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/usage/moderations", params)
    }

    pub fn vector_stores(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(&self.runtime, "/organization/usage/vector_stores", params)
    }

    pub fn web_search_calls(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminValue>, OpenAIError> {
        get_query(
            &self.runtime,
            "/organization/usage/web_search_calls",
            params,
        )
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

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminInvite>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/invites",
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(&self, invite_id: &str) -> Result<ApiResponse<AdminInvite>, OpenAIError> {
        let invite_id = path_id("invite_id", invite_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/invites/{invite_id}"),
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminInviteListResponse>, OpenAIError> {
        self.runtime.execute_json(
            "GET",
            path_with_query("/organization/invites", params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        invite_id: &str,
    ) -> Result<ApiResponse<AdminInviteDeleteResponse>, OpenAIError> {
        let invite_id = path_id("invite_id", invite_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/invites/{invite_id}"),
            RequestOptions::default(),
        )
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

    pub fn retrieve(
        &self,
        user_id: &str,
    ) -> Result<ApiResponse<AdminOrganizationUser>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/users/{user_id}"),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        user_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminOrganizationUser>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/users/{user_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminOrganizationUserListResponse>, OpenAIError> {
        self.runtime.execute_json(
            "GET",
            path_with_query("/organization/users", params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        user_id: &str,
    ) -> Result<ApiResponse<AdminOrganizationUserDeleteResponse>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/users/{user_id}"),
            RequestOptions::default(),
        )
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
    ) -> Result<ApiResponse<AdminUserRoleCreateResponse>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/users/{user_id}/roles"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminUserRoleRetrieveResponse>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        let role_id = path_id("role_id", role_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/users/{user_id}/roles/{role_id}"),
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        user_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminUserRoleListResponse>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(format!("/organization/users/{user_id}/roles"), params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminRoleAssignmentDeleteResponse>, OpenAIError> {
        let user_id = path_id("user_id", user_id)?;
        let role_id = path_id("role_id", role_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/users/{user_id}/roles/{role_id}"),
            RequestOptions::default(),
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

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminGroup>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/groups",
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(&self, group_id: &str) -> Result<ApiResponse<AdminGroup>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/groups/{group_id}"),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        group_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminGroupUpdateResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/groups/{group_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminGroupListResponse>, OpenAIError> {
        self.runtime.execute_json(
            "GET",
            path_with_query("/organization/groups", params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        group_id: &str,
    ) -> Result<ApiResponse<AdminGroupDeleteResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/groups/{group_id}"),
            RequestOptions::default(),
        )
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
    ) -> Result<ApiResponse<AdminGroupUserCreateResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/groups/{group_id}/users"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(
        &self,
        group_id: &str,
        user_id: &str,
    ) -> Result<ApiResponse<AdminGroupUserRetrieveResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/groups/{group_id}/users/{user_id}"),
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        group_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminGroupUserListResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(format!("/organization/groups/{group_id}/users"), params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        group_id: &str,
        user_id: &str,
    ) -> Result<ApiResponse<AdminGroupUserDeleteResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/groups/{group_id}/users/{user_id}"),
            RequestOptions::default(),
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
    ) -> Result<ApiResponse<AdminGroupRoleCreateResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/groups/{group_id}/roles"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(
        &self,
        group_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminGroupRoleRetrieveResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        let role_id = path_id("role_id", role_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/groups/{group_id}/roles/{role_id}"),
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        group_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminGroupRoleListResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(format!("/organization/groups/{group_id}/roles"), params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        group_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminRoleAssignmentDeleteResponse>, OpenAIError> {
        let group_id = path_id("group_id", group_id)?;
        let role_id = path_id("role_id", role_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/groups/{group_id}/roles/{role_id}"),
            RequestOptions::default(),
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

    pub fn create<B: Serialize>(&self, params: B) -> Result<ApiResponse<AdminRole>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/roles",
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(&self, role_id: &str) -> Result<ApiResponse<AdminRole>, OpenAIError> {
        let role_id = path_id("role_id", role_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/roles/{role_id}"),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        role_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminRole>, OpenAIError> {
        let role_id = path_id("role_id", role_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/roles/{role_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminRoleListResponse>, OpenAIError> {
        self.runtime.execute_json(
            "GET",
            path_with_query("/organization/roles", params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        role_id: &str,
    ) -> Result<ApiResponse<AdminRoleDeleteResponse>, OpenAIError> {
        let role_id = path_id("role_id", role_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/roles/{role_id}"),
            RequestOptions::default(),
        )
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

    pub fn retrieve(&self) -> Result<ApiResponse<AdminOrganizationDataRetention>, OpenAIError> {
        self.runtime.execute_json(
            "GET",
            "/organization/data_retention",
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        params: B,
    ) -> Result<ApiResponse<AdminOrganizationDataRetention>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/data_retention",
            &params,
            RequestOptions::default(),
        )
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

    pub fn create<B: Serialize>(
        &self,
        params: B,
    ) -> Result<ApiResponse<AdminOrganizationSpendAlert>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/spend_alerts",
            &params,
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        alert_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminOrganizationSpendAlert>, OpenAIError> {
        let alert_id = path_id("alert_id", alert_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/spend_alerts/{alert_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminOrganizationSpendAlertListResponse>, OpenAIError> {
        self.runtime.execute_json(
            "GET",
            path_with_query("/organization/spend_alerts", params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        alert_id: &str,
    ) -> Result<ApiResponse<AdminOrganizationSpendAlertDeleted>, OpenAIError> {
        let alert_id = path_id("alert_id", alert_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/spend_alerts/{alert_id}"),
            RequestOptions::default(),
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

    pub fn create<B: Serialize>(
        &self,
        params: B,
    ) -> Result<ApiResponse<AdminCertificate>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/certificates",
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(
        &self,
        certificate_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminCertificate>, OpenAIError> {
        let certificate_id = path_id("certificate_id", certificate_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(
                format!("/organization/certificates/{certificate_id}"),
                params,
            ),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        certificate_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminCertificate>, OpenAIError> {
        let certificate_id = path_id("certificate_id", certificate_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/certificates/{certificate_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminCertificateListResponse>, OpenAIError> {
        self.runtime.execute_json(
            "GET",
            path_with_query("/organization/certificates", params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        certificate_id: &str,
    ) -> Result<ApiResponse<AdminCertificateDeleteResponse>, OpenAIError> {
        let certificate_id = path_id("certificate_id", certificate_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/certificates/{certificate_id}"),
            RequestOptions::default(),
        )
    }

    pub fn activate<B: Serialize>(
        &self,
        params: B,
    ) -> Result<ApiResponse<AdminCertificateActionResponse>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/certificates/activate",
            &params,
            RequestOptions::default(),
        )
    }

    pub fn deactivate<B: Serialize>(
        &self,
        params: B,
    ) -> Result<ApiResponse<AdminCertificateActionResponse>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/certificates/deactivate",
            &params,
            RequestOptions::default(),
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

    pub fn create<B: Serialize>(
        &self,
        params: B,
    ) -> Result<ApiResponse<AdminProject>, OpenAIError> {
        self.runtime.execute_json_with_body(
            "POST",
            "/organization/projects",
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(&self, project_id: &str) -> Result<ApiResponse<AdminProject>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/projects/{project_id}"),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminProject>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminProjectListResponse>, OpenAIError> {
        self.runtime.execute_json(
            "GET",
            path_with_query("/organization/projects", params),
            RequestOptions::default(),
        )
    }

    pub fn archive(&self, project_id: &str) -> Result<ApiResponse<AdminProject>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "POST",
            format!("/organization/projects/{project_id}/archive"),
            RequestOptions::default(),
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
    ) -> Result<ApiResponse<AdminProjectUser>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/users"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<ApiResponse<AdminProjectUser>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/projects/{project_id}/users/{user_id}"),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        user_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminProjectUser>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/users/{user_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminProjectUserListResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(format!("/organization/projects/{project_id}/users"), params),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<ApiResponse<AdminProjectUserDeleteResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/projects/{project_id}/users/{user_id}"),
            RequestOptions::default(),
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
    ) -> Result<ApiResponse<AdminProjectUserRoleCreateResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/projects/{project_id}/users/{user_id}/roles"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        user_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminProjectUserRoleRetrieveResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        let role_id = path_id("role_id", role_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/projects/{project_id}/users/{user_id}/roles/{role_id}"),
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        user_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminProjectUserRoleListResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(
                format!("/projects/{project_id}/users/{user_id}/roles"),
                params,
            ),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        user_id: &str,
        role_id: &str,
    ) -> Result<ApiResponse<AdminProjectUserRoleDeleteResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let user_id = path_id("user_id", user_id)?;
        let role_id = path_id("role_id", role_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/projects/{project_id}/users/{user_id}/roles/{role_id}"),
            RequestOptions::default(),
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
    ) -> Result<ApiResponse<AdminProjectServiceAccountCreateResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/service_accounts"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn retrieve(
        &self,
        project_id: &str,
        service_account_id: &str,
    ) -> Result<ApiResponse<AdminProjectServiceAccount>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let service_account_id = path_id("service_account_id", service_account_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/projects/{project_id}/service_accounts/{service_account_id}"),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        service_account_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminProjectServiceAccount>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let service_account_id = path_id("service_account_id", service_account_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/service_accounts/{service_account_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminProjectServiceAccountListResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(
                format!("/organization/projects/{project_id}/service_accounts"),
                params,
            ),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        service_account_id: &str,
    ) -> Result<ApiResponse<AdminProjectServiceAccountDeleteResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let service_account_id = path_id("service_account_id", service_account_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/projects/{project_id}/service_accounts/{service_account_id}"),
            RequestOptions::default(),
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
    ) -> Result<ApiResponse<ProjectApiKey>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let api_key_id = path_id("api_key_id", api_key_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/projects/{project_id}/api_keys/{api_key_id}"),
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<ProjectApiKeyListResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(
                format!("/organization/projects/{project_id}/api_keys"),
                params,
            ),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        api_key_id: &str,
    ) -> Result<ApiResponse<ProjectApiKeyDeleteResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let api_key_id = path_id("api_key_id", api_key_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/projects/{project_id}/api_keys/{api_key_id}"),
            RequestOptions::default(),
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

    pub fn retrieve(
        &self,
        project_id: &str,
    ) -> Result<ApiResponse<AdminProjectModelPermissions>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/projects/{project_id}/model_permissions"),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminProjectModelPermissions>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/model_permissions"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
    ) -> Result<ApiResponse<AdminProjectModelPermissionsDeleted>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/projects/{project_id}/model_permissions"),
            RequestOptions::default(),
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

    pub fn retrieve(
        &self,
        project_id: &str,
    ) -> Result<ApiResponse<AdminProjectHostedToolPermissions>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/projects/{project_id}/hosted_tool_permissions"),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminProjectHostedToolPermissions>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/hosted_tool_permissions"),
            &params,
            RequestOptions::default(),
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
            format!("/projects/{project_id}/groups/{group_id}/roles"),
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
            format!("/projects/{project_id}/groups/{group_id}/roles/{role_id}"),
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
            format!("/projects/{project_id}/groups/{group_id}/roles"),
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
            format!("/projects/{project_id}/groups/{group_id}/roles/{role_id}"),
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
            format!("/projects/{project_id}/roles"),
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
            format!("/projects/{project_id}/roles/{role_id}"),
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
            format!("/projects/{project_id}/roles/{role_id}"),
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
            format!("/projects/{project_id}/roles"),
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
            format!("/projects/{project_id}/roles/{role_id}"),
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

    pub fn retrieve(
        &self,
        project_id: &str,
    ) -> Result<ApiResponse<AdminProjectDataRetention>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "GET",
            format!("/organization/projects/{project_id}/data_retention"),
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminProjectDataRetention>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/data_retention"),
            &params,
            RequestOptions::default(),
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
    ) -> Result<ApiResponse<AdminProjectSpendAlert>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/spend_alerts"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn update<B: Serialize>(
        &self,
        project_id: &str,
        alert_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminProjectSpendAlert>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let alert_id = path_id("alert_id", alert_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/spend_alerts/{alert_id}"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn list(
        &self,
        project_id: &str,
        params: impl Into<AdminQueryParams>,
    ) -> Result<ApiResponse<AdminProjectSpendAlertListResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(
                format!("/organization/projects/{project_id}/spend_alerts"),
                params,
            ),
            RequestOptions::default(),
        )
    }

    pub fn delete(
        &self,
        project_id: &str,
        alert_id: &str,
    ) -> Result<ApiResponse<AdminProjectSpendAlertDeleted>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        let alert_id = path_id("alert_id", alert_id)?;
        self.runtime.execute_json(
            "DELETE",
            format!("/organization/projects/{project_id}/spend_alerts/{alert_id}"),
            RequestOptions::default(),
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
    ) -> Result<ApiResponse<AdminProjectCertificateListResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json(
            "GET",
            path_with_query(
                format!("/organization/projects/{project_id}/certificates"),
                params,
            ),
            RequestOptions::default(),
        )
    }

    pub fn activate<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminProjectCertificateActionResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/certificates/activate"),
            &params,
            RequestOptions::default(),
        )
    }

    pub fn deactivate<B: Serialize>(
        &self,
        project_id: &str,
        params: B,
    ) -> Result<ApiResponse<AdminProjectCertificateActionResponse>, OpenAIError> {
        let project_id = path_id("project_id", project_id)?;
        self.runtime.execute_json_with_body(
            "POST",
            format!("/organization/projects/{project_id}/certificates/deactivate"),
            &params,
            RequestOptions::default(),
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
