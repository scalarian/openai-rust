//! Realtime scaffolding kept separate from REST transport.

pub mod client;
pub mod events;
pub mod state;

pub use crate::resources::responses::ResponsePrompt;
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
    RealtimeConversationMessageContentPart, RealtimeErrorInfo, RealtimeFunctionTool,
    RealtimeInclude, RealtimeMaxOutputTokens, RealtimeMcpAllowedTools, RealtimeMcpConnectorId,
    RealtimeMcpRequireApproval, RealtimeMcpRequireApprovalFilter, RealtimeMcpTool,
    RealtimeMcpToolFilter, RealtimeNullable, RealtimeOtherTool, RealtimeOutputModality,
    RealtimeReasoning, RealtimeReasoningEffort, RealtimeResponseCreateParams, RealtimeServerEvent,
    RealtimeSessionConfig, RealtimeSessionType, RealtimeTool, RealtimeToolChoice,
    RealtimeToolChoiceFunction, RealtimeToolChoiceMcp, RealtimeToolChoiceOther,
    RealtimeToolsConfig, RealtimeTracing, RealtimeTracingConfiguration, RealtimeTruncation,
    RealtimeTruncationRetentionRatio, RealtimeTruncationRetentionRatioType,
    RealtimeTruncationTokenLimits, decode_server_event, decode_server_event_text,
};
pub use state::{RealtimeAudioBufferState, RealtimeEventState, RealtimeResponseState};
