/// Shared cursor pagination scaffold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CursorPage<T> {
    /// Items on the current page.
    pub data: Vec<T>,
    /// Whether additional pages exist.
    pub has_more: bool,
}
