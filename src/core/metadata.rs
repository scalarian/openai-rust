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

impl ResponseMetadata {
    /// Returns the HTTP status code for the response.
    pub fn status_code(&self) -> u16 {
        self.status_code
    }

    /// Returns all normalized response headers.
    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    /// Returns a normalized response header value when present.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Returns the captured request identifier when present.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}
