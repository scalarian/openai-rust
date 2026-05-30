use serde::{Deserialize, Serialize};

/// Message content accepted by eval and fine-tuning model graders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GraderMessageContent {
    Text(String),
    Part(GraderMessageContentPart),
    Parts(Vec<GraderMessageContentPart>),
}

impl From<String> for GraderMessageContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for GraderMessageContent {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<GraderMessageContentPart> for GraderMessageContent {
    fn from(value: GraderMessageContentPart) -> Self {
        Self::Part(value)
    }
}

impl From<Vec<GraderMessageContentPart>> for GraderMessageContent {
    fn from(value: Vec<GraderMessageContentPart>) -> Self {
        Self::Parts(value)
    }
}

/// Single grader content item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GraderMessageContentPart {
    Text(String),
    Structured(GraderStructuredContentPart),
}

impl From<String> for GraderMessageContentPart {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for GraderMessageContentPart {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<GraderStructuredContentPart> for GraderMessageContentPart {
    fn from(value: GraderStructuredContentPart) -> Self {
        Self::Structured(value)
    }
}

/// Structured grader content items.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraderStructuredContentPart {
    InputText {
        text: String,
    },
    OutputText {
        text: String,
    },
    InputImage {
        image_url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<GraderInputImageDetail>,
    },
    InputAudio {
        input_audio: GraderInputAudio,
    },
}

impl GraderStructuredContentPart {
    pub fn input_text(text: impl Into<String>) -> Self {
        Self::InputText { text: text.into() }
    }

    pub fn output_text(text: impl Into<String>) -> Self {
        Self::OutputText { text: text.into() }
    }

    pub fn input_image(
        image_url: impl Into<String>,
        detail: Option<GraderInputImageDetail>,
    ) -> Self {
        Self::InputImage {
            image_url: image_url.into(),
            detail,
        }
    }

    pub fn input_audio(input_audio: GraderInputAudio) -> Self {
        Self::InputAudio { input_audio }
    }
}

/// Grader image detail control.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraderInputImageDetail {
    Auto,
    Low,
    High,
}

/// Grader base64 audio input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GraderInputAudio {
    pub data: String,
    pub format: GraderInputAudioFormat,
}

/// Grader input audio format.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraderInputAudioFormat {
    Mp3,
    Wav,
}
