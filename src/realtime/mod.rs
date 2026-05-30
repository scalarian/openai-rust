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
    RealtimeAudioCompactFormat, RealtimeAudioConfig, RealtimeAudioFormat, RealtimeAudioInputConfig,
    RealtimeAudioInputTurnDetection, RealtimeAudioOutputConfig, RealtimeAudioPcmFormat,
    RealtimeAudioTranscription, RealtimeAudioTranscriptionDelay, RealtimeClientEvent,
    RealtimeConversationContentType, RealtimeConversationItem, RealtimeConversationItemRole,
    RealtimeConversationItemStatus, RealtimeConversationItemType,
    RealtimeConversationMessageContentPart, RealtimeErrorInfo, RealtimeFunctionTool,
    RealtimeInclude, RealtimeMaxOutputTokens, RealtimeMcpAllowedTools, RealtimeMcpConnectorId,
    RealtimeMcpRequireApproval, RealtimeMcpRequireApprovalFilter, RealtimeMcpTool,
    RealtimeMcpToolFilter, RealtimeNoiseReduction, RealtimeNoiseReductionType, RealtimeNullable,
    RealtimeOtherAudioFormat, RealtimeOtherTool, RealtimeOtherTurnDetection,
    RealtimeOutputModality, RealtimeReasoning, RealtimeReasoningEffort,
    RealtimeResponseAudioConfig, RealtimeResponseAudioOutputConfig, RealtimeResponseCreateParams,
    RealtimeSemanticVadEagerness, RealtimeSemanticVadTurnDetection, RealtimeServerEvent,
    RealtimeServerVadTurnDetection, RealtimeSessionConfig, RealtimeSessionType, RealtimeTool,
    RealtimeToolChoice, RealtimeToolChoiceFunction, RealtimeToolChoiceMcp, RealtimeToolChoiceOther,
    RealtimeToolsConfig, RealtimeTracing, RealtimeTracingConfiguration, RealtimeTruncation,
    RealtimeTruncationRetentionRatio, RealtimeTruncationRetentionRatioType,
    RealtimeTruncationTokenLimits, RealtimeVoice, RealtimeVoiceId, RealtimeVoiceName,
    decode_server_event, decode_server_event_text,
};
pub use state::{RealtimeAudioBufferState, RealtimeEventState, RealtimeResponseState};
