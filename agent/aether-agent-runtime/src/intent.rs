// Agent Runtime - Intent model
//
// Structured intent representation. The LLM may propose intents, but
// risk levels are validated by trusted system code — never by the LLM
// directly assigning unrestricted privileges.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique intent identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntentId(Uuid);

impl IntentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for IntentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for IntentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Types of intent the agent can recognize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentType {
    // Application intents
    ApplicationLaunch,
    ApplicationClose,
    ApplicationStatus,
    ApplicationList,

    // Window intents
    WindowList,
    WindowFocus,
    WindowMinimize,
    WindowMaximize,
    WindowClose,

    // Filesystem intents
    FileList,
    FileRead,
    FileCreate,
    FileWrite,
    FileSearch,
    FileRename,
    FileMove,
    FileDelete,

    // Process intents
    ProcessList,
    ProcessInspect,

    // Network intents
    NetworkStatus,
    NetworkInterfaces,

    // System intents
    SystemStatus,
    SystemInfo,
    SystemResources,
    SystemUptime,

    // Storage intents
    StorageStatus,

    // Context intents
    ContextGet,

    // Chat (no structured action needed)
    Chat,
}

impl IntentType {
    /// Parse a string slug like "application.launch" into an `IntentType`.
    /// Used to validate LLM-produced structured output.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "application.launch" => Some(Self::ApplicationLaunch),
            "application.close" => Some(Self::ApplicationClose),
            "application.status" => Some(Self::ApplicationStatus),
            "application.list" => Some(Self::ApplicationList),
            "window.list" => Some(Self::WindowList),
            "window.focus" => Some(Self::WindowFocus),
            "window.minimize" => Some(Self::WindowMinimize),
            "window.maximize" => Some(Self::WindowMaximize),
            "window.close" => Some(Self::WindowClose),
            "file.list" => Some(Self::FileList),
            "file.read" => Some(Self::FileRead),
            "file.create" => Some(Self::FileCreate),
            "file.write" => Some(Self::FileWrite),
            "file.search" => Some(Self::FileSearch),
            "file.rename" => Some(Self::FileRename),
            "file.move" => Some(Self::FileMove),
            "file.delete" => Some(Self::FileDelete),
            "process.list" => Some(Self::ProcessList),
            "process.inspect" => Some(Self::ProcessInspect),
            "network.status" => Some(Self::NetworkStatus),
            "network.interfaces" => Some(Self::NetworkInterfaces),
            "system.status" => Some(Self::SystemStatus),
            "system.info" => Some(Self::SystemInfo),
            "system.resources" => Some(Self::SystemResources),
            "system.uptime" => Some(Self::SystemUptime),
            "storage.status" => Some(Self::StorageStatus),
            "context.get" => Some(Self::ContextGet),
            "chat" => Some(Self::Chat),
            _ => None,
        }
    }

    /// All valid slugs. Used to build LLM prompts and to validate responses.
    pub fn all_slugs() -> &'static [&'static str] {
        &[
            "application.launch",
            "application.close",
            "application.status",
            "application.list",
            "window.list",
            "window.focus",
            "window.minimize",
            "window.maximize",
            "window.close",
            "file.list",
            "file.read",
            "file.create",
            "file.write",
            "file.search",
            "file.rename",
            "file.move",
            "file.delete",
            "process.list",
            "process.inspect",
            "network.status",
            "network.interfaces",
            "system.status",
            "system.info",
            "system.resources",
            "system.uptime",
            "storage.status",
            "context.get",
            "chat",
        ]
    }
}

impl fmt::Display for IntentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::ApplicationLaunch => "application.launch",
            Self::ApplicationClose => "application.close",
            Self::ApplicationStatus => "application.status",
            Self::ApplicationList => "application.list",
            Self::WindowList => "window.list",
            Self::WindowFocus => "window.focus",
            Self::WindowMinimize => "window.minimize",
            Self::WindowMaximize => "window.maximize",
            Self::WindowClose => "window.close",
            Self::FileList => "file.list",
            Self::FileRead => "file.read",
            Self::FileCreate => "file.create",
            Self::FileWrite => "file.write",
            Self::FileSearch => "file.search",
            Self::FileRename => "file.rename",
            Self::FileMove => "file.move",
            Self::FileDelete => "file.delete",
            Self::ProcessList => "process.list",
            Self::ProcessInspect => "process.inspect",
            Self::NetworkStatus => "network.status",
            Self::NetworkInterfaces => "network.interfaces",
            Self::SystemStatus => "system.status",
            Self::SystemInfo => "system.info",
            Self::SystemResources => "system.resources",
            Self::SystemUptime => "system.uptime",
            Self::StorageStatus => "storage.status",
            Self::ContextGet => "context.get",
            Self::Chat => "chat",
        };
        write!(f, "{s}")
    }
}

/// Confidence level for intent classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Confidence(pub u8);

impl Confidence {
    pub const LOW: Self = Self(25);
    pub const MEDIUM: Self = Self(50);
    pub const HIGH: Self = Self(75);
    pub const CERTAIN: Self = Self(100);

    pub fn as_f32(&self) -> f32 {
        self.0 as f32 / 100.0
    }
}

/// A structured intent extracted from user input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub id: IntentId,
    pub request_id: String,
    pub intent_type: IntentType,
    pub confidence: Confidence,
    pub entities: serde_json::Value,
    pub constraints: Vec<String>,
    /// Risk level assigned by trusted system code, NOT by the LLM.
    pub risk_level: RiskLevel,
    /// LLM-supplied reason explaining the classification (or empty for
    /// deterministic sources).
    pub reason: String,
}

/// Risk level for intents — validated by system code, never assigned by LLM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl Intent {
    pub fn new(
        request_id: &str,
        intent_type: IntentType,
        confidence: Confidence,
        entities: serde_json::Value,
    ) -> Self {
        Self {
            id: IntentId::new(),
            request_id: request_id.to_string(),
            intent_type,
            confidence,
            entities,
            constraints: Vec::new(),
            risk_level: RiskLevel::Low,
            reason: String::new(),
        }
    }

    /// Sets the risk level — called by trusted validation code.
    pub fn with_risk_level(mut self, level: RiskLevel) -> Self {
        self.risk_level = level;
        self
    }

    /// Sets the LLM-supplied reason.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    /// Validates the intent structure.
    pub fn validate(&self) -> Result<(), String> {
        if self.request_id.is_empty() {
            return Err("Intent must reference a request ID".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_creation() {
        let i = Intent::new(
            "req-1",
            IntentType::ApplicationLaunch,
            Confidence::HIGH,
            serde_json::json!({"app": "calculator"}),
        );
        assert_eq!(i.intent_type, IntentType::ApplicationLaunch);
        assert_eq!(i.confidence, Confidence::HIGH);
        assert_eq!(i.risk_level, RiskLevel::Low);
    }

    #[test]
    fn confidence_ordering() {
        assert!(Confidence::LOW < Confidence::MEDIUM);
        assert!(Confidence::MEDIUM < Confidence::HIGH);
        assert!(Confidence::HIGH < Confidence::CERTAIN);
    }

    #[test]
    fn risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[test]
    fn intent_validate_rejects_empty_request_id() {
        let i = Intent::new("", IntentType::Chat, Confidence::LOW, serde_json::json!({}));
        assert!(i.validate().is_err());
    }

    #[test]
    fn intent_with_risk_level() {
        let i = Intent::new("r", IntentType::FileDelete, Confidence::HIGH, serde_json::json!({}))
            .with_risk_level(RiskLevel::High);
        assert_eq!(i.risk_level, RiskLevel::High);
    }

    #[test]
    fn intent_type_display() {
        assert_eq!(IntentType::ApplicationLaunch.to_string(), "application.launch");
        assert_eq!(IntentType::FileDelete.to_string(), "file.delete");
        assert_eq!(IntentType::Chat.to_string(), "chat");
    }
}
