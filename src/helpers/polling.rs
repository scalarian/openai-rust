use std::time::Duration;

/// Shared polling helper scaffold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PollingConfig {
    /// Delay between polling attempts.
    pub interval: Duration,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(1),
        }
    }
}
