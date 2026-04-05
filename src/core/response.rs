use crate::core::metadata::ResponseMetadata;

/// Wrapper that pairs parsed output with response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiResponse<T> {
    /// Parsed output value.
    pub output: T,
    /// Captured response metadata.
    pub metadata: ResponseMetadata,
}
