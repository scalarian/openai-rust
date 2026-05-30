//! Realtime scaffolding kept separate from REST transport.

pub mod client;
pub mod events;
pub mod state;

pub use client::{
    Calls as RealtimeCalls, PreparedRealtimeWsTarget, Realtime, RealtimeAuth,
    RealtimeCallAcceptParams, RealtimeCallCreateParams, RealtimeCallReferParams,
    RealtimeCallRejectParams, RealtimeClientSecret, RealtimeClientSecretCreateParams,
    RealtimeClientSecretCreateResponse, RealtimeConnectOptions, RealtimeConnection,
    RealtimeConversationItemResource, RealtimeConversationResource, RealtimeEventHandlerId,
    RealtimeInputAudioBufferResource, RealtimeOutputAudioBufferResource, RealtimeResponseResource,
    RealtimeSessionResource, RealtimeSessionTTL, RealtimeSessionTTLAnchor,
};
pub use events::{
    RealtimeClientEvent, RealtimeConversationContentType, RealtimeConversationItem,
    RealtimeConversationItemRole, RealtimeConversationItemStatus, RealtimeConversationItemType,
    RealtimeConversationMessageContentPart, RealtimeErrorInfo, RealtimeInclude,
    RealtimeMaxOutputTokens, RealtimeNullable, RealtimeOutputModality, RealtimeReasoning,
    RealtimeReasoningEffort, RealtimeResponseCreateParams, RealtimeServerEvent,
    RealtimeSessionConfig, RealtimeSessionType, RealtimeToolChoice, RealtimeToolChoiceFunction,
    RealtimeToolChoiceMcp, RealtimeToolChoiceOther, RealtimeTracing, RealtimeTracingConfiguration,
    RealtimeTruncation, RealtimeTruncationRetentionRatio, RealtimeTruncationRetentionRatioType,
    RealtimeTruncationTokenLimits, decode_server_event, decode_server_event_text,
};
pub use state::{RealtimeAudioBufferState, RealtimeEventState, RealtimeResponseState};
