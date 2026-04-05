use std::{error::Error, fmt};

/// Top-level error scaffold for shared runtime work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAIError {
    /// Error classification.
    pub kind: ErrorKind,
    /// Human-readable message.
    pub message: String,
}

impl OpenAIError {
    /// Creates a new scaffold error.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for OpenAIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for OpenAIError {}

/// Shared error classifications reserved for later core features.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// Client-side configuration or validation failure.
    Configuration,
    /// Transport-level failure.
    Transport,
    /// API-status failure.
    Api,
    /// Response parsing or validation failure.
    Parse,
    /// Timeout failure.
    Timeout,
    /// Placeholder for scaffold-only stubs.
    Unimplemented,
}
