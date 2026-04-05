use std::{collections::BTreeMap, error::Error, fmt};

/// Top-level shared runtime error.
#[derive(Debug)]
pub struct OpenAIError {
    /// Error classification.
    pub kind: ErrorKind,
    /// Human-readable message.
    pub message: String,
    response: Option<Box<ErrorResponseContext>>,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl Clone for OpenAIError {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            message: self.message.clone(),
            response: self.response.clone(),
            source: None,
        }
    }
}

impl OpenAIError {
    /// Creates a new error with the given classification.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            response: None,
            source: None,
        }
    }

    /// Attaches response metadata to the error.
    pub fn with_response_metadata(
        mut self,
        status_code: u16,
        headers: BTreeMap<String, String>,
        request_id: Option<String>,
    ) -> Self {
        let mut response = self
            .response
            .take()
            .unwrap_or_else(|| Box::new(ErrorResponseContext::default()));
        response.status_code = Some(status_code);
        response.headers = headers;
        response.request_id = request_id;
        self.response = Some(response);
        self
    }

    /// Attaches a parsed API error payload.
    pub fn with_api_error(mut self, api_error: ApiErrorPayload) -> Self {
        let mut response = self
            .response
            .take()
            .unwrap_or_else(|| Box::new(ErrorResponseContext::default()));
        response.api_error = Some(api_error);
        self.response = Some(response);
        self
    }

    /// Attaches a source error.
    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }

    /// Returns the HTTP status code when available.
    pub fn status_code(&self) -> Option<u16> {
        self.response
            .as_ref()
            .and_then(|response| response.status_code)
    }

    /// Returns the surfaced request id when available.
    pub fn request_id(&self) -> Option<&str> {
        self.response
            .as_ref()
            .and_then(|response| response.request_id.as_deref())
    }

    /// Returns a lower-cased response header by name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.response
            .as_ref()
            .and_then(|response| response.headers.get(&name.to_ascii_lowercase()))
            .map(String::as_str)
    }

    /// Returns the parsed API error payload when available.
    pub fn api_error(&self) -> Option<&ApiErrorPayload> {
        self.response
            .as_ref()
            .and_then(|response| response.api_error.as_ref())
    }

    /// Returns the underlying source error when available.
    pub fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[derive(Clone, Debug, Default)]
struct ErrorResponseContext {
    status_code: Option<u16>,
    headers: BTreeMap<String, String>,
    request_id: Option<String>,
    api_error: Option<ApiErrorPayload>,
}

impl fmt::Display for OpenAIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for OpenAIError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source()
    }
}

/// Shared top-level error classifications.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// Client-side configuration or validation failure.
    Configuration,
    /// Client-side request validation failure.
    Validation,
    /// Transport-level failure.
    Transport,
    /// API-status failure.
    Api(ApiErrorKind),
    /// Response parsing or validation failure.
    Parse,
    /// Timeout failure.
    Timeout,
}

/// Typed API-status classifications.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiErrorKind {
    BadRequest,
    Authentication,
    PermissionDenied,
    NotFound,
    Conflict,
    UnprocessableEntity,
    RateLimit,
    Server,
    Other(u16),
}

/// Parsed API error payload from the platform response body.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct ApiErrorPayload {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub code: Option<String>,
    pub param: Option<String>,
}
