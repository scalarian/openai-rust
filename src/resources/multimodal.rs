use serde::{Deserialize, Serialize};

/// Shared image detail controls for multimodal inputs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetail {
    Auto,
    Low,
    High,
    Original,
}

/// Chat-completions image detail controls.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatImageDetail {
    Auto,
    Low,
    High,
}

/// Supported encoded input-audio formats.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputAudioFormat {
    Mp3,
    Wav,
}

/// Base64-encoded audio payload embedded in multimodal requests.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InputAudioData {
    pub data: String,
    pub format: InputAudioFormat,
}

/// Typed Responses input message preserving ordered multimodal content parts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResponseInputMessage {
    pub role: String,
    pub content: Vec<ResponseInputPart>,
}

impl ResponseInputMessage {
    pub fn user(content: Vec<ResponseInputPart>) -> Self {
        Self {
            role: String::from("user"),
            content,
        }
    }
}

/// Typed Responses multimodal content part.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseInputPart {
    InputText {
        text: String,
    },
    InputImage {
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
    },
    InputFile {
        #[serde(skip_serializing_if = "Option::is_none")]
        file_data: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
    },
    InputAudio {
        input_audio: InputAudioData,
    },
}

impl ResponseInputPart {
    pub fn input_text(text: impl Into<String>) -> Self {
        Self::InputText { text: text.into() }
    }

    pub fn input_image_url(image_url: impl Into<String>, detail: Option<ImageDetail>) -> Self {
        Self::InputImage {
            detail,
            file_id: None,
            image_url: Some(image_url.into()),
        }
    }

    pub fn input_image_file(file_id: impl Into<String>, detail: Option<ImageDetail>) -> Self {
        Self::InputImage {
            detail,
            file_id: Some(file_id.into()),
            image_url: None,
        }
    }

    pub fn input_file_id(file_id: impl Into<String>) -> Self {
        Self::InputFile {
            file_data: None,
            file_id: Some(file_id.into()),
            file_url: None,
            filename: None,
        }
    }

    pub fn input_file_url(file_url: impl Into<String>) -> Self {
        Self::InputFile {
            file_data: None,
            file_id: None,
            file_url: Some(file_url.into()),
            filename: None,
        }
    }

    pub fn input_file_data(file_data: impl Into<String>, filename: impl Into<String>) -> Self {
        Self::InputFile {
            file_data: Some(file_data.into()),
            file_id: None,
            file_url: None,
            filename: Some(filename.into()),
        }
    }

    pub fn input_audio(input_audio: InputAudioData) -> Self {
        Self::InputAudio { input_audio }
    }
}

/// Typed chat-completions request message.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChatCompletionMessage {
    pub role: String,
    pub content: ChatCompletionMessageContent,
}

impl ChatCompletionMessage {
    pub fn user_parts(content: Vec<ChatCompletionContentPart>) -> Self {
        Self {
            role: String::from("user"),
            content: ChatCompletionMessageContent::Parts(content),
        }
    }
}

/// Chat request message content can be a bare string or ordered multimodal parts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionMessageContent {
    Text(String),
    Parts(Vec<ChatCompletionContentPart>),
}

/// Typed chat multimodal content part.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatCompletionContentPart {
    Text {
        text: String,
    },
    #[serde(rename = "image_url")]
    ImageUrl {
        image_url: ChatImageUrl,
    },
    InputAudio {
        input_audio: InputAudioData,
    },
}

impl ChatCompletionContentPart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn image_url(url: impl Into<String>, detail: Option<ChatImageDetail>) -> Self {
        Self::ImageUrl {
            image_url: ChatImageUrl {
                url: url.into(),
                detail,
            },
        }
    }

    pub fn input_audio(input_audio: InputAudioData) -> Self {
        Self::InputAudio { input_audio }
    }
}

/// Nested chat image descriptor preserving `url` and `detail` field names.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChatImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ChatImageDetail>,
}
