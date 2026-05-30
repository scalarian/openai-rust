use serde::{Deserialize, Serialize};

/// Shared reasoning-effort literal used by chat, responses, evals, graders, and Assistants.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReasoningEffort {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    XHigh,
}

impl ReasoningEffort {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
        }
    }
}

impl AsRef<str> for ReasoningEffort {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Prompt-cache retention policy literal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PromptCacheRetention {
    #[serde(rename = "in_memory")]
    InMemory,
    #[serde(rename = "24h")]
    TwentyFourHours,
}

impl PromptCacheRetention {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InMemory => "in_memory",
            Self::TwentyFourHours => "24h",
        }
    }
}

impl AsRef<str> for PromptCacheRetention {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Request/response service tier literal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTier {
    Auto,
    Default,
    Flex,
    Scale,
    Priority,
}

impl ServiceTier {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Default => "default",
            Self::Flex => "flex",
            Self::Scale => "scale",
            Self::Priority => "priority",
        }
    }
}

impl AsRef<str> for ServiceTier {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Model response truncation strategy literal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Truncation {
    Auto,
    Disabled,
}

impl Truncation {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Disabled => "disabled",
        }
    }
}

impl AsRef<str> for Truncation {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Text verbosity literal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    Low,
    Medium,
    High,
}

impl Verbosity {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl AsRef<str> for Verbosity {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Reasoning-summary style literal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningSummary {
    Auto,
    Concise,
    Detailed,
}

impl ReasoningSummary {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Concise => "concise",
            Self::Detailed => "detailed",
        }
    }
}

impl AsRef<str> for ReasoningSummary {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Web-search context-size literal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchContextSize {
    Low,
    Medium,
    High,
}

impl SearchContextSize {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl AsRef<str> for SearchContextSize {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
