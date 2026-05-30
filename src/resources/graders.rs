use serde::{Deserialize, Serialize};

macro_rules! grader_string_literal_enum {
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

grader_string_literal_enum! {
    /// Role accepted by model-based grader input messages.
    pub enum GraderMessageRole {
        User => "user",
        Assistant => "assistant",
        System => "system",
        Developer => "developer",
    }
}

grader_string_literal_enum! {
    /// Optional discriminator for model-based grader input messages.
    pub enum GraderMessageType {
        Message => "message",
    }
}

grader_string_literal_enum! {
    /// String comparison operations accepted by string-check graders.
    pub enum GraderStringCheckOperation {
        Eq => "eq",
        Ne => "ne",
        Like => "like",
        Ilike => "ilike",
    }
}

grader_string_literal_enum! {
    /// Text-similarity metrics accepted by text-similarity graders.
    pub enum GraderTextSimilarityMetric {
        Cosine => "cosine",
        FuzzyMatch => "fuzzy_match",
        Bleu => "bleu",
        Gleu => "gleu",
        Meteor => "meteor",
        Rouge1 => "rouge_1",
        Rouge2 => "rouge_2",
        Rouge3 => "rouge_3",
        Rouge4 => "rouge_4",
        Rouge5 => "rouge_5",
        RougeL => "rouge_l",
    }
}

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
