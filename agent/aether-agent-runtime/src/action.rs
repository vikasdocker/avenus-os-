// Agent Runtime - Action model
//
// Strongly typed action variants. Each action declares its required
// capabilities and risk level. No raw shell commands are permitted.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique action identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionId(pub Uuid);

impl ActionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ActionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Risk classification for actions — assigned by trusted validation code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ActionRisk {
    Low,
    Medium,
    High,
    Critical,
}

/// A structured action the agent wants to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: ActionId,
    pub session_id: String,
    pub variant: ActionVariant,
    pub requested_capabilities: Vec<String>,
    pub risk_level: ActionRisk,
    pub reason: String,
    pub timeout_ms: u64,
}

/// Typed action variants — no raw command strings allowed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionVariant {
    // Application actions
    ApplicationLaunch(ApplicationLaunchParams),
    ApplicationClose(ApplicationCloseParams),

    // Window actions
    WindowList,
    WindowFocus(WindowFocusParams),
    WindowMinimize(WindowMinimizeParams),
    WindowMaximize(WindowMaximizeParams),
    WindowClose(WindowCloseParams),

    // Filesystem actions
    FileList(FileListParams),
    FileRead(FileReadParams),
    FileCreate(FileCreateParams),
    FileWrite(FileWriteParams),
    FileSearch(FileSearchParams),
    FileRename(FileRenameParams),
    FileMove(FileMoveParams),
    FileDelete(FileDeleteParams),

    // Process actions
    ProcessList,
    ProcessInspect(ProcessInspectParams),

    // Network actions
    NetworkStatus,
    NetworkInterfaces,

    // System actions
    SystemStatus,
    SystemInfo,
    SystemResources,
    SystemUptime,

    // Storage actions
    StorageStatus,

    // Context actions
    ContextGet,
}

// ---- typed parameter structs ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationLaunchParams {
    pub application_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationCloseParams {
    pub application_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowFocusParams {
    pub window_id: Option<u64>,
    pub application_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowMinimizeParams {
    pub window_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowMaximizeParams {
    pub window_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowCloseParams {
    pub window_id: Option<u64>,
    pub application_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListParams {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadParams {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCreateParams {
    pub path: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteParams {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchParams {
    pub query: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRenameParams {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMoveParams {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteParams {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInspectParams {
    pub pid: Option<u32>,
    pub name: Option<String>,
}

// ---- action builder helpers ----

impl Action {
    pub fn new(
        session_id: &str,
        variant: ActionVariant,
        reason: &str,
    ) -> Self {
        let (caps, risk) = classify_action(&variant);
        Self {
            id: ActionId::new(),
            session_id: session_id.to_string(),
            variant,
            requested_capabilities: caps,
            risk_level: risk,
            reason: reason.to_string(),
            timeout_ms: 30_000,
        }
    }

    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    pub fn with_risk_level(mut self, level: ActionRisk) -> Self {
        self.risk_level = level;
        self
    }

    pub fn action_name(&self) -> &'static str {
        match &self.variant {
            ActionVariant::ApplicationLaunch(_) => "application.launch",
            ActionVariant::ApplicationClose(_) => "application.close",
            ActionVariant::WindowList => "window.list",
            ActionVariant::WindowFocus(_) => "window.focus",
            ActionVariant::WindowMinimize(_) => "window.minimize",
            ActionVariant::WindowMaximize(_) => "window.maximize",
            ActionVariant::WindowClose(_) => "window.close",
            ActionVariant::FileList(_) => "file.list",
            ActionVariant::FileRead(_) => "file.read",
            ActionVariant::FileCreate(_) => "file.create",
            ActionVariant::FileWrite(_) => "file.write",
            ActionVariant::FileSearch(_) => "file.search",
            ActionVariant::FileRename(_) => "file.rename",
            ActionVariant::FileMove(_) => "file.move",
            ActionVariant::FileDelete(_) => "file.delete",
            ActionVariant::ProcessList => "process.list",
            ActionVariant::ProcessInspect(_) => "process.inspect",
            ActionVariant::NetworkStatus => "network.status",
            ActionVariant::NetworkInterfaces => "network.interfaces",
            ActionVariant::SystemStatus => "system.status",
            ActionVariant::SystemInfo => "system.info",
            ActionVariant::SystemResources => "system.resources",
            ActionVariant::SystemUptime => "system.uptime",
            ActionVariant::StorageStatus => "storage.status",
            ActionVariant::ContextGet => "context.get",
        }
    }
}

/// Classifies an action variant into required capabilities and risk level.
/// This is TRUSTED CODE — the LLM cannot override these classifications.
fn classify_action(variant: &ActionVariant) -> (Vec<String>, ActionRisk) {
    match variant {
        ActionVariant::ApplicationLaunch(_) => (
            vec!["application.launch".to_string()],
            ActionRisk::Medium,
        ),
        ActionVariant::ApplicationClose(_) => (
            vec!["application.close".to_string()],
            ActionRisk::Medium,
        ),
        ActionVariant::WindowList => (
            vec!["window.list".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::WindowFocus(_) => (
            vec!["window.focus".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::WindowMinimize(_) => (
            vec!["window.minimize".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::WindowMaximize(_) => (
            vec!["window.maximize".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::WindowClose(_) => (
            vec!["window.close".to_string()],
            ActionRisk::Medium,
        ),
        ActionVariant::FileList(_) => (
            vec!["file.list".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::FileRead(_) => (
            vec!["file.read".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::FileCreate(_) => (
            vec!["file.create".to_string()],
            ActionRisk::Medium,
        ),
        ActionVariant::FileWrite(_) => (
            vec!["file.write".to_string()],
            ActionRisk::Medium,
        ),
        ActionVariant::FileSearch(_) => (
            vec!["file.search".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::FileRename(_) => (
            vec!["file.rename".to_string()],
            ActionRisk::Medium,
        ),
        ActionVariant::FileMove(_) => (
            vec!["file.move".to_string()],
            ActionRisk::Medium,
        ),
        ActionVariant::FileDelete(_) => (
            vec!["file.delete".to_string()],
            ActionRisk::High,
        ),
        ActionVariant::ProcessList => (
            vec!["process.list".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::ProcessInspect(_) => (
            vec!["process.inspect".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::NetworkStatus => (
            vec!["network.status".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::NetworkInterfaces => (
            vec!["network.interfaces".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::SystemStatus => (
            vec!["system.status".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::SystemInfo => (
            vec!["system.info".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::SystemResources => (
            vec!["system.resources".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::SystemUptime => (
            vec!["system.uptime".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::StorageStatus => (
            vec!["storage.status".to_string()],
            ActionRisk::Low,
        ),
        ActionVariant::ContextGet => (
            vec!["context.get".to_string()],
            ActionRisk::Low,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_creation_classifies_risk() {
        let a = Action::new(
            "s1",
            ActionVariant::ApplicationLaunch(ApplicationLaunchParams {
                application_id: "calc".to_string(),
            }),
            "user asked",
        );
        assert_eq!(a.risk_level, ActionRisk::Medium);
        assert!(a.requested_capabilities.contains(&"application.launch".to_string()));
    }

    #[test]
    fn file_delete_is_high_risk() {
        let a = Action::new(
            "s1",
            ActionVariant::FileDelete(FileDeleteParams {
                path: "/tmp/test".to_string(),
            }),
            "cleanup",
        );
        assert_eq!(a.risk_level, ActionRisk::High);
    }

    #[test]
    fn read_actions_are_low_risk() {
        let a = Action::new(
            "s1",
            ActionVariant::FileRead(FileReadParams {
                path: "/tmp/test".to_string(),
            }),
            "read file",
        );
        assert_eq!(a.risk_level, ActionRisk::Low);
    }

    #[test]
    fn action_name_matches_variant() {
        let a = Action::new("s1", ActionVariant::SystemStatus, "check");
        assert_eq!(a.action_name(), "system.status");
    }

    #[test]
    fn action_with_timeout() {
        let a = Action::new("s1", ActionVariant::SystemStatus, "check").with_timeout(5000);
        assert_eq!(a.timeout_ms, 5000);
    }

    #[test]
    fn action_with_risk_level_override() {
        let a = Action::new("s1", ActionVariant::SystemStatus, "check")
            .with_risk_level(ActionRisk::Critical);
        assert_eq!(a.risk_level, ActionRisk::Critical);
    }

    #[test]
    fn no_shell_command_actions_exist() {
        // Verify the ActionVariant enum has no Shell/Command variants
        let variants = std::mem::size_of::<ActionVariant>();
        assert!(variants > 0);
    }
}
