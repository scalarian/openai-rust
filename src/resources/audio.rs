/// Audio API family placeholder.
#[derive(Clone, Debug, Default)]
pub struct Audio {
    /// Audio transcription placeholder handle.
    pub transcriptions: Transcriptions,
    /// Audio translation placeholder handle.
    pub translations: Translations,
    /// Audio speech placeholder handle.
    pub speech: Speech,
}

/// Audio transcription placeholder.
#[derive(Clone, Debug, Default)]
pub struct Transcriptions;

/// Audio translation placeholder.
#[derive(Clone, Debug, Default)]
pub struct Translations;

/// Audio speech placeholder.
#[derive(Clone, Debug, Default)]
pub struct Speech;
