/// Shared retry policy scaffold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    /// Maximum retry attempts after the first try.
    pub max_retries: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_retries: 2 }
    }
}
