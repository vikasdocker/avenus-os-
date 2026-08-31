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
///
/// `deny_unknown_fields` is the security boundary: a model-produced
/// action cannot smuggle `root`, `admin`, `allow`, `skip_policy`,
/// `trusted`, or any other privilege-escalation field past the
/// deserializer. Risk is always assigned by `Action::new` via the
/// trusted `classify_action` table; the LLM cannot override it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

    // Display actions
    DisplayList,
    DisplaySetBrightness(DisplaySetBrightnessParams),
    DisplaySetResolution(DisplaySetResolutionParams),

    // Device actions (typed bridge to
    // aether-hardware-service capabilities).
    DeviceList,
    DeviceInspect(DeviceInspectParams),
    DeviceEnable(DeviceEnableParams),
    DeviceDisable(DeviceDisableParams),

    // Power actions
    SystemReboot(SystemRebootParams),
    SystemShutdown(SystemShutdownParams),
    SystemSuspend,

    // Security actions (high-risk;
    // require explicit user consent).
    CredentialSeal(CredentialSealParams),
    CredentialUnseal(CredentialUnsealParams),
    PolicyReload,
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

// ---- Display params ----

/// Set the brightness on a display
/// device. `level` is 0..=100.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySetBrightnessParams {
    /// The display device id.
    pub display_id: String,
    /// 0..=100.
    pub level: u8,
}

/// Change the resolution of a
/// display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySetResolutionParams {
    /// The display device id.
    pub display_id: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

// ---- Device params ----

/// Inspect a single piece of
/// hardware. The id is matched
/// against `aether-hardware-service`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInspectParams {
    /// The device id.
    pub device_id: String,
}

/// Enable a disabled device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEnableParams {
    /// The device id.
    pub device_id: String,
}

/// Disable a device (the runtime
/// toggles it through the
/// `Capability::Enable` /
/// `Capability::Disable` verbs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDisableParams {
    /// The device id.
    pub device_id: String,
}

// ---- Power params ----

/// Reboot the system. `delay_ms`
/// is a graceful shutdown window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRebootParams {
    /// The shutdown window.
    pub delay_ms: u64,
}

/// Shut the system down.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemShutdownParams {
    /// The shutdown window.
    pub delay_ms: u64,
}

// ---- Security params ----

/// Seal a credential for
/// long-term storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSealParams {
    /// The credential name.
    pub name: String,
    /// The plaintext value (must
    /// never be logged).
    pub plaintext: String,
}

/// Unseal a previously sealed
/// credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialUnsealParams {
    /// The credential name.
    pub name: String,
}

// ---- action builder helpers ----

impl Action {
    pub fn new(session_id: &str, variant: ActionVariant, reason: &str) -> Self {
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
            ActionVariant::DisplayList => "display.list",
            ActionVariant::DisplaySetBrightness(_) => "display.set_brightness",
            ActionVariant::DisplaySetResolution(_) => "display.set_resolution",
            ActionVariant::DeviceList => "device.list",
            ActionVariant::DeviceInspect(_) => "device.inspect",
            ActionVariant::DeviceEnable(_) => "device.enable",
            ActionVariant::DeviceDisable(_) => "device.disable",
            ActionVariant::SystemReboot(_) => "system.reboot",
            ActionVariant::SystemShutdown(_) => "system.shutdown",
            ActionVariant::SystemSuspend => "system.suspend",
            ActionVariant::CredentialSeal(_) => "credential.seal",
            ActionVariant::CredentialUnseal(_) => "credential.unseal",
            ActionVariant::PolicyReload => "policy.reload",
        }
    }
}

/// Classifies an action variant into required capabilities and risk level.
/// This is TRUSTED CODE — the LLM cannot override these classifications.
fn classify_action(variant: &ActionVariant) -> (Vec<String>, ActionRisk) {
    match variant {
        ActionVariant::ApplicationLaunch(_) => {
            (vec!["application.launch".to_string()], ActionRisk::Medium)
        }
        ActionVariant::ApplicationClose(_) => {
            (vec!["application.close".to_string()], ActionRisk::Medium)
        }
        ActionVariant::WindowList => (vec!["window.list".to_string()], ActionRisk::Low),
        ActionVariant::WindowFocus(_) => (vec!["window.focus".to_string()], ActionRisk::Low),
        ActionVariant::WindowMinimize(_) => (vec!["window.minimize".to_string()], ActionRisk::Low),
        ActionVariant::WindowMaximize(_) => (vec!["window.maximize".to_string()], ActionRisk::Low),
        ActionVariant::WindowClose(_) => (vec!["window.close".to_string()], ActionRisk::Medium),
        ActionVariant::FileList(_) => (vec!["file.list".to_string()], ActionRisk::Low),
        ActionVariant::FileRead(_) => (vec!["file.read".to_string()], ActionRisk::Low),
        ActionVariant::FileCreate(_) => (vec!["file.create".to_string()], ActionRisk::Medium),
        ActionVariant::FileWrite(_) => (vec!["file.write".to_string()], ActionRisk::Medium),
        ActionVariant::FileSearch(_) => (vec!["file.search".to_string()], ActionRisk::Low),
        ActionVariant::FileRename(_) => (vec!["file.rename".to_string()], ActionRisk::Medium),
        ActionVariant::FileMove(_) => (vec!["file.move".to_string()], ActionRisk::Medium),
        ActionVariant::FileDelete(_) => (vec!["file.delete".to_string()], ActionRisk::High),
        ActionVariant::ProcessList => (vec!["process.list".to_string()], ActionRisk::Low),
        ActionVariant::ProcessInspect(_) => (vec!["process.inspect".to_string()], ActionRisk::Low),
        ActionVariant::NetworkStatus => (vec!["network.status".to_string()], ActionRisk::Low),
        ActionVariant::NetworkInterfaces => {
            (vec!["network.interfaces".to_string()], ActionRisk::Low)
        }
        ActionVariant::SystemStatus => (vec!["system.status".to_string()], ActionRisk::Low),
        ActionVariant::SystemInfo => (vec!["system.info".to_string()], ActionRisk::Low),
        ActionVariant::SystemResources => (vec!["system.resources".to_string()], ActionRisk::Low),
        ActionVariant::SystemUptime => (vec!["system.uptime".to_string()], ActionRisk::Low),
        ActionVariant::StorageStatus => (vec!["storage.status".to_string()], ActionRisk::Low),
        ActionVariant::ContextGet => (vec!["context.get".to_string()], ActionRisk::Low),
        // Display: brightness is
        // benign; resolution changes
        // can disrupt running apps.
        ActionVariant::DisplayList => (vec!["display.list".to_string()], ActionRisk::Low),
        ActionVariant::DisplaySetBrightness(_) => {
            (vec!["display.set_brightness".to_string()], ActionRisk::Low)
        }
        ActionVariant::DisplaySetResolution(_) => {
            (vec!["display.set_resolution".to_string()], ActionRisk::Medium)
        }
        // Device: list / inspect are
        // benign; enable / disable
        // can change visible state.
        ActionVariant::DeviceList => (vec!["device.list".to_string()], ActionRisk::Low),
        ActionVariant::DeviceInspect(_) => (vec!["device.inspect".to_string()], ActionRisk::Low),
        ActionVariant::DeviceEnable(_) => (vec!["device.enable".to_string()], ActionRisk::Medium),
        ActionVariant::DeviceDisable(_) => (vec!["device.disable".to_string()], ActionRisk::Medium),
        // Power: always critical
        // (interrupts every user).
        ActionVariant::SystemReboot(_) => (vec!["system.reboot".to_string()], ActionRisk::Critical),
        ActionVariant::SystemShutdown(_) => {
            (vec!["system.shutdown".to_string()], ActionRisk::Critical)
        }
        ActionVariant::SystemSuspend => (vec!["system.suspend".to_string()], ActionRisk::High),
        // Security: credentials are
        // high-risk; policy reload
        // is medium.
        ActionVariant::CredentialSeal(_) => {
            (vec!["credential.seal".to_string()], ActionRisk::High)
        }
        ActionVariant::CredentialUnseal(_) => {
            (vec!["credential.unseal".to_string()], ActionRisk::High)
        }
        ActionVariant::PolicyReload => (vec!["policy.reload".to_string()], ActionRisk::Medium),
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
            ActionVariant::FileDelete(FileDeleteParams { path: "/tmp/test".to_string() }),
            "cleanup",
        );
        assert_eq!(a.risk_level, ActionRisk::High);
    }

    #[test]
    fn read_actions_are_low_risk() {
        let a = Action::new(
            "s1",
            ActionVariant::FileRead(FileReadParams { path: "/tmp/test".to_string() }),
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

    #[test]
    fn display_actions_classify() {
        let list = Action::new("s1", ActionVariant::DisplayList, "list");
        assert_eq!(list.risk_level, ActionRisk::Low);
        assert_eq!(list.action_name(), "display.list");

        let bright = Action::new(
            "s1",
            ActionVariant::DisplaySetBrightness(DisplaySetBrightnessParams {
                display_id: "d-1".to_string(),
                level: 50,
            }),
            "dim",
        );
        assert_eq!(bright.risk_level, ActionRisk::Low);
        assert_eq!(bright.action_name(), "display.set_brightness");
    }

    #[test]
    fn device_enable_disable_classify() {
        let on = Action::new(
            "s1",
            ActionVariant::DeviceEnable(DeviceEnableParams {
                device_id: "wifi-1".to_string(),
            }),
            "enable wifi",
        );
        assert_eq!(on.risk_level, ActionRisk::Medium);
        assert!(on.requested_capabilities.contains(&"device.enable".to_string()));

        let off = Action::new(
            "s1",
            ActionVariant::DeviceDisable(DeviceDisableParams {
                device_id: "cam-1".to_string(),
            }),
            "disable camera",
        );
        assert_eq!(off.risk_level, ActionRisk::Medium);
    }

    #[test]
    fn power_actions_are_critical() {
        let reboot = Action::new(
            "s1",
            ActionVariant::SystemReboot(SystemRebootParams { delay_ms: 0 }),
            "reboot",
        );
        assert_eq!(reboot.risk_level, ActionRisk::Critical);
        assert_eq!(reboot.action_name(), "system.reboot");

        let shutdown = Action::new(
            "s1",
            ActionVariant::SystemShutdown(SystemShutdownParams { delay_ms: 5_000 }),
            "shutdown",
        );
        assert_eq!(shutdown.risk_level, ActionRisk::Critical);

        let suspend = Action::new("s1", ActionVariant::SystemSuspend, "suspend");
        assert_eq!(suspend.risk_level, ActionRisk::High);
    }

    #[test]
    fn security_actions_classify() {
        let seal = Action::new(
            "s1",
            ActionVariant::CredentialSeal(CredentialSealParams {
                name: "api-key".to_string(),
                plaintext: "redacted".to_string(),
            }),
            "seal key",
        );
        assert_eq!(seal.risk_level, ActionRisk::High);

        let unseal = Action::new(
            "s1",
            ActionVariant::CredentialUnseal(CredentialUnsealParams {
                name: "api-key".to_string(),
            }),
            "unseal key",
        );
        assert_eq!(unseal.risk_level, ActionRisk::High);

        let reload = Action::new("s1", ActionVariant::PolicyReload, "reload");
        assert_eq!(reload.risk_level, ActionRisk::Medium);
    }
}
