use crate::{
    config::{ClientConfig, ResolvedClientConfig},
    core::request::{PreparedRequest, RequestOptions, ResolvedRequestOptions},
    realtime::Realtime,
    resources::ResourceFamilies,
};
use serde::de::DeserializeOwned;
use url::Url;

/// Root async-first SDK client scaffold.
#[derive(Clone, Debug, Default)]
pub struct OpenAI {
    config: ClientConfig,
    resources: ResourceFamilies,
    realtime: Realtime,
}

impl OpenAI {
    /// Creates a client with default scaffold configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts building a client configuration.
    pub fn builder() -> OpenAIBuilder {
        OpenAIBuilder::default()
    }

    /// Returns the current client configuration scaffold.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Resolves the current configuration against environment defaults.
    pub fn resolved_config(&self) -> Result<ResolvedClientConfig, crate::OpenAIError> {
        self.config.resolve()
    }

    /// Prepares an authenticated REST request before any transport is attempted.
    pub fn prepare_request(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> Result<PreparedRequest, crate::OpenAIError> {
        let method = method.as_ref().trim().to_ascii_uppercase();
        if method.is_empty() {
            return Err(crate::OpenAIError::new(
                crate::ErrorKind::Validation,
                "request method cannot be blank",
            ));
        }

        let endpoint = normalize_endpoint(path.as_ref());
        if endpoint.is_empty() {
            return Err(crate::OpenAIError::new(
                crate::ErrorKind::Validation,
                "request path cannot be blank",
            ));
        }

        let resolved = self.resolved_config()?;

        Ok(PreparedRequest {
            method,
            url: join_url(&resolved.base_url, &endpoint)?,
            headers: resolved.headers(),
        })
    }

    /// Resolves per-request execution options against client defaults.
    pub fn resolve_request_options(
        &self,
        options: &RequestOptions,
    ) -> Result<ResolvedRequestOptions, crate::OpenAIError> {
        let resolved = self.resolved_config()?;
        Ok(ResolvedRequestOptions {
            timeout: options.timeout.unwrap_or(resolved.timeout),
            max_retries: options.max_retries.unwrap_or(resolved.max_retries),
        })
    }

    /// Executes a JSON request through the shared transport path.
    pub fn execute_json<T>(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<crate::core::response::ApiResponse<T>, crate::OpenAIError>
    where
        T: DeserializeOwned,
    {
        let request = self.prepare_request(method, path)?;
        let resolved_options = self.resolve_request_options(&options)?;
        crate::core::transport::execute_json(&request, &resolved_options)
    }

    /// Accesses the responses family handle.
    pub fn responses(&self) -> &crate::resources::responses::Responses {
        &self.resources.responses
    }

    /// Accesses the conversations family handle.
    pub fn conversations(&self) -> &crate::resources::conversations::Conversations {
        &self.resources.conversations
    }

    /// Accesses the chat completions compatibility handle.
    pub fn chat(&self) -> &crate::resources::chat::Chat {
        &self.resources.chat
    }

    /// Accesses the legacy completions compatibility handle.
    pub fn completions(&self) -> &crate::resources::completions::Completions {
        &self.resources.completions
    }

    /// Accesses the embeddings family handle.
    pub fn embeddings(&self) -> &crate::resources::embeddings::Embeddings {
        &self.resources.embeddings
    }

    /// Accesses the models family handle.
    pub fn models(&self) -> &crate::resources::models::Models {
        &self.resources.models
    }

    /// Accesses the moderations family handle.
    pub fn moderations(&self) -> &crate::resources::moderations::Moderations {
        &self.resources.moderations
    }

    /// Accesses the images family handle.
    pub fn images(&self) -> &crate::resources::images::Images {
        &self.resources.images
    }

    /// Accesses the audio family handle.
    pub fn audio(&self) -> &crate::resources::audio::Audio {
        &self.resources.audio
    }

    /// Accesses the files family handle.
    pub fn files(&self) -> &crate::resources::files::Files {
        &self.resources.files
    }

    /// Accesses the uploads family handle.
    pub fn uploads(&self) -> &crate::resources::uploads::Uploads {
        &self.resources.uploads
    }

    /// Accesses the vector stores family handle.
    pub fn vector_stores(&self) -> &crate::resources::vector_stores::VectorStores {
        &self.resources.vector_stores
    }

    /// Accesses the batches family handle.
    pub fn batches(&self) -> &crate::resources::batches::Batches {
        &self.resources.batches
    }

    /// Accesses the webhook helpers handle.
    pub fn webhooks(&self) -> &crate::resources::webhooks::Webhooks {
        &self.resources.webhooks
    }

    /// Accesses the fine-tuning family handle.
    pub fn fine_tuning(&self) -> &crate::resources::fine_tuning::FineTuning {
        &self.resources.fine_tuning
    }

    /// Accesses the evals family handle.
    pub fn evals(&self) -> &crate::resources::evals::Evals {
        &self.resources.evals
    }

    /// Accesses the containers family handle.
    pub fn containers(&self) -> &crate::resources::containers::Containers {
        &self.resources.containers
    }

    /// Accesses the skills family handle.
    pub fn skills(&self) -> &crate::resources::skills::Skills {
        &self.resources.skills
    }

    /// Accesses the videos family handle.
    pub fn videos(&self) -> &crate::resources::videos::Videos {
        &self.resources.videos
    }

    /// Accesses realtime support scaffolding.
    pub fn realtime(&self) -> &Realtime {
        &self.realtime
    }
}

/// Builder for the root SDK client scaffold.
#[derive(Clone, Debug, Default)]
pub struct OpenAIBuilder {
    config: ClientConfig,
}

impl OpenAIBuilder {
    /// Replaces the scaffold configuration.
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets an explicit API key.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.config.api_key = Some(api_key.into());
        self
    }

    /// Sets an explicit base URL.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.config.base_url = Some(base_url.into());
        self
    }

    /// Sets an explicit organization identifier.
    pub fn organization(mut self, organization: impl Into<String>) -> Self {
        self.config.organization = Some(organization.into());
        self
    }

    /// Sets an explicit project identifier.
    pub fn project(mut self, project: impl Into<String>) -> Self {
        self.config.project = Some(project.into());
        self
    }

    /// Sets a custom user-agent token or prefix.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.config.user_agent = Some(user_agent.into());
        self
    }

    /// Sets a client-level timeout budget.
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.timeout = Some(timeout);
        self
    }

    /// Sets a client-level retry budget.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.config.max_retries = Some(max_retries);
        self
    }

    /// Builds the scaffold client.
    pub fn build(self) -> OpenAI {
        OpenAI {
            config: self.config,
            resources: ResourceFamilies::default(),
            realtime: Realtime,
        }
    }
}

fn normalize_endpoint(path: &str) -> String {
    path.trim()
        .trim_start_matches('/')
        .trim_start_matches("v1/")
        .to_string()
}

fn join_url(base_url: &str, endpoint: &str) -> Result<String, crate::OpenAIError> {
    let mut url = Url::parse(base_url).map_err(|error| {
        crate::OpenAIError::new(
            crate::ErrorKind::Configuration,
            format!("invalid OpenAI base URL `{base_url}`: {error}"),
        )
    })?;
    let mut path = url.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        path.push_str("/v1");
    }
    path.push('/');
    path.push_str(endpoint);
    url.set_path(&path);
    Ok(url.to_string().trim_end_matches('/').to_string())
}
