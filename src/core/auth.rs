/// Authentication placeholder state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuthState {
    /// Whether the client will eventually authenticate requests.
    pub enabled: bool,
}
