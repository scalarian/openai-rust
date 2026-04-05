use crate::core::metadata::ResponseMetadata;

/// Wrapper that pairs parsed output with response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiResponse<T> {
    /// Parsed output value.
    pub output: T,
    /// Captured response metadata.
    pub metadata: ResponseMetadata,
}

impl<T> ApiResponse<T> {
    /// Returns the parsed output value by reference.
    pub fn output(&self) -> &T {
        &self.output
    }

    /// Returns the captured response metadata by reference.
    pub fn metadata(&self) -> &ResponseMetadata {
        &self.metadata
    }

    /// Returns the HTTP status code for the response.
    pub fn status_code(&self) -> u16 {
        self.metadata.status_code()
    }

    /// Returns a normalized response header value when present.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.metadata.header(name)
    }

    /// Returns the captured request identifier when present.
    pub fn request_id(&self) -> Option<&str> {
        self.metadata.request_id()
    }

    /// Splits the parsed output from the captured metadata.
    pub fn into_parts(self) -> (ResponseMetadata, T) {
        (self.metadata, self.output)
    }
}
