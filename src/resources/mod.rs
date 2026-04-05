//! Public API family placeholders aligned to the clean-room architecture.

pub mod audio;
pub mod batches;
pub mod chat;
pub mod completions;
pub mod containers;
pub mod conversations;
pub mod embeddings;
pub mod evals;
pub mod files;
pub mod fine_tuning;
pub mod images;
pub mod models;
pub mod moderations;
pub mod responses;
pub mod skills;
pub mod uploads;
pub mod vector_stores;
pub mod videos;
pub mod webhooks;

/// Root collection of resource-family handles.
#[derive(Clone, Debug, Default)]
pub struct ResourceFamilies {
    pub(crate) responses: responses::Responses,
    pub(crate) conversations: conversations::Conversations,
    pub(crate) chat: chat::Chat,
    pub(crate) completions: completions::Completions,
    pub(crate) embeddings: embeddings::Embeddings,
    pub(crate) models: models::Models,
    pub(crate) moderations: moderations::Moderations,
    pub(crate) images: images::Images,
    pub(crate) audio: audio::Audio,
    pub(crate) files: files::Files,
    pub(crate) uploads: uploads::Uploads,
    pub(crate) vector_stores: vector_stores::VectorStores,
    pub(crate) batches: batches::Batches,
    pub(crate) webhooks: webhooks::Webhooks,
    pub(crate) fine_tuning: fine_tuning::FineTuning,
    pub(crate) evals: evals::Evals,
    pub(crate) containers: containers::Containers,
    pub(crate) skills: skills::Skills,
    pub(crate) videos: videos::Videos,
}
