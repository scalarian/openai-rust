use crate::{config::ClientConfig, realtime::Realtime, resources::ResourceFamilies};

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

    /// Builds the scaffold client.
    pub fn build(self) -> OpenAI {
        OpenAI {
            config: self.config,
            resources: ResourceFamilies::default(),
            realtime: Realtime,
        }
    }
}
