//! Realtime scaffolding kept separate from REST transport.

pub mod client;
pub mod events;
pub mod state;

pub use client::{
    Calls as RealtimeCalls, PreparedRealtimeWsTarget, Realtime, RealtimeAuth,
    RealtimeCallAcceptParams, RealtimeCallCreateParams, RealtimeCallReferParams,
    RealtimeCallRejectParams, RealtimeClientSecret, RealtimeClientSecretCreateParams,
    RealtimeClientSecretCreateResponse, RealtimeConnectOptions, RealtimeConnection,
    RealtimeSessionTTL,
};
pub use events::{
    RealtimeClientEvent, RealtimeConversationItem, RealtimeConversationMessageContentPart,
    RealtimeErrorInfo, RealtimeOutputModality, RealtimeServerEvent, RealtimeSessionConfig,
    RealtimeSessionType, decode_server_event, decode_server_event_text,
};
pub use state::{RealtimeAudioBufferState, RealtimeEventState, RealtimeResponseState};
