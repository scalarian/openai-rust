use std::{collections::BTreeMap, time::Duration};

/// Shared per-request options scaffold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RequestOptions {
    /// Optional per-request timeout override.
    pub timeout: Option<Duration>,
    /// Optional per-request retry-budget override.
    pub max_retries: Option<u32>,
}

/// Effective request options after applying client defaults and overrides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRequestOptions {
    pub timeout: Duration,
    pub max_retries: u32,
}

/// Prepared request parts emitted by the shared client core before transport.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PreparedRequest {
    /// Uppercase HTTP method.
    pub method: String,
    /// Fully-resolved request URL.
    pub url: String,
    /// Lower-cased request headers.
    pub headers: BTreeMap<String, String>,
    /// Optional raw request body.
    pub body: Option<Vec<u8>>,
}
