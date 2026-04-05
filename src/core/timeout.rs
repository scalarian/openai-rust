use std::time::Duration;

/// Shared timeout policy scaffold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeoutPolicy {
    /// Default request timeout budget.
    pub request_timeout: Duration,
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(600),
        }
    }
}
