/// Shared immutable client configuration scaffold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientConfig {
    /// Optional API key placeholder.
    pub api_key: Option<String>,
    /// Optional base URL override placeholder.
    pub base_url: Option<String>,
    /// Optional organization header placeholder.
    pub organization: Option<String>,
    /// Optional project header placeholder.
    pub project: Option<String>,
    /// Optional user-agent override placeholder.
    pub user_agent: Option<String>,
}
