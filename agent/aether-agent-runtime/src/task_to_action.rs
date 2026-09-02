// Agent Runtime — convert an approved `AgentTask`
// into a typed `Action` for the executor.
//
// This is the Phase 13.3 bridge: proposals become
// tasks via `proposal_to_task`; tasks become
// actions here; the executor runs them through
// the existing capability / policy / IPC stack.

use aether_agent_core::{AgentTask, TaskKind};
use serde_json::Value;

use crate::action::{
    Action, ActionVariant, ApplicationLaunchParams, DeviceDisableParams, DeviceEnableParams,
    DisplaySetBrightnessParams, ServiceRestartParams, SystemRebootParams, SystemShutdownParams,
};

/// Why an `AgentTask` cannot be turned into an
/// `Action`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskToActionError {
    /// The session id was empty.
    EmptySessionId,
    /// The task kind has no executor mapping yet.
    UnsupportedKind { kind: TaskKind, reason: String },
    /// A required field was missing from the task
    /// or its `arguments` payload.
    MissingField { field: String },
    /// The `arguments` payload had an unexpected
    /// shape.
    InvalidArguments { message: String },
}

impl std::fmt::Display for TaskToActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySessionId => f.write_str("session_id is required"),
            Self::UnsupportedKind { kind, reason } => {
                write!(f, "task kind '{}' is not executable: {reason}", kind.as_str())
            }
            Self::MissingField { field } => write!(f, "missing required field '{field}'"),
            Self::InvalidArguments { message } => write!(f, "invalid arguments: {message}"),
        }
    }
}

impl std::error::Error for TaskToActionError {}

/// Converts an approved `AgentTask` into a typed
/// `Action`. The returned action's `reason` is the
/// task description; risk is assigned by
/// `Action::new` from the variant table.
pub fn task_to_action(session_id: &str, task: &AgentTask) -> Result<Action, TaskToActionError> {
    if session_id.trim().is_empty() {
        return Err(TaskToActionError::EmptySessionId);
    }
    let args = task.arguments.as_ref();
    let variant = match task.kind {
        TaskKind::RestartService => {
            let service = string_field(task.target.as_deref(), args, &["service"])?;
            ActionVariant::ServiceRestart(ServiceRestartParams { service_id: service })
        }
        TaskKind::Notify => {
            // Informational tasks acknowledge through
            // a read-only context snapshot.
            ActionVariant::ContextGet
        }
        TaskKind::DeviceControl => {
            let device_id = string_field(task.target.as_deref(), args, &["device_id"])?;
            let enable =
                args.and_then(|a| a.get("enable")).and_then(|v| v.as_bool()).unwrap_or(true);
            if enable {
                ActionVariant::DeviceEnable(DeviceEnableParams { device_id })
            } else {
                ActionVariant::DeviceDisable(DeviceDisableParams { device_id })
            }
        }
        TaskKind::DisplayControl => {
            let display_id = string_field(task.target.as_deref(), args, &["display_id"])?;
            let level = args
                .and_then(|a| a.get("level"))
                .and_then(|v| v.as_u64())
                .ok_or_else(|| TaskToActionError::MissingField { field: "level".to_string() })?;
            ActionVariant::DisplaySetBrightness(DisplaySetBrightnessParams {
                display_id,
                level: level.min(100) as u8,
            })
        }
        TaskKind::PowerControl => power_variant(args)?,
        TaskKind::SecurityControl => security_variant(args)?,
        TaskKind::ProposeCleanup => cleanup_variant(args)?,
        TaskKind::Custom => custom_variant(args)?,
        TaskKind::ProposeUpdate | TaskKind::ProposeInstall | TaskKind::ProposeSecurityScan => {
            return Err(TaskToActionError::UnsupportedKind {
                kind: task.kind,
                reason: "meta-proposals require a human operator or a future update pipeline"
                    .to_string(),
            });
        }
        // `TaskKind` is `#[non_exhaustive]`; any
        // future variant the runtime has not been
        // updated for is refused with the same
        // typed error rather than silently turning
        // into a Todo.
        _ => {
            return Err(TaskToActionError::UnsupportedKind {
                kind: task.kind,
                reason: "no executor mapping for this task kind yet".to_string(),
            });
        }
    };
    Ok(Action::new(session_id, variant, &task.description))
}

fn power_variant(args: Option<&Value>) -> Result<ActionVariant, TaskToActionError> {
    let op = args
        .and_then(|a| a.get("operation"))
        .or_else(|| args.and_then(|a| a.get("kind")))
        .and_then(|v| v.as_str())
        .unwrap_or("reboot");
    match op {
        "reboot" | "restart" => {
            let delay_ms =
                args.and_then(|a| a.get("delay_ms")).and_then(|v| v.as_u64()).unwrap_or(0);
            Ok(ActionVariant::SystemReboot(SystemRebootParams { delay_ms }))
        }
        "shutdown" => {
            let delay_ms =
                args.and_then(|a| a.get("delay_ms")).and_then(|v| v.as_u64()).unwrap_or(0);
            Ok(ActionVariant::SystemShutdown(SystemShutdownParams { delay_ms }))
        }
        "suspend" | "hibernate" => Ok(ActionVariant::SystemSuspend),
        other => Err(TaskToActionError::InvalidArguments {
            message: format!("unknown power operation '{other}'"),
        }),
    }
}

fn security_variant(args: Option<&Value>) -> Result<ActionVariant, TaskToActionError> {
    let op = args
        .and_then(|a| a.get("operation"))
        .or_else(|| args.and_then(|a| a.get("kind")))
        .and_then(|v| v.as_str())
        .unwrap_or("policy.reload");
    match op {
        "policy.reload" | "policy_reload" => Ok(ActionVariant::PolicyReload),
        other => Err(TaskToActionError::UnsupportedKind {
            kind: TaskKind::SecurityControl,
            reason: format!("security operation '{other}' is not wired yet"),
        }),
    }
}

fn cleanup_variant(args: Option<&Value>) -> Result<ActionVariant, TaskToActionError> {
    let kind = args.and_then(|a| a.get("kind")).and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "drop_page_cache" | "free_disk_cache" => {
            // No typed cache-drop IPC yet; surface
            // storage pressure so the operator can
            // decide next steps.
            Ok(ActionVariant::StorageStatus)
        }
        "" => Ok(ActionVariant::StorageStatus),
        other => Err(TaskToActionError::UnsupportedKind {
            kind: TaskKind::ProposeCleanup,
            reason: format!("cleanup kind '{other}' is not wired yet"),
        }),
    }
}

fn custom_variant(args: Option<&Value>) -> Result<ActionVariant, TaskToActionError> {
    let kind = args
        .and_then(|a| a.get("kind"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| TaskToActionError::MissingField { field: "kind".to_string() })?;
    match kind {
        "restart_service" => {
            let service = string_field(None, args, &["service"])?;
            Ok(ActionVariant::ServiceRestart(ServiceRestartParams { service_id: service }))
        }
        "restart_app" => {
            let app_id = string_field(None, args, &["app_id"])?;
            Ok(ActionVariant::ApplicationLaunch(ApplicationLaunchParams { application_id: app_id }))
        }
        "inform_user" => Ok(ActionVariant::ContextGet),
        "kill_process" => {
            // No typed kill IPC in the runtime yet.
            Err(TaskToActionError::UnsupportedKind {
                kind: TaskKind::Custom,
                reason: "kill_process requires a future process.kill action".to_string(),
            })
        }
        other => Err(TaskToActionError::UnsupportedKind {
            kind: TaskKind::Custom,
            reason: format!("custom kind '{other}' is not wired yet"),
        }),
    }
}

fn string_field(
    target: Option<&str>,
    args: Option<&Value>,
    keys: &[&str],
) -> Result<String, TaskToActionError> {
    if let Some(t) = target {
        if !t.is_empty() {
            return Ok(t.to_string());
        }
    }
    if let Some(a) = args {
        for key in keys {
            if let Some(s) = a.get(*key).and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    return Ok(s.to_string());
                }
            }
        }
    }
    Err(TaskToActionError::MissingField {
        field: keys.first().copied().unwrap_or("value").to_string(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_agent_core::TaskId;

    fn task(kind: TaskKind) -> AgentTask {
        AgentTask::new("t1", kind, "title", "do the thing").expect("valid task")
    }

    #[test]
    fn rejects_empty_session_id() {
        let t = task(TaskKind::Notify);
        let err = task_to_action("", &t).unwrap_err();
        assert_eq!(err, TaskToActionError::EmptySessionId);
    }

    #[test]
    fn notify_maps_to_context_get() {
        let t = task(TaskKind::Notify);
        let action = task_to_action("s1", &t).expect("action");
        assert!(matches!(action.variant, ActionVariant::ContextGet));
    }

    #[test]
    fn restart_service_uses_target() {
        let t = task(TaskKind::RestartService).with_target("aether-agentd");
        let action = task_to_action("s1", &t).expect("action");
        match action.variant {
            ActionVariant::ServiceRestart(p) => assert_eq!(p.service_id, "aether-agentd"),
            other => panic!("expected ServiceRestart, got {other:?}"),
        }
    }

    #[test]
    fn restart_service_reads_arguments_service() {
        let t = task(TaskKind::RestartService)
            .with_arguments(serde_json::json!({"service": "aether-network"}));
        let action = task_to_action("s1", &t).expect("action");
        match action.variant {
            ActionVariant::ServiceRestart(p) => assert_eq!(p.service_id, "aether-network"),
            other => panic!("expected ServiceRestart, got {other:?}"),
        }
    }

    #[test]
    fn device_control_enable() {
        let t = task(TaskKind::DeviceControl)
            .with_target("wifi-0")
            .with_arguments(serde_json::json!({"enable": true}));
        let action = task_to_action("s1", &t).expect("action");
        match action.variant {
            ActionVariant::DeviceEnable(p) => assert_eq!(p.device_id, "wifi-0"),
            other => panic!("expected DeviceEnable, got {other:?}"),
        }
    }

    #[test]
    fn device_control_disable() {
        let t = task(TaskKind::DeviceControl)
            .with_target("cam-0")
            .with_arguments(serde_json::json!({"enable": false}));
        let action = task_to_action("s1", &t).expect("action");
        assert!(matches!(action.variant, ActionVariant::DeviceDisable(_)));
    }

    #[test]
    fn power_reboot() {
        let t = task(TaskKind::PowerControl)
            .with_arguments(serde_json::json!({"operation": "reboot", "delay_ms": 100}));
        let action = task_to_action("s1", &t).expect("action");
        match action.variant {
            ActionVariant::SystemReboot(p) => assert_eq!(p.delay_ms, 100),
            other => panic!("expected SystemReboot, got {other:?}"),
        }
    }

    #[test]
    fn custom_restart_service() {
        let t = task(TaskKind::Custom).with_arguments(serde_json::json!({
            "kind": "restart_service",
            "service": "aether-proactive",
        }));
        let action = task_to_action("s1", &t).expect("action");
        match action.variant {
            ActionVariant::ServiceRestart(p) => assert_eq!(p.service_id, "aether-proactive"),
            other => panic!("expected ServiceRestart, got {other:?}"),
        }
    }

    #[test]
    fn custom_kill_process_is_unsupported() {
        let t = task(TaskKind::Custom).with_arguments(serde_json::json!({
            "kind": "kill_process",
            "pid": 42,
        }));
        let err = task_to_action("s1", &t).unwrap_err();
        assert!(matches!(err, TaskToActionError::UnsupportedKind { .. }));
    }

    #[test]
    fn propose_update_is_unsupported() {
        let t = task(TaskKind::ProposeUpdate);
        let err = task_to_action("s1", &t).unwrap_err();
        assert!(matches!(err, TaskToActionError::UnsupportedKind { .. }));
    }

    #[test]
    fn cleanup_drop_page_cache_maps_to_storage_status() {
        let t = task(TaskKind::ProposeCleanup)
            .with_arguments(serde_json::json!({"kind": "drop_page_cache"}));
        let action = task_to_action("s1", &t).expect("action");
        assert!(matches!(action.variant, ActionVariant::StorageStatus));
    }

    #[test]
    fn custom_restart_app_launches_application() {
        let t = task(TaskKind::Custom).with_arguments(serde_json::json!({
            "kind": "restart_app",
            "app_id": "calculator",
        }));
        let action = task_to_action("s1", &t).expect("action");
        match action.variant {
            ActionVariant::ApplicationLaunch(p) => assert_eq!(p.application_id, "calculator"),
            other => panic!("expected ApplicationLaunch, got {other:?}"),
        }
    }

    #[test]
    fn action_carries_task_description_as_reason() {
        let t = AgentTask::new("t1", TaskKind::Notify, "title", "disk is full").expect("valid");
        let action = task_to_action("sess", &t).expect("action");
        assert_eq!(action.reason, "disk is full");
        assert_eq!(action.session_id, "sess");
    }

    #[test]
    fn task_id_is_not_required_for_conversion() {
        let _ = TaskId::new("t1");
        let t = task(TaskKind::Notify);
        assert!(task_to_action("s1", &t).is_ok());
    }
}
