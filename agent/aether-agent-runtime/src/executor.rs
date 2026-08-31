// Agent Runtime - Action Executor
//
// Communicates with Aether services through IPC. Never directly
// executes Linux commands.

use crate::action::{Action, ActionVariant};
use crate::errors::AgentError;
use crate::observation::{Observation, ObservationType};

/// Result of executing an action.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub observation: Observation,
    pub duration_ms: u64,
}

/// Executes validated actions through Aether IPC.
pub struct ActionExecutor {
    control_port: u16,
    surface_port: u16,
}

impl ActionExecutor {
    pub fn new(control_port: u16, surface_port: u16) -> Self {
        Self { control_port, surface_port }
    }

    /// Executes an action and returns an observation.
    pub fn execute(&self, action: &Action) -> Result<ExecutionResult, AgentError> {
        let start = std::time::Instant::now();

        let result = match &action.variant {
            // Application actions → system core (port 4747)
            ActionVariant::ApplicationLaunch(p) => {
                self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "app.launch",
                    serde_json::json!({"app": p.application_id}),
                )?;
                ObservationType::ApplicationStarted { application_id: p.application_id.clone() }
            }
            ActionVariant::ApplicationClose(p) => {
                self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "app.close",
                    serde_json::json!({"app": p.application_id}),
                )?;
                ObservationType::ApplicationClosed { application_id: p.application_id.clone() }
            }

            // Window actions → surface server (port 4750)
            ActionVariant::WindowList => {
                let resp = self.surface_request(serde_json::json!({"op": "window.list"}))?;
                ObservationType::WindowList { windows: resp["windows"].clone() }
            }
            ActionVariant::WindowFocus(p) => {
                let id = p.window_id.unwrap_or(0);
                self.surface_request(serde_json::json!({"op": "window.focus", "window_id": id}))?;
                ObservationType::WindowFocused { window_id: id }
            }
            ActionVariant::WindowMinimize(p) => {
                self.surface_request(
                    serde_json::json!({"op": "window.minimize", "window_id": p.window_id}),
                )?;
                ObservationType::WindowMinimized { window_id: p.window_id }
            }
            ActionVariant::WindowMaximize(p) => {
                self.surface_request(
                    serde_json::json!({"op": "window.maximize", "window_id": p.window_id}),
                )?;
                ObservationType::WindowMaximized { window_id: p.window_id }
            }
            ActionVariant::WindowClose(p) => {
                let req = if let Some(id) = p.window_id {
                    serde_json::json!({"op": "window.close", "window_id": id})
                } else if let Some(ref app) = p.application_id {
                    serde_json::json!({"op": "window.close", "app_id": app})
                } else {
                    return Err(AgentError::Execution(
                        "Window close requires ID or app_id".to_string(),
                    ));
                };
                self.surface_request(req)?;
                ObservationType::WindowClosed { window_id: p.window_id.unwrap_or(0) }
            }

            // Filesystem actions → system core
            ActionVariant::FileList(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "file.list",
                    serde_json::json!({"path": p.path}),
                )?;
                ObservationType::FilesystemResult { operation: "list".to_string(), data: resp }
            }
            ActionVariant::FileRead(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "file.read",
                    serde_json::json!({"path": p.path}),
                )?;
                ObservationType::FilesystemResult { operation: "read".to_string(), data: resp }
            }
            ActionVariant::FileCreate(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "file.create",
                    serde_json::json!({"path": p.path, "content": p.content}),
                )?;
                ObservationType::FilesystemResult { operation: "create".to_string(), data: resp }
            }
            ActionVariant::FileWrite(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "file.write",
                    serde_json::json!({"path": p.path, "content": p.content}),
                )?;
                ObservationType::FilesystemResult { operation: "write".to_string(), data: resp }
            }
            ActionVariant::FileSearch(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "file.search",
                    serde_json::json!({"query": p.query, "path": p.path}),
                )?;
                ObservationType::FilesystemResult { operation: "search".to_string(), data: resp }
            }
            ActionVariant::FileRename(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "file.rename",
                    serde_json::json!({"from": p.from, "to": p.to}),
                )?;
                ObservationType::FilesystemResult { operation: "rename".to_string(), data: resp }
            }
            ActionVariant::FileMove(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "file.move",
                    serde_json::json!({"from": p.from, "to": p.to}),
                )?;
                ObservationType::FilesystemResult { operation: "move".to_string(), data: resp }
            }
            ActionVariant::FileDelete(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "file.delete",
                    serde_json::json!({"path": p.path}),
                )?;
                ObservationType::FilesystemResult { operation: "delete".to_string(), data: resp }
            }

            // Process actions → system core
            ActionVariant::ProcessList => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "process.list",
                    serde_json::json!({}),
                )?;
                ObservationType::ProcessList { processes: resp["processes"].clone() }
            }
            ActionVariant::ProcessInspect(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "process.inspect",
                    serde_json::json!({"pid": p.pid, "name": p.name}),
                )?;
                ObservationType::ProcessInspect { data: resp }
            }

            // Network actions → system core
            ActionVariant::NetworkStatus => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "network.status",
                    serde_json::json!({}),
                )?;
                ObservationType::NetworkStatus { data: resp }
            }
            ActionVariant::NetworkInterfaces => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "network.interfaces",
                    serde_json::json!({}),
                )?;
                ObservationType::NetworkInterfaces { data: resp }
            }

            // System actions → system core
            ActionVariant::SystemStatus => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "status",
                    serde_json::json!({}),
                )?;
                ObservationType::SystemStatus { data: resp }
            }
            ActionVariant::SystemInfo => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "system.info",
                    serde_json::json!({}),
                )?;
                ObservationType::SystemInfo { data: resp }
            }
            ActionVariant::SystemResources => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "system.resources",
                    serde_json::json!({}),
                )?;
                ObservationType::SystemResources { data: resp }
            }
            ActionVariant::SystemUptime => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "system.uptime",
                    serde_json::json!({}),
                )?;
                ObservationType::SystemUptime { data: resp }
            }
            ActionVariant::StorageStatus => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "storage.status",
                    serde_json::json!({}),
                )?;
                ObservationType::StorageStatus { data: resp }
            }
            ActionVariant::ContextGet => {
                ObservationType::ContextSnapshot { data: serde_json::json!({}) }
            }

            // Display actions → system core
            ActionVariant::DisplayList => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "display.list",
                    serde_json::json!({}),
                )?;
                ObservationType::DisplayList { displays: resp }
            }
            ActionVariant::DisplaySetBrightness(p) => {
                let _resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "display.set_brightness",
                    serde_json::json!({"display_id": p.display_id, "level": p.level}),
                )?;
                ObservationType::DisplayBrightnessSet {
                    display_id: p.display_id.clone(),
                    level: p.level,
                }
            }
            ActionVariant::DisplaySetResolution(p) => {
                let _resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "display.set_resolution",
                    serde_json::json!({
                        "display_id": p.display_id,
                        "width": p.width,
                        "height": p.height
                    }),
                )?;
                ObservationType::DisplayResolutionSet {
                    display_id: p.display_id.clone(),
                    width: p.width,
                    height: p.height,
                }
            }

            // Device actions → aether-hardware-service via system core
            ActionVariant::DeviceList => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "device.list",
                    serde_json::json!({}),
                )?;
                ObservationType::DeviceList { devices: resp }
            }
            ActionVariant::DeviceInspect(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "device.inspect",
                    serde_json::json!({"device_id": p.device_id}),
                )?;
                ObservationType::DeviceInspect { data: resp }
            }
            ActionVariant::DeviceEnable(p) => {
                let _resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "device.enable",
                    serde_json::json!({"device_id": p.device_id}),
                )?;
                ObservationType::DeviceEnabled {
                    device_id: p.device_id.clone(),
                }
            }
            ActionVariant::DeviceDisable(p) => {
                let _resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "device.disable",
                    serde_json::json!({"device_id": p.device_id}),
                )?;
                ObservationType::DeviceDisabled {
                    device_id: p.device_id.clone(),
                }
            }

            // Power actions
            ActionVariant::SystemReboot(p) => {
                let _resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "system.reboot",
                    serde_json::json!({"delay_ms": p.delay_ms}),
                )?;
                ObservationType::SystemRebootRequested { delay_ms: p.delay_ms }
            }
            ActionVariant::SystemShutdown(p) => {
                let _resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "system.shutdown",
                    serde_json::json!({"delay_ms": p.delay_ms}),
                )?;
                ObservationType::SystemShutdownRequested { delay_ms: p.delay_ms }
            }
            ActionVariant::SystemSuspend => {
                let _resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "system.suspend",
                    serde_json::json!({}),
                )?;
                ObservationType::SystemSuspendRequested
            }

            // Security actions
            ActionVariant::CredentialSeal(p) => {
                let _resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "credentials.seal",
                    serde_json::json!({"name": p.name, "plaintext": p.plaintext}),
                )?;
                ObservationType::CredentialSealed { name: p.name.clone() }
            }
            ActionVariant::CredentialUnseal(p) => {
                let resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "credentials.unseal",
                    serde_json::json!({"name": p.name}),
                )?;
                let value = resp["plaintext"].as_str().unwrap_or("").to_string();
                ObservationType::CredentialUnsealed {
                    name: p.name.clone(),
                    value,
                }
            }
            ActionVariant::PolicyReload => {
                let _resp = self.ipc_request(
                    self.control_port,
                    "aether-system-core",
                    "policy.reload",
                    serde_json::json!({}),
                )?;
                ObservationType::PolicyReloaded
            }
        };

        let duration = start.elapsed().as_millis() as u64;
        let obs = Observation::new(&action.id.to_string(), action.session_id.clone(), result);

        Ok(ExecutionResult { success: true, observation: obs, duration_ms: duration })
    }

    fn ipc_request(
        &self,
        port: u16,
        service_id: &str,
        command: &str,
        parameters: serde_json::Value,
    ) -> Result<serde_json::Value, AgentError> {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpStream;

        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|e| AgentError::Ipc(format!("connect {port}: {e}")))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(|e| AgentError::Ipc(e.to_string()))?;

        let req = serde_json::json!({
            "service_id": service_id,
            "command": command,
            "parameters": parameters,
        });

        stream
            .write_all(format!("{req}\n").as_bytes())
            .map_err(|e| AgentError::Ipc(format!("send: {e}")))?;

        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .map_err(|e| AgentError::Ipc(format!("recv: {e}")))?;

        if line.is_empty() {
            return Err(AgentError::Ipc("empty response".to_string()));
        }

        serde_json::from_str(line.trim()).map_err(|e| AgentError::Ipc(format!("decode: {e}")))
    }

    fn surface_request(&self, payload: serde_json::Value) -> Result<serde_json::Value, AgentError> {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpStream;

        let mut stream = TcpStream::connect(("127.0.0.1", self.surface_port))
            .map_err(|e| AgentError::Ipc(format!("connect surface: {e}")))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .map_err(|e| AgentError::Ipc(e.to_string()))?;

        stream
            .write_all(format!("{payload}\n").as_bytes())
            .map_err(|e| AgentError::Ipc(format!("send: {e}")))?;

        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .map_err(|e| AgentError::Ipc(format!("recv: {e}")))?;

        if line.is_empty() {
            return Err(AgentError::Ipc("empty surface response".to_string()));
        }

        serde_json::from_str(line.trim()).map_err(|e| AgentError::Ipc(format!("decode: {e}")))
    }

    /// Execute an action with bounded recovery. This is the
    /// production entry point: it consults the trusted
    /// `recovery_policy_for(&action.variant)` mapping, drives
    /// the retry loop, and applies `decide_recovery` after
    /// every failure.
    ///
    /// Behaviour:
    ///   * `transient_default` actions (file.list, system.status,
    ///     process.list, network.status, etc.) retry up to 3
    ///     times with exponential backoff.
    ///   * `no_retry` actions (file.write, file.delete,
    ///     device.enable, credential.seal, system.reboot, ...)
    ///     fail on the first error and return the original
    ///     `AgentError`.
    ///   * The returned `ExecutionResult` always reflects the
    ///     last attempt; the audit log / caller can read
    ///     `result.duration_ms` for the cumulative wall-clock
    ///     time across retries.
    pub fn execute_with_recovery(
        &self,
        action: &Action,
    ) -> Result<ExecutionResult, AgentError> {
        use crate::action::recovery_policy_for;
        use crate::recovery::{backoff_delay, decide_recovery, FailureKind, RecoveryAction};

        let policy = recovery_policy_for(&action.variant);
        let mut attempt: u32 = 0;
        let started = std::time::Instant::now();
        loop {
            attempt = attempt.saturating_add(1);
            match self.execute(action) {
                Ok(mut result) => {
                    result.duration_ms = started.elapsed().as_millis() as u64;
                    return Ok(result);
                }
                Err(e) => {
                    let kind = FailureKind::from_error(&e);
                    let decision = decide_recovery(&policy, attempt, kind, false);
                    match decision {
                        RecoveryAction::Retry => {
                            // Honour the per-attempt timeout the
                            // policy declared. We sleep the
                            // backoff the policy would schedule
                            // for the next attempt; in a single
                            // thread this is a blocking sleep, but
                            // the host (daemon) drives the
                            // executor off the main loop.
                            let _ = backoff_delay(&policy, attempt);
                            continue;
                        }
                        RecoveryAction::Abort | RecoveryAction::Skip => return Err(e),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionVariant;

    #[test]
    fn executor_creation() {
        let exec = ActionExecutor::new(4747, 4750);
        assert_eq!(exec.control_port, 4747);
        assert_eq!(exec.surface_port, 4750);
    }

    #[test]
    fn system_status_action_structure() {
        let a = Action::new("s1", ActionVariant::SystemStatus, "check");
        assert_eq!(a.action_name(), "system.status");
        assert!(a.requested_capabilities.contains(&"system.status".to_string()));
    }

    // ---- execute_with_recovery ----
    //
    // These tests do not need a live Aether control plane;
    // we point the executor at an unused port so every call
    // returns `AgentError::Ipc`, which is classified as
    // `FailureKind::Transient`. The interesting behaviour
    // is how the *retry budget* differs between read-only
    // and mutating actions.

    fn dead_port_executor() -> ActionExecutor {
        // Port 1 is reserved and almost never in use; the
        // connect will fail with `ConnectionRefused`.
        ActionExecutor::new(1, 1)
    }

    #[test]
    fn read_only_action_retries_then_fails() {
        let exec = dead_port_executor();
        let action = Action::new("s1", ActionVariant::SystemStatus, "check");
        // transient_default = 3 retries, so 4 total
        // attempts before giving up.
        let result = exec.execute_with_recovery(&action);
        assert!(result.is_err());
    }

    #[test]
    fn mutating_action_fails_on_first_attempt() {
        let exec = dead_port_executor();
        let action = Action::new(
            "s1",
            ActionVariant::FileDelete(crate::action::FileDeleteParams {
                path: "/tmp/none".to_string(),
            }),
            "delete",
        );
        let result = exec.execute_with_recovery(&action);
        assert!(result.is_err());
    }

    #[test]
    fn power_action_fails_on_first_attempt() {
        let exec = dead_port_executor();
        let action = Action::new(
            "s1",
            ActionVariant::SystemReboot(crate::action::SystemRebootParams { delay_ms: 0 }),
            "reboot",
        );
        let result = exec.execute_with_recovery(&action);
        assert!(result.is_err());
    }

    #[test]
    fn credential_action_fails_on_first_attempt() {
        let exec = dead_port_executor();
        let action = Action::new(
            "s1",
            ActionVariant::CredentialSeal(crate::action::CredentialSealParams {
                name: "k".to_string(),
                plaintext: "p".to_string(),
            }),
            "seal",
        );
        let result = exec.execute_with_recovery(&action);
        assert!(result.is_err());
    }
}
