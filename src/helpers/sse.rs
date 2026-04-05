/// Shared SSE helper placeholder.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SseFrame {
    /// Optional event name.
    pub event: Option<String>,
    /// Event data payload.
    pub data: String,
}
