use std::collections::BTreeMap;

/// Shared response metadata captured from a successful HTTP response.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResponseMetadata {
    /// HTTP status code.
    pub status_code: u16,
    /// Lower-cased response headers.
    pub headers: BTreeMap<String, String>,
    /// Optional request identifier captured from response headers.
    pub request_id: Option<String>,
}
