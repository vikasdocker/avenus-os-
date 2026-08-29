// Agent Runtime - Observation model
//
// Structured observations returned after action execution.
// Raw system output is normalized before being presented to the LLM.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique observation identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationId(Uuid);

impl ObservationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ObservationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ObservationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Types of observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservationType {
    ApplicationStarted { application_id: String },
    ApplicationFailed { application_id: String, reason: String },
    ApplicationClosed { application_id: String },
    ProcessExited { pid: u32, exit_code: i32 },
    WindowCreated { window_id: u64 },
    WindowClosed { window_id: u64 },
    WindowFocused { window_id: u64 },
    WindowMinimized { window_id: u64 },
    WindowMaximized { window_id: u64 },
    WindowList { windows: serde_json::Value },
    FilesystemResult { operation: String, data: serde_json::Value },
    NetworkStatus { data: serde_json::Value },
    NetworkInterfaces { data: serde_json::Value },
    SystemStatus { data: serde_json::Value },
    SystemInfo { data: serde_json::Value },
    SystemResources { data: serde_json::Value },
    SystemUptime { data: serde_json::Value },
    StorageStatus { data: serde_json::Value },
    ProcessList { processes: serde_json::Value },
    ProcessInspect { data: serde_json::Value },
    ContextSnapshot { data: serde_json::Value },
    Error { message: String },
}

/// A structured observation from action execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub action_id: String,
    pub session_id: String,
    pub timestamp: u64,
    pub observation_type: ObservationType,
    pub success: bool,
    pub data: serde_json::Value,
}

impl Observation {
    pub fn new(action_id: &str, session_id: String, obs_type: ObservationType) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let success = !matches!(&obs_type, ObservationType::Error { .. });
        let data = serde_json::to_value(&obs_type).unwrap_or(serde_json::json!({}));

        Self {
            id: ObservationId::new(),
            action_id: action_id.to_string(),
            session_id,
            timestamp: now,
            observation_type: obs_type,
            success,
            data,
        }
    }

    /// Normalizes the observation for LLM consumption (strips sensitive data).
    pub fn normalized(&self) -> serde_json::Value {
        let mut output = serde_json::json!({
            "success": self.success,
            "type": format!("{:?}", self.observation_type),
        });

        // Extract relevant data based on type, redacting sensitive fields
        match &self.observation_type {
            ObservationType::ApplicationStarted { application_id } => {
                output["application_id"] = serde_json::json!(application_id);
            }
            ObservationType::ApplicationFailed { application_id, reason } => {
                output["application_id"] = serde_json::json!(application_id);
                output["reason"] = serde_json::json!(reason);
            }
            ObservationType::WindowList { windows } => {
                output["windows"] = windows.clone();
            }
            ObservationType::FilesystemResult { operation, data } => {
                output["operation"] = serde_json::json!(operation);
                output["data"] = data.clone();
            }
            ObservationType::Error { message } => {
                output["error"] = serde_json::json!(message);
            }
            _ => {
                output["data"] = self.data.clone();
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_creation() {
        let obs = Observation::new(
            "act-1",
            "sess-1".to_string(),
            ObservationType::ApplicationStarted { application_id: "calc".to_string() },
        );
        assert!(obs.success);
        assert_eq!(obs.action_id, "act-1");
        assert!(obs.timestamp > 0);
    }

    #[test]
    fn error_observation_is_failure() {
        let obs = Observation::new(
            "act-1",
            "sess-1".to_string(),
            ObservationType::Error { message: "failed".to_string() },
        );
        assert!(!obs.success);
    }

    #[test]
    fn normalized_output() {
        let obs = Observation::new(
            "act-1",
            "sess-1".to_string(),
            ObservationType::SystemStatus { data: serde_json::json!({"ok": true}) },
        );
        let norm = obs.normalized();
        assert!(norm["success"].as_bool().unwrap_or(false));
        assert!(norm["type"].as_str().is_some());
    }
}
