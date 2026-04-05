use std::{collections::BTreeMap, time::Duration};

/// Shared per-request options scaffold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RequestOptions {
    /// Optional per-request timeout override.
    pub timeout: Option<Duration>,
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
}
