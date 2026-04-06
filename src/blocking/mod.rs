use serde::de::DeserializeOwned;

use crate::{
    OpenAIError,
    config::{ClientConfig, ResolvedClientConfig},
    core::{
        request::{PreparedRequest, RequestOptions, ResolvedRequestOptions},
        response::ApiResponse,
    },
};

/// Feature-gated blocking REST facade over the shared client/runtime.
#[derive(Clone, Debug, Default)]
pub struct OpenAI {
    inner: crate::OpenAI,
}

impl OpenAI {
    /// Creates a blocking client with default scaffold configuration.
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Starts building a blocking client configuration.
    pub fn builder() -> OpenAIBuilder {
        OpenAIBuilder::default()
    }

    /// Returns the current client configuration scaffold.
    pub fn config(&self) -> &ClientConfig {
        self.inner.config()
    }

    /// Resolves the current configuration against environment defaults.
    pub fn resolved_config(&self) -> Result<ResolvedClientConfig, OpenAIError> {
        self.inner.resolved_config()
    }

    /// Prepares an authenticated REST request before any transport is attempted.
    pub fn prepare_request(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> Result<PreparedRequest, OpenAIError> {
        self.inner.prepare_request(method, path)
    }

    /// Resolves per-request execution options against client defaults.
    pub fn resolve_request_options(
        &self,
        options: &RequestOptions,
    ) -> Result<ResolvedRequestOptions, OpenAIError> {
        self.inner.resolve_request_options(options)
    }

    /// Executes a JSON request through the shared transport path.
    pub fn execute_json<T>(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<ApiResponse<T>, OpenAIError>
    where
        T: DeserializeOwned,
    {
        self.inner.execute_json(method, path, options)
    }

    /// Accesses the responses family handle.
    pub fn responses(&self) -> crate::resources::responses::Responses {
        self.inner.responses().clone()
    }

    /// Accesses the conversations family handle.
    pub fn conversations(&self) -> crate::resources::conversations::Conversations {
        self.inner.conversations().clone()
    }

    /// Accesses the chat completions compatibility handle.
    pub fn chat(&self) -> crate::resources::chat::Chat {
        self.inner.chat().clone()
    }

    /// Accesses the legacy completions compatibility handle.
    pub fn completions(&self) -> crate::resources::completions::Completions {
        self.inner.completions().clone()
    }

    /// Accesses the embeddings family handle.
    pub fn embeddings(&self) -> crate::resources::embeddings::Embeddings {
        self.inner.embeddings().clone()
    }

    /// Accesses the models family handle.
    pub fn models(&self) -> crate::resources::models::Models {
        self.inner.models().clone()
    }

    /// Accesses the moderations family handle.
    pub fn moderations(&self) -> crate::resources::moderations::Moderations {
        self.inner.moderations().clone()
    }

    /// Accesses the images family handle.
    pub fn images(&self) -> crate::resources::images::Images {
        self.inner.images().clone()
    }

    /// Accesses the audio family handle.
    pub fn audio(&self) -> crate::resources::audio::Audio {
        self.inner.audio().clone()
    }

    /// Accesses the files family handle.
    pub fn files(&self) -> crate::resources::files::Files {
        self.inner.files().clone()
    }

    /// Accesses the uploads family handle.
    pub fn uploads(&self) -> crate::resources::uploads::Uploads {
        self.inner.uploads().clone()
    }

    /// Accesses the vector stores family handle.
    pub fn vector_stores(&self) -> crate::resources::vector_stores::VectorStores {
        self.inner.vector_stores().clone()
    }

    /// Accesses the batches family handle.
    pub fn batches(&self) -> crate::resources::batches::Batches {
        self.inner.batches().clone()
    }

    /// Accesses the webhook helpers handle.
    pub fn webhooks(&self) -> crate::resources::webhooks::Webhooks {
        self.inner.webhooks().clone()
    }

    /// Accesses the fine-tuning family handle.
    pub fn fine_tuning(&self) -> crate::resources::fine_tuning::FineTuning {
        self.inner.fine_tuning().clone()
    }

    /// Accesses the evals family handle.
    pub fn evals(&self) -> crate::resources::evals::Evals {
        self.inner.evals().clone()
    }

    /// Accesses the containers family handle.
    pub fn containers(&self) -> crate::resources::containers::Containers {
        self.inner.containers().clone()
    }

    /// Accesses the skills family handle.
    pub fn skills(&self) -> crate::resources::skills::Skills {
        self.inner.skills().clone()
    }

    /// Accesses the videos family handle.
    pub fn videos(&self) -> crate::resources::videos::Videos {
        self.inner.videos().clone()
    }
}

impl From<crate::OpenAI> for OpenAI {
    fn from(inner: crate::OpenAI) -> Self {
        Self { inner }
    }
}

impl From<OpenAI> for crate::OpenAI {
    fn from(client: OpenAI) -> Self {
        client.inner
    }
}

/// Builder for the feature-gated blocking facade.
#[derive(Clone, Debug, Default)]
pub struct OpenAIBuilder {
    inner: crate::OpenAIBuilder,
}

impl OpenAIBuilder {
    /// Replaces the scaffold configuration.
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.inner = self.inner.config(config);
        self
    }

    /// Sets an explicit API key.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.inner = self.inner.api_key(api_key);
        self
    }

    /// Sets an explicit base URL.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.inner = self.inner.base_url(base_url);
        self
    }

    /// Sets an explicit organization identifier.
    pub fn organization(mut self, organization: impl Into<String>) -> Self {
        self.inner = self.inner.organization(organization);
        self
    }

    /// Sets an explicit project identifier.
    pub fn project(mut self, project: impl Into<String>) -> Self {
        self.inner = self.inner.project(project);
        self
    }

    /// Sets a custom user-agent token or prefix.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.inner = self.inner.user_agent(user_agent);
        self
    }

    /// Sets a default webhook secret for signature verification helpers.
    pub fn webhook_secret(mut self, webhook_secret: impl Into<String>) -> Self {
        self.inner = self.inner.webhook_secret(webhook_secret);
        self
    }

    /// Sets a client-level timeout budget.
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.inner = self.inner.timeout(timeout);
        self
    }

    /// Sets a client-level retry budget.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.inner = self.inner.max_retries(max_retries);
        self
    }

    /// Builds the blocking facade over the shared runtime.
    pub fn build(self) -> OpenAI {
        OpenAI {
            inner: self.inner.build(),
        }
    }
}
