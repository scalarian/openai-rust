#![allow(dead_code)]

/// One SSE event in a transcript.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SseEvent {
    event: Option<String>,
    data: Vec<String>,
}

impl SseEvent {
    /// Creates a named event.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            event: Some(name.into()),
            data: Vec::new(),
        }
    }

    /// Appends one logical data payload line.
    pub fn json(mut self, payload: impl Into<String>) -> Self {
        self.data.push(payload.into());
        self
    }
}

/// Encoded SSE transcript helper.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SseTranscript {
    events: Vec<SseEvent>,
}

impl SseTranscript {
    /// Builds a transcript from ordered events.
    pub fn from_events(events: Vec<SseEvent>) -> Self {
        Self { events }
    }

    /// Encodes the transcript into canonical SSE text.
    pub fn encode(&self) -> String {
        let mut encoded = String::new();
        for event in &self.events {
            if let Some(name) = &event.event {
                encoded.push_str("event: ");
                encoded.push_str(name);
                encoded.push('\n');
            }
            for line in &event.data {
                encoded.push_str("data: ");
                encoded.push_str(line);
                encoded.push('\n');
            }
            encoded.push('\n');
        }
        encoded
    }

    /// Fragments the encoded transcript into deterministic chunks.
    pub fn fragment(&self, chunk_sizes: &[usize]) -> Vec<Vec<u8>> {
        let encoded = self.encode().into_bytes();
        if chunk_sizes.is_empty() {
            return vec![encoded];
        }
        let mut fragments = Vec::new();
        let mut cursor = 0usize;
        let mut index = 0usize;
        while cursor < encoded.len() {
            let size = chunk_sizes[index % chunk_sizes.len()].max(1);
            let next = (cursor + size).min(encoded.len());
            fragments.push(encoded[cursor..next].to_vec());
            cursor = next;
            index += 1;
        }
        fragments
    }
}
