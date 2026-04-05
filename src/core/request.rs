use std::time::Duration;

/// Shared per-request options scaffold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RequestOptions {
    /// Optional per-request timeout override.
    pub timeout: Option<Duration>,
}
