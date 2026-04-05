/// Shared response metadata scaffold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResponseMetadata {
    /// Optional request identifier captured from response headers.
    pub request_id: Option<String>,
}
