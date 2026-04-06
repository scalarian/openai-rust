use serde_json::Value;

use crate::{OpenAIError, error::ErrorKind};

use super::events::{
    RealtimeConversationItem, RealtimeConversationMessageContentPart, RealtimeServerEvent,
    RealtimeSessionConfig,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RealtimeAudioBufferState {
    pub committed_item_id: Option<String>,
    pub previous_item_id: Option<String>,
    pub speech_started_ms: Option<u64>,
    pub speech_stopped_ms: Option<u64>,
    pub cleared: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealtimeResponseState {
    pub response: Value,
    pub output: Vec<RealtimeConversationItem>,
    output_text: String,
}

impl RealtimeResponseState {
    pub fn output_text(&self) -> &str {
        &self.output_text
    }

    fn from_value(response: Value) -> Result<Self, OpenAIError> {
        let output = response
            .get("output")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![]));
        let output: Vec<RealtimeConversationItem> =
            serde_json::from_value(output).map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Parse,
                    format!("failed to parse Realtime response output: {error}"),
                )
                .with_source(error)
            })?;
        let output_text = aggregate_output_text(&output);
        Ok(Self {
            response,
            output,
            output_text,
        })
    }

    fn sync_response(&mut self) {
        let object = self
            .response
            .as_object_mut()
            .expect("realtime response snapshots must be JSON objects");
        object.insert(
            String::from("output"),
            serde_json::to_value(&self.output).expect("realtime output should serialize"),
        );
        self.output_text = aggregate_output_text(&self.output);
    }
}

#[derive(Clone, Debug, Default)]
pub struct RealtimeEventState {
    session: Option<RealtimeSessionConfig>,
    conversation: Vec<RealtimeConversationItem>,
    audio_buffer: RealtimeAudioBufferState,
    current_response: Option<RealtimeResponseState>,
    terminal_response: Option<RealtimeResponseState>,
}

impl RealtimeEventState {
    pub fn session(&self) -> Option<&RealtimeSessionConfig> {
        self.session.as_ref()
    }

    pub fn conversation_items(&self) -> &[RealtimeConversationItem] {
        &self.conversation
    }

    pub fn conversation_item(&self, item_id: &str) -> Option<&RealtimeConversationItem> {
        self.conversation
            .iter()
            .find(|item| item.id.as_deref() == Some(item_id))
    }

    pub fn audio_buffer(&self) -> &RealtimeAudioBufferState {
        &self.audio_buffer
    }

    pub fn current_response(&self) -> Option<&RealtimeResponseState> {
        self.current_response.as_ref()
    }

    pub fn terminal_response(&self) -> Option<&RealtimeResponseState> {
        self.terminal_response.as_ref()
    }

    pub fn apply(&mut self, event: &RealtimeServerEvent) -> Result<(), OpenAIError> {
        match event {
            RealtimeServerEvent::SessionCreated { session, .. }
            | RealtimeServerEvent::SessionUpdated { session, .. } => {
                self.session = Some(session.clone());
            }
            RealtimeServerEvent::ConversationItemCreated {
                previous_item_id,
                item,
                ..
            } => {
                upsert_conversation_item(
                    &mut self.conversation,
                    previous_item_id.as_deref(),
                    item.clone(),
                );
            }
            RealtimeServerEvent::InputAudioBufferCommitted {
                item_id,
                previous_item_id,
                ..
            } => {
                self.audio_buffer.committed_item_id = Some(item_id.clone());
                self.audio_buffer.previous_item_id = previous_item_id.clone();
                self.audio_buffer.cleared = false;
            }
            RealtimeServerEvent::InputAudioBufferSpeechStarted {
                item_id,
                audio_start_ms,
                ..
            } => {
                self.audio_buffer.committed_item_id = Some(item_id.clone());
                self.audio_buffer.speech_started_ms = Some(*audio_start_ms);
                self.audio_buffer.cleared = false;
            }
            RealtimeServerEvent::InputAudioBufferSpeechStopped {
                item_id,
                audio_end_ms,
                ..
            } => {
                self.audio_buffer.committed_item_id = Some(item_id.clone());
                self.audio_buffer.speech_stopped_ms = Some(*audio_end_ms);
                self.audio_buffer.cleared = false;
            }
            RealtimeServerEvent::InputAudioBufferCleared { .. } => {
                self.audio_buffer.cleared = true;
            }
            RealtimeServerEvent::ConversationItemTruncated {
                item_id,
                content_index,
                audio_end_ms,
                ..
            } => {
                let item = get_conversation_item_mut(&mut self.conversation, item_id)?;
                let part = item.content.get_mut(*content_index).ok_or_else(|| {
                    OpenAIError::new(
                        ErrorKind::Validation,
                        format!(
                            "conversation item `{item_id}` missing content_index {content_index}"
                        ),
                    )
                })?;
                part.extra
                    .insert(String::from("audio_end_ms"), Value::from(*audio_end_ms));
                if part.transcript.is_some() || part.part_type == "audio" {
                    part.transcript = Some(String::new());
                }
            }
            RealtimeServerEvent::ResponseCreated { response, .. } => {
                self.current_response = Some(RealtimeResponseState::from_value(response.clone())?);
            }
            RealtimeServerEvent::ResponseDone { response, .. } => {
                let snapshot = RealtimeResponseState::from_value(response.clone())?;
                self.current_response = Some(snapshot.clone());
                self.terminal_response = Some(snapshot);
            }
            RealtimeServerEvent::ResponseOutputItemAdded {
                output_index, item, ..
            } => {
                let response = self.current_response_mut()?;
                if *output_index > response.output.len() {
                    return Err(OpenAIError::new(
                        ErrorKind::Validation,
                        format!("realtime response referenced missing output_index {output_index}"),
                    ));
                }
                response.output.insert(*output_index, item.clone());
                response.sync_response();
            }
            RealtimeServerEvent::ResponseOutputItemDone {
                output_index, item, ..
            } => {
                let response = self.current_response_mut()?;
                let output = response.output.get_mut(*output_index).ok_or_else(|| {
                    OpenAIError::new(
                        ErrorKind::Validation,
                        format!("realtime response referenced missing output_index {output_index}"),
                    )
                })?;
                *output = item.clone();
                response.sync_response();
            }
            RealtimeServerEvent::ResponseContentPartAdded {
                output_index,
                content_index,
                part,
                ..
            } => {
                let item = self.response_item_mut(*output_index)?;
                if *content_index > item.content.len() {
                    return Err(OpenAIError::new(
                        ErrorKind::Validation,
                        format!(
                            "realtime response referenced missing content_index {content_index}"
                        ),
                    ));
                }
                item.content.insert(*content_index, part.clone());
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::ResponseContentPartDone {
                output_index,
                content_index,
                part,
                ..
            } => {
                let item = self.response_item_mut(*output_index)?;
                let content = item.content.get_mut(*content_index).ok_or_else(|| {
                    OpenAIError::new(
                        ErrorKind::Validation,
                        format!(
                            "realtime response referenced missing content_index {content_index}"
                        ),
                    )
                })?;
                *content = part.clone();
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::OutputTextDelta {
                output_index,
                content_index,
                delta,
                ..
            } => {
                let part = self.response_content_mut(*output_index, *content_index)?;
                part.text.get_or_insert_with(String::new).push_str(delta);
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::OutputTextDone {
                output_index,
                content_index,
                text,
                ..
            } => {
                let part = self.response_content_mut(*output_index, *content_index)?;
                part.text = Some(text.clone());
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::OutputAudioDelta {
                output_index,
                content_index,
                delta,
                ..
            } => {
                let part = self.response_content_mut(*output_index, *content_index)?;
                part.audio.get_or_insert_with(String::new).push_str(delta);
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::OutputAudioDone { .. } => {}
            RealtimeServerEvent::OutputAudioTranscriptDelta {
                output_index,
                content_index,
                delta,
                ..
            } => {
                let part = self.response_content_mut(*output_index, *content_index)?;
                part.transcript
                    .get_or_insert_with(String::new)
                    .push_str(delta);
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::OutputAudioTranscriptDone {
                output_index,
                content_index,
                transcript,
                ..
            } => {
                let part = self.response_content_mut(*output_index, *content_index)?;
                part.transcript = Some(transcript.clone());
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::FunctionCallArgumentsDelta {
                output_index,
                delta,
                ..
            }
            | RealtimeServerEvent::McpCallArgumentsDelta {
                output_index,
                delta,
                ..
            } => {
                let item = self.response_item_mut(*output_index)?;
                item.arguments
                    .get_or_insert_with(String::new)
                    .push_str(delta);
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::FunctionCallArgumentsDone {
                output_index,
                arguments,
                name,
                ..
            } => {
                let item = self.response_item_mut(*output_index)?;
                item.arguments = Some(arguments.clone());
                if let Some(name) = name {
                    item.name = Some(name.clone());
                }
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::McpCallArgumentsDone {
                output_index,
                arguments,
                ..
            } => {
                let item = self.response_item_mut(*output_index)?;
                item.arguments = Some(arguments.clone());
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::ResponseItemStatus {
                output_index,
                event_type,
                ..
            } => {
                let item = self.response_item_mut(*output_index)?;
                item.status = Some(status_from_event_type(event_type).to_string());
                self.current_response_mut()?.sync_response();
            }
            RealtimeServerEvent::OutputAudioBufferStarted { .. }
            | RealtimeServerEvent::OutputAudioBufferStopped { .. }
            | RealtimeServerEvent::OutputAudioBufferCleared { .. }
            | RealtimeServerEvent::Error { .. }
            | RealtimeServerEvent::Unknown { .. } => {}
        }

        Ok(())
    }

    fn current_response_mut(&mut self) -> Result<&mut RealtimeResponseState, OpenAIError> {
        self.current_response.as_mut().ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                "realtime event arrived before response.created",
            )
        })
    }

    fn response_item_mut(
        &mut self,
        output_index: usize,
    ) -> Result<&mut RealtimeConversationItem, OpenAIError> {
        self.current_response_mut()?
            .output
            .get_mut(output_index)
            .ok_or_else(|| {
                OpenAIError::new(
                    ErrorKind::Validation,
                    format!("realtime response referenced missing output_index {output_index}"),
                )
            })
    }

    fn response_content_mut(
        &mut self,
        output_index: usize,
        content_index: usize,
    ) -> Result<&mut RealtimeConversationMessageContentPart, OpenAIError> {
        self.response_item_mut(output_index)?
            .content
            .get_mut(content_index)
            .ok_or_else(|| {
                OpenAIError::new(
                    ErrorKind::Validation,
                    format!("realtime response referenced missing content_index {content_index}"),
                )
            })
    }
}

fn upsert_conversation_item(
    items: &mut Vec<RealtimeConversationItem>,
    previous_item_id: Option<&str>,
    item: RealtimeConversationItem,
) {
    if let Some(position) = item.id.as_deref().and_then(|id| {
        items
            .iter()
            .position(|existing| existing.id.as_deref() == Some(id))
    }) {
        items[position] = item;
        return;
    }

    if let Some(previous_item_id) = previous_item_id {
        if let Some(position) = items
            .iter()
            .position(|existing| existing.id.as_deref() == Some(previous_item_id))
        {
            items.insert(position + 1, item);
            return;
        }
    }

    items.push(item);
}

fn get_conversation_item_mut<'a>(
    items: &'a mut [RealtimeConversationItem],
    item_id: &str,
) -> Result<&'a mut RealtimeConversationItem, OpenAIError> {
    items
        .iter_mut()
        .find(|item| item.id.as_deref() == Some(item_id))
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("conversation item `{item_id}` was not found"),
            )
        })
}

fn aggregate_output_text(output: &[RealtimeConversationItem]) -> String {
    let mut text = String::new();
    for item in output {
        if item.item_type != "message" {
            continue;
        }
        for content in &item.content {
            if matches!(content.part_type.as_str(), "text" | "output_text") {
                if let Some(value) = &content.text {
                    text.push_str(value);
                }
            }
        }
    }
    text
}

fn status_from_event_type(event_type: &str) -> &str {
    event_type.rsplit('.').next().unwrap_or(event_type)
}
