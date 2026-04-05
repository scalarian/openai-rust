use crate::{core::request::RequestOptions, error::OpenAIError};

/// Shared transport abstraction placeholder.
pub trait Transport: Send + Sync {
    /// Executes a transport operation using shared request options.
    fn execute(&self, _options: &RequestOptions) -> Result<(), OpenAIError>;
}
