use std::{collections::BTreeMap, env, time::Duration};

use url::Url;

use crate::{
    DEFAULT_BASE_URL,
    error::{ErrorKind, OpenAIError},
};

pub const OPENAI_API_KEY_ENV: &str = "OPENAI_API_KEY";
pub const OPENAI_BASE_URL_ENV: &str = "OPENAI_BASE_URL";
pub const OPENAI_ORG_ID_ENV: &str = "OPENAI_ORG_ID";
pub const OPENAI_PROJECT_ID_ENV: &str = "OPENAI_PROJECT_ID";
pub const OPENAI_WEBHOOK_SECRET_ENV: &str = "OPENAI_WEBHOOK_SECRET";

/// Shared immutable client configuration scaffold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientConfig {
    /// Optional explicit API key override.
    pub api_key: Option<String>,
    /// Optional explicit base URL override.
    pub base_url: Option<String>,
    /// Optional explicit organization header.
    pub organization: Option<String>,
    /// Optional explicit project header.
    pub project: Option<String>,
    /// Optional custom user-agent value.
    pub user_agent: Option<String>,
    /// Optional webhook secret for signature verification helpers.
    pub webhook_secret: Option<String>,
    /// Optional client-level timeout override.
    pub timeout: Option<Duration>,
    /// Optional retry-budget override.
    pub max_retries: Option<u32>,
}

/// Fully-resolved client configuration after environment loading and validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedClientConfig {
    pub api_key: String,
    pub base_url: String,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub user_agent: String,
    pub timeout: Duration,
    pub max_retries: u32,
}

impl ClientConfig {
    /// Captures a configuration snapshot from the current process environment.
    pub fn from_env() -> Self {
        Self {
            api_key: env::var(OPENAI_API_KEY_ENV).ok(),
            base_url: env::var(OPENAI_BASE_URL_ENV).ok(),
            organization: env::var(OPENAI_ORG_ID_ENV).ok(),
            project: env::var(OPENAI_PROJECT_ID_ENV).ok(),
            user_agent: None,
            webhook_secret: env::var(OPENAI_WEBHOOK_SECRET_ENV).ok(),
            timeout: None,
            max_retries: None,
        }
    }

    /// Freezes environment defaults into this config without overriding explicit values.
    pub fn with_env_defaults(&self) -> Self {
        let env_config = Self::from_env();
        Self {
            api_key: self.api_key.clone().or(env_config.api_key),
            base_url: self.base_url.clone().or(env_config.base_url),
            organization: self.organization.clone().or(env_config.organization),
            project: self.project.clone().or(env_config.project),
            user_agent: self.user_agent.clone(),
            webhook_secret: self.webhook_secret.clone().or(env_config.webhook_secret),
            timeout: self.timeout,
            max_retries: self.max_retries,
        }
    }

    /// Resolves explicit configuration against environment defaults.
    pub fn resolve(&self) -> Result<ResolvedClientConfig, OpenAIError> {
        let api_key = self
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                OpenAIError::new(
                    ErrorKind::Configuration,
                    "missing OpenAI API key: provide api_key or set OPENAI_API_KEY",
                )
            })?
            .to_string();

        let base_url = normalize_base_url(self.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL))?;

        let organization = normalize_optional(self.organization.as_deref());
        let project = normalize_optional(self.project.as_deref());
        let user_agent = build_user_agent(self.user_agent.as_deref());
        let timeout = self
            .timeout
            .unwrap_or(crate::core::timeout::TimeoutPolicy::DEFAULT_REQUEST_TIMEOUT);
        let max_retries = self
            .max_retries
            .unwrap_or(crate::core::retry::RetryPolicy::DEFAULT_MAX_RETRIES);

        Ok(ResolvedClientConfig {
            api_key,
            base_url,
            organization,
            project,
            user_agent,
            timeout,
            max_retries,
        })
    }
}

impl ResolvedClientConfig {
    /// Builds default request headers for authenticated REST calls.
    pub fn headers(&self) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        headers.insert(
            String::from("authorization"),
            format!("Bearer {}", self.api_key),
        );
        headers.insert(String::from("user-agent"), self.user_agent.clone());
        if let Some(organization) = &self.organization {
            headers.insert(String::from("openai-organization"), organization.clone());
        }
        if let Some(project) = &self.project {
            headers.insert(String::from("openai-project"), project.clone());
        }
        headers
    }
}

pub(crate) fn normalize_base_url(input: &str) -> Result<String, OpenAIError> {
    let trimmed = input.trim();
    let candidate = if trimmed.is_empty() {
        DEFAULT_BASE_URL
    } else {
        trimmed
    };

    let parsed = Url::parse(candidate).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Configuration,
            format!("invalid OpenAI base URL `{candidate}`: {error}"),
        )
    })?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(OpenAIError::new(
                ErrorKind::Configuration,
                format!("invalid OpenAI base URL scheme `{other}`: expected http or https"),
            ));
        }
    }

    Ok(candidate.trim_end_matches('/').to_string())
}

pub(crate) fn build_user_agent(custom: Option<&str>) -> String {
    let default = format!("openai-rust/{}", env!("CARGO_PKG_VERSION"));
    match normalize_optional(custom) {
        Some(custom) => format!("{custom} {default}"),
        None => default,
    }
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
