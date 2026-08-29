// Agent Runtime - Error types

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum AgentError {
    #[error("Session error: {0}")]
    Session(String),

    #[error("Request error: {0}")]
    Request(String),

    #[error("Intent error: {0}")]
    Intent(String),

    #[error("Action error: {0}")]
    Action(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Capability denied: {0}")]
    CapabilityDenied(String),

    #[error("Policy denied: {0}")]
    PolicyDenied(String),

    #[error("Approval required: {0}")]
    ApprovalRequired(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("Planner error: {0}")]
    Planner(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Observation error: {0}")]
    Observation(String),

    #[error("Cancellation: {0}")]
    Cancellation(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<aether_core::error::AetherError> for AgentError {
    fn from(err: aether_core::error::AetherError) -> Self {
        AgentError::Ipc(err.to_string())
    }
}
