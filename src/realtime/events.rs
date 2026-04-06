use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{OpenAIError, error::ErrorKind};

/// Realtime session kind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RealtimeSessionType {
    #[default]
    Realtime,
    Transcription,
}

/// Realtime output modality.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RealtimeOutputModality {
    Text,
    Audio,
}

/// Typed Realtime session configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeSessionConfig {
    #[serde(rename = "type")]
    pub session_type: RealtimeSessionType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<Vec<RealtimeOutputModality>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracing: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_detection: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for RealtimeSessionConfig {
    fn default() -> Self {
        Self {
            session_type: RealtimeSessionType::Realtime,
            id: None,
            model: None,
            instructions: None,
            output_modalities: None,
            audio: None,
            include: None,
            max_output_tokens: None,
            prompt: None,
            tool_choice: None,
            tools: None,
            tracing: None,
            truncation: None,
            turn_detection: None,
            extra: BTreeMap::new(),
        }
    }
}

/// One message content part in a Realtime conversation item.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeConversationMessageContentPart {
    #[serde(rename = "type")]
    pub part_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl RealtimeConversationMessageContentPart {
    pub fn input_text(text: impl Into<String>) -> Self {
        Self {
            part_type: String::from("input_text"),
            text: Some(text.into()),
            audio: None,
            transcript: None,
            extra: BTreeMap::new(),
        }
    }
}

/// Typed conversation item.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeConversationItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<RealtimeConversationMessageContentPart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl RealtimeConversationItem {
    pub fn user_message(content: Vec<RealtimeConversationMessageContentPart>) -> Self {
        Self {
            item_type: String::from("message"),
            role: Some(String::from("user")),
            content,
            ..Default::default()
        }
    }
}

/// Structured Realtime error information.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RealtimeErrorInfo {
    pub message: String,
    #[serde(default, rename = "type")]
    pub error_type: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub param: Option<String>,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Typed client event helpers for the text/bootstrap path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealtimeClientEvent {
    SessionUpdate {
        event_id: Option<String>,
        session: RealtimeSessionConfig,
    },
    ConversationItemCreate {
        event_id: Option<String>,
        previous_item_id: Option<String>,
        item: RealtimeConversationItem,
    },
    ResponseCreate {
        event_id: Option<String>,
        response: Option<Value>,
    },
}

impl RealtimeClientEvent {
    pub fn session_update(session: RealtimeSessionConfig) -> Self {
        Self::SessionUpdate {
            event_id: None,
            session,
        }
    }

    pub fn conversation_item_create(item: RealtimeConversationItem) -> Self {
        Self::ConversationItemCreate {
            event_id: None,
            previous_item_id: None,
            item,
        }
    }

    pub fn response_create(response: Option<Value>) -> Self {
        Self::ResponseCreate {
            event_id: None,
            response,
        }
    }

    pub fn with_event_id(mut self, event_id: impl Into<String>) -> Self {
        match &mut self {
            Self::SessionUpdate { event_id: slot, .. }
            | Self::ConversationItemCreate { event_id: slot, .. }
            | Self::ResponseCreate { event_id: slot, .. } => *slot = Some(event_id.into()),
        }
        self
    }

    pub fn with_previous_item_id(mut self, previous_item_id: impl Into<String>) -> Self {
        if let Self::ConversationItemCreate {
            previous_item_id: slot,
            ..
        } = &mut self
        {
            *slot = Some(previous_item_id.into());
        }
        self
    }

    pub fn to_json_value(&self) -> Value {
        match self {
            Self::SessionUpdate { event_id, session } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("session.update")),
                );
                object.insert(
                    String::from("session"),
                    serde_json::to_value(session).unwrap_or(Value::Null),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                Value::Object(object)
            }
            Self::ConversationItemCreate {
                event_id,
                previous_item_id,
                item,
            } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("conversation.item.create")),
                );
                object.insert(
                    String::from("item"),
                    serde_json::to_value(item).unwrap_or(Value::Null),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                if let Some(previous_item_id) = previous_item_id {
                    object.insert(
                        String::from("previous_item_id"),
                        Value::String(previous_item_id.clone()),
                    );
                }
                Value::Object(object)
            }
            Self::ResponseCreate { event_id, response } => {
                let mut object = Map::new();
                object.insert(
                    String::from("type"),
                    Value::String(String::from("response.create")),
                );
                if let Some(event_id) = event_id {
                    object.insert(String::from("event_id"), Value::String(event_id.clone()));
                }
                if let Some(response) = response {
                    object.insert(String::from("response"), response.clone());
                }
                Value::Object(object)
            }
        }
    }
}

/// Realtime server events needed for bootstrap, text, and clean shutdown flows.
#[derive(Clone, Debug, PartialEq)]
pub enum RealtimeServerEvent {
    SessionCreated {
        event_id: String,
        session: RealtimeSessionConfig,
    },
    SessionUpdated {
        event_id: String,
        session: RealtimeSessionConfig,
    },
    ConversationItemCreated {
        event_id: String,
        previous_item_id: Option<String>,
        item: RealtimeConversationItem,
    },
    InputAudioBufferCommitted {
        event_id: String,
        item_id: String,
        previous_item_id: Option<String>,
    },
    InputAudioBufferSpeechStarted {
        event_id: String,
        item_id: String,
        audio_start_ms: u64,
    },
    InputAudioBufferSpeechStopped {
        event_id: String,
        item_id: String,
        audio_end_ms: u64,
    },
    InputAudioBufferCleared {
        event_id: String,
    },
    ConversationItemTruncated {
        event_id: String,
        item_id: String,
        content_index: usize,
        audio_end_ms: u64,
    },
    OutputTextDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        delta: String,
    },
    OutputTextDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        text: String,
    },
    ResponseOutputItemAdded {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        item: RealtimeConversationItem,
    },
    ResponseOutputItemDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        item: RealtimeConversationItem,
    },
    ResponseContentPartAdded {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        part: RealtimeConversationMessageContentPart,
    },
    ResponseContentPartDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        part: RealtimeConversationMessageContentPart,
    },
    OutputAudioDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        delta: String,
    },
    OutputAudioDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
    },
    OutputAudioTranscriptDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        delta: String,
    },
    OutputAudioTranscriptDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        content_index: usize,
        transcript: String,
    },
    FunctionCallArgumentsDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        delta: String,
    },
    FunctionCallArgumentsDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        arguments: String,
        name: Option<String>,
    },
    McpCallArgumentsDelta {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        delta: String,
        obfuscation: Option<String>,
    },
    McpCallArgumentsDone {
        event_id: String,
        response_id: String,
        item_id: String,
        output_index: usize,
        arguments: String,
    },
    ResponseItemStatus {
        event_id: String,
        event_type: String,
        item_id: String,
        output_index: usize,
    },
    OutputAudioBufferStarted {
        event_id: String,
        response_id: String,
    },
    OutputAudioBufferStopped {
        event_id: String,
        response_id: String,
    },
    OutputAudioBufferCleared {
        event_id: String,
        response_id: String,
    },
    ResponseCreated {
        event_id: String,
        response: Value,
    },
    ResponseDone {
        event_id: String,
        response: Value,
    },
    Error {
        event_id: String,
        error: RealtimeErrorInfo,
    },
    Unknown {
        event_id: Option<String>,
        event_type: String,
        raw: Value,
    },
}

impl RealtimeServerEvent {
    pub fn event_type(&self) -> &str {
        match self {
            Self::SessionCreated { .. } => "session.created",
            Self::SessionUpdated { .. } => "session.updated",
            Self::ConversationItemCreated { .. } => "conversation.item.created",
            Self::InputAudioBufferCommitted { .. } => "input_audio_buffer.committed",
            Self::InputAudioBufferSpeechStarted { .. } => "input_audio_buffer.speech_started",
            Self::InputAudioBufferSpeechStopped { .. } => "input_audio_buffer.speech_stopped",
            Self::InputAudioBufferCleared { .. } => "input_audio_buffer.cleared",
            Self::ConversationItemTruncated { .. } => "conversation.item.truncated",
            Self::OutputTextDelta { .. } => "response.output_text.delta",
            Self::OutputTextDone { .. } => "response.output_text.done",
            Self::ResponseOutputItemAdded { .. } => "response.output_item.added",
            Self::ResponseOutputItemDone { .. } => "response.output_item.done",
            Self::ResponseContentPartAdded { .. } => "response.content_part.added",
            Self::ResponseContentPartDone { .. } => "response.content_part.done",
            Self::OutputAudioDelta { .. } => "response.output_audio.delta",
            Self::OutputAudioDone { .. } => "response.output_audio.done",
            Self::OutputAudioTranscriptDelta { .. } => "response.output_audio_transcript.delta",
            Self::OutputAudioTranscriptDone { .. } => "response.output_audio_transcript.done",
            Self::FunctionCallArgumentsDelta { .. } => "response.function_call_arguments.delta",
            Self::FunctionCallArgumentsDone { .. } => "response.function_call_arguments.done",
            Self::McpCallArgumentsDelta { .. } => "response.mcp_call_arguments.delta",
            Self::McpCallArgumentsDone { .. } => "response.mcp_call_arguments.done",
            Self::ResponseItemStatus { event_type, .. } => event_type.as_str(),
            Self::OutputAudioBufferStarted { .. } => "output_audio_buffer.started",
            Self::OutputAudioBufferStopped { .. } => "output_audio_buffer.stopped",
            Self::OutputAudioBufferCleared { .. } => "output_audio_buffer.cleared",
            Self::ResponseCreated { .. } => "response.created",
            Self::ResponseDone { .. } => "response.done",
            Self::Error { .. } => "error",
            Self::Unknown { event_type, .. } => event_type.as_str(),
        }
    }
}

/// Decodes one typed Realtime server event from a JSON payload.
pub fn decode_server_event(value: &Value) -> Result<RealtimeServerEvent, OpenAIError> {
    let object = value.as_object().ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Parse,
            "failed to parse Realtime websocket event: expected a JSON object",
        )
    })?;
    let event_type = object.get("type").and_then(Value::as_str).ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Parse,
            "failed to parse Realtime websocket event: missing `type`",
        )
    })?;

    match event_type {
        "session.created" => Ok(RealtimeServerEvent::SessionCreated {
            event_id: required_string(object, "event_id")?,
            session: required_json(object, "session")?,
        }),
        "session.updated" => Ok(RealtimeServerEvent::SessionUpdated {
            event_id: required_string(object, "event_id")?,
            session: required_json(object, "session")?,
        }),
        "conversation.item.created" => Ok(RealtimeServerEvent::ConversationItemCreated {
            event_id: required_string(object, "event_id")?,
            previous_item_id: optional_string(object, "previous_item_id"),
            item: required_json(object, "item")?,
        }),
        "input_audio_buffer.committed" => Ok(RealtimeServerEvent::InputAudioBufferCommitted {
            event_id: required_string(object, "event_id")?,
            item_id: required_string(object, "item_id")?,
            previous_item_id: optional_string(object, "previous_item_id"),
        }),
        "input_audio_buffer.speech_started" => {
            Ok(RealtimeServerEvent::InputAudioBufferSpeechStarted {
                event_id: required_string(object, "event_id")?,
                item_id: required_string(object, "item_id")?,
                audio_start_ms: required_u64(object, "audio_start_ms")?,
            })
        }
        "input_audio_buffer.speech_stopped" => {
            Ok(RealtimeServerEvent::InputAudioBufferSpeechStopped {
                event_id: required_string(object, "event_id")?,
                item_id: required_string(object, "item_id")?,
                audio_end_ms: required_u64(object, "audio_end_ms")?,
            })
        }
        "input_audio_buffer.cleared" => Ok(RealtimeServerEvent::InputAudioBufferCleared {
            event_id: required_string(object, "event_id")?,
        }),
        "conversation.item.truncated" => Ok(RealtimeServerEvent::ConversationItemTruncated {
            event_id: required_string(object, "event_id")?,
            item_id: required_string(object, "item_id")?,
            content_index: required_usize(object, "content_index")?,
            audio_end_ms: required_u64(object, "audio_end_ms")?,
        }),
        "response.output_text.delta" => Ok(RealtimeServerEvent::OutputTextDelta {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            delta: required_string(object, "delta")?,
        }),
        "response.output_text.done" => Ok(RealtimeServerEvent::OutputTextDone {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            text: required_string(object, "text")?,
        }),
        "response.output_item.added" => {
            let item: RealtimeConversationItem = required_json(object, "item")?;
            Ok(RealtimeServerEvent::ResponseOutputItemAdded {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: response_output_item_id(object, &item)?,
                output_index: required_usize(object, "output_index")?,
                item,
            })
        }
        "response.output_item.done" => {
            let item: RealtimeConversationItem = required_json(object, "item")?;
            Ok(RealtimeServerEvent::ResponseOutputItemDone {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: response_output_item_id(object, &item)?,
                output_index: required_usize(object, "output_index")?,
                item,
            })
        }
        "response.content_part.added" => Ok(RealtimeServerEvent::ResponseContentPartAdded {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            part: required_json(object, "part")?,
        }),
        "response.content_part.done" => Ok(RealtimeServerEvent::ResponseContentPartDone {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            part: required_json(object, "part")?,
        }),
        "response.output_audio.delta" => Ok(RealtimeServerEvent::OutputAudioDelta {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
            delta: required_string(object, "delta")?,
        }),
        "response.output_audio.done" => Ok(RealtimeServerEvent::OutputAudioDone {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            content_index: required_usize(object, "content_index")?,
        }),
        "response.output_audio_transcript.delta" => {
            Ok(RealtimeServerEvent::OutputAudioTranscriptDelta {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: required_string(object, "item_id")?,
                output_index: required_usize(object, "output_index")?,
                content_index: required_usize(object, "content_index")?,
                delta: required_string(object, "delta")?,
            })
        }
        "response.output_audio_transcript.done" => {
            Ok(RealtimeServerEvent::OutputAudioTranscriptDone {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: required_string(object, "item_id")?,
                output_index: required_usize(object, "output_index")?,
                content_index: required_usize(object, "content_index")?,
                transcript: required_string(object, "transcript")?,
            })
        }
        "response.function_call_arguments.delta" => {
            Ok(RealtimeServerEvent::FunctionCallArgumentsDelta {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: required_string(object, "item_id")?,
                output_index: required_usize(object, "output_index")?,
                delta: required_string(object, "delta")?,
            })
        }
        "response.function_call_arguments.done" => {
            Ok(RealtimeServerEvent::FunctionCallArgumentsDone {
                event_id: required_string(object, "event_id")?,
                response_id: required_string(object, "response_id")?,
                item_id: required_string(object, "item_id")?,
                output_index: required_usize(object, "output_index")?,
                arguments: required_string(object, "arguments")?,
                name: optional_string(object, "name"),
            })
        }
        "response.mcp_call_arguments.delta" => Ok(RealtimeServerEvent::McpCallArgumentsDelta {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            delta: required_string(object, "delta")?,
            obfuscation: optional_string(object, "obfuscation"),
        }),
        "response.mcp_call_arguments.done" => Ok(RealtimeServerEvent::McpCallArgumentsDone {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
            arguments: required_string(object, "arguments")?,
        }),
        "response.mcp_call.in_progress"
        | "response.mcp_call.completed"
        | "response.mcp_call.failed" => Ok(RealtimeServerEvent::ResponseItemStatus {
            event_id: required_string(object, "event_id")?,
            event_type: event_type.to_string(),
            item_id: required_string(object, "item_id")?,
            output_index: required_usize(object, "output_index")?,
        }),
        "output_audio_buffer.started" => Ok(RealtimeServerEvent::OutputAudioBufferStarted {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
        }),
        "output_audio_buffer.stopped" => Ok(RealtimeServerEvent::OutputAudioBufferStopped {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
        }),
        "output_audio_buffer.cleared" => Ok(RealtimeServerEvent::OutputAudioBufferCleared {
            event_id: required_string(object, "event_id")?,
            response_id: required_string(object, "response_id")?,
        }),
        "response.created" => Ok(RealtimeServerEvent::ResponseCreated {
            event_id: required_string(object, "event_id")?,
            response: object.get("response").cloned().unwrap_or(Value::Null),
        }),
        "response.done" => Ok(RealtimeServerEvent::ResponseDone {
            event_id: required_string(object, "event_id")?,
            response: object.get("response").cloned().unwrap_or(Value::Null),
        }),
        "error" => Ok(RealtimeServerEvent::Error {
            event_id: required_string(object, "event_id")?,
            error: required_json(object, "error")?,
        }),
        _ => Ok(RealtimeServerEvent::Unknown {
            event_id: optional_string(object, "event_id"),
            event_type: event_type.to_string(),
            raw: value.clone(),
        }),
    }
}

/// Parses and decodes one typed Realtime server event from text.
pub fn decode_server_event_text(text: &str) -> Result<RealtimeServerEvent, OpenAIError> {
    let value = serde_json::from_str::<Value>(text).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Realtime websocket event JSON: {error}"),
        )
        .with_source(error)
    })?;
    decode_server_event(&value)
}

fn required_json<T>(object: &Map<String, Value>, key: &str) -> Result<T, OpenAIError>
where
    T: for<'de> Deserialize<'de>,
{
    let value = object.get(key).cloned().ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Realtime websocket event: missing `{key}`"),
        )
    })?;
    serde_json::from_value(value).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Realtime websocket event field `{key}`: {error}"),
        )
        .with_source(error)
    })
}

fn response_output_item_id(
    object: &Map<String, Value>,
    item: &RealtimeConversationItem,
) -> Result<String, OpenAIError> {
    optional_string(object, "item_id")
        .or_else(|| item.id.clone())
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                "failed to parse Realtime websocket event: missing `item_id`",
            )
        })
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String, OpenAIError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to parse Realtime websocket event: missing `{key}`"),
            )
        })
}

fn optional_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_string)
}

fn required_usize(object: &Map<String, Value>, key: &str) -> Result<usize, OpenAIError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("failed to parse Realtime websocket event: missing `{key}`"),
            )
        })
}

fn required_u64(object: &Map<String, Value>, key: &str) -> Result<u64, OpenAIError> {
    object.get(key).and_then(Value::as_u64).ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse Realtime websocket event: missing `{key}`"),
        )
    })
}
